use crate::{
    analyzer::private::AnalyzerInternal,
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
    signals: Vec<SignalPath>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

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

        let Ok(signals) = Python::with_gil(|py| -> PyResult<Vec<String>> {
            obj.getattr(py, "get_yaml_signals")?
                .call0(py)?
                .extract::<Vec<String>>(py)
        }) else {
            return Err("Python plugin returned bad signal names")?;
        };
        let signals = signals
            .iter()
            .map(|s| match i[s.as_str()].as_str() {
                Some(string) => Ok(SignalPath {
                    scope: common.module_scope().clone(),
                    name: string.to_owned(),
                }),
                None => Err(format!("Yaml should define {} signal", s)),
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
        })
    }
}

impl AnalyzerInternal for PythonAnalyzer {
    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.signals.iter().collect());

        signals
    }

    fn calculate(
        &mut self,
        loaded: Vec<(wellen::SignalRef, wellen::Signal)>,
        time_table: &TimeTable,
    ) {
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

        let loaded: Vec<_> = loaded
            .iter()
            .map(|(_, signal)| {
                signal
                    .iter_changes()
                    .map(|(t, v)| (t, v.to_bit_string().expect("Function never returns None")))
                    .collect::<Vec<(u32, String)>>()
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
        .unwrap_or_else(|_| {
            panic!(
                "Python plugin returned bad result {} ",
                self.common.bus_name()
            )
        });
        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            time_table[2],
            self.x_rate,
            self.y_rate,
            reset as u64 * time_table[1],
        );
        for (time, resp_time, last_write, first_data, resp, next) in results {
            let [time, resp_time, last_write, first_data, next] =
                [time, resp_time, last_write, first_data, next].map(|i| time_table[i as usize]);
            usage.add_transaction(time, resp_time, last_write, first_data, &resp, next);
        }
        usage.end(reset);

        self.result = Some(BusUsage::MultiChannel(usage));
    }
}

impl Analyzer for PythonAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }
}
