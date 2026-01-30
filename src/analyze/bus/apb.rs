use std::rc::Rc;

use crate::analyze::bus::{BusCommon, BusDescription, ExtraSignals, bus_description};

use super::{LockstepAnalyzer, SignalPath, ValueType, bus_from_yaml, get_value};
use libbusperf::CycleType;

use wellen::SignalValue;
use yaml_rust2::Yaml;

pub struct APBBus {
    common: Rc<BusCommon>,
    psel: SignalPath,
    penable: SignalPath,
    pready: SignalPath,
    extra_signals: ExtraSignals,
}

impl APBBus {
    bus_from_yaml!(APBBus, psel, penable, pready);
    pub fn new(
        common: Rc<BusCommon>,
        psel: SignalPath,
        penable: SignalPath,
        pready: SignalPath,
        extra_signals: ExtraSignals,
    ) -> Self {
        APBBus {
            common,
            psel,
            penable,
            pready,
            extra_signals,
        }
    }
}

bus_description!(APBBus, psel, penable, pready);

pub struct APBAnalyzer;

impl LockstepAnalyzer for APBAnalyzer {
    fn interpret_cycle(&self, signals: &[SignalValue<'_>], _time: u32) -> CycleType {
        let psel = signals[0];
        let penable = signals[1];
        let pready = signals[2];

        if let Some(psel) = get_value(psel)
            && let Some(penable) = get_value(penable)
            && let Some(pready) = get_value(pready)
        {
            use ValueType::V0;
            use ValueType::V1;
            match (psel, penable, pready) {
                (V0, _, _) => CycleType::Free,
                (V1, V0, _) => CycleType::Busy,
                (V1, V1, V0) => CycleType::Backpressure,
                (V1, V1, V1) => CycleType::Busy,
                (_, _, _) => CycleType::Unknown,
            }
        } else {
            CycleType::Unknown
        }
    }
}
