pub mod ahb;
pub mod apb;
pub mod axi;
pub mod credit_valid;
#[cfg(feature = "python-plugins")]
pub mod custom_python;

use std::rc::Rc;

use ahb::AHBBus;
use apb::APBBus;
use axi::AXIBus;
use credit_valid::CreditValidBus;
use yaml_rust2::Yaml;

use libbusperf::CycleType;

pub use libbusperf::SignalPath;

pub struct SignalPathFromYaml {}

impl SignalPathFromYaml {
    pub fn from_yaml_ref_with_prefix(
        scope: &[String],
        yaml: &Yaml,
    ) -> Result<SignalPath, Box<dyn std::error::Error>> {
        match yaml {
            Yaml::String(name) => Ok(SignalPath {
                scope: scope.to_vec(),
                name: name.to_owned(),
            }),
            Yaml::Array(_yaml_scope) => {
                let mut yaml_scope = parse_scope(yaml)?;
                let name = yaml_scope.pop().ok_or("No signal name")?;
                let mut scope = scope.to_vec();
                scope.append(&mut yaml_scope);
                Ok(SignalPath { scope, name })
            }
            Yaml::BadValue => Err("not found")?,
            _ => Err(format!("invalid value {:?}", yaml))?,
        }
    }
}

pub const COMMON_YAML: &[&str] = &[
    "scope",
    "clk_rst_if",
    "clk_rst_if.clock",
    "clock",
    "clk_rst_if.reset",
    "reset",
    "clk_rst_if.reset_type",
    "reset_type",
    "custom_analyzer",
    "start_triggers",
    "end_triggers",
    "custom_handshake",
    "handshake",
    "activate_on",
    "deactivate_on",
    "triggers",
];

pub type ExtraSignals = Vec<(String, SignalPath)>;

macro_rules! bus_from_yaml {
    ( $bus_type:tt, $($signal_name:ident),* ) => {
        const HANDLED: &[&str] =
                constcat::concat_slices!([&str]: crate::analyze::bus::COMMON_YAML, &[
                    $(
                        stringify!($signal_name),
                    )*
                ]);
        pub fn from_yaml_with_common(common: std::rc::Rc<BusCommon>, yaml: &Yaml) -> Result<Self, Box<dyn std::error::Error>> {
            use crate::analyze::bus::SignalPathFromYaml;

            $(
            let $signal_name = SignalPathFromYaml::from_yaml_ref_with_prefix(
                common.module_scope(),
                &yaml[stringify!($signal_name)]
            ).map_err(|_| concat!(stringify!($bus_type), " bus requires ", stringify!($signal_name), " signal"))?;
            )*
            let mut extra_signals = Vec::new();
            for (name, yaml) in yaml.as_hash().expect("already checked") {
                let name = name.as_str().ok_or("invalid signal name")?;
                if !$bus_type::HANDLED.contains(&name) {
                    extra_signals.push((
                        name.to_owned(),
                        SignalPathFromYaml::from_yaml_ref_with_prefix(&common.module_scope, yaml)?,
                    ));
                }
            }
            Ok($bus_type::new(
                common,
                $(
                    $signal_name,
                )*
                extra_signals,
            )
            )
        }
        pub fn from_yaml(name: String, yaml: &Yaml) -> Result<Self, Box<dyn std::error::Error>> {
            use crate::analyze::bus::BusCommon;

            use std::rc::Rc;
            let common = Rc::new(BusCommon::from_yaml(
                name,
                yaml,
            )?);
            $bus_type::from_yaml_with_common(common, yaml)
        }
    };
}
pub(crate) use bus_from_yaml;

