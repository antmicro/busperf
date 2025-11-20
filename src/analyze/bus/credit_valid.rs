use std::cell::Cell;
use wellen::SignalValue;
use yaml_rust2::Yaml;

use super::{BusDescription, SignalPath, ValueType, bus_from_yaml, get_value};
use crate::CycleType;

#[derive(Debug)]
pub struct CreditValidBus {
    credit: SignalPath,
    valid: SignalPath,
    credits: Cell<u32>,
}

impl CreditValidBus {
    bus_from_yaml!(CreditValidBus, credit, valid);
    pub fn new(credit: SignalPath, valid: SignalPath) -> Self {
        CreditValidBus {
            credit,
            valid,
            credits: 0.into(),
        }
    }
}

impl BusDescription for CreditValidBus {
    fn signals(&self) -> Vec<&SignalPath> {
        vec![&self.credit, &self.valid]
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
                        "[WARN] credit is 0 and valid 1 on credit/valid bus time: {}",
                        time
                    );
                    CycleType::Busy
                }
                (0, V0) => CycleType::NoTransaction,
                _ => {
                    eprintln!(
                        "[WARN] signal has invalid value credit: {} valid: {}",
                        credit, valid
                    );
                    CycleType::Unknown
                }
            }
        } else {
            eprintln!(
                "[WARN] bus in unknown state outside reset credit: {}, valid: {}",
                credit, valid
            );
            CycleType::Unknown
        }
    }
}
