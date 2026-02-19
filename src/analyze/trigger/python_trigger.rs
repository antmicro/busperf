use libbusperf::bus_usage::RealTime;

use crate::analyze::trigger::{TriggerResult, TriggerSource};

pub struct PythonTrigger {
    result: TriggerResult,
}

impl PythonTrigger {
    pub fn new(name: String) -> Self {
        Self {
            result: (
                name,
                Err("plugin defined a trigger but did not return its activation times".into()),
            ),
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
        _loaded: &[&(wellen::SignalRef, wellen::Signal)],
        _intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        _done_triggers: &crate::analyze::DoneTriggers,
        _bus_usage: &Result<libbusperf::bus_usage::BusUsage, Box<dyn std::error::Error>>,
    ) -> TriggerResult {
        self.result
    }
}
