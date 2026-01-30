use std::error::Error;

use libbusperf::bus_usage::RealTime;
use yaml_rust2::Yaml;

use crate::analyze::trigger::Control;

pub struct TimeControl {
    name: String,
    triggers: Vec<TimeTrigger>,
}

pub struct TimeTrigger {
    name: String,
    times: Vec<RealTime>,
}

impl TimeTrigger {
    pub fn new(name: String, times: Vec<RealTime>) -> Self {
        Self { times, name }
    }
}

impl TimeControl {
    pub fn build_from_yaml(name: String, yaml: &Yaml) -> Result<Box<dyn Control>, Box<dyn Error>> {
        let triggers = yaml["triggers"].as_hash().ok_or("triggers not specified")?;
        let triggers = triggers
            .iter()
            .map(|(n, times)| {
                let name = format!("control_only.{name}.{}", n.as_str().ok_or("invalid name")?);
                let times = times
                    .as_vec()
                    .ok_or("times should be in array")?
                    .iter()
                    .map(|t| {
                        t.as_i64()
                            .map(|t| t as u64)
                            .ok_or("time should be a number")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(TimeTrigger::new(name, times))
            })
            .collect::<Result<_, Box<dyn Error>>>()?;
        Ok(Box::new(Self { triggers, name }))
    }
}

impl Control for TimeControl {
    fn requires(&self) -> Vec<&str> {
        vec![]
    }

    fn names(&self) -> Vec<&str> {
        self.triggers.iter().map(|t| t.name.as_str()).collect()
    }

    fn analyze(
        self: Box<Self>,
        _done_trigger: &crate::analyze::DoneTriggers,
    ) -> Vec<(String, Result<Vec<RealTime>, Box<dyn std::error::Error>>)> {
        self.triggers.into_iter().map(|t| t.analyze()).collect()
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl TimeTrigger {
    fn analyze(self) -> (String, Result<Vec<RealTime>, Box<dyn std::error::Error>>) {
        (self.name, Ok(self.times))
    }
}
