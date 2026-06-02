use std::error::Error;

use libbusperf::bus_usage::BusUsage;
use yaml_rust2::Yaml;

use crate::analyze::trigger::TriggerSource;

pub struct TransactionTrigger {
    name: String,
    moment: TransactionMoment,
}

enum TransactionMoment {
    Start,
    End,
}

impl TransactionTrigger {
    pub fn from_yaml(name: String, type_: &Yaml) -> Result<Box<dyn TriggerSource>, Box<dyn Error>> {
        let moment = match type_.as_str() {
            Some("_start") => TransactionMoment::Start,
            Some("_end") => TransactionMoment::End,
            Some(other) => Err(format!("invalid multichannel bus trigger {other}"))?,
            None => Err(format!("invalid multichannel bus trigger {type_:?}"))?,
        };
        Ok(Box::new(Self { name, moment }))
    }
}

impl TriggerSource for TransactionTrigger {
    fn name(&self) -> &str {
        &self.name
    }

    fn into_name(self: Box<Self>) -> String {
        self.name
    }

    fn analyze(
        self: Box<Self>,
        _simulation_data: &mut crate::analyze::SimulationData,
        _loaded: &[&wellen::Signal],
        _intervals: &[[libbusperf::bus_usage::RealTime; 2]],
        _done_triggers: &crate::analyze::DoneTriggers,
        bus_usage: &Result<BusUsage, Box<dyn std::error::Error>>,
    ) -> (
        String,
        Result<Vec<libbusperf::bus_usage::RealTime>, Box<dyn std::error::Error>>,
    ) {
        (
            self.name,
            match (bus_usage, self.moment) {
                (Ok(BusUsage::SingleChannel(bus_usage)), TransactionMoment::Start) => Ok(bus_usage
                    .get_bursts_start_end()
                    .iter()
                    .map(|p| p.start())
                    .collect()),
                (Ok(BusUsage::SingleChannel(bus_usage)), TransactionMoment::End) => Ok(bus_usage
                    .get_bursts_start_end()
                    .iter()
                    .map(|p| p.end())
                    .collect()),
                (Ok(BusUsage::MultiChannel(bus_usage)), TransactionMoment::Start) => Ok(bus_usage
                    .get_transactions_start_end()
                    .iter()
                    .map(|p| p.start())
                    .collect()),
                (Ok(BusUsage::MultiChannel(bus_usage)), TransactionMoment::End) => Ok(bus_usage
                    .get_transactions_start_end()
                    .iter()
                    .map(|p| p.start())
                    .collect()),
                (Err(e), _) => Err(format!("required analyzer failed {e}").into()),
            },
        )
    }
}
