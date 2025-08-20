use crate::CycleType;

use super::{BusCommon, BusDescription, CyclesNum};

#[derive(Debug)]
pub struct AHBBus {
    common: BusCommon,
    htrans: String,
    hready: String,
}

impl AHBBus {
    pub fn new(
        bus_name: String,
        module_scope: Vec<String>,
        clk_name: String,
        rst_name: String,
        rst_active_value: u8,
        max_burst_delay: CyclesNum,
        htrans: String,
        hready: String,
    ) -> Self {
        AHBBus {
            common: BusCommon {
                bus_name,
                module_scope,
                clk_name,
                rst_name,
                rst_active_value,
                max_burst_delay,
            },
            htrans,
            hready,
        }
    }
}

impl BusDescription for AHBBus {
    fn bus_name(&self) -> &str {
        &self.common.bus_name
    }

    fn common(&self) -> &BusCommon {
        &self.common
    }

    fn signals(&self) -> Vec<&str> {
        vec![self.htrans.as_str(), self.hready.as_str()]
    }

    fn interpret_cycle(&self, signals: Vec<wellen::SignalValue>, time: u32) -> crate::CycleType {
        let htrans = signals[0];
        let hready = signals[1];
        if let Some(htrans) = htrans.to_bit_string()
            && let Ok(hready) = hready.to_bit_string().unwrap().parse::<u32>()
        {
            /*
            00 - IDLE
            01 - BUSY
            10 - NOSEQ
            11 - SEQ
            */
            match (htrans.as_str(), hready) {
                ("11", 1) | ("10", 1) => CycleType::Busy,
                ("00", 1) => CycleType::Free,
                ("01", 1) => CycleType::NoData,
                ("00", 0) | ("01", 0) => {
                    eprintln!(
                        "ahb bus in disallowed state htrans: {} hready: {}, time: {}",
                        htrans, hready, time
                    );
                    CycleType::Backpressure
                }
                (_, 0) => CycleType::Backpressure,
                _ => panic!(
                    "signal has invalid value hready: {} htrans: {}",
                    hready, htrans
                ),
            }
        } else {
            eprintln!(
                "bus in unknown state outside reset hready: {}, htrans: {}",
                hready, htrans
            );
            CycleType::NoTransaction
        }
    }
}
