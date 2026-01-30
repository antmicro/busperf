use std::{cmp::Reverse, collections::BinaryHeap, error::Error};

use libbusperf::{
    bus_usage::{BusUsage, RealTime},
    CycleType, SignalPath,
};
use wellen::{Signal, SignalRef, SignalValue, TimeTable};
use yaml_rust2::Yaml;

use crate::analyze::{
    bus::{BusCommon, BusDescriptionBuilder, LockstepAnalyzer, SignalPathFromYaml},
    SignalsIterator,
};

#[derive(Debug)]
pub struct Trigger {
    start: TriggerGroup,
    end: TriggerGroup,
}

#[derive(Debug)]
struct TriggerGroup {
    triggers: Vec<TriggerType>,
}

enum TriggerType {
    None,
    Time(Vec<RealTime>),
    Signal(SignalPath, BitMatch),
    BusState(BusCommon, Box<dyn LockstepAnalyzer>, CycleType),
    Transaction(String /*analyzer name*/, TransactionTrigger),
    Complex(Vec<TriggerType>),
}

#[derive(Debug)]
enum TransactionTrigger {
    Start,
    End,
}

#[derive(Debug)]
struct BitMatch {
    filter: Vec<BitFilter>,
}

struct BitFilter {
    mask: i64,
    value: i64,
}

impl Trigger {
    pub fn depends_on(&self) -> Vec<String> {
        self.start
            .triggers
            .iter()
            .chain(self.end.triggers.iter())
            .filter_map(|t| t.depends_on())
            .flatten()
            .collect()
    }
    pub fn get_signals(&self) -> Vec<&SignalPath> {
        self.start
            .get_signals()
            .into_iter()
            .chain(self.end.get_signals())
            .collect()
    }
    pub fn from_yaml(yaml: &Yaml, plugins_path: &str) -> Result<Self, Box<dyn Error>> {
        let start = TriggerGroup::from_yaml(&yaml["start_triggers"], plugins_path)
            .map_err(|e| format!("start triggers invalid: {e}"))?;
        let end = TriggerGroup::from_yaml(&yaml["end_triggers"], plugins_path)
            .map_err(|e| format!("end triggers invalid: {e}"))?;
        Ok(Trigger { start, end })
    }
    pub fn get_intervals(
        &self,
        loaded: &[&(SignalRef, Signal)],
        time_table: &TimeTable,
        depends: Vec<BusUsage>,
    ) -> Result<Vec<[RealTime; 2]>, Box<dyn Error>> {
        let num = self.start.triggers.iter().map(|t| t.signals_num()).sum();
        let start_iter = self
            .start
            .trigger_iter(&loaded[..num], time_table, &depends)?;
        let mut end_iter = self
            .end
            .trigger_iter(&loaded[num..], time_table, &depends)?;

        let mut intervals = vec![];
        let mut last_end = 0;
        for start in start_iter {
            if start < last_end {
                continue;
            }

            let end = end_iter
                .find(|&t| t > start)
                .unwrap_or(*time_table.last().expect("Simulation should not be empty"));
            intervals.push([start, end]);
            last_end = end;
        }

        Ok(intervals)
    }
}

