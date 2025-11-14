use default_analyzer::DefaultAnalyzer;
use python_analyzer::PythonAnalyzer;
use yaml_rust2::Yaml;

use crate::{
    SimulationData,
    analyzer::axi_analyzer::{AXIRdAnalyzer, AXIWrAnalyzer},
    bus::{BusCommon, CyclesNum},
    bus_usage::BusUsage,
    load_signals,
};

mod axi_analyzer;
mod default_analyzer;
mod python_analyzer;

const COMMON_YAML: &[&str] = &[
    "scope",
    "clk_rst_if.clock",
    "clock",
    "clk_rst_if.reset",
    "reset",
    "clk_rst_if.reset_type",
    "reset_type",
    "custom_analyzer",
    "intervals",
];

pub(crate) struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(
        yaml: (Yaml, Yaml),
        default_max_burst_delay: CyclesNum,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Box<dyn Analyzer>, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        let to_check = dict.clone();
        let analyzer: Box<dyn Analyzer> = if let Some(custom) = dict["custom_analyzer"].as_str() {
            match custom {
                "AXIWrAnalyzer" => Box::new(AXIWrAnalyzer::build_from_yaml(
                    (name, dict),
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                "AXIRdAnalyzer" => Box::new(AXIRdAnalyzer::build_from_yaml(
                    (name, dict),
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                _ => {
                    let common = BusCommon::from_yaml(
                        name.into_string().ok_or("Bus should have a valid name")?,
                        &dict,
                        default_max_burst_delay,
                    )?;
                    Box::new(
                        PythonAnalyzer::new(custom, common, &dict, window_length, x_rate, y_rate)
                            .map_err(|e| format!("plugin {custom}: {e}"))?,
                    )
                }
            }
        } else {
            Box::new(DefaultAnalyzer::from_yaml(
                (name, dict),
                default_max_burst_delay,
            )?)
        };
        check_unused_signals(
            &to_check,
            &analyzer.required_yaml_definitions(),
            &mut vec![],
        );
        Ok(analyzer)
    }
}

mod private {
    use crate::bus::SignalPath;
    use wellen::{Signal, SignalRef, TimeTable};

    pub trait AnalyzerInternal {
        fn bus_name(&self) -> &str;
        // Returns waveform scope paths to every signal required by the analyzer.
        fn get_signals(&self) -> Vec<&SignalPath>;
        // Method that should perform all calculations for an analysis of the bus
        fn calculate(&mut self, loaded: Vec<(SignalRef, Signal)>, time_table: &TimeTable);
    }
}

pub trait Analyzer: private::AnalyzerInternal {
    /// Trait method that performs an analysis of a loaded bus.
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool) {
        let start = std::time::Instant::now();
        let signal_paths = self.get_signals();
        let loaded = load_signals(simulation_data, &signal_paths);
        if verbose {
            println!(
                "Loading signals for {} took {:?}",
                self.bus_name(),
                start.elapsed()
            );
        }

        let start = std::time::Instant::now();
        self.calculate(loaded, &simulation_data.body.time_table);
        if verbose {
            println!(
                "Calculating statistics for {} took {:?}",
                self.bus_name(),
                start.elapsed()
            );
        }
    }
    /// If the analysis was run returns [Some] result of the analysis. If not - returns [None].
    fn get_results(&self) -> Option<&BusUsage>;
    fn finished_analysis(&self) -> bool {
        self.get_results().is_some()
    }

    fn required_yaml_definitions(&self) -> Vec<&str>;
}

fn check_unused_signals(yaml: &Yaml, used: &[&str], path: &mut Vec<String>) {
    match yaml {
        Yaml::Hash(linked_hash_map) => {
            for (k, v) in linked_hash_map {
                if let Yaml::String(s) = k {
                    path.push(s.clone());
                    check_unused_signals(v, used, path);
                    path.pop();
                } else {
                    eprintln!("[WARN] Non string hash key {}.{k:?}", path.join("."))
                }
            }
        }
        _ => {
            let path = path.join(".");
            if !used.contains(&path.as_str()) {
                eprintln!("[WARN] YAML value {path} is not used by the analyzer.");
            }
        }
    }
}
