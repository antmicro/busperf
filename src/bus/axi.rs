use wellen::SignalValue;
use yaml_rust2::Yaml;

use super::{BusDescription, SignalPath, ValueType, bus_from_yaml, is_value_of_type};
use crate::CycleType;

#[derive(Debug)]
pub struct AXIBus {
    ready: SignalPath,
    valid: SignalPath,
}

impl AXIBus {
    bus_from_yaml!(AXIBus, ready, valid);
    pub fn new(ready: SignalPath, valid: SignalPath) -> Self {
        AXIBus { ready, valid }
    }
}

impl BusDescription for AXIBus {
    fn signals(&self) -> Vec<&SignalPath> {
        vec![&self.ready, &self.valid]
    }

    fn interpret_cycle(&self, signals: &[SignalValue<'_>], _time: u32) -> CycleType {
        let ready = signals[0];
        let valid = signals[1];
        match (
            is_value_of_type(ready, ValueType::V1),
            is_value_of_type(valid, ValueType::V1),
        ) {
            (true, true) => CycleType::Busy,
            (false, false) => CycleType::Free,
            (true, false) => CycleType::NoData,
            (false, true) => CycleType::Backpressure,
        }
    }
}
