use std::{collections::HashMap, error::Error, rc::Rc};

use crate::analyze::{
    AnalyzersConfig, TimeTable,
    analyzer::{axi_analyzer::ReadyValidTransactionIterator, get_value_at_time},
    bus::{
        BusCommon, BusDescription, COMMON_YAML, SignalPath, SignalPathFromYaml, is_value_of_type,
    },
    plugins::load_python_plugin,
    trigger::{
        PythonTrigger, SignalTrigger, TransactionTrigger, TriggerName, TriggerSink, TriggerSource,
    },
};
use libbusperf::bus_usage::{BusUsage, MultiChannelBusUsage, RealTime};
use owo_colors::OwoColorize;

use super::Analyzer;
use pyo3::{prelude::*, types::PyTuple};
use yaml_rust2::Yaml;

pub struct PythonDescription {
    common: BusCommon,
    signals: Vec<SignalInfo>,
    signals_hash: HashMap<String, SignalPath>,
}

impl PythonDescription {
    fn new(
        common: BusCommon,
        signals: Vec<SignalInfo>,
        signals_hash: HashMap<String, SignalPath>,
    ) -> Self {
        Self {
            common,
            signals,
            signals_hash,
        }
    }
}

impl BusDescription for PythonDescription {
    fn name(&self) -> &str {
        self.common.bus_name()
    }

    fn common(&self) -> &BusCommon {
        &self.common
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        self.signals
            .iter()
            .flat_map(|(_, signals)| signals.iter())
            .collect()
    }

    fn get_unique_signals(&self) -> Vec<&SignalPath> {
        self.get_signals()
    }

    fn get_by_name(&self, name: &str) -> Option<&SignalPath> {
        match name {
            "clock" => Some(self.common.clk_path()),
            "reset" => Some(self.common.rst_path()),
            other => self.signals_hash.get(other),
        }
    }

    /// PANICS when not a last existing Rc
    fn into_signals(self: Rc<Self>) -> Vec<SignalPath> {
        if let Some(s) = Rc::into_inner(self) {
            let mut signals = s.common.into_signals_owned();
            signals.append(&mut s.signals.into_iter().flat_map(|(_, s)| s).collect());
            signals
        } else {
            panic!("into signals called when strong count != 1")
        }
    }
}

pub struct PythonAnalyzer {
    description: Rc<PythonDescription>,
    obj: Py<PyAny>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,

    required: Vec<TriggerName>,
    sink: TriggerSink,
    provided: Vec<Box<dyn TriggerSource>>,
    provided_python: Vec<PythonTrigger>,
}

#[pyclass]
#[derive(Clone)]
struct Transaction {
    start: RealTime,
    first_data: RealTime,
    last_data: RealTime,
    resp_time: RealTime,
    resp: String,
    next_start: RealTime,
}

#[pymethods]
impl Transaction {
    #[new]
    fn new(
        start: RealTime,
        first_data: RealTime,
        last_data: RealTime,
        resp_time: RealTime,
        resp: String,
        next_start: RealTime,
    ) -> PyResult<Self> {
        Ok(Transaction {
            start,
            first_data,
            last_data,
            resp_time,
            resp,
            next_start,
        })
    }
}

#[pyclass]
#[derive(Clone, Copy, Debug)]
enum SignalType {
    Signal,
    RisingSignal,
    ReadyValid,
}

impl std::fmt::Display for SignalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalType::Signal => f.write_str("Signal"),
            SignalType::RisingSignal => f.write_str("RisingSignal"),
            SignalType::ReadyValid => f.write_str("ReadyValid"),
        }
    }
}

type SignalInfo = (SignalType, Vec<SignalPath>);

