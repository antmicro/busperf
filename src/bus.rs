pub type CyclesNum = u32;
pub type DelaysNum = u32;

pub mod ahb;
pub mod axi;
pub mod credit_valid;
pub mod custom_python;

use wellen::SignalValue;

use crate::CycleType;

#[derive(Debug)]
pub struct BusCommon {
    bus_name: String,
    module_scope: Vec<String>,
    clk_name: String,
    rst_name: String,
    rst_active_value: u8,
    max_burst_delay: CyclesNum,
}

impl BusCommon {
    pub fn module_scope(&self) -> &Vec<String> {
        &self.module_scope
    }
    pub fn clk_name(&self) -> &str {
        &self.clk_name
    }

    pub fn rst_name(&self) -> &str {
        &self.rst_name
    }

    pub fn max_burst_delay(&self) -> CyclesNum {
        self.max_burst_delay
    }

    pub fn rst_active_value(&self) -> u8 {
        self.rst_active_value
    }
}

pub trait BusDescription {
    fn bus_name(&self) -> &str;
    fn common(&self) -> &BusCommon;
    fn signals(&self) -> Vec<&str>;
    fn interpret_cycle(&self, signals: Vec<SignalValue>, time: u32) -> CycleType;
}
