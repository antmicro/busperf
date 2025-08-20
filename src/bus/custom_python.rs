use std::ffi::CString;

use super::BusDescription;
use pyo3::{ffi::c_str, prelude::*};

pub struct PythonCustomBus {
    class: String,
    obj: Py<PyAny>,
}

impl PythonCustomBus {
    pub fn new(class_name: &str) -> Self {
        todo!();
        let mut s = String::from("plugins/python");
        s.push_str(class_name);
        let code = CString::new(s).unwrap();

        let from_python = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
            Into::<Py<PyAny>>::into(
                PyModule::from_code(
                    py,
                    &code,
                    &CString::new(class_name).unwrap(),
                    &CString::new(class_name).unwrap(),
                )?
                .getattr("create")?,
            )
            .call0(py)
        });
        PythonCustomBus {
            class: class_name.to_owned(),
            obj: from_python.unwrap(),
        }
    }
}

impl BusDescription for PythonCustomBus {
    fn bus_name(&self) -> &str {
        todo!()
    }

    fn common(&self) -> &super::BusCommon {
        todo!()
    }

    fn signals(&self) -> Vec<&str> {
        todo!()
    }

    fn interpret_cycle(&self, signals: Vec<wellen::SignalValue>, time: u32) -> crate::CycleType {
        todo!()
    }
}
