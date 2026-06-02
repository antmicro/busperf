use std::rc::Rc;

use yaml_rust2::Yaml;

use crate::analyze::{
    SignalValue,
    bus::{BusCommon, ExtraSignals, bus_description},
};

use super::{
    BusDescription, LockstepAnalyzer, SignalPath, ValueType, bus_from_yaml, is_value_of_type,
};
use libbusperf::CycleType;

#[derive(Debug)]
pub struct AXIBus {
    common: Rc<BusCommon>,
    ready: SignalPath,
    valid: SignalPath,
    extra_signals: ExtraSignals,
}

impl AXIBus {
    bus_from_yaml!(AXIBus, ready, valid);
    pub fn new(
        common: Rc<BusCommon>,
        ready: SignalPath,
        valid: SignalPath,
        extra_signals: ExtraSignals,
    ) -> Self {
        AXIBus {
            common,
            ready,
            valid,
            extra_signals,
        }
    }
}

bus_description!(AXIBus, ready, valid);

pub struct ReadyValidAnalyzer;

impl ReadyValidAnalyzer {
    pub fn new() -> Self {
        ReadyValidAnalyzer {}
    }
}

impl LockstepAnalyzer for ReadyValidAnalyzer {
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