impl PythonAnalyzer {
    pub fn new(
        name: String,
        class_name: &str,
        i: &Yaml,
        config: &AnalyzersConfig,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let plugins_path = &config.plugins_path;
        let common = BusCommon::from_yaml(name, i)?;
        Python::with_gil(|py| -> PyResult<()> {
            let module = match py.import("sys")?.getattr("modules")?.get_item("busperf") {
                Ok(module) => module.extract()?,
                _ => PyModule::new(py, "busperf")?,
            };
            module.add_class::<SignalType>()?;
            module.add_class::<Transaction>()?;
            py.import("sys")?
                .getattr("modules")?
                .set_item("busperf", module)?;
            Ok(())
        })?;
        let obj = load_python_plugin(plugins_path, class_name)?;

        let signals = Python::with_gil(
            #[allow(clippy::type_complexity)]
            |py| -> Result<Vec<(SignalType, Vec<String>)>, Box<dyn std::error::Error>> {
                Ok(obj
                    .getattr(py, "get_yaml_signals")?
                    .call0(py)
                    .map_err(|_| "'get_yaml_signals' object is not callable")?
                    .extract::<Vec<(SignalType, Vec<String>)>>(py)
                    .map_err(|_| "get_yaml_signals returned invalid value")?)
            },
        )?;
        let signals: Vec<_> = signals
            .iter()
            .map(|(type_, path)| {
                let mut i = i;
                for s in path {
                    i = &i[s.as_str()];
                }
                let name = path.join(".");
                let signal: Result<SignalInfo, Box<dyn std::error::Error>> = match type_ {
                    SignalType::Signal | SignalType::RisingSignal => {
                        match SignalPathFromYaml::from_yaml_ref_with_prefix(
                            common.module_scope(),
                            i,
                        ) {
                            Ok(path) => Ok((*type_, vec![path])),
                            Err(_) => Err(format!("Yaml should define {} signal", name))?,
                        }
                    }
                    SignalType::ReadyValid => {
                        let r = SignalPathFromYaml::from_yaml_ref_with_prefix(
                            common.module_scope(),
                            &i["ready"],
                        )
                        .map_err(|_| format!("Yaml is missing ready signal for {name}",))?;
                        let v = SignalPathFromYaml::from_yaml_ref_with_prefix(
                            common.module_scope(),
                            &i["valid"],
                        )
                        .map_err(|_| format!("Yaml is missing valid signal for {name}",))?;
                        Ok((*type_, vec![r, v]))
                    }
                };
                signal
            })
            .collect::<Result<_, _>>()?;
        let mut signals_hash = HashMap::new();
        for (name, yaml) in i.as_hash().expect("Already checked").iter() {
            let name = name.as_str().ok_or("invalid name")?;
            if !COMMON_YAML.contains(&name) {
                match SignalPathFromYaml::from_yaml_ref_with_prefix(common.module_scope(), yaml) {
                    Ok(path) => {
                        signals_hash.insert(name.to_owned(), path);
                    }
                    Err(_) => {
                        for (signal_name, yaml) in yaml
                            .as_hash()
                            .ok_or(format!("invalid signal definition {:?}", yaml))?
                        {
                            let signal_name = signal_name.as_str().ok_or("invalid name")?;
                            let path = SignalPathFromYaml::from_yaml_ref_with_prefix(
                                common.module_scope(),
                                yaml,
                            )?;
                            signals_hash.insert(format!("{}.{}", name, signal_name), path);
                        }
                    }
                }
            }
        }
        let sink = TriggerSink::build_from_yaml(i)?;
        let required = sink.required();
        let description = Rc::new(PythonDescription::new(common, signals, signals_hash));

        let provided = match &i["triggers"] {
            yaml_rust2::Yaml::Hash(hash) => {
                let mut provided = vec![];
                for (name, yaml) in hash {
                    let name = name.as_str().ok_or("invalid trigger name")?;
                    if name == "_" {
                        let mut new = yaml
                            .as_hash()
                            .ok_or("invalid syntax")?
                            .iter()
                            .map(|(type_, n)| {
                                let name = format!(
                                    "interfaces.{}.{}",
                                    description.name(),
                                    n.as_str().ok_or("invalid name")?
                                );
                                TransactionTrigger::from_yaml(name, type_)
                            })
                            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
                        provided.append(&mut new);
                    } else {
                        let name = format!("interfaces.{}.{}", description.name(), name);
                        let clk_path = description.get_by_name("clock").expect("Should have clock");
                        let new = SignalTrigger::combination_from_yaml(
                            name,
                            yaml,
                            &(Rc::clone(&description) as Rc<dyn BusDescription>),
                            clk_path,
                        )?;
                        provided.push(new);
                    }
                }
                provided
            }
            yaml_rust2::Yaml::BadValue => vec![],
            _ => Err("bad triggers")?,
        };

        let provided_python = PythonTrigger::vec_from_obj(&obj, description.name(), false)?;

        Ok(PythonAnalyzer {
            description,
            obj,
            window_length: config.window_length,
            x_rate: config.x_rate,
            y_rate: config.y_rate,
            required,
            sink,
            provided,
            provided_python,
        })
    }
}

impl Analyzer for PythonAnalyzer {
    fn bus_name(&self) -> &str {
        self.description.common.bus_name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![
            self.description.common.clk_path(),
            self.description.common.rst_path(),
        ];
        signals.append(
            &mut self
                .description
                .signals
                .iter()
                .flat_map(|(_, path)| path)
                .collect(),
        );

        signals
    }

    fn calculate(
        &mut self,
        loaded: &[&wellen::Signal],
        intervals: &[[RealTime; 2]],
        time_table: &TimeTable,
    ) -> Result<BusUsage, Box<dyn std::error::Error + 'static>> {
        let clk = &loaded[0];
        let rst = &loaded[1];
        let mut last = 0;
        let mut reset = 0;
        for (time, value) in rst.iter_changes() {
            if is_value_of_type(value, self.description.common.rst_active_value()) {
                last = time;
            } else {
                reset += time - last;
            }
        }
        reset /= 2;
        let (time_end, _) = clk
            .iter_changes()
            .last()
            .ok_or("clock should have cycles")?;
        let mut usage = MultiChannelBusUsage::new(
            self.description.common.bus_name(),
            self.window_length,
            *time_table.get(2).ok_or(
                "trace is too short (less than 3 time indices), cannot calculate clock period",
            )?,
            self.x_rate,
            self.y_rate,
        );

