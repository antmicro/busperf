use std::{error::Error, rc::Rc};

use default_analyzer::DefaultAnalyzer;
use itertools::Itertools;
#[cfg(feature = "python-plugins")]
use python_analyzer::PythonAnalyzer;
use wellen::{Signal, SignalRef, TimeTable};
use yaml_rust2::Yaml;

use crate::analyze::{AnalyzersConfig, DoneTriggers};
use crate::analyze::{
    SimulationData,
    analyzer::axi_analyzer::{AXIRdAnalyzer, AXIWrAnalyzer},
    bus::BusDescription,
    load_signals,
    trigger::{TriggerResult, TriggerSink, TriggerSource},
};
use libbusperf::{
    SignalPath,
    bus_usage::{BusData, BusUsage, RealTime},
};

mod axi_analyzer;
mod default_analyzer;
#[cfg(feature = "python-plugins")]
mod python_analyzer;

pub(crate) struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(
        yaml: (Yaml, Yaml),
        config: &AnalyzersConfig,
    ) -> Result<Box<dyn Analyzer>, Box<dyn Error>> {
        let (name, dict) = yaml;
        let name = name.into_string().ok_or("Invalid bus name")?;
        let analyzer: Box<dyn Analyzer> = if let Some(custom) = dict["custom_analyzer"].as_str() {
            match custom {
                "AXIWrAnalyzer" => Box::new(AXIWrAnalyzer::build_from_yaml(name, dict, config)?),
                "AXIRdAnalyzer" => Box::new(AXIRdAnalyzer::build_from_yaml(name, dict, config)?),
                _ => {
                    #[cfg(feature = "python-plugins")]
                    {
                        Box::new(
                            PythonAnalyzer::new(name, custom, &dict, config)
                                .map_err(|e| format!("plugin {custom}: {e}"))?,
                        )
                    }
                    #[cfg(not(feature = "python-plugins"))]
                    {
                        Err(format!(
                            "Analyzer {} does not exist or Python plugins are disabled",
                            custom
                        ))?
                    }
                }
            }
        } else {
            Box::new(DefaultAnalyzer::from_yaml(name, dict, config)?)
        };
        Ok(analyzer)
    }
}

impl PartialEq for dyn Analyzer {
    fn eq(&self, other: &Self) -> bool {
        self.bus_name() == other.bus_name()
    }
}

pub struct AnalyzerResult {
    pub name: String,
    pub result: Result<BusData, Box<dyn Error>>,
    pub triggers: Vec<TriggerResult>,
}

/// [Analyzer::analyze()] helper function.
fn get_result<T>(
    mut analyzer: Box<T>,
    simulation_data: &mut SimulationData,
    loaded: &[&(SignalRef, Signal)],
    intervals: &[[libbusperf::bus_usage::RealTime; 2]],
    done_triggers: &DoneTriggers,
    verbose: bool,
) -> AnalyzerResult
where
    T: Analyzer + ?Sized,
{
    let start = std::time::Instant::now();
    let usage = analyzer.calculate(loaded, intervals, &simulation_data.body.time_table);
    if verbose {
        println!(
            "Calculating statistics for {} took {:?}",
            analyzer.bus_name(),
            start.elapsed()
        );
    }
    let (name, description, triggers) = analyzer.consume();
    let triggers = triggers
        .into_iter()
        .map(|t| t.analyze(simulation_data, loaded, intervals, done_triggers, &usage))
        .collect();
    let signals = description.into_signals();
    let result = usage.map(|usage| BusData { usage, signals });

    AnalyzerResult {
        name,
        result,
        triggers,
    }
}

/// Trait that each busperf analyzer should implement.
pub trait Analyzer {
    fn bus_name(&self) -> &str;
    /// Return this analyzer's [TriggerSink].
    fn sink(&self) -> &TriggerSink;
    /// Returns list of triggers required to be completed to run this analyzer.
    fn requires(&self) -> Vec<&str>;
    /// Returns list of triggers that this analyzer will provide when calculated.
    fn provides(&self) -> Vec<&str>;
    /// Returns waveform scope paths to every signal required by the analyzer.
    fn get_signals(&self) -> Vec<&SignalPath>;
    /// Method that should perform all calculations for an analysis of the bus
    fn calculate(
        &mut self,
        loaded: &[&(SignalRef, Signal)],
        intervals: &[[RealTime; 2]],
        time_table: &TimeTable,
    ) -> Result<BusUsage, Box<dyn Error>>;
    /// Convert Analyzer into its name, description it used and list of triggers it provides.
    fn consume(self: Box<Self>) -> (String, Rc<dyn BusDescription>, Vec<Box<dyn TriggerSource>>);
    /// Trait method that performs an analysis of a loaded bus.
    fn analyze(
        self: Box<Self>,
        simulation_data: &mut SimulationData,
        done_triggers: &DoneTriggers,
        verbose: bool,
    ) -> AnalyzerResult {
        let start = std::time::Instant::now();
        let mut buffer = Vec::new();
        let loaded = match load_signals(simulation_data, &self.get_signals(), &mut buffer) {
            Ok(l) => l,
            Err(e) => {
                let (name, _signals, triggers) = self.consume();
                let triggers = triggers
                    .into_iter()
                    .map(|t| (t.into_name(), Err(format!("{e}").into())))
                    .collect_vec();
                return AnalyzerResult {
                    name,
                    result: Err(e),
                    triggers,
                };
            }
        };
        if verbose {
            println!(
                "Loading signals for {} took {:?}",
                self.bus_name(),
                start.elapsed()
            );
        }

        let intervals = self.sink().get_intervals(
            done_triggers,
            *simulation_data.body.time_table.last().unwrap_or(&0),
        );
        match intervals {
            Ok(intervals) => get_result(
                self,
                simulation_data,
                &loaded,
                &intervals,
                done_triggers,
                verbose,
            ),
            Err(e) => {
                let result = Err(format!("bad intervals: {e}").into());
                let (name, _signals, triggers) = self.consume();
                let triggers = triggers
                    .into_iter()
                    .map(|t| (t.into_name(), Err(format!("bad intervals: {e}").into())))
                    .collect();

                AnalyzerResult {
                    name,
                    result,
                    triggers,
                }
            }
        }
    }
}