impl TriggerGroup {
    fn from_yaml(yaml: &Yaml, plugins_path: &str) -> Result<Self, Box<dyn Error>> {
        Ok(TriggerGroup {
            triggers: match yaml {
                Yaml::Array(yamls) => yamls
                    .iter()
                    .map(|trigger| TriggerType::from_yaml(trigger, plugins_path))
                    .collect::<Result<Vec<_>, _>>()?,
                Yaml::BadValue => vec![TriggerType::None],
                _ => Err("not an array")?,
            },
        })
    }
    fn get_signals(&self) -> Vec<&SignalPath> {
        self.triggers
            .iter()
            .flat_map(|t| t.get_signals())
            .collect::<Vec<_>>()
    }
    fn trigger_iter<'b>(
        &self,
        loaded: &[&'b (SignalRef, Signal)],
        time_table: &'b TimeTable,
        depends: &[BusUsage],
    ) -> Result<Box<dyn Iterator<Item = RealTime> + '_>, Box<dyn Error>> {
        let mut trigger_times = vec![];
        let mut offset = 0;
        let mut triggers = self
            .triggers
            .iter()
            .map(|t| {
                let needed_signals_num = t.get_signals().len();
                let ret = t.iter(
                    &loaded[offset..offset + needed_signals_num],
                    time_table,
                    depends,
                );
                offset += needed_signals_num;
                ret
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut heap = BinaryHeap::new();
        for (i, t) in triggers.iter_mut().enumerate() {
            if let Some(time) = t.next() {
                heap.push(Reverse((time, i)));
            }
        }

        while let Some(Reverse((time, i))) = heap.pop() {
            trigger_times.push(time);
            if let Some(next_time) = triggers[i].next() {
                heap.push(Reverse((next_time, i)));
            }
        }

        Ok(Box::new(trigger_times.into_iter()))
    }
}

impl std::fmt::Debug for BitFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BitFilter")
            .field("mask ", &format!("{:x}", &self.mask))
            .field("value", &format!("{:x}", &self.value))
            .finish()
    }
}

impl std::fmt::Debug for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Time(arg0) => f.debug_tuple("Time").field(arg0).finish(),
            Self::Signal(arg0, arg1) => f.debug_tuple("Signal").field(arg0).field(arg1).finish(),
            Self::BusState(arg0, arg1, arg2) => f
                .debug_tuple("Bus")
                .field(arg2)
                .field(arg0)
                .field(&arg1.signals())
                .finish(),
            TriggerType::None => f.write_str("None"),
            TriggerType::Transaction(arg0, arg1) => f
                .debug_tuple("Transaction")
                .field(arg0)
                .field(arg1)
                .finish(),
            TriggerType::Complex(arg0) => f.debug_tuple("Complex").field(arg0).finish(),
        }
    }
}

impl TriggerType {
    pub fn from_yaml(yaml: &Yaml, plugins_path: &str) -> Result<Self, Box<dyn Error>> {
        let hash = yaml.as_hash().ok_or("should define a trigger")?;
        if hash.contains_key(&Yaml::String("signal".to_string())) {
            let signal = SignalPathFromYaml::from_yaml_ref_with_prefix(&[], &yaml["signal"])?;
            let bit_match = BitMatch::from_yaml(yaml)?;

            return Ok(TriggerType::Signal(signal, bit_match));
        }
        if hash.contains_key(&Yaml::String("time".to_string())) {
            return Ok(TriggerType::Time(match &yaml["time"] {
                Yaml::Integer(t) => vec![u64::try_from(*t)?],
                Yaml::Array(yamls) => yamls
                    .iter()
                    .map(|y| Ok(u64::try_from(y.as_i64().ok_or("invalid time")?)?))
                    .collect::<Result<_, Box<dyn Error>>>()?,
                _ => Err("invalid time")?,
            }));
        }
        if hash.contains_key(&Yaml::String("bus".to_string())) {
            let common =
                BusCommon::from_yaml("trigger".to_string(), &yaml["bus"], 0, plugins_path)?;
            let desc = BusDescriptionBuilder::build(
                yaml["bus"].clone(),
                common.module_scope(),
                plugins_path,
            )?;
            let state = match yaml["state"].as_str().ok_or("state not defined")? {
                "busy" => CycleType::Busy,
                "free" => CycleType::Free,
                "no transaction" => CycleType::NoTransaction,
                "backpressure" => CycleType::Backpressure,
                "no data" => CycleType::NoData,
                "reset" => CycleType::Reset,
                "unknown" => CycleType::Unknown,
                other => Err(format!("{other} is not a state"))?,
            };
            return Ok(TriggerType::BusState(common, desc, state));
        }
        if hash.contains_key(&Yaml::String("transaction_start".to_string())) {
            let name = yaml["transaction_start"]
                .as_str()
                .ok_or("transaction trigger expects interface name")?
                .to_owned();
            return Ok(TriggerType::Transaction(name, TransactionTrigger::Start));
        }
        if hash.contains_key(&Yaml::String("transaction_end".to_string())) {
            let name = yaml["transaction_end"]
                .as_str()
                .ok_or("transaction trigger expects interface name")?
                .to_owned();
            return Ok(TriggerType::Transaction(name, TransactionTrigger::End));
        }
        if hash.contains_key(&Yaml::String("all".to_string())) {
            let sub_triggers = yaml["all"]
                .as_vec()
                .ok_or("triggers under all keyword should be an YAML array")?
                .iter()
                .map(|y| TriggerType::from_yaml(y, plugins_path))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(TriggerType::Complex(sub_triggers));
        }
        Err(format!(
            "invalid trigger type {}",
            match hash.iter().next() {
                Some((k, _)) => k
                    .as_str()
                    .map(|s| s.to_owned())
                    .unwrap_or(format!("{:?}", k)),
                None => "none".to_owned(),
            }
        ))?
    }
    fn depends_on(&self) -> Option<Vec<String>> {
        match self {
            TriggerType::Transaction(analyzer_name, _) => Some(vec![analyzer_name.clone()]),
            TriggerType::Complex(triggers) => Some(
                triggers
                    .iter()
                    .filter_map(|t| t.depends_on())
                    .flatten()
                    .collect(),
            ),
            _ => None,
        }
    }
    pub fn get_signals(&self) -> Vec<&SignalPath> {
        match self {
            TriggerType::Time(_) => vec![],
            TriggerType::Signal(signal_path, _) => vec![signal_path],
            TriggerType::BusState(common, bus_description, _) => {
                let mut signals = vec![common.clk_path()];
                signals.append(&mut bus_description.signals());
                signals
            }
            TriggerType::None => vec![],
            TriggerType::Transaction(_, _) => {
                vec![]
            }
            TriggerType::Complex(trigger_types) => {
                trigger_types.iter().flat_map(|t| t.get_signals()).collect()
            }
        }
    }
    pub fn signals_num(&self) -> usize {
        self.get_signals().len()
    }

