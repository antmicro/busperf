use std::{ffi::CString, path::PathBuf};

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
    pub fn new(class_name: &str, i: &Yaml) -> Result<Self, Box<dyn std::error::Error>> {
        // if CARGO_MANIFEST_DIR is set we search in that directory if not we want to search in the location of the binary
        let mut path = match std::env::var("CARGO_MANIFEST_DIR") {
            Ok(path) => PathBuf::from(path),
            Err(_) => {
                match std::env::current_exe() {
                    Ok(mut path) => {
                        path.pop(); // remove executable name
                        path
                    }
                    Err(_) => Err("Failed to get plugins path.")?,
                }
            }
        };
        path.push(format!("plugins/python/{}.py", class_name)); // add path to the plugin
        let code = CString::new(
            std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to load plugin {}, {}", class_name, e))?,
        )?;

        let obj = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let app: Py<PyAny> = PyModule::from_code(
                py,
                &code,
                &CString::new(class_name)?,
                &CString::new(class_name)?,
            )?
            .getattr("create")?
            .into();

            app.call0(py)
        })?;

        let signals = Python::with_gil(|py| -> PyResult<Vec<String>> {
            obj.getattr(py, "get_signals")?
                .call0(py)?
                .extract::<Vec<String>>(py)
        })?;
        let signals = signals
            .iter()
            .map(|s| match i[s.as_str()].as_str() {
                Some(string) => Ok(string.to_owned()),
                None => Err(format!("Yaml should define {} signal", s)),
            })
            .collect::<Result<_, _>>()?;
        Ok(PythonCustomBus { obj, signals })
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
