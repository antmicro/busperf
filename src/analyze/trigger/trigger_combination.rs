use std::error::Error;

use itertools::Itertools;
use libbusperf::bus_usage::{BusUsage, RealTime};
use wellen::Signal;

use crate::analyze::{DoneTriggers, SimulationData, trigger::TriggerSource};

pub struct TriggerSourceCombination {
    name: String,
    triggers: Vec<Box<dyn TriggerSource>>,
    combination_type: CombinationType,
}

impl TriggerSourceCombination {
    pub fn new(
        name: String,
        triggers: Vec<Box<dyn TriggerSource>>,
        combination_type: CombinationType,
    ) -> Self {
        Self {
            name,
            triggers,
            combination_type,
        }
    }
}

#[derive(Debug)]
pub enum CombinationType {
    Any,
    All,
}

impl TriggerSource for TriggerSourceCombination {
    fn analyze(
        self: Box<Self>,
        simulation_data: &mut SimulationData,
        loaded: &[&Signal],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        done_triggers: &DoneTriggers,
        bus_usage: &Result<BusUsage, Box<dyn Error>>,
    ) -> (
        std::string::String,
        Result<Vec<RealTime>, Box<dyn std::error::Error + 'static>>,
    ) {
        let num = self.triggers.len();
        let triggers = self
            .triggers
            .into_iter()
            .map(|t| {
                let (_, ret) =
                    t.analyze(simulation_data, loaded, intervals, done_triggers, bus_usage);
                ret
            })
            .collect::<Result<Vec<_>, _>>();

        match triggers {
            Ok(triggers) => match self.combination_type {
                CombinationType::Any => {
                    let merged = triggers.into_iter().kmerge().dedup().collect();
                    (self.name, Ok(merged))
                }
                CombinationType::All => {
                    let counts = triggers.into_iter().kmerge().dedup_with_count();
                    let times = counts
                        .into_iter()
                        .filter_map(|(count, time)| if count == num { Some(time) } else { None })
                        .collect();
                    (self.name, Ok(times))
                }
            },
            Err(e) => (self.name, Err(e)),
        }
    }

    fn into_name(self: Box<Self>) -> String {
        self.name
    }

    fn name(&self) -> &str {
        &self.name
    }
}
