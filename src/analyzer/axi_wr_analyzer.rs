use crate::{
    analyzer::default_analyzer::DefaultAnalyzer,
    bus::{axi::AXIBus, BusDescription, BusDescriptionBuilder},
};

use super::Analyzer;

pub struct AXIWrAnalyzer {
    aw: AXIBus,
    w: AXIBus,
    b: AXIBus,
}

impl AXIWrAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
        todo!();
    }
}

impl Analyzer for AXIWrAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        todo!()
    }

    fn get_results(&self) -> &crate::BusUsage {
        todo!()
    }
}
