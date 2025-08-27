use std::ffi::CString;

use crate::{
    bus::BusCommon, bus_usage::MultiChannelBusUsage, load_signals, BusUsage, SingleChannelBusUsage,
};

use super::Analyzer;
use pyo3::{prelude::*, types::PyTuple};
use yaml_rust2::Yaml;

pub struct PythonAnalyzer {
    common: BusCommon,
    obj: Py<PyAny>,
    result: Option<BusUsage>,
    signals: Vec<String>,
}

impl PythonAnalyzer {
    pub fn new(class_name: &str, common: BusCommon, i: &Yaml) -> Self {
        let mut s = String::from(concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/python/"));
        s.push_str(class_name);
        s.push_str(".py");
        let code = CString::new(
            std::fs::read_to_string(s).expect(&format!("{} does not exist", class_name)),
        )
        .unwrap();

        let obj = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let app: Py<PyAny> = PyModule::from_code(
                py,
                &code,
                &CString::new(class_name).unwrap(),
                &CString::new(class_name).unwrap(),
            )
            .unwrap()
            .getattr("create")?
            .into();

            app.call0(py)
        })
        .expect(&format!("Could not initialize {}", class_name));

        let signals = Python::with_gil(|py| -> PyResult<Vec<String>> {
            obj.getattr(py, "get_yaml_signals")?
                .call0(py)?
                .extract::<Vec<String>>(py)
        })
        .expect("Python plugin returned bad signal names");
        let signals = signals
            .iter()
            .map(|s| {
                i[s.as_str()]
                    .as_str()
                    .expect(&format!("yaml does not have {} signal", s))
                    .to_owned()
            })
            .collect();

        PythonAnalyzer {
            common,
            obj,
            result: None,
            signals,
        }
    }
}

impl Analyzer for PythonAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let mut signals = vec![self.common.clk_name(), self.common.rst_name()];
        signals.append(&mut self.signals.iter().map(|s| s.as_str()).collect());

        let start = std::time::Instant::now();
        let loaded = load_signals(simulation_data, self.common.module_scope(), &signals);
        let (_, rst) = &loaded[1];
        let mut last = 0;
        let mut reset = 0;
        for (time, value) in rst.iter_changes() {
            if value.to_bit_string().unwrap() == self.common.rst_active_value().to_string() {
                last = time;
            } else {
                reset += time - last;
            }
        }
        reset = reset / 2;

        let loaded: Vec<_> = loaded
            .iter()
            .map(|(_, signal)| {
                signal
                    .iter_changes()
                    .map(|(t, v)| (t, v.to_bit_string().unwrap()))
                    .collect::<Vec<(u32, String)>>()
            })
            .collect();
        if verbose {
            println!(
                "Loading {} took {:?}",
                self.common.bus_name(),
                start.elapsed()
            );
        }
        let results = Python::with_gil(|py| -> PyResult<Vec<(u32, u32, u32, u32, String, u32)>> {
            self.obj
                .getattr(py, "analyze")?
                .call1(py, PyTuple::new(py, loaded).unwrap())?
                .extract(py)
        })
        .expect("Python plugin returned bad result");
        let mut usage = MultiChannelBusUsage::new(self.common.bus_name(), 10000, 0.0006, 0.00001);
        for r in results {
            usage.add_transaction(r.0, r.1, r.2, r.3, &r.4, r.5);
        }

        usage.end(reset);
        self.result = Some(BusUsage::MultiChannel(usage));
    }

    fn get_results(&self) -> &crate::BusUsage {
        self.result.as_ref().unwrap()
    }
}
