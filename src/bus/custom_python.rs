use std::ffi::CString;

use super::{BusCommon, BusDescription};
use pyo3::{
    prelude::*,
    types::{PyList, PyTuple},
};
use yaml_rust2::Yaml;

pub struct PythonCustomBus {
    common: BusCommon,
    obj: Py<PyAny>,
    signals: Vec<String>,
}

impl PythonCustomBus {
    pub fn new(common: BusCommon, class_name: &str, i: &Yaml) -> Self {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/python/");
        let mut s = String::from(concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/python/"));
        s.push_str(class_name);
        s.push_str(".py");
        let code = CString::new(std::fs::read_to_string(s).unwrap()).unwrap();
        // println!("{:?}", code);

        let obj = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            let syspath = py
                .import("sys")?
                .getattr("path")?
                .downcast_into::<PyList>()?;
            syspath.insert(0, path)?;
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
            PyResult::Ok(
                obj.getattr(py, "get_signals")?
                    .call0(py)?
                    .extract::<Vec<String>>(py)?,
            )
        })
        .unwrap();
        let signals = signals
            .iter()
            .map(|s| i[s.as_str()].as_str().unwrap().to_owned())
            .collect();
        PythonCustomBus {
            common,
            obj,
            signals,
        }
    }
}

impl BusDescription for PythonCustomBus {
    fn signals(&self) -> Vec<&str> {
        self.signals.iter().map(|s| s.as_str()).collect()
    }

    fn interpret_cycle(&self, signals: Vec<wellen::SignalValue>, _time: u32) -> crate::CycleType {
        let signals: Vec<String> = signals.iter().map(|s| s.to_bit_string().unwrap()).collect();
        let ret = Python::with_gil(|py| -> PyResult<u32> {
            PyResult::Ok(
                self.obj
                    .getattr(py, "interpret_cycle")?
                    .call1(py, PyTuple::new(py, PyList::new(py, signals)).unwrap())?
                    .extract(py)?,
            )
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
