use crate::{
    bus::{BusCommon, BusDescription, BusDescriptionBuilder},
    BusUsage,
};

use super::{analyze_single_bus, Analyzer};

pub struct DefaultAnalyzer {
    common: BusCommon,
    bus_desc: Box<dyn BusDescription>,
    result: Option<BusUsage>,
}

impl DefaultAnalyzer {
    pub fn from_yaml(
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Self {
        let name = yaml.0.as_str().expect("Invalid bus name");
        let common = BusCommon::from_yaml(name, yaml.1, default_max_burst_delay).unwrap();
        let bus_desc = BusDescriptionBuilder::build(name, yaml.1, default_max_burst_delay)
            .expect("Failed to load bus");
        DefaultAnalyzer {
            common,
            bus_desc,
            result: None,
        }
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
