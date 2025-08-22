use std::ffi::CString;

use pyo3::prelude::*;

use crate::bus::BusCommon;

use super::Analyzer;

pub struct PythonAnalyzer {
    obj: Py<PyAny>,
}

impl PythonAnalyzer {
    pub fn new(class_name: &str) -> Self {
        let mut s = String::from(concat!(env!("CARGO_MANIFEST_DIR"), "/plugins/python/"));
        s.push_str(class_name);
        s.push_str(".py");
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
        PythonAnalyzer { obj }
    }
}

impl Analyzer for PythonAnalyzer {
    fn load_buses(
        &self,
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Vec<Box<dyn crate::bus::BusDescription>>, Box<dyn std::error::Error>> {
        let common = BusCommon::from_yaml(yaml, default_max_burst_delay);
        todo!()
    }

    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        todo!()
    }

    fn get_results(&self) -> &crate::BusUsage {
        todo!()
    }
}
