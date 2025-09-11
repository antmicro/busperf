use std::{error::Error, ffi::CString, path::PathBuf};

use pyo3::prelude::*;

pub fn load_python_plugin(class_name: &str) -> Result<Py<PyAny>, Box<dyn Error>> {
    let code = load_python_code(class_name)?;
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
    })
    .map_err(|e| format!("Failed to load plugin {class_name}, {e}"))?;
    Ok(obj)
}

fn load_python_code(class_name: &str) -> Result<CString, Box<dyn Error>> {
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
    path.push(format!("plugins/python/{class_name}.py")); // add path to the plugin
    Ok(CString::new(std::fs::read_to_string(path)?)?)
}
