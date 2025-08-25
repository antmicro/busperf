use std::cell::Cell;

use crate::CycleType;

use super::{BusCommon, BusDescription, CyclesNum};

#[derive(Debug)]
pub struct CreditValidBus {
    common: BusCommon,
    credit: String,
    valid: String,
    credits: Cell<u32>,
}

impl CreditValidBus {
    pub fn new(
        bus_name: String,
        module_scope: Vec<String>,
        clk_name: String,
        rst_name: String,
        rst_active_value: u8,
        max_burst_delay: CyclesNum,
        credit: String,
        valid: String,
    ) -> Self {
        CreditValidBus {
            common: BusCommon {
                bus_name,
                module_scope,
                clk_name,
                rst_name,
                rst_active_value,
                max_burst_delay,
            },
            credit,
            valid,
            credits: 0.into(),
        }
    }
}

impl BusDescription for CreditValidBus {
    fn bus_name(&self) -> &str {
        &self.common.bus_name
    }

    fn common(&self) -> &super::BusCommon {
        &self.common
    }

    fn signals(&self) -> Vec<&str> {
        vec![self.credit.as_str(), self.valid.as_str()]
    }

    fn interpret_cycle(&self, signals: Vec<wellen::SignalValue>, time: u32) -> crate::CycleType {
        let credit = signals[0];
        let valid = signals[1];
        if let Ok(credit) = credit.to_bit_string().unwrap().parse::<u32>()
            && let Ok(valid) = valid.to_bit_string().unwrap().parse::<u32>()
        {
            if credit > 0 {
                self.credits.update(|c| c + 1);
            }
            match (self.credits.get(), valid) {
                (1.., 1) => {
                    self.credits.update(|c| c - 1);
                    CycleType::Busy
                }
                (1.., 0) => CycleType::Free,
                (0, 1) => {
                    eprintln!(
                        "[WARN]: Credit is 0 and valid 1 on credit/valid bus {} time: {}",
                        self.common.bus_name, time
                    );
                    CycleType::Busy
                }
                (0, 0) => CycleType::NoTransaction,
                _ => panic!(
                    "signal has invalid value credit: {} valid: {}",
                    credit, valid
                ),
            }
        } else {
            eprintln!(
                "bus in unknown state outside reset credit: {}, valid: {}",
                credit, valid
            );
            CycleType::NoTransaction
        }
    }
}
