use std::cell::Cell;
use wellen::SignalValue;

use crate::{
    CycleType,
    bus::{ValueType, get_value},
};

use super::{BusCommon, BusDescription};

#[derive(Debug)]
pub struct CreditValidBus {
    common: BusCommon,
    credit: String,
    valid: String,
    credits: Cell<u32>,
}

impl CreditValidBus {
    pub fn new(common: BusCommon, credit: String, valid: String) -> Self {
        CreditValidBus {
            common,
            credit,
            valid,
            credits: 0.into(),
        }
    }
}

impl BusDescription for CreditValidBus {
    fn signals(&self) -> Vec<&str> {
        vec![self.credit.as_str(), self.valid.as_str()]
    }

    fn interpret_cycle(&self, signals: &[SignalValue<'_>], time: u32) -> crate::CycleType {
        let credit = signals[0];
        let valid = signals[1];
        if let Some(credit_v) = get_value(credit)
            && let Some(valid_v) = get_value(valid)
        {
            use ValueType::V0;
            use ValueType::V1;
            if matches!(credit_v, V1) {
                self.credits.update(|c| c + 1);
            }
            match (self.credits.get(), valid_v) {
                (1.., V1) => {
                    self.credits.update(|c| c - 1);
                    CycleType::Busy
                }
                (1.., V0) => CycleType::Free,
                (0, V1) => {
                    eprintln!(
                        "[WARN]: Credit is 0 and valid 1 on credit/valid bus {} time: {}",
                        self.common.bus_name, time
                    );
                    CycleType::Busy
                }
                (0, V0) => CycleType::NoTransaction,
                _ => {
                    eprintln!(
                        "signal has invalid value credit: {} valid: {}",
                        credit, valid
                    );
                    CycleType::NoTransaction
                }
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
