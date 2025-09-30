use crate::{bus::SignalPath, plugins::load_python_plugin};

use super::BusDescription;
use pyo3::{
    prelude::*,
    types::{PyList, PyTuple},
};
use wellen::SignalValue;
use yaml_rust2::Yaml;

pub struct PythonCustomBus {
    obj: Py<PyAny>,
    signals: Vec<SignalPath>,
}

impl PythonCustomBus {
    pub fn from_yaml(
        class_name: &str,
        i: &Yaml,
        bus_scope: &[String],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let obj = load_python_plugin(class_name)?;
        let signals = Python::with_gil(|py| -> PyResult<Vec<String>> {
            obj.getattr(py, "get_signals")?
                .call0(py)?
                .extract::<Vec<String>>(py)
        })?;
        let signals = signals
            .iter()
            .map(|s| SignalPath::from_yaml_ref_with_prefix(bus_scope, &i[s.as_str()]))
            .collect::<Result<_, _>>()?;
        Ok(PythonCustomBus { obj, signals })
    }
}

impl BusDescription for PythonCustomBus {
    fn signals(&self) -> Vec<&SignalPath> {
        self.signals.iter().collect()
    }

    fn interpret_cycle(&self, signals: &[SignalValue<'_>], _time: u32) -> crate::CycleType {
        let signals: Vec<String> = signals
            .iter()
            .map(|s| s.to_bit_string().expect("Function never returns None"))
            .collect();
        let ret = Python::with_gil(|py| -> PyResult<u32> {
            self.obj
                .getattr(py, "interpret_cycle")?
                .call1(py, PyTuple::new(py, PyList::new(py, signals))?)?
                .extract(py)
        })
        .expect("Python returned bad result");
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
