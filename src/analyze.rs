//! Module containing analyzing logic.
use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read},
    iter::Peekable,
    ops::Index,
    sync::{Arc, atomic::AtomicU64},
};

use hashbrown::HashMap;
use itertools::Itertools;
use wellen::{
    Hierarchy, LoadOptions, Signal, SignalSource, SignalValue, TimeTableIdx, Timescale,
    viewers::{self, BodyResult},
};
use yaml_rust2::YamlLoader;

use analyzer::{Analyzer, AnalyzerBuilder};
use bus::SignalPath;
use libbusperf::{
    CyclesNum,
    bus_usage::{BusData, RealTime},
};

use crate::analyze::{
    analyzer::AnalyzerResult,
    bus::{ValueType, get_value},
    trigger::{Control, ControlBuilder, TriggerName},
};

mod analyzer;
mod bus;
#[cfg(feature = "python-plugins")]
mod plugins;
mod trigger;

/// Configuration for the analyzers.
///
/// - `default_max_burst_delay` - number of consecutive non busy clock cycles that can occur on the bus that will not end the burst
/// - `window_length` - size of rolling window when calculating bandwidth
/// - `x_rate` - threshold for calculating percentage of time that bandwidth was higher than
/// - `y_rate` - threshold for calculating percentage of time that bandwidth was lower than
/// - `plugins_path` - path where to search for python plugins
pub struct AnalyzersConfig {
    default_max_burst_delay: CyclesNum,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
    plugins_path: String,
}

impl AnalyzersConfig {
    pub fn new(
        default_max_burst_delay: CyclesNum,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
        plugins_path: String,
    ) -> Self {
        Self {
            default_max_burst_delay,
            window_length,
            x_rate,
            y_rate,
            plugins_path,
        }
    }
    pub fn set_max_burst_delay(&mut self, num: CyclesNum) {
        self.default_max_burst_delay = num;
    }
    pub fn set_plugins_path(&mut self, s: &str) {
        self.plugins_path = s.to_owned();
    }
}

impl Default for AnalyzersConfig {
    fn default() -> Self {
        Self {
            default_max_burst_delay: 0,
            window_length: 10000,
            x_rate: 0.0001,
            y_rate: 0.00001,
            plugins_path: String::from("./plugins/python"),
        }
    }
}

pub struct AnalyzersGraph {
    analyzers: Vec<Box<dyn Analyzer>>,
    control: Vec<Box<dyn Control>>,
}

impl AnalyzersGraph {
    pub fn into_analyzers(self) -> Vec<Box<dyn Analyzer>> {
        self.analyzers
    }
}