macro_rules! bus_description {
    ( $bus_type:tt, $($signal_name:ident),* ) => {
        impl BusDescription for $bus_type {
            fn name(&self) -> &str {
                &self.common.bus_name
            }
            fn common(&self) -> &BusCommon {
                &self.common
            }
            fn get_signals(&self) -> Vec<&SignalPath> {
                let mut signals = vec![self.common.clk_path(), self.common.rst_path(),
                    $(
                        &self.$signal_name,
                    )*
                ];
                signals.append(&mut self.extra_signals.iter().map(|(_, path)| path).collect());
                signals
            }
            /// PANICS when not a last existing Rc
            fn into_signals(self: Rc<Self>) -> Vec<SignalPath> {
                if let Some(s) = Rc::into_inner(self) {
                    if let Some(common) = Rc::into_inner(s.common) {
                        let mut signals = vec![common.clk_path, common.rst_path,
                            $(
                                s.$signal_name,
                            )*
                        ];
                        signals.append(&mut s.extra_signals.into_iter().map(|(_, path)| path).collect());
                        signals
                    } else {
                        let mut signals = vec![
                            $(
                                s.$signal_name,
                            )*
                        ];
                        signals.append(&mut s.extra_signals.into_iter().map(|(_, path)| path).collect());
                        signals
                    }
                } else {
                    panic!("all triggers should be analyzed and therefore dropped before this function is called");
                }
            }
            fn get_unique_signals(&self) -> Vec<&SignalPath> {
                vec![
                    $(
                        &self.$signal_name,
                    )*
                ]
            }
            fn get_by_name(&self, name: &str) -> Option<&SignalPath> {
                match name {
                    $(
                        stringify!($signal_name) => Some(&self.$signal_name),
                    )*
                    other => if let Some(signal) = self.common.signal_path_by_name(other) { Some(signal) } else {self.extra_signals.iter().find_map(|(name, path)| if name == other {Some(path)} else {None})
                        },
                }
            }
        }
    };
}
pub(crate) use bus_description;

use crate::analyze::{
    SignalValue,
    bus::{ahb::AHBAnalyzer, apb::APBAnalyzer, axi::ReadyValidAnalyzer},
};

/// Common info about the bus.
#[derive(Debug)]
pub struct BusCommon {
    bus_name: String,
    module_scope: Vec<String>,
    clk_path: SignalPath,
    rst_path: SignalPath,
    rst_active_value: u8,
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
        Err(format!("Invalid scope. {:?}", yaml))?
    }
}

