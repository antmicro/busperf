pub type CyclesNum = i32;

pub mod ahb;
pub mod apb;
pub mod axi;
pub mod credit_valid;
pub mod custom_python;

use ahb::AHBBus;
use apb::APBBus;
use axi::AXIBus;
use credit_valid::CreditValidBus;
use custom_python::PythonCustomBus;
use wellen::SignalValue;
use yaml_rust2::Yaml;

use crate::{CycleType, bus_usage::RealTime};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct SignalPath {
    pub scope: Vec<String>,
    pub name: String,
}

impl std::fmt::Display for SignalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for s in self.scope.iter() {
            write!(f, "{}.", s)?;
        }
        write!(f, "{}", self.name)?;
        Ok(())
    }
}

impl SignalPath {
    pub fn from_yaml_with_prefix(
        scope: &[String],
        yaml: Yaml,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        SignalPath::from_yaml_ref_with_prefix(scope, &yaml)
    }

    pub fn from_yaml_ref_with_prefix(
        scope: &[String],
        yaml: &Yaml,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match yaml {
            Yaml::String(name) => Ok(SignalPath {
                scope: scope.to_vec(),
                name: name.to_owned(),
            }),
            Yaml::Array(yaml_scope) => {
                let mut yaml_scope = yaml_scope
                    .iter()
                    .map(|y| y.as_str().map(|y| y.to_owned()))
                    .collect::<Option<Vec<_>>>()
                    .ok_or("Signal scope should be a valid string")?;
                let name = yaml_scope.pop().ok_or("No signal name")?;
                let mut scope = scope.to_vec();
                scope.append(&mut yaml_scope);
                Ok(SignalPath { scope, name })
            }
            _ => Err("Invalid YAML")?,
        }
    }
}

macro_rules! bus_from_yaml {
    ( $bus_type:tt, $($signal_name:ident),* ) => {
        pub fn from_yaml(yaml: Yaml, bus_scope: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
            let mut yaml = yaml.into_hash().ok_or("Bus yaml should not be empty")?;
            $(
            let $signal_name = SignalPath::from_yaml_with_prefix(
                bus_scope,
                yaml.remove(&Yaml::from_str(stringify!($signal_name)))
                    .ok_or(concat!(stringify!($name), " bus requires ready signal"))?,
            )?;
            )*
            Ok($bus_type::new(
                $(
                    $signal_name,
                )*
            )
            )
        }
    };
}
pub(crate) use bus_from_yaml;

#[derive(Debug)]
pub struct BusCommon {
    bus_name: String,
    module_scope: Vec<String>,
    clk_path: SignalPath,
    rst_path: SignalPath,
    rst_active_value: u8,
    max_burst_delay: CyclesNum,
    intervals: Vec<[RealTime; 2]>,
}

fn parse_scope(yaml: &Yaml) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if let Some(vec) = yaml.as_vec() {
        Ok(vec
            .iter()
            .map(|scope| parse_scope(scope))
            .collect::<Result<Vec<Vec<_>>, _>>()?
            .into_iter()
            .flatten()
            .collect())
    } else if let Some(s) = yaml.as_str() {
        Ok(vec![s.to_owned()])
    } else {
        Err("Invalid scope.")?
    }
}

fn parse_intervals(yaml: &Yaml) -> Result<Vec<[RealTime; 2]>, Box<dyn std::error::Error>> {
    if let Some(intervals) = yaml["intervals"].as_vec() {
        let mut intervals = intervals
            .iter()
            .map(|i| -> Result<_, Box<dyn std::error::Error>> {
                let i = i
                    .as_vec()
                    .ok_or("Interval should be a tuple of two numbers")?;
                if i.len() != 2 {
                    Err("Each interval should be a 2 element list, defining start and end")?
                }
                Ok([
                    i[0].as_i64().ok_or("Interval start should be a number")? as u64,
                    i[1].as_i64().ok_or("Interval end should be a number")? as u64,
                ])
            })
            .collect::<Result<Vec<_>, _>>()?;
        intervals.sort_by(|a, b| a[0].cmp(&b[0]));
        Ok(intervals)
    } else {
        Ok(vec![])
    }
}

