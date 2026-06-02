use std::rc::Rc;

use yaml_rust2::Yaml;

use crate::analyze::{
    SignalValue,
    bus::{BusCommon, BusDescription, ExtraSignals, bus_description},
};

use super::{LockstepAnalyzer, SignalPath, ValueType, bus_from_yaml, get_value};
use libbusperf::CycleType;

#[derive(Debug)]
pub struct AHBBus {
    common: Rc<BusCommon>,
    htrans: SignalPath,
    hready: SignalPath,
    extra_signals: ExtraSignals,
}

impl AHBBus {
    bus_from_yaml!(AHBBus, htrans, hready);
    pub fn new(
        common: Rc<BusCommon>,
        htrans: SignalPath,
        hready: SignalPath,
        extra_signals: ExtraSignals,
    ) -> Self {
        AHBBus {
            common,
            htrans,
            hready,
            extra_signals,
        }
    }
}

bus_description!(AHBBus, htrans, hready);

pub struct AHBAnalyzer;

impl LockstepAnalyzer for AHBAnalyzer {
    fn interpret_cycle(&self, signals: &[SignalValue<'_>], time: u32) -> CycleType {
        let htrans = signals[0];
        let hready = signals[1];
        if let SignalValue::BitVec(htrans_v) = htrans
            && let Some(htrans_v) = htrans_v.be_bytes()
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
            match (htrans_v, hready_v) {
                (&[0b11], V1) | (&[0b10], V1) => CycleType::Busy,
                (&[0b00], V1) => CycleType::Free,
                (&[0b01], V1) => CycleType::NoData,
                (&[0b00], V0) | (&[0b01], V0) => {
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