impl BusCommon {
    pub fn from_yaml(name: String, yaml: &Yaml) -> Result<Self, Box<dyn std::error::Error>> {
        let mut i = yaml;
        let scope = parse_scope(&i["scope"])?;
        match &i["clk_rst_if"] {
            Yaml::Hash(_) => {
                if let Yaml::BadValue = i["clock"]
                    && let Yaml::BadValue = i["reset"]
                    && let Yaml::BadValue = i["reset_type"]
                {
                    i = &i["clk_rst_if"];
                } else {
                    Err(
                        "clock, reset and reset_type all should be defined inside clk_rst_if or all outside",
                    )?;
                }
            }
            Yaml::BadValue => (),
            _ => Err("clk_rst_if should be a mapping containing clock, reset, and reset_type")?,
        }
        let clk = SignalPathFromYaml::from_yaml_ref_with_prefix(&scope, &i["clock"])
            .map_err(|e| format!("clock signal definition {e}"))?;
        let rst = SignalPathFromYaml::from_yaml_ref_with_prefix(&scope, &i["reset"])
            .map_err(|e| format!("reset signal definition {e}"))?;
        let rst_type = i["reset_type"]
            .as_str()
            .ok_or("reset type should be \"high\" or \"low\"")?;
        let rst_type = if rst_type == "low" {
            0
        } else if rst_type == "high" {
            1
        } else {
            Err("reset type should be \"high\" or \"low\"")?
        };

        Ok(Self::new(name, scope, clk, rst, rst_type))
    }
    /// PANICS when not a last existing Rc
    pub fn into_signals(self: Rc<Self>) -> Vec<SignalPath> {
        if let Some(s) = Rc::into_inner(self) {
            vec![s.clk_path, s.rst_path]
        } else {
            panic!("into signals called when strong count != 1")
        }
    }
    pub fn into_signals_owned(self) -> Vec<SignalPath> {
        vec![self.clk_path, self.rst_path]
    }
    pub fn new(
        bus_name: String,
        module_scope: Vec<String>,
        clk_path: SignalPath,
        rst_path: SignalPath,
        rst_active_value: u8,
    ) -> Self {
        BusCommon {
            bus_name,
            module_scope,
            clk_path,
            rst_path,
            rst_active_value,
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

    pub fn rst_active_value(&self) -> ValueType {
        match self.rst_active_value {
            0 => ValueType::V0,
            1 => ValueType::V1,
            _ => ValueType::X,
        }
    }
    pub fn signal_path_by_name(&self, name: &str) -> Option<&SignalPath> {
        match name {
            "clock" => Some(self.clk_path()),
            "reset" => Some(self.rst_path()),
            _ => None,
        }
    }
}

pub struct BusDescriptionBuilder {}

type DescriptionBuilderResult =
    Result<(Rc<dyn BusDescription>, Rc<dyn LockstepAnalyzer>), Box<dyn std::error::Error>>;

impl BusDescriptionBuilder {
    pub fn build(name: String, yaml: &Yaml, _plugins_path: &str) -> DescriptionBuilderResult {
        let i = yaml;

        let handshake = i["handshake"]
            .as_str()
            .ok_or("Bus should have handshake defined")?;

        match handshake {
            "ReadyValid" => {
                return Ok((
                    Rc::new(AXIBus::from_yaml(name, i)?),
                    Rc::new(ReadyValidAnalyzer::new()),
                ));
            }
            "CreditValid" => {
                let bus = Rc::new(CreditValidBus::from_yaml(name, i)?);
                Ok((Rc::clone(&bus) as Rc<dyn BusDescription>, bus))
            }
            "AHB" => Ok((Rc::new(AHBBus::from_yaml(name, i)?), Rc::new(AHBAnalyzer))),
            "APB" => Ok((Rc::new(APBBus::from_yaml(name, i)?), Rc::new(APBAnalyzer))),
            _ => Err(format!("invalid handshake {}", handshake))?,
        }
    }
}

/// Trait to implement by any struct that is to be used as BusDescription.
///
/// Main task of any such struct is to contain information about the bus and its signals.
/// It should contain how the signals has been named in YAML and what is its waveform signal path.
/// Also see [BusCommon].
pub trait BusDescription {
    fn name(&self) -> &str {
        self.common().bus_name()
    }
    fn common(&self) -> &BusCommon;
    /// Returns list of signal paths for that bus.
    fn get_signals(&self) -> Vec<&SignalPath>;
    /// Returns list of owned signal paths.
    /// Can PANIC when self is not a last existing Rc.
    fn into_signals(self: Rc<Self>) -> Vec<SignalPath>;
    /// Returns list of signals that are specific for that bus, e.g. skips clk and rst from BusCommon.
    fn get_unique_signals(&self) -> Vec<&SignalPath>;
    /// Return signal path for a signal named `name` in YAML.
    fn get_by_name(&self, name: &str) -> Option<&SignalPath>;
}

/// Trait for use in DefaultAnalyzer.
pub trait LockstepAnalyzer {
    /// For each clock cycle it calls this method from this trait
    /// to determine the state of the bus.
    fn interpret_cycle(&self, signals: &[SignalValue], time: u32) -> CycleType;
}

#[derive(Clone, Copy, PartialEq)]
pub enum ValueType {
    V0,
    V1,
    X,
    Z,
}

pub fn get_value(value: SignalValue<'_>) -> Option<ValueType> {
    match value {
        SignalValue::BitVec(bit_vec_ref) => match Into::<u8>::into(bit_vec_ref.get_bit(0)) {
            0 => Some(ValueType::V0),
            1 => Some(ValueType::V1),
            2 => Some(ValueType::X),
            3 => Some(ValueType::Z),
            _ => None,
        },
        SignalValue::Event => None,
        SignalValue::String(_) => None,
        SignalValue::Real(_) => None,
    }
}

pub fn is_value_of_type(value: SignalValue<'_>, type_: ValueType) -> bool {
    if let Some(value) = get_value(value) {
        value == type_
    } else {
        false
    }
}
