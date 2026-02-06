use std::{error::Error, rc::Rc};

use libbusperf::{
    CycleType, SignalPath,
    bus_usage::{BusUsage, RealTime},
};
use yaml_rust2::Yaml;

use crate::analyze::{
    SignalsIterator,
    bus::{BusDescription, LockstepAnalyzer},
    trigger::{
        TriggerSource,
        signal_trigger::SignalTrigger,
        trigger_combination::{CombinationType, TriggerSourceCombination},
    },
};

pub struct ChannelTrigger {
    name: String,
    type_: CycleType,
    analyzer: Rc<dyn LockstepAnalyzer>,
}

impl TriggerSource for ChannelTrigger {
    fn name(&self) -> &str {
        &self.name
    }

    fn into_name(self: Box<Self>) -> String {
        self.name
    }

    fn analyze(
        self: Box<Self>,
        simulation_data: &mut crate::analyze::SimulationData,
        loaded: &[&(wellen::SignalRef, wellen::Signal)],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        _done_triggers: &crate::analyze::DoneTriggers,
        _bus_usage: &Result<BusUsage, Box<dyn Error>>,
    ) -> (
        String,
        Result<Vec<libbusperf::bus_usage::RealTime>, Box<dyn Error>>,
    ) {
        let times = self.analyze_internal(simulation_data, loaded, intervals);
        (self.name, times)
    }
}

impl ChannelTrigger {
    fn analyze_internal(
        &self,
        simulation_data: &mut crate::analyze::SimulationData,
        loaded: &[&(wellen::SignalRef, wellen::Signal)],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
    ) -> Result<Vec<RealTime>, Box<dyn Error>> {
        let (_, clk_signal) = &loaded[0];
        let iterator =
            SignalsIterator::new(clk_signal, loaded[2..].iter().map(|(_, s)| s).collect());
        let time_table = &simulation_data.time_table;
        iterator
            .filter_map(
                |(time, values)| match values.into_iter().collect::<Option<Vec<_>>>() {
                    Some(values) => {
                        if intervals.iter().any(|&[s, e]| {
                            time_table[time as usize] >= s && time_table[time as usize] <= e
                        }) {
                            let type_ = self.analyzer.interpret_cycle(&values, time);
                            if type_ == self.type_ {
                                Some(Ok(time_table[time as usize]))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    None => Some(Err("missing value".into())),
                },
            )
            .collect::<Result<_, _>>()
    }
    pub fn new(name: String, type_: CycleType, analyzer: Rc<dyn LockstepAnalyzer>) -> Self {
        Self {
            name,
            type_,
            analyzer,
        }
    }

    pub fn from_yaml(
        name: String,
        yaml: &Yaml,
        bus_description: &Rc<dyn BusDescription>,
        analyzer: &Rc<dyn LockstepAnalyzer>,
        clk_path: &SignalPath,
    ) -> Result<Box<dyn TriggerSource>, Box<dyn Error>> {
        match yaml {
            Yaml::Array(yamls) => {
                let sub = yamls
                    .iter()
                    .map(|y| {
                        ChannelTrigger::from_yaml(
                            String::new(),
                            y,
                            bus_description,
                            analyzer,
                            clk_path,
                        )
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Box::new(TriggerSourceCombination::new(
                    name,
                    sub,
                    CombinationType::Any,
                )))
            }
            Yaml::Hash(hash) => {
                if let Some(y) = hash.get(&Yaml::String(String::from("all"))) {
                    let sub = y
                        .as_vec()
                        .ok_or("all requires array")?
                        .iter()
                        .map(|t| {
                            ChannelTrigger::from_yaml(
                                String::new(),
                                t,
                                bus_description,
                                analyzer,
                                clk_path,
                            )
                        })
                        .collect::<Result<_, _>>()?;
                    return Ok(Box::new(TriggerSourceCombination::new(
                        name,
                        sub,
                        CombinationType::All,
                    )));
                }

                if let Some(y) = hash.get(&Yaml::String(String::from("state"))) {
                    let type_ = match y.as_str().ok_or("bus state should be a string")? {
                        "busy" => CycleType::Busy,
                        "free" => CycleType::Free,
                        "no transaction" => CycleType::NoTransaction,
                        "backpressure" => CycleType::Backpressure,
                        "no data" => CycleType::NoData,
                        "reset" => CycleType::Reset,
                        "unknown" => CycleType::Unknown,
                        other => Err(format!("{other} is not a state"))?,
                    };
                    return Ok(Box::new(ChannelTrigger::new(
                        name,
                        type_,
                        Rc::clone(analyzer),
                    )));
                }
                if let Some(signal_name) = hash.get(&Yaml::String(String::from("signal"))) {
                    let signal_name = signal_name.as_str().ok_or("invalid signal name")?;
                    let signal = bus_description
                        .get_by_name(signal_name)
                        .ok_or(format!("signal {signal_name} not defined in description"))?;
                    return SignalTrigger::from_yaml(name, yaml, clk_path.clone(), signal.clone())
                        .map(|t| Box::new(t) as Box<dyn TriggerSource>);
                }

                return Err(format!("unknown trigger type {:?}", hash))?;
            }
            other => Err(format!("bad trigger type {:?}", other))?,
        }
    }
}
