use wellen::SignalValue;

use crate::CycleType;

use super::BusCommon;
use super::BusDescription;
use super::CyclesNum;

#[derive(Debug)]
pub struct AXIBus {
    common: BusCommon,
    ready: String,
    valid: String,
}

impl AXIBus {
    pub fn new(
        bus_name: String,
        module_scope: Vec<String>,
        clk_name: String,
        rst_name: String,
        rst_active_value: u8,
        max_burst_delay: CyclesNum,
        ready: String,
        valid: String,
    ) -> Self {
        AXIBus {
            common: BusCommon {
                bus_name,
                module_scope,
                clk_name,
                rst_name,
                rst_active_value,
                max_burst_delay,
            },
            ready,
            valid,
        }
    }
}

impl BusDescription for AXIBus {
    fn bus_name(&self) -> &str {
        &self.common.bus_name
    }

    fn common(&self) -> &super::BusCommon {
        &self.common
    }

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
                self.bus_name(),
                ready,
                valid,
                time
            );
            CycleType::NoTransaction
        }
    }
}
