pub type CyclesNum = u32;
pub type DelaysNum = u32;

pub mod ahb;
pub mod axi;
pub mod credit_valid;
pub mod custom_python;

use wellen::SignalValue;
use yaml_rust2::Yaml;

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
    pub fn from_yaml(
        yaml: (&Yaml, &Yaml),
        default_max_burst: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let i = yaml;
        let name = i.0.as_str().ok_or("Invalid bus name")?;
        let scope = i.1["scope"]
            .as_vec()
            .ok_or("Scope should be array of strings")?;
        let scope = scope
            .iter()
            .map(|module| module.as_str().unwrap().to_owned())
            .collect();
        let clk = i.1["clock"]
            .as_str()
            .ok_or("Bus should have clock signal")?;
        let rst = i.1["reset"]
            .as_str()
            .ok_or("Bus should have reset signal")?;
        let rst_type = i.1["reset_type"]
            .as_str()
            .ok_or("Bus should have reset type defined")?;
        let rst_type = if rst_type == "low" {
            0
        } else if rst_type == "high" {
            1
        } else {
            Err("Reset type can be \"high\" or \"low\"")?
        };

        Ok(Self::new(
            name,
            scope,
            clk,
            rst,
            rst_type,
            default_max_burst,
        ))
    }
    pub fn new(
        bus_name: &str,
        module_scope: Vec<String>,
        clk_name: &str,
        rst_name: &str,
        rst_active_value: u8,
        max_burst_delay: CyclesNum,
    ) -> Self {
        BusCommon {
            bus_name: bus_name.to_owned(),
            module_scope,
            clk_name: clk_name.to_owned(),
            rst_name: rst_name.to_owned(),
            rst_active_value,
            max_burst_delay,
        }
    }

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
