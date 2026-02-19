use std::error::Error;

use itertools::Itertools;
use libbusperf::bus_usage::{BusUsage, RealTime};
use wellen::{Signal, SignalRef};
use yaml_rust2::Yaml;

use crate::analyze::{
    DoneTriggers, SimulationData,
    trigger::{fsm_trigger::Fsm, time_trigger::TimeControl},
};

mod channel_trigger;
mod fsm_trigger;
mod python_trigger;
mod signal_trigger;
mod time_trigger;
mod transaction_trigger;
mod trigger_combination;

pub use channel_trigger::ChannelTrigger;
pub use python_trigger::PythonTrigger;
pub use signal_trigger::SignalTrigger;
pub use transaction_trigger::TransactionTrigger;

pub type TriggerResult = (String, Result<Vec<RealTime>, Box<dyn Error>>);

/// Trait to implement by triggers that need signal values and/or result of the analyzer to determine when they activate.
///
/// For triggers that do not need any signals use [Control] trait.
pub trait TriggerSource {
    fn name(&self) -> &str;
    fn into_name(self: Box<Self>) -> String;
    fn analyze(
        self: Box<Self>,
        simulation_data: &mut SimulationData,
        loaded: &[&(SignalRef, Signal)],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        done_triggers: &DoneTriggers,
        bus_usage: &Result<BusUsage, Box<dyn Error>>,
    ) -> TriggerResult;
}

pub struct ControlBuilder;

type ControlResult = Vec<(String, Result<Vec<RealTime>, Box<dyn Error>>)>;

/// Trait to implement by triggers that depend only on other triggers.
///
/// For triggers that also require signals and/or analyzer's result use [TriggerSource]
pub trait Control {
    fn requires(&self) -> Vec<&str>;
    fn names(&self) -> Vec<&str>;
    fn name(&self) -> &str;
    fn analyze(self: Box<Self>, done_trigger: &DoneTriggers) -> ControlResult;
}

impl ControlBuilder {
    pub fn build_from_yaml(name: String, yaml: &Yaml) -> Result<Box<dyn Control>, Box<dyn Error>> {
        let t = yaml["type"].as_str().ok_or("invalid type")?;
        match t {
            "timer" => TimeControl::build_from_yaml(name, yaml),
            "fsm" => Ok(Box::new(Fsm::build_from_yaml(name, yaml)?)),
            _ => Err("invalid type")?,
        }
    }
}

/// Struct that defines when should an analyzer activate and deactivate.
pub struct TriggerSink {
    activate: TriggerCombination,
    deactivate: TriggerCombination,
}

impl TriggerSink {
    pub fn build_from_yaml(yaml: &Yaml) -> Result<Self, String> {
        let activate = TriggerCombination::from_yaml(&yaml["activate_on"])
            .map_err(|e| format!("activate triggers invalid: {e}"))?;
        let deactivate = TriggerCombination::from_yaml(&yaml["deactivate_on"])
            .map_err(|e| format!("deactivate triggers invalid: {e}"))?;
        Ok(TriggerSink {
            activate,
            deactivate,
        })
    }

    // Returns names of triggers that must be analyzed so that the sink can determine intervals in which the analyzer will be active.
    pub fn required(&self) -> Vec<String> {
        let mut req = self.activate.get_names();
        req.append(&mut self.deactivate.get_names());
        req
    }

    // Returns list of time periods (intervals), during which the analyzer should be active.
    // Returns error when any of the required triggers had not been calculated or failed.
    pub fn get_intervals(
        &self,
        triggers: &DoneTriggers,
        time_end: RealTime,
    ) -> Result<Vec<[RealTime; 2]>, Box<dyn Error>> {
        let start_iter = self
            .activate
            .get_times(triggers)
            .map_err(|e| format!("{e}"))?;
        let mut end_iter = self
            .deactivate
            .get_times(triggers)
            .map_err(|e| format!("{e}"))?
            .into_iter();

        let mut intervals = vec![];
        let mut last_end = 0;
        for start in start_iter {
            if start < last_end {
                continue;
            }

            let end = end_iter.find(|&t| t > start).unwrap_or(time_end);
            intervals.push([start, end]);
            last_end = end;
        }

        Ok(intervals)
    }
}

pub type TriggerName = String;

/// Enum of was of combining triggers
enum TriggerCombination {
    /// No trigger.
    None,
    /// Only one trigger.
    Single(TriggerName),
    /// Any combination - activates whenever any of the subtriggers activates.
    Any(Vec<TriggerCombination>),
    /// All combination - activates when all subtriggers are active at the same time.
    All(Vec<TriggerCombination>),
}

impl TriggerCombination {
    /// Get names of triggers used in this combination.
    fn get_names(&self) -> Vec<String> {
        match self {
            TriggerCombination::None => vec![],
            TriggerCombination::Single(name) => vec![name.clone()],
            TriggerCombination::Any(trigger_combinations) => trigger_combinations
                .iter()
                .flat_map(|i| i.get_names())
                .collect(),
            TriggerCombination::All(trigger_combinations) => trigger_combinations
                .iter()
                .flat_map(|i| i.get_names())
                .collect(),
        }
    }
}

impl TriggerCombination {
    fn from_yaml(yaml: &Yaml) -> Result<Self, Box<dyn Error>> {
        match yaml {
            Yaml::String(name) => Ok(TriggerCombination::Single(name.to_owned())),
            Yaml::Array(yamls) => Ok(TriggerCombination::Any(
                yamls
                    .iter()
                    .map(|y| TriggerCombination::from_yaml(y))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Yaml::Hash(linked_hash_map) => {
                if linked_hash_map.len() != 1 {
                    return Err(format!("other item next to any {:?}", linked_hash_map))?;
                }
                let Some(Yaml::Array(yaml)) = linked_hash_map.get(&Yaml::String("all".into()))
                else {
                    return Err("combination should be named all")?;
                };
                Ok(TriggerCombination::All(
                    yaml.iter()
                        .map(|y| TriggerCombination::from_yaml(y))
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
            Yaml::BadValue => Ok(TriggerCombination::None),
            other => Err(format!("non string trigger name {:?}", other))?,
        }
    }

    /// Get times during which this trigger combination is active.
    #[allow(clippy::borrowed_box)]
    fn get_times<'a>(
        &self,
        triggers: &'a DoneTriggers,
    ) -> Result<Vec<RealTime>, &'a Box<dyn Error>> {
        match self {
            TriggerCombination::None => Ok(vec![0]),
            TriggerCombination::Single(name) => triggers[name].as_ref().map(|r| r.clone()),
            TriggerCombination::Any(trigger_combinations) => {
                let a = trigger_combinations
                    .iter()
                    .map(|c| c.get_times(triggers))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .kmerge()
                    .dedup()
                    .collect();
                Ok(a)
            }
            TriggerCombination::All(trigger_combinations) => {
                let num = trigger_combinations.len();
                let counts = trigger_combinations
                    .iter()
                    .map(|c| c.get_times(triggers))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .kmerge()
                    .counts();
                let times = counts
                    .into_iter()
                    .filter_map(|(time, count)| if count == num { Some(time) } else { None })
                    .collect();
                Ok(times)
            }
        }
    }
}
