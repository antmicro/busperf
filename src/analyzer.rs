use axi_wr_analyzer::AXIWrAnalyzer;
use default_analyzer::DefaultAnalyzer;
use python_analyzer::PythonAnalyzer;
use yaml_rust2::Yaml;

use crate::{BusUsage, SimulationData};

pub mod axi_wr_analyzer;
pub mod default_analyzer;
pub mod python_analyzer;

pub struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(yaml: (&Yaml, &Yaml), default_max_burst_delay: u32) -> Box<dyn Analyzer> {
        if let Some(custom) = yaml.1["custom_analyzer"].as_str() {
            if custom == "AXIWrAnalyzer" {
                Box::new(AXIWrAnalyzer::new(yaml, default_max_burst_delay))
            } else {
                Box::new(PythonAnalyzer::new(custom))
            }
        } else {
            Box::new(DefaultAnalyzer::new(yaml, default_max_burst_delay))
        }
    }
}

pub trait Analyzer {
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool);
    fn get_results(&self) -> &BusUsage;
}