        for &[start, end] in intervals {
            let start_idx = time_table
                .binary_search(&start)
                .unwrap_or_else(|e| e - 1)
                .saturating_sub(1) as u32;
            let mut i = 0;
            let loaded: Vec<_> = [
                (SignalType::Signal, vec![]),
                (SignalType::RisingSignal, vec![]),
            ]
            .iter()
            .chain(self.description.signals.iter())
            .map(|(type_, _)| match type_ {
                SignalType::Signal | SignalType::RisingSignal => {
                    let signal = &loaded[i];
                    i += 1;
                    let start_value = vec![(
                        time_table[start_idx as usize],
                        get_value_at_time(signal, start_idx)
                            .ok_or(format!("signal is invalid at {}", start))?
                            .to_bit_string()
                            .expect("never returns none"),
                    )]
                    .into_iter();
                    let changes = signal
                        .iter_changes()
                        .filter_map(|(t, v)| {
                            let time = time_table[t as usize];
                            if time >= start && time <= end {
                                match v.to_bit_string() {
                                    Some(v) => Some(Ok((time, v))),
                                    None => Some(Err(format!(
                                        "signal is invalid at {}",
                                        time_table[time as usize]
                                    ))),
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Result<Vec<(RealTime, String)>, _>>()?;
                    Ok::<_, Box<dyn Error>>(start_value.chain(changes).collect::<Vec<_>>())
                }
                SignalType::ReadyValid => {
                    let ready = &loaded[i];
                    let valid = &loaded[i + 1];
                    i += 2;
                    let a = ReadyValidTransactionIterator::new(clk, ready, valid, time_end);
                    a.filter_map(|time_idx| {
                        let time = time_table[time_idx as usize];
                        if time >= start && time <= end {
                            Some(Ok((time_table[time_idx as usize], String::new())))
                        } else {
                            None
                        }
                    })
                    .collect()
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

            match Python::with_gil(|py| -> PyResult<Vec<Transaction>> {
                let res = self
                    .obj
                    .getattr(py, "analyze")?
                    .call1(py, PyTuple::new(py, loaded)?)?;
                res.extract(py)
            }) {
                Ok(results) => {
                    for Transaction {
                        start: time,
                        resp_time,
                        last_data: last_write,
                        first_data,
                        resp,
                        next_start: next,
                    } in results
                    {
                        usage.add_transaction(time, resp_time, last_write, first_data, &resp, next);
                    }
                }
                Err(e) => Err(format!(
                    "{} {} {}",
                    "python plugin returned bad result for bus".bright_red(),
                    self.description.common.bus_name().bright_red(),
                    e.bright_red()
                ))?,
            };
        }
        usage.add_time(time_table[time_end as usize]);
        usage.end(reset, intervals);

        if let Ok(method) = Python::with_gil(|py| self.obj.getattr(py, "get_trigger_times")) {
            match Python::with_gil(|py| -> PyResult<HashMap<String, Vec<RealTime>>> {
                method.call0(py)?.extract(py)
            }) {
                Ok(results) => {
                    for (name, times) in results {
                        self.provided_python
                            .iter_mut()
                            .find(|p| {
                                p.name()
                                    == format!("interfaces.{}.{}", self.description.name(), name)
                            })
                            .ok_or("python plugin error - returns trigger it did not define")?
                            .set_result(times);
                    }
                }
                Err(e) => Err(format!(
                    "python plugin failed - defines triggers but provides bad value: {e}"
                ))?,
            }
        }

        Ok(BusUsage::MultiChannel(usage))
    }

    fn requires(&self) -> Vec<&str> {
        self.required.iter().map(|n| n.as_str()).collect()
    }

    fn provides(&self) -> Vec<&str> {
        self.provided
            .iter()
            .map(|p| p.name())
            .chain(self.provided_python.iter().map(|p| p.name()))
            .collect()
    }

    fn sink(&self) -> &TriggerSink {
        &self.sink
    }

    fn consume(
        mut self: Box<Self>,
    ) -> (
        String,
        std::rc::Rc<dyn crate::analyze::bus::BusDescription>,
        Vec<Box<dyn crate::analyze::trigger::TriggerSource>>,
    ) {
        self.provided_python
            .into_iter()
            .for_each(|p| self.provided.push(Box::new(p)));
        (
            self.description.common.bus_name().to_owned(),
            self.description,
            self.provided,
        )
    }
}
