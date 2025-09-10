use crate::plugins::load_python_plugin;

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
        let obj = load_python_plugin(class_name)?;
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
