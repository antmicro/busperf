use crate::{
    analyzer::{axi_analyzer::ReadyValidTransactionIterator, private::AnalyzerInternal},
    bus::{BusCommon, SignalPath, is_value_of_type},
    bus_usage::{BusUsage, MultiChannelBusUsage},
    plugins::load_python_plugin,
};

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

// u32 is an enum
// 1 - Signal
// 2 - RisingSignal
// 3 - ReadyValid
type SignalInfo = (u32, Vec<SignalPath>);

impl PythonAnalyzer {
    pub fn new(
        class_name: &str,
        common: BusCommon,
        i: &Yaml,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let obj = load_python_plugin(class_name)?;

        let signals = Python::with_gil(
            #[allow(clippy::type_complexity)]
            |py| -> Result<Vec<(u32, Vec<String>)>, Box<dyn std::error::Error>> {
                Ok(obj
                    .getattr(py, "get_yaml_signals")?
                    .call0(py)
                    .map_err(|_| "'get_yaml_signals' object is not callable")?
                    .extract::<Vec<(u32, Vec<String>)>>(py)
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
                let a: Result<(u32, Vec<SignalPath>), Box<dyn std::error::Error>> = match type_ {
                    1 | 2 => {
                        match SignalPath::from_yaml_ref_with_prefix(common.module_scope(), i) {
                            Ok(path) => {
                                used_yaml.push(name);
                                Ok((*type_, vec![path]))
                            }
                            Err(_) => Err(format!("Yaml should define {} signal", name))?,
                        }
                    }
                    3 => {
                        used_yaml.push(name.clone() + ".ready");
                        used_yaml.push(name.clone() + ".valid");
                        let r = SignalPath::from_yaml_ref_with_prefix(
                            common.module_scope(),
                            &i["ready"],
                        )
                        .map_err(|_| format!("Yaml is missing ready signal for {name}",))?;
                        let v = SignalPath::from_yaml_ref_with_prefix(
                            common.module_scope(),
                            &i["valid"],
                        )
                        .map_err(|_| format!("Yaml is missing valid signal for {name}",))?;
                        Ok((*type_, vec![r, v]))
                    }
                    other => Err(format!(
                        "Invalid type of signal {other} for {name}, but can be only 1, 2, 3"
                    ))?,
                };
                a
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
        loaded: Vec<(wellen::SignalRef, wellen::Signal)>,
        time_table: &TimeTable,
    ) {
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
        let (time_end, _) = clk.iter_changes().last().expect("Clock should have cycles");

        let mut i = 0;
        let loaded: Vec<_> = [(1, vec![]), (2, vec![])]
            .iter()
            .chain(self.signals.iter())
            .map(|(type_, _)| match type_ {
                1 | 2 => {
                    let (_, signal) = &loaded[i];
                    i += 1;
                    signal
                        .iter_changes()
                        .map(|(t, v)| (t, v.to_bit_string().expect("Function never returns None")))
                        .collect::<Vec<(u32, String)>>()
                }
                3 => {
                    let (_, ready) = &loaded[i];
                    let (_, valid) = &loaded[i + 1];
                    i += 2;
                    let a = ReadyValidTransactionIterator::new(clk, ready, valid, time_end);
                    a.map(|time| (time, String::new())).collect()
                }
                _ => unreachable!("Would fail during loading of signals"),
            })
            .collect();

        #[allow(clippy::type_complexity)]
        let results = Python::with_gil(|py| -> PyResult<Vec<(u32, u32, u32, u32, String, u32)>> {
            let res = self
                .obj
                .getattr(py, "analyze")?
                .call1(py, PyTuple::new(py, loaded)?)?;
            res.extract(py)
        })
        .unwrap_or_else(|e| {
            panic!(
                "Python plugin returned bad result for bus {} {e}",
                self.common.bus_name()
            )
        });
        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            time_table[2],
            self.x_rate,
            self.y_rate,
        );
        usage.add_time(time_table[time_end as usize]);
        for (time, resp_time, last_write, first_data, resp, next) in results {
            let [time, resp_time, last_write, first_data, next] =
                [time, resp_time, last_write, first_data, next].map(|i| time_table[i as usize]);
            usage.add_transaction(time, resp_time, last_write, first_data, &resp, next);
        }
        usage.end(reset, vec![]);

        self.result = Some(BusUsage::MultiChannel(usage));
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
