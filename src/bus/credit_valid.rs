use crate::CycleType;

use super::{BusCommon, BusDescription};

#[derive(Debug)]
pub struct CreditValidBus {
    common: BusCommon,
    credit: String,
    valid: String,
}

impl CreditValidBus {
    pub fn new(common: BusCommon, credit: String, valid: String) -> Self {
        CreditValidBus {
            common,
            credit,
            valid,
        }
    }
}

impl BusDescription for CreditValidBus {
    fn signals(&self) -> Vec<&str> {
        vec![self.credit.as_str(), self.valid.as_str()]
    }

    fn interpret_cycle(&self, signals: Vec<wellen::SignalValue>, time: u32) -> crate::CycleType {
        let credit = signals[0];
        let valid = signals[1];
        if let Ok(credit) = credit.to_bit_string().unwrap().parse::<u32>()
            && let Ok(valid) = valid.to_bit_string().unwrap().parse::<u32>()
        {
            match (credit, valid) {
                (1.., 1) => CycleType::Busy,
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