impl BusCommon {
    pub fn from_yaml(
        name: String,
        yaml: &Yaml,
        default_max_burst: CyclesNum,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let i = yaml;
        let scope = parse_scope(&i["scope"])?;
        let clk = SignalPath::from_yaml_ref_with_prefix(&scope, &i["clock"])
            .map_err(|e| format!("Bus should have clock signal: {e}"))?;
        let rst = SignalPath::from_yaml_ref_with_prefix(&scope, &i["reset"])
            .map_err(|e| format!("Bus should have reset signal: {e}"))?;
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
        let intervals = parse_intervals(yaml)?;

        Ok(Self::new(
            name,
            scope,
            clk,
            rst,
            rst_type,
            default_max_burst,
            intervals,
        ))
    }
    pub fn new(
        bus_name: String,
        module_scope: Vec<String>,
        clk_path: SignalPath,
        rst_path: SignalPath,
        rst_active_value: u8,
        max_burst_delay: CyclesNum,
        intervals: Vec<[RealTime; 2]>,
    ) -> Self {
        BusCommon {
            bus_name,
            module_scope,
            clk_path,
            rst_path,
            rst_active_value,
            max_burst_delay,
            intervals,
        }
    }

    pub fn bus_name(&self) -> &str {
        &self.bus_name
    }

    pub fn module_scope(&self) -> &Vec<String> {
        &self.module_scope
    }
    pub fn clk_path(&self) -> &SignalPath {
        &self.clk_path
    }

    pub fn rst_path(&self) -> &SignalPath {
        &self.rst_path
    }

    pub fn max_burst_delay(&self) -> CyclesNum {
        self.max_burst_delay
    }

    pub fn rst_active_value(&self) -> ValueType {
        match self.rst_active_value {
            0 => ValueType::V0,
            1 => ValueType::V1,
            _ => ValueType::X,
        }
    }
    pub fn intervals(&self) -> &Vec<[RealTime; 2]> {
        &self.intervals
    }
}

pub struct BusDescriptionBuilder {}

impl BusDescriptionBuilder {
    pub fn build(
        yaml: Yaml,
        scope: &[String],
    ) -> Result<Box<dyn BusDescription>, Box<dyn std::error::Error>> {
        let i = yaml;

        let handshake = i["handshake"]
            .as_str()
            .ok_or("Bus should have handshake defined")?;

        match handshake {
            "ReadyValid" => {
                return Ok(Box::new(AXIBus::from_yaml(i, scope)?));
            }
            "CreditValid" => Ok(Box::new(CreditValidBus::from_yaml(i, scope)?)),
            "AHB" => Ok(Box::new(AHBBus::from_yaml(i, scope)?)),
            "APB" => Ok(Box::new(APBBus::from_yaml(i, scope)?)),
            "Custom" => {
                let handshake = i["custom_handshake"]
                    .as_str()
                    .ok_or("Custom bus has to specify handshake interpreter")?;
                Ok(Box::new(PythonCustomBus::from_yaml(handshake, &i, scope)?))
            }

            _ => Err(format!("Invalid handshake {}", handshake))?,
        }
    }
}

pub trait BusDescription {
    fn signals(&self) -> Vec<&SignalPath>;
    fn interpret_cycle(&self, signals: &[SignalValue], time: u32) -> CycleType;
}

#[derive(Clone, Copy, PartialEq)]
pub enum ValueType {
    V0,
    V1,
    X,
    Z,
}

pub fn get_value(value: SignalValue) -> Option<ValueType> {
    match value {
        SignalValue::Binary(items, 1) => match items[0] {
            0 => Some(ValueType::V0),
            1 => Some(ValueType::V1),
            _ => unreachable!(),
        },
        SignalValue::Binary(_, _) => None,
        SignalValue::FourValue(items, 1) => match items[0] {
            // if value was 0 or 1 then it would be Binary not FourValue
            66 => Some(ValueType::X),
            67 => Some(ValueType::Z),
            _ => unreachable!(),
        },
        SignalValue::FourValue(_, _) => None,
        SignalValue::NineValue(_, _) => None,
        SignalValue::String(_) => None,
        SignalValue::Real(_) => None,
    }
}

pub fn is_value_of_type(value: SignalValue, type_: ValueType) -> bool {
    match value {
        SignalValue::Binary(items, 1) => match type_ {
            ValueType::V0 => items[0] == 0,
            ValueType::V1 => items[0] == 1,
            ValueType::X => false,
            ValueType::Z => false,
        },
        SignalValue::Binary(_, _) => false,
        SignalValue::FourValue(_items, 1) => false,
        SignalValue::FourValue(_items, _) => panic!(),
        SignalValue::NineValue(_items, _) => todo!(),
        SignalValue::String(_) => false,
        SignalValue::Real(_) => false,
    }
}
