use std::{error::Error, rc::Rc};

use libbusperf::{SignalPath, bus_usage::BusUsage};
use wellen::{Signal, SignalRef, SignalValue};
use yaml_rust2::Yaml;

use super::TriggerSource;
use crate::analyze::{
    DoneTriggers, SignalsIterator, SimulationData,
    bus::BusDescription,
    trigger::trigger_combination::{CombinationType, TriggerSourceCombination},
};

pub struct SignalTrigger {
    name: String,
    clk_path: SignalPath,
    signal: SignalPath,
    bit_match: BitMatch,
}

struct BitMatch {
    filter: Vec<BitFilter>,
}

struct BitFilter {
    mask: i64,
    value: i64,
}
impl BitMatch {
    fn from_yaml(yaml: &Yaml) -> Result<Self, Box<dyn Error>> {
        if let Yaml::Integer(value) = yaml["value"] {
            return Ok(BitMatch {
                filter: vec![BitFilter {
                    mask: i64::MAX,
                    value,
                }],
            });
        }
        let masks = yaml["range"].as_vec().ok_or("no range specified")?;
        let values = yaml["value"].as_vec().ok_or("no values")?;
        let filter = masks
            .iter()
            .zip(values)
            .map(|(range_text, value)| {
                let range_text = range_text.as_str().ok_or("invalid range set")?;
                let i = range_text.find(":").ok_or("invalid range")?;
                let top = range_text[..i].parse::<u32>()?;
                let bottom = range_text[i + 1..].parse::<u32>()?;
                let width = top - bottom + 1;
                let mask = ((1i64 << width) - 1) << bottom;
                let value = value.as_i64().ok_or(format!("no value set {value:?}"))? << bottom;

                Ok(BitFilter { mask, value })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;

        Ok(BitMatch { filter })
    }
    fn compare_with_value(&self, signal: SignalValue) -> bool {
        let SignalValue::Binary(s, bits) = signal else {
            return false;
        };
        let mut buf = [0u8; 8];
        buf[8 - (bits + 7) as usize / 8..].copy_from_slice(s);
        self.matches(i64::from_be_bytes(buf))
    }

    fn matches(&self, signal_value: i64) -> bool {
        for m in self.filter.iter() {
            let BitFilter { mask, value } = m;
            if signal_value & mask != *value {
                return false;
            }
        }

        true
    }
}

impl SignalTrigger {
    pub fn from_yaml(
        name: String,
        yaml: &Yaml,
        clk_path: SignalPath,
        signal: SignalPath,
    ) -> Result<Self, Box<dyn Error>> {
        let bit_match = BitMatch::from_yaml(yaml)?;
        Ok(Self {
            name,
            clk_path,
            signal,
            bit_match,
        })
    }

    pub fn combination_from_yaml(
        name: String,
        yaml: &Yaml,
        bus_description: &Rc<dyn BusDescription>,
        clk_path: &SignalPath,
    ) -> Result<Box<dyn TriggerSource>, Box<dyn Error>> {
        match yaml {
            Yaml::Array(yamls) => {
                let sub = yamls
                    .iter()
                    .map(|y| {
                        SignalTrigger::combination_from_yaml(
                            String::new(),
                            y,
                            bus_description,
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
                            SignalTrigger::combination_from_yaml(
                                String::new(),
                                t,
                                bus_description,
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

    fn analyze_internal(
        &self,
        simulation_data: &mut SimulationData,
        loaded: &[&(SignalRef, Signal)],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + 'static>> {
        let h = &simulation_data.hierarchy;
        let clk_ref = h[h
            .lookup_var(&self.clk_path.scope, &self.clk_path.name)
            .ok_or("signal not found")?]
        .signal_ref();
        let signal_ref = h[h
            .lookup_var(&self.signal.scope, &self.signal.name)
            .ok_or("signal not found")?]
        .signal_ref();
        let (_, clk) = loaded
            .iter()
            .find(|&(r, _)| *r == clk_ref)
            .ok_or("signal not loaded")?;
        let (_, signal) = loaded
            .iter()
            .find(|&(r, _)| *r == signal_ref)
            .ok_or("signal not loaded")?;
        let iter = SignalsIterator::new(clk, vec![signal]);

        let mut intervals = intervals.iter();
        let mut trigger_times = vec![];
        if let Some(mut current_interval) = intervals.next() {
            for (time, values) in iter {
                let real_time = simulation_data.body.time_table[time as usize];
                let &[mut start, end] = current_interval;
                if real_time > end {
                    let Some(n) = intervals.next() else {
                        break;
                    };
                    current_interval = n;
                    start = n[0];
                }
                if real_time < start {
                    continue;
                }

                let value = values[0].ok_or("signal has no value at")?;
                if self.bit_match.compare_with_value(value) {
                    trigger_times.push(real_time);
                }
            }
        }

        Ok(trigger_times)
    }
}

impl TriggerSource for SignalTrigger {
    fn analyze(
        self: Box<Self>,
        simulation_data: &mut SimulationData,
        loaded: &[&(SignalRef, Signal)],
        intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        _done_triggers: &DoneTriggers,
        _bus_usage: &Result<BusUsage, Box<dyn Error>>,
    ) -> (
        std::string::String,
        Result<Vec<u64>, Box<dyn std::error::Error + 'static>>,
    ) {
        let result = self.analyze_internal(simulation_data, loaded, intervals);
        (self.name, result)
    }

    fn into_name(self: Box<Self>) -> String {
        self.name
    }

    fn name(&self) -> &str {
        &self.name
    }
}
