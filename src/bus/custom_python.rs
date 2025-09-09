use std::ffi::CString;

use super::BusDescription;
use pyo3::{
    prelude::*,
    types::{PyList, PyTuple},
};
use wellen::SignalValue;
use yaml_rust2::Yaml;

pub struct PythonCustomBus {
    obj: Py<PyAny>,
    signals: Vec<String>,
}

impl PythonCustomBus {
    pub fn new(class_name: &str, i: &Yaml) -> Self {
        // we want to search in the location of the binary
        let s = match std::env::current_exe() {
            Ok(mut path) => {
                path.pop(); // remove executable name
                path.push(format!("plugins/python/{}.py", class_name)); // add path to the plugin
                path
            }
            Err(_) => todo!(),
        };
        let code = CString::new(std::fs::read_to_string(s).unwrap()).unwrap();

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
        .unwrap();

        let signals = Python::with_gil(|py| -> PyResult<Vec<String>> {
            obj.getattr(py, "get_signals")?
                .call0(py)?
                .extract::<Vec<String>>(py)
        })
        .unwrap();
        let signals = signals
            .iter()
            .map(|s| i[s.as_str()].as_str().unwrap().to_owned())
            .collect();
        PythonCustomBus { obj, signals }
    }
}

impl BusDescription for PythonCustomBus {
    fn signals(&self) -> Vec<&str> {
        self.signals.iter().map(|s| s.as_str()).collect()
    }

    fn interpret_cycle(&self, signals: &[SignalValue<'_>], _time: u32) -> crate::CycleType {
        let signals: Vec<String> = signals.iter().map(|s| s.to_bit_string().unwrap()).collect();
        let ret = Python::with_gil(|py| -> PyResult<u32> {
            self.obj
                .getattr(py, "interpret_cycle")?
                .call1(py, PyTuple::new(py, PyList::new(py, signals)).unwrap())?
                .extract(py)
        })
        .unwrap();
        match ret {
            0 => crate::CycleType::Busy,
            1 => crate::CycleType::Free,
            2 => crate::CycleType::NoTransaction,
            3 => crate::CycleType::Backpressure,
            4 => crate::CycleType::NoData,
            _ => panic!("Invalid return from python"),
        }
    }
}
