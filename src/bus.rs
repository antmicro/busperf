pub type CyclesNum = u32;
pub type DelaysNum = u32;

pub mod ahb;
pub mod axi;
pub mod credit_valid;
pub mod custom_python;

use ahb::AHBBus;
use axi::AXIBus;
use credit_valid::CreditValidBus;
use custom_python::PythonCustomBus;
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
        name: &str,
        yaml: &Yaml,
        default_max_burst: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let i = yaml;
        let scope = i["scope"]
            .as_vec()
            .ok_or("Scope should be array of strings")?;
        let scope = scope
            .iter()
            .map(|module| module.as_str().unwrap().to_owned())
            .collect();
        let clk = i["clock"].as_str().ok_or("Bus should have clock signal")?;
        let rst = i["reset"].as_str().ok_or("Bus should have reset signal")?;
        let rst_type = i["reset_type"]
            .as_str()
            .ok_or("Bus should have reset type defined")?;
        let rst_type = if rst_type == "low" {
            0
        } else if rst_type == "high" {
            1
        } else {
            Err("Reset type can be \"high\" or \"low\"")?
        };

        // let max_burst_delay = i.1["max_burst_delay"].as_i64();
        // let max_burst_delay = if max_burst_delay.is_some() {
        //     max_burst_delay.unwrap().try_into().unwrap()
        // } else {
        //     default_max_burst_delay
        // };

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

    pub fn bus_name(&self) -> &str {
        &self.bus_name
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

pub struct BusDescriptionBuilder {}

impl BusDescriptionBuilder {
    pub fn build(
        name: &str,
        yaml: &yaml_rust2::Yaml,
        default_max_burst_delay: u32,
    ) -> Result<Box<dyn BusDescription>, Box<dyn std::error::Error>> {
        let i = yaml;

        let common = BusCommon::from_yaml(name, i, default_max_burst_delay)?;

        let handshake = i["handshake"]
            .as_str()
            .ok_or("Bus should have handshake defined")?;

        match handshake {
            "ReadyValid" => {
                return Ok(Box::new(AXIBus::from_yaml(i)?));
            }
            "CreditValid" => {
                let credit = i["credit"]
                    .as_str()
                    .ok_or("CreditValid bus requires credit signal")?;
                let valid = i["valid"]
                    .as_str()
                    .ok_or("CreditValid bus requires valid signal")?;
                return Ok(Box::new(CreditValidBus::new(
                    common,
                    credit.to_owned(),
                    valid.to_owned(),
                )));
            }
            "AHB" => {
                let htrans = i["htrans"]
                    .as_str()
                    .ok_or("AHB bus requires htrans signal")?;
                let hready = i["hready"]
                    .as_str()
                    .ok_or("AHB bus requires hready signal")?;
                return Ok(Box::new(AHBBus::new(
                    common,
                    htrans.to_owned(),
                    hready.to_owned(),
                )));
            }
            "Custom" => {
                let handshake = i["custom_handshake"]
                    .as_str()
                    .ok_or("Custom bus has to specify handshake interpreter")?;
                return Ok(Box::new(PythonCustomBus::new(common, handshake, i)));
            }

            _ => Err(format!("Invalid handshake {}", handshake))?,
        }
    }
}

pub trait BusDescription {
    fn signals(&self) -> Vec<&str>;
    fn interpret_cycle(&self, signals: Vec<SignalValue>, time: u32) -> CycleType;
}
