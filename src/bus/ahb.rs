use wellen::SignalValue;
use yaml_rust2::Yaml;

use super::{BusDescription, SignalPath, ValueType, bus_from_yaml, get_value};
use crate::CycleType;

#[derive(Debug)]
pub struct AHBBus {
    htrans: SignalPath,
    hready: SignalPath,
}

impl AHBBus {
    bus_from_yaml!(AHBBus, htrans, hready);
    pub fn new(htrans: SignalPath, hready: SignalPath) -> Self {
        AHBBus { htrans, hready }
    }
}

impl BusDescription for AHBBus {
    fn signals(&self) -> Vec<&SignalPath> {
        vec![&self.htrans, &self.hready]
    }

    fn interpret_cycle(&self, signals: &[SignalValue<'_>], time: u32) -> crate::CycleType {
        let htrans = signals[0];
        let hready = signals[1];
        if let SignalValue::Binary(htrans_v, 2) = htrans
            && let Some(hready_v) = get_value(hready)
        {
            /*
            00 - IDLE
            01 - BUSY
            10 - NOSEQ
            11 - SEQ
            */
            use ValueType::V0;
            use ValueType::V1;
            match (htrans_v[0], hready_v) {
                (0b11, V1) | (0b10, V1) => CycleType::Busy,
                (0b00, V1) => CycleType::Free,
                (0b01, V1) => CycleType::NoData,
                (0b00, V0) | (0b01, V0) => {
                    eprintln!(
                        "ahb bus in disallowed state htrans: {} hready: {}, time: {}",
                        htrans, hready, time
                    );
                    CycleType::Backpressure
                }
                (_, V0) => CycleType::Backpressure,
                _ => {
                    eprintln!(
                        "signal has invalid value hready: {} htrans: {}",
                        hready, htrans
                    );
                    CycleType::Unknown
                }
            }
        } else {
            eprintln!(
                "bus in unknown state outside reset hready: {}, htrans: {}",
                hready, htrans
            );
            CycleType::Unknown
        }
    }
}