/// Loads descriptions of the buses from yaml file with given name.
pub fn load_bus_analyzers(
    filename: &str,
    config: &AnalyzersConfig,
) -> Result<AnalyzersGraph, Box<dyn std::error::Error>> {
    let mut f = File::open(filename)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let mut yaml = YamlLoader::load_from_str(&s)?;
    let mut doc = yaml
        .remove(0)
        .into_hash()
        .ok_or("Yaml should not be empty")?;
    let interfaces = doc
        .remove(&yaml_rust2::Yaml::from_str("interfaces"))
        .ok_or("Yaml should define interfaces")?
        .into_hash()
        .ok_or("Invalid yaml format")?;
    let control_only = doc.remove(&yaml_rust2::Yaml::from_str("control_only"));
    let unused = doc
        .into_iter()
        .filter_map(|(name, _)| {
            if let Some(s) = name.into_string()
                && s != "scopes"
                && s != "common_clk_rst_ifs"
            {
                Some(s)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !unused.is_empty() {
        Err(format!(
            "Yaml can only have interfaces, scopes(optional) and common_clk_rst_ifs(optional) in top level, but has extra: {}",
            unused.join(", ")
        ))?;
    }
    let mut analyzers = vec![];
    for (name, dict) in interfaces {
        let n = name
            .as_str()
            .ok_or("Each bus should have a name")?
            .to_owned();
        analyzers.push(
            AnalyzerBuilder::build((name, dict), config).map_err(|e| format!("bus {n}, {e}"))?,
        );
    }

    let control = match control_only {
        Some(control_only) => {
            let mut control = vec![];
            let control_only = control_only
                .into_hash()
                .ok_or("control_only should define extra triggers")?;
            for (name, yaml) in control_only {
                let name = name.into_string().ok_or("invalid control trigger name")?;
                control.push(ControlBuilder::build_from_yaml(name, &yaml)?);
            }
            control
        }
        None => vec![],
    };

    let graph = AnalyzersGraph { analyzers, control };

    check_dependencies(&graph)?;

    Ok(graph)
}

fn check_dependencies(analyzers: &AnalyzersGraph) -> Result<(), Box<dyn Error>> {
    let existing_triggers = analyzers
        .control
        .iter()
        .flat_map(|c| c.names())
        .chain(
            analyzers
                .analyzers
                .iter()
                .flat_map(|a| a.provides().into_iter()),
        )
        .collect_vec();
    for a in analyzers.analyzers.iter() {
        if let Some(req) = a
            .requires()
            .iter()
            .find(|req| !existing_triggers.contains(req))
        {
            Err(format!(
                "analyzer {} requires {} trigger which is not defined",
                a.bus_name(),
                req
            ))?;
        }
    }
    for c in analyzers.control.iter() {
        if let Some(req) = c
            .requires()
            .iter()
            .find(|req| !existing_triggers.contains(req))
        {
            Err(format!(
                "analyzer {} requires {} trigger which is not defined",
                c.name(),
                req
            ))?;
        }
    }

    Ok(())
}

type DoneTriggers = HashMap<TriggerName, Result<Vec<RealTime>, Box<dyn Error>>>;

/// Analyze all analyzers in passed analyzers graph
///
/// Returns `Vec` containing `Result` of every analyzers' calculations
pub fn analyze_all(
    analyzers: AnalyzersGraph,
    simulation_data: &mut SimulationData,
    verbose: bool,
) -> Vec<Result<BusData, Box<dyn Error>>> {
    let mut done_triggers: DoneTriggers = HashMap::new();
    let mut results: HashMap<String, Result<BusData, Box<dyn Error>>> = HashMap::new();
    let AnalyzersGraph {
        mut analyzers,
        mut control,
    } = analyzers;
    let analyzers_order = analyzers
        .iter()
        .map(|a| a.bus_name().to_owned())
        .collect_vec();
    while !analyzers.is_empty() {
        let mut anything_was_analyzed = false;
        for control in control
            .extract_if(.., |c| {
                c.requires()
                    .iter()
                    .all(|req| done_triggers.keys().any(|done| done == req))
            })
            .collect_vec()
        {
            anything_was_analyzed = true;
            if verbose {
                println!("[Info] Started analysing {}", control.name());
            }
            for (name, res) in control.analyze(&done_triggers) {
                if verbose {
                    println!("[Info] Finished analysing {name}");
                }
                done_triggers.insert(name, res);
            }
        }

        for analyzer in analyzers
            .extract_if(.., |c| {
                c.requires()
                    .iter()
                    .all(|req| done_triggers.keys().any(|done| done == req))
            })
            .collect_vec()
        {
            if verbose {
                println!("[Info] Started analysing {}", analyzer.bus_name());
            }
            anything_was_analyzed = true;
            let AnalyzerResult {
                name,
                result,
                triggers,
            } = analyzer.analyze(simulation_data, &done_triggers, verbose);
            if verbose {
                println!("[Info] Finished analysing {name}");
            }
            results.insert(name, result);
            triggers.into_iter().for_each(|(name, res)| {
                done_triggers.insert(name, res);
            });
        }
        if !anything_was_analyzed {
            let not_analyzed = analyzers
                .iter()
                .map(|a| a.bus_name())
                .chain(control.iter().map(|c| c.name()))
                .join(", ");
            eprintln!(
                "[ERROR] {not_analyzed} were not analyzed because of trigger dependency cycle"
            );
            return analyzers_order
                .iter()
                .filter_map(|name| results.remove(name))
                .collect_vec();
        }
    }
    analyzers_order
        .iter()
        .map(|name| {
            results
                .remove(name)
                .expect("All analyzers have been analyzed")
        })
        .collect_vec()
}

pub struct TimeTable {
    table: Vec<u64>,
}

impl TimeTable {
    fn new(table: Vec<u64>) -> Self {
        Self { table }
    }
    pub fn iter(&self) -> std::slice::Iter<'_, u64> {
        self.table.iter()
    }
    pub fn get(&self, index: usize) -> Option<&u64> {
        self.table.get(index)
    }
    pub fn last(&self) -> u64 {
        self.table.last().copied().unwrap_or(0)
    }
    pub fn binary_search(&self, value: &u64) -> Result<usize, usize> {
        self.table.binary_search(value)
    }
}

impl Index<usize> for TimeTable {
    type Output = RealTime;

    fn index(&self, index: usize) -> &Self::Output {
        &self.table[index]
    }
}

pub struct SimulationData {
    hierarchy: Hierarchy,
    signal_source: SignalSource,
    time_table: TimeTable,
}

impl SimulationData {
    pub fn timescale(&self) -> libbusperf::Timescale {
        let timescale = self.hierarchy.timescale().unwrap_or(Timescale {
            factor: 1,
            unit: wellen::TimescaleUnit::Seconds,
        });
        libbusperf::Timescale {
            factor: timescale.factor,
            order: timescale.unit.to_exponent().unwrap_or(0),
        }
    }
}

/// Loads waveform file.
///
/// * `filename` - path to file.
/// * `verbose` - prints how long it took to load.
pub fn load_simulation_trace(
    filename: &str,
    verbose: bool,
) -> Result<SimulationData, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let load_options = LoadOptions {
        multi_thread: true,
        remove_scopes_with_empty_name: false,
    };
    let file = BufReader::new(std::fs::File::open(filename)?);
    let header = viewers::read_header(file, &load_options)?;
    let hierarchy = header.hierarchy;
    let BodyResult { source, time_table } =
        viewers::read_body(header.body, &hierarchy, Some(Arc::new(AtomicU64::new(0))))?;
    let time_table = TimeTable::new(time_table);
    if verbose {
        println!("Loading trace took {:?}", start.elapsed());
    }
    Ok(SimulationData {
        hierarchy,
        signal_source: source,
        time_table,
    })
}

fn load_signals<'signal_buffer>(
    simulation_data: &mut SimulationData,
    signal_paths: &Vec<&SignalPath>,
    buffer: &'signal_buffer mut Vec<(wellen::SignalRef, wellen::Signal)>,
) -> Result<Vec<&'signal_buffer (wellen::SignalRef, wellen::Signal)>, Box<dyn Error>> {
    let hierarchy = &simulation_data.hierarchy;
    let source = &mut simulation_data.signal_source;
    let signal_refs: Vec<wellen::SignalRef> = signal_paths
        .iter()
        .map(|path| {
            Ok(hierarchy[hierarchy
                .lookup_var(&path.scope, &path.name)
                .ok_or(format!("signal \"{}\" does not exist", path))?]
            .signal_ref())
        })
        .collect::<Result<_, Box<dyn Error>>>()?;

    *buffer = source.load_signals(&signal_refs, hierarchy, true);
    // SignalSource::load_signals can return a vector of different size than passsed signals refs vector
    // e.g when some ref is duplicated (this can happen if a user uses the same simulation signal in two
    // analyzer signals) but we want to return loaded signals in same order as in requested paths
    let loaded = signal_refs
        .iter()
        .map(|signal_ref| {
            buffer
                .iter()
                .find(|(r, _)| signal_ref == r)
                .expect("Signal should be loaded for each SignalRef")
        })
        .collect();

    Ok(loaded)
}

