use std::error::Error;

use super::private::AnalyzerInternal;
use crate::analyze::{
    analyzer::axi_analyzer::ReadyValidTransactionIterator,
    bus::{is_value_of_type, BusCommon, SignalPath, SignalPathFromYaml},
    plugins::load_python_plugin,
};
use libbusperf::bus_usage::{BusUsage, MultiChannelBusUsage, RealTime};
use owo_colors::OwoColorize;

use super::Analyzer;
use pyo3::{prelude::*, types::PyTuple};
use wellen::TimeTable;
use yaml_rust2::Yaml;

pub struct PythonAnalyzer {
    common: BusCommon,
    obj: Py<PyAny>,
    result: Option<BusUsage>,
    signals: Vec<SignalInfo>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
    used_yaml: Vec<String>,
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
#[derive(Clone, Copy)]
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
        class_name: &str,
        common: BusCommon,
        i: &Yaml,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
        plugins_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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
        let mut used_yaml: Vec<_> = super::COMMON_YAML.iter().map(|s| s.to_string()).collect();
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
                            Ok(path) => {
                                used_yaml.push(name);
                                Ok((*type_, vec![path]))
                            }
                            Err(_) => Err(format!("Yaml should define {} signal", name))?,
                        }
                    }
                    SignalType::ReadyValid => {
                        used_yaml.push(name.clone() + ".ready");
                        used_yaml.push(name.clone() + ".valid");
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

        Ok(PythonAnalyzer {
            common,
            obj,
            result: None,
            signals,
            window_length,
            x_rate,
            y_rate,
            used_yaml,
        })
    }
}

impl AnalyzerInternal for PythonAnalyzer {
    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.signals.iter().flat_map(|(_, path)| path).collect());

        signals
    }

    fn calculate(
        &mut self,
        loaded: Vec<&(wellen::SignalRef, wellen::Signal)>,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let mut last = 0;
        let mut reset = 0;
        for (time, value) in rst.iter_changes() {
            if is_value_of_type(value, self.common.rst_active_value()) {
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
            self.common.bus_name(),
            self.window_length,
            *time_table.get(2).ok_or(
                "trace is too short (less than 3 time indices), cannot calculate clock period",
            )?,
            self.x_rate,
            self.y_rate,
        );

        let intervals = if self.common.intervals().is_empty() {
            vec![[0, time_table[time_end as usize]]]
        } else {
            self.common.intervals().clone()
        };
        for [start, end] in intervals {
            let mut i = 0;
            let loaded: Vec<_> = [
                (SignalType::Signal, vec![]),
                (SignalType::RisingSignal, vec![]),
            ]
            .iter()
            .chain(self.signals.iter())
            .map(|(type_, _)| match type_ {
                SignalType::Signal | SignalType::RisingSignal => {
                    let (_, signal) = &loaded[i];
                    i += 1;
                    signal
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
                        .collect::<Result<Vec<(RealTime, String)>, _>>()
                }
                SignalType::ReadyValid => {
                    let (_, ready) = &loaded[i];
                    let (_, valid) = &loaded[i + 1];
                    i += 2;
                    let a = ReadyValidTransactionIterator::new(clk, ready, valid, time_end);
                    a.filter_map(|time_idx| {
                        let time = time_table[time_idx as usize];
                        if time >= start && time < end {
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
                    self.common.bus_name().bright_red(),
                    e.bright_red()
                ))?,
            };
        }
        usage.add_time(time_table[time_end as usize]);
        usage.end(reset, vec![[0, time_table[time_end as usize]]]);

        self.result = Some(BusUsage::MultiChannel(usage));
        Ok(())
    }
}

impl Analyzer for PythonAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }

    fn required_yaml_definitions(&self) -> Vec<&str> {
        self.used_yaml.iter().map(|s| s.as_str()).collect()
    }
}
