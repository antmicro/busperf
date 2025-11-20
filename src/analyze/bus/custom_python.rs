use crate::CycleType;
use crate::analyze::bus::SignalPath;
use crate::analyze::plugins::load_python_plugin;

use super::BusDescription;
use owo_colors::OwoColorize;
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
        plugins_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Python::with_gil(|py| -> PyResult<()> {
            let module = match py.import("sys")?.getattr("modules")?.get_item("busperf") {
                Ok(module) => module.extract()?,
                _ => PyModule::new(py, "busperf")?,
            };
            module.add_class::<CycleType>()?;
            py.import("sys")?
                .getattr("modules")?
                .set_item("busperf", module)?;
            Ok(())
        })?;
        let obj = load_python_plugin(plugins_path, class_name)?;
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

        Python::with_gil(|py| -> PyResult<CycleType> {
            let obj = self
                .obj
                .getattr(py, "interpret_cycle")?
                .call1(py, PyTuple::new(py, PyList::new(py, signals))?)?;

            let o = obj.extract::<Py<CycleType>>(py)?;
            Ok(*o.borrow(py))
        })
        .unwrap_or_else(|e| {
            eprintln!(
                "{} {}",
                "[ERROR] Python returned bad result".bright_red(),
                e.bright_red()
            );
            crate::CycleType::Unknown
        })
    }
}
