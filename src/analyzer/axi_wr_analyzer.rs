use crate::{
    analyzer::default_analyzer::DefaultAnalyzer,
    bus::{axi::AXIBus, BusCommon, BusDescription, BusDescriptionBuilder},
};

use super::Analyzer;

pub struct AXIWrAnalyzer {
    aw: AXIBus,
    w: AXIBus,
    b: AXIBus,
}

impl AXIWrAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
        let common_aw = BusCommon::from_yaml("aw", yaml.1, default_max_burst_delay).unwrap();
        let aw = AXIBus::from_yaml(common_aw, &yaml.1["aw"]).unwrap();
        let common_w = BusCommon::from_yaml("w", yaml.1, default_max_burst_delay).unwrap();
        let w = AXIBus::from_yaml(common_w, &yaml.1["w"]).unwrap();
        let common_b = BusCommon::from_yaml("b", yaml.1, default_max_burst_delay).unwrap();
        let b = AXIBus::from_yaml(common_b, &yaml.1["b"]).unwrap();
        AXIWrAnalyzer { aw, w, b }
    }
}

impl Analyzer for AXIWrAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let mut signals = vec![self];
    }

    fn get_results(&self) -> &crate::BusUsage {
        todo!()
    }
}
