use crate::bus::{self, axi::AXIBus, BusCommon, BusDescription};

use super::Analyzer;

pub struct DefaultAnalyzer {}

impl DefaultAnalyzer {
    pub fn new() -> Self {
        DefaultAnalyzer {}
    }
}

impl Analyzer for DefaultAnalyzer {
    fn load_buses(
        &self,
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Vec<Box<dyn crate::bus::BusDescription>>, Box<dyn std::error::Error>> {
        let i = yaml;
        let mut descs = Vec::<Box<dyn BusDescription>>::with_capacity(1);

        let common = BusCommon::from_yaml(i, default_max_burst_delay)?;

        let handshake = i.1["handshake"]
            .as_str()
            .ok_or("Bus should have handshake defined")?;

        match handshake {
            "ReadyValid" => {
                let ready = i.1["ready"]
                    .as_str()
                    .ok_or("ReadyValid bus requires ready signal")?;
                let valid = i.1["valid"]
                    .as_str()
                    .ok_or("ReadyValid bus requires valid signal")?;
                let max_burst_delay = i.1["max_burst_delay"].as_i64();
                let max_burst_delay = if max_burst_delay.is_some() {
                    max_burst_delay.unwrap().try_into().unwrap()
                } else {
                    default_max_burst_delay
                };
                descs.push(Box::new(AXIBus::new(
                    common,
                    ready.to_owned(),
                    valid.to_owned(),
                )));
            }
            "CreditValid" => {
                let credit = i.1["credit"]
                    .as_str()
                    .ok_or("CreditValid bus requires credit signal")?;
                let valid = i.1["valid"]
                    .as_str()
                    .ok_or("CreditValid bus requires valid signal")?;
                descs.push(Box::new(bus::credit_valid::CreditValidBus::new(
                    common,
                    credit.to_owned(),
                    valid.to_owned(),
                )))
            }
            "AHB" => {
                let htrans = i.1["htrans"]
                    .as_str()
                    .ok_or("AHB bus requires htrans signal")?;
                let hready = i.1["hready"]
                    .as_str()
                    .ok_or("AHB bus requires hready signal")?;
                descs.push(Box::new(bus::ahb::AHBBus::new(
                    common,
                    htrans.to_owned(),
                    hready.to_owned(),
                )))
            }
            "Custom" => {
                let handshake = i.1["custom_handshake"]
                    .as_str()
                    .ok_or("Custom bus has to specify handshake interpreter")?;
                descs.push(Box::new(bus::custom_python::PythonCustomBus::new(
                    common, handshake, i.1,
                )));
            }

            _ => Err(format!("Invalid handshake {}", handshake))?,
        }
        Ok(descs)
    }
}
