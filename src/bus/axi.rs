use wellen::SignalValue;
use yaml_rust2::Yaml;

use crate::CycleType;

use super::BusCommon;
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
        return Ok(AXIBus::new(ready.to_owned(), valid.to_owned()));
    }
    pub fn new(ready: String, valid: String) -> Self {
        AXIBus { ready, valid }
    }
}

impl BusDescription for AXIBus {
    fn signals(&self) -> Vec<&str> {
        vec![self.ready.as_str(), self.valid.as_str()]
    }

    fn interpret_cycle(&self, signals: Vec<SignalValue>, time: u32) -> CycleType {
        let ready = signals[0];
        let valid = signals[1];
        if let Ok(ready) = ready.to_bit_string().unwrap().parse::<u32>()
            && let Ok(valid) = valid.to_bit_string().unwrap().parse::<u32>()
        {
            let t = match (ready, valid) {
                (1, 1) => CycleType::Busy,
                (0, 0) => CycleType::Free,
                (1, 0) => CycleType::NoData,
                (0, 1) => CycleType::Backpressure,
                _ => panic!("signal has invalid value ready: {} valid: {}", ready, valid),
            };
            t
        } else {
            eprintln!(
                "bus \"{}\" in unknown state outside reset - ready: {}, valid: {}, time: {}",
                "FIXME", ready, valid, time
            );
            CycleType::NoTransaction
        }
    }
}
