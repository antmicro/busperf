use yaml_rust2::Yaml;

use crate::bus::BusDescription;

pub mod default_analyzer;
pub mod python_analyzer;

pub trait Analyzer {
    fn load_buses(
        &self,
        yaml: (&Yaml, &Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Vec<Box<dyn BusDescription>>, Box<dyn std::error::Error>>;
}
