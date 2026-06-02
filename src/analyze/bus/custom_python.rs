use std::error::Error;
use std::rc::Rc;

use crate::analyze::SignalValue;
use crate::analyze::bus::{
    BusCommon, BusDescription, COMMON_YAML, ExtraSignals, SignalPath, SignalPathFromYaml,
    bus_description,
};
use crate::analyze::plugins::load_python_plugin;
use crate::analyze::trigger::PythonTrigger;

use super::LockstepAnalyzer;
use owo_colors::OwoColorize;
use pyo3::{
    prelude::*,
    types::{PyList, PyTuple},
};
use yaml_rust2::Yaml;

pub struct PythonCustomBus {
    common: Rc<BusCommon>,
    obj: Py<PyAny>,
    extra_signals: ExtraSignals,
}

#[pyclass]
#[derive(Clone, Copy)]
pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
    Reset,
    Unknown,
}

impl From<CycleType> for libbusperf::CycleType {
    fn from(value: CycleType) -> Self {
        match value {
            CycleType::Busy => libbusperf::CycleType::Busy,
            CycleType::Free => libbusperf::CycleType::Free,
            CycleType::NoTransaction => libbusperf::CycleType::NoTransaction,
            CycleType::Backpressure => libbusperf::CycleType::Backpressure,
            CycleType::NoData => libbusperf::CycleType::NoData,
            CycleType::Reset => libbusperf::CycleType::Reset,
            CycleType::Unknown => libbusperf::CycleType::Unknown,
        }
    }
}

impl PythonCustomBus {
    pub fn from_yaml(
        class_name: &str,
        name: String,
        i: &Yaml,
        plugins_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let common = Rc::new(BusCommon::from_yaml(name, i)?);
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

        let handled = COMMON_YAML
            .iter()
            .copied()
            .chain(signals.iter().map(|s| s.as_str()))
            .collect::<Vec<_>>();
        let mut unhandled_signals = Vec::new();
        for (name, yaml) in i.as_hash().expect("already checked") {
            let name = name.as_str().ok_or("invalid signal name")?;
            if !handled.contains(&name) {
                unhandled_signals.push((
                    name.to_owned(),
                    SignalPathFromYaml::from_yaml_ref_with_prefix(&common.module_scope, yaml)?,
                ));
            }
        }

        let paths: Vec<SignalPath> = signals
            .iter()
            .map(|s| {
                SignalPathFromYaml::from_yaml_ref_with_prefix(&common.module_scope, &i[s.as_str()])
            })
            .collect::<Result<_, _>>()?;
        let mut extra_signals: Vec<_> = signals.into_iter().zip(paths).collect();
        extra_signals.append(&mut unhandled_signals);
        Ok(PythonCustomBus {
            common,
            obj,
            extra_signals,
        })
    }
    pub fn provides(&self) -> Result<Vec<PythonTrigger>, Box<dyn Error>> {
        PythonTrigger::vec_from_obj(&self.obj, self.name(), true)
    }
    pub fn get_triggers(&self, signals: &[SignalValue<'_>]) -> Result<Vec<String>, Box<dyn Error>> {
        let signals: Vec<String> = signals
            .iter()
            .map(|s| s.to_bit_string().ok_or("invalid signal value at"))
            .collect::<Result<_, _>>()?;

        Ok(Python::with_gil(|py| -> PyResult<Vec<String>> {
            let method = self.obj.getattr(py, "get_trigger");
            match method {
                Ok(method) => {
                    let obj = method.call1(py, PyTuple::new(py, PyList::new(py, signals))?)?;
                    obj.extract(py)
                }
                Err(_) => Ok(vec![]),
            }
        })?)
    }
}

bus_description!(PythonCustomBus,);

impl LockstepAnalyzer for PythonCustomBus {
    fn interpret_cycle(&self, signals: &[SignalValue<'_>], _time: u32) -> libbusperf::CycleType {
        let signals: Vec<String> = match signals
            .iter()
            .map(|s| {
                s.to_bit_string()
                    .ok_or(format!("invalid signal value at {}", _time))
            })
            .collect::<Result<_, _>>()
        {
            Ok(signals) => signals,
            Err(e) => {
                eprintln!("{e}");
                return libbusperf::CycleType::Unknown;
            }
        };

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
            CycleType::Unknown
        })
        .into()
    }
}
