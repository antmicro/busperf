use wellen::SignalValue;
use yaml_rust2::Yaml;

use crate::{
    CycleType,
    bus::{ValueType, is_value_of_type},
};

use super::BusDescription;

#[derive(Debug)]
pub struct AXIBus {
    ready: String,
    valid: String,
}

impl AXIBus {
    pub fn from_yaml(yaml: &Yaml) -> Result<Self, Box<dyn std::error::Error>> {
        let ready = yaml["ready"]
            .as_str()
            .ok_or("ReadyValid bus requires ready signal")?;
        let valid = yaml["valid"]
            .as_str()
            .ok_or("ReadyValid bus requires valid signal")?;
        Ok(AXIBus::new(ready.to_owned(), valid.to_owned()))
    }
    pub fn new(ready: String, valid: String) -> Self {
        AXIBus { ready, valid }
    }
}

impl BusDescription for AXIBus {
    fn signals(&self) -> Vec<&str> {
        vec![self.ready.as_str(), self.valid.as_str()]
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