struct RisingSignalIterator<'a> {
    signal: Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>,
    peeked: Option<TimeTableIdx>,
}

impl<'a> RisingSignalIterator<'a> {
    fn new(signal: &'a Signal) -> Self {
        let signal: Box<dyn Iterator<Item = _>> = Box::new(signal.iter_changes());
        let signal = signal.peekable();
        Self {
            signal,
            peeked: None,
        }
    }

    fn find_non_consuming<P>(&mut self, mut predicate: P) -> Option<TimeTableIdx>
    where
        P: FnMut(&TimeTableIdx) -> bool,
    {
        loop {
            if let Some(t) = self.next() {
                if predicate(&t) {
                    self.peeked = Some(t);
                    break Some(t);
                }
            } else {
                break None;
            }
        }
    }
}

impl<'a> Iterator for RisingSignalIterator<'a> {
    type Item = TimeTableIdx;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(t) = self.peeked {
            self.peeked = None;
            Some(t)
        } else {
            loop {
                match self.signal.next() {
                    Some((_, value)) => {
                        if matches!(get_value(value), Some(ValueType::V0))
                            && let Some((time, next_value)) = self.signal.next()
                            && matches!(get_value(next_value), Some(ValueType::V1))
                        {
                            return Some(time);
                        }
                    }
                    None => return None,
                }
            }
        }
    }
}

type Iter<'a> = Peekable<Box<dyn Iterator<Item = (u32, SignalValue<'a>)> + 'a>>;
struct SignalsIterator<'a> {
    clk: RisingSignalIterator<'a>,
    signals: Vec<(Option<SignalValue<'a>>, Iter<'a>)>,
}

impl<'a> SignalsIterator<'a> {
    fn new(clk: &'a Signal, signals: Vec<&'a Signal>) -> Self {
        let signals = signals
            .into_iter()
            .map(|s| {
                let iter: Box<dyn Iterator<Item = _>> = Box::new(s.iter_changes());
                (None, iter.peekable())
            })
            .collect();
        Self {
            clk: RisingSignalIterator::new(clk),
            signals,
        }
    }
}

impl<'a> Iterator for SignalsIterator<'a> {
    type Item = (TimeTableIdx, Vec<Option<SignalValue<'a>>>);

    fn next(&mut self) -> Option<Self::Item> {
        let clock_time = self.clk.next()?;
        let signal_values = self
            .signals
            .iter_mut()
            .map(|(last, iter)| {
                while let Some((time, _)) = iter.peek()
                    && *time < clock_time
                {
                    *last = Some(iter.next().expect("Is Some").1);
                }
                *last
            })
            .collect();
        Some((clock_time, signal_values))
    }
}
