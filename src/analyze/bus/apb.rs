use super::{BusDescription, SignalPath, ValueType, bus_from_yaml, get_value};
use libbusperf::CycleType;

use wellen::SignalValue;
use yaml_rust2::Yaml;

pub struct APBBus {
    psel: SignalPath,
    penable: SignalPath,
    pready: SignalPath,
}

impl APBBus {
    bus_from_yaml!(APBBus, psel, penable, pready);
    pub fn new(psel: SignalPath, penable: SignalPath, pready: SignalPath) -> Self {
        APBBus {
            psel,
            penable,
            pready,
        }
    }
}

impl BusDescription for APBBus {
    fn signals(&self) -> std::vec::Vec<&SignalPath> {
        vec![&self.psel, &self.penable, &self.pready]
    }

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
