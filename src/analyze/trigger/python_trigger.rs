use std::error::Error;

use libbusperf::bus_usage::RealTime;
use pyo3::{Py, PyAny, PyResult, Python};

use crate::analyze::trigger::{TriggerResult, TriggerSource};

pub struct PythonTrigger {
    result: TriggerResult,
}

impl PythonTrigger {
    pub fn vec_from_obj(
        obj: &Py<PyAny>,
        bus_name: &str,
        initialized: bool,
    ) -> Result<Vec<PythonTrigger>, Box<dyn Error>> {
        if let Ok(method) = Python::with_gil(|py| obj.getattr(py, "provides")) {
            match Python::with_gil(|py| -> PyResult<Vec<String>> { method.call0(py)?.extract(py) })
            {
                Ok(results) => Ok(results
                    .into_iter()
                    .map(|name| {
                        if initialized {
                            PythonTrigger::new_empty(format!("interfaces.{}.{}", bus_name, name))
                        } else {
                            PythonTrigger::new(format!("interfaces.{}.{}", bus_name, name))
                        }
                    })
                    .collect()),
                Err(e) => Err(format!(
                    "python plugin failed - bad return from provides method: {e}"
                ))?,
            }
        } else {
            Ok(vec![])
        }
    }
    pub fn new(name: String) -> Self {
        Self {
            result: (
                name,
                Err("plugin defined a trigger but did not return its activation times".into()),
            ),
        }
    }
    pub fn new_empty(name: String) -> Self {
        Self {
            result: (name, Ok(vec![])),
        }
    }
    pub fn add_time(&mut self, time: RealTime) {
        match &mut self.result.1 {
            Ok(times) => times.push(time),
            Err(_) => self.result.1 = Ok(vec![time]),
        }
    }
    pub fn set_result(&mut self, times: Vec<RealTime>) {
        self.result.1 = Ok(times);
    }
}

impl TriggerSource for PythonTrigger {
    fn name(&self) -> &str {
        let (name, _) = &self.result;
        name
    }

    fn into_name(self: Box<Self>) -> String {
        let (name, _) = self.result;
        name
    }

    fn analyze(
        self: Box<Self>,
        _simulation_data: &mut crate::analyze::SimulationData,
        _loaded: &[&wellen::Signal],
        _intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        _done_triggers: &crate::analyze::DoneTriggers,
        _bus_usage: &Result<libbusperf::bus_usage::BusUsage, Box<dyn std::error::Error>>,
    ) -> TriggerResult {
        self.result
    }
}
