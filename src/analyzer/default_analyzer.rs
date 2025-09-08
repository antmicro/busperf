use crate::{
    BusUsage,
    bus::{BusCommon, BusDescription, BusDescriptionBuilder},
};

use super::{Analyzer, analyze_single_bus};

pub struct DefaultAnalyzer {
    common: BusCommon,
    bus_desc: Box<dyn BusDescription>,
    result: Option<BusUsage>,
}

impl DefaultAnalyzer {
    pub fn from_yaml(
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        let name = name
            .as_str()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, dict, default_max_burst_delay)?;
        let bus_desc = BusDescriptionBuilder::build(name, dict, default_max_burst_delay)?;
        Ok(DefaultAnalyzer {
            common,
            bus_desc,
            result: None,
        })
    }
}

impl Analyzer for DefaultAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let usage = analyze_single_bus(&self.common, &*self.bus_desc, simulation_data, verbose);
        self.result = Some(BusUsage::SingleChannel(usage));
    }

    fn get_results(&self) -> &crate::BusUsage {
        self.result.as_ref().unwrap()
    }
}