    fn active_at(
        &self,
        time: u32,
        loaded: &[&(SignalRef, Signal)],
        time_table: &TimeTable,
        depends: &[BusUsage],
    ) -> Result<bool, Box<dyn Error>> {
        match self {
            TriggerType::None => Ok(time == 0),
            TriggerType::Time(items) => Ok(items.contains(&time_table[time as usize])),
            TriggerType::Signal(_, bit_match) => {
                let (_, signal) = &loaded[0];
                Ok(compare_values(
                    signal.get_value_at(
                        &signal.get_offset(time).ok_or(format!(
                            "invalid signal value at {}",
                            time_table[time as usize]
                        ))?,
                        0,
                    ),
                    bit_match,
                ))
            }
            TriggerType::BusState(_, bus_description, cycle_type) => {
                // We subtract one to use values just before clock edge
                let time = time.saturating_sub(1);
                let values: Vec<SignalValue> = loaded[1..]
                    .iter()
                    .map(|(_, s)| {
                        Ok::<_, Box<dyn Error>>(s.get_value_at(
                            &s.get_offset(time).ok_or(format!(
                                "signal does not have value at {}",
                                time_table[time as usize]
                            ))?,
                            0,
                        ))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(bus_description.interpret_cycle(&values, time) == *cycle_type)
            }
            TriggerType::Transaction(analyzer_name, transaction_trigger) => {
                let Some(results) = depends.iter().find(|a| a.get_name() == analyzer_name) else {
                    Err(format!("required analyzer {} failed", analyzer_name))?
                };
                let BusUsage::MultiChannel(result) = results else {
                    Err(format!(
                        "analyzer for bus {} does not provide transaction triggers",
                        analyzer_name
                    ))?
                };
                match transaction_trigger {
                    TransactionTrigger::Start => Ok(result
                        .get_transactions_start_end()
                        .iter()
                        .any(|p| p.start() == time_table[time as usize])),
                    TransactionTrigger::End => Ok(result
                        .get_transactions_start_end()
                        .iter()
                        .any(|p| p.end() == time_table[time as usize])),
                }
            }
            TriggerType::Complex(triggers) => {
                let mut offset = 0;
                for t in triggers {
                    if !t.active_at(
                        time,
                        &loaded[offset..offset + t.get_signals().len()],
                        time_table,
                        depends,
                    )? {
                        return Ok(false);
                    }
                    offset += t.get_signals().len();
                }
                Ok(true)
            }
        }
    }

    pub fn iter<'b>(
        &'b self,
        loaded: &[&'b (SignalRef, Signal)],
        time_table: &'b TimeTable,
        depends: &[BusUsage],
    ) -> Result<Box<dyn Iterator<Item = RealTime> + '_>, Box<dyn Error>> {
        Ok(match self {
            TriggerType::Time(items) => Box::new(items.iter().copied()),
            TriggerType::Signal(_, value) => {
                let (_, signal) = &loaded[0];
                let mut start = None;
                let iter = signal
                    .iter_changes()
                    .chain(vec![(
                        time_table.len() as u32,
                        SignalValue::String("unused"),
                    )])
                    .filter_map(move |(time, v)| {
                        if compare_values(v, value) {
                            start = Some(time);
                            None
                        } else {
                            start.take().map(|start| {
                                (start..time)
                                    .map(|t| time_table[t as usize])
                                    .collect::<Vec<_>>()
                            })
                        }
                    })
                    .flatten();
                Box::new(iter)
            }
            TriggerType::BusState(_, bus_description, cycle_type) => {
                let (_, clk_signal) = &loaded[0];
                let iterator =
                    SignalsIterator::new(clk_signal, loaded[1..].iter().map(|(_, s)| s).collect());
                let times = iterator.filter_map(|(time, values)| {
                    let values = values.into_iter().collect::<Option<Vec<_>>>()?;
                    let type_ = bus_description.interpret_cycle(&values, time);
                    if type_ == *cycle_type {
                        Some(time_table[time as usize])
                    } else {
                        None
                    }
                });

                Box::new(times)
            }
            TriggerType::None => Box::new(vec![0].into_iter()),
            TriggerType::Transaction(analyzer_name, transaction_trigger) => {
                let Some(results) = depends.iter().find(|a| a.get_name() == analyzer_name) else {
                    Err(format!("required analyzer {} failed", analyzer_name))?
                };
                let BusUsage::MultiChannel(result) = results else {
                    Err(format!(
                        "analyzer for bus {} does not provide transaction triggers",
                        analyzer_name
                    ))?
                };
                match transaction_trigger {
                    TransactionTrigger::Start => Box::new(
                        result
                            .get_transactions_start_end()
                            .iter()
                            .map(|p| p.start())
                            .collect::<Vec<_>>()
                            .into_iter(),
                    ),
                    TransactionTrigger::End => Box::new(
                        result
                            .get_transactions_start_end()
                            .iter()
                            .map(|p| p.end())
                            .collect::<Vec<_>>()
                            .into_iter(),
                    ),
                }
            }
            TriggerType::Complex(triggers) => {
                let main_trigger = &triggers[0];
                let iter = main_trigger.iter(
                    &loaded[0..main_trigger.get_signals().len()],
                    time_table,
                    depends,
                )?;
                let offset = main_trigger.get_signals().len();
                let iter = iter
                    .map(|time| {
                        let time_idx = time_table
                            .binary_search(&time)
                            .map_err(|_| "invalid time table")?
                            as u32;
                        let mut offset = offset;
                        for t in &triggers[1..] {
                            let needed = t.get_signals().len();
                            if !t.active_at(
                                time_idx,
                                &loaded[offset..offset + needed],
                                time_table,
                                depends,
                            )? {
                                return Ok((false, time));
                            }
                            offset += needed;
                        }
                        Ok((true, time))
                    })
                    .collect::<Result<Vec<_>, Box<dyn Error>>>()?
                    .into_iter()
                    .filter_map(|(ok, time)| if ok { Some(time) } else { None });
                Box::new(iter)
            }
        })
    }
}

fn compare_values(signal: SignalValue, value: &BitMatch) -> bool {
    let SignalValue::Binary(s, bits) = signal else {
        return false;
    };
    let mut buf = [0u8; 8];
    buf[8 - (bits + 7) as usize / 8..].copy_from_slice(s);
    value.matches(i64::from_be_bytes(buf))
}

impl BitMatch {
    pub fn from_yaml(yaml: &Yaml) -> Result<Self, Box<dyn Error>> {
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
