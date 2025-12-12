use std::error::Error;

use constcat::concat_slices;
use wellen::{SignalValue, TimeTable};

use crate::analyze::{
    analyzer::private::AnalyzerInternal,
    bus::{BusCommon, BusDescription, BusDescriptionBuilder, SignalPath, is_value_of_type},
};
use libbusperf::bus_usage::{BusUsage, SingleChannelBusUsage};
use libbusperf::{CycleType, CyclesNum};

use super::Analyzer;

pub struct DefaultAnalyzer {
    common: BusCommon,
    bus_desc: Box<dyn BusDescription>,
    result: Option<BusUsage>,
}

impl DefaultAnalyzer {
    pub fn from_yaml(
        yaml: (yaml_rust2::Yaml, yaml_rust2::Yaml),
        default_max_burst_delay: CyclesNum,
        plugins_path: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        let name = name
            .into_string()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, &dict, default_max_burst_delay)?;
        let bus_desc = BusDescriptionBuilder::build(dict, common.module_scope(), plugins_path)?;
        Ok(DefaultAnalyzer {
            common,
            bus_desc,
            result: None,
        })
    }
}

impl AnalyzerInternal for DefaultAnalyzer {
    fn bus_name(&self) -> &str {
        self.common.bus_name()
    }

    fn get_signals(&self) -> Vec<&SignalPath> {
        let mut signals = vec![self.common.clk_path(), self.common.rst_path()];
        signals.append(&mut self.bus_desc.signals());

        signals
    }

    fn calculate(
        &mut self,
        loaded: Vec<&(wellen::SignalRef, wellen::Signal)>,
        time_table: &TimeTable,
    ) -> Result<(), Box<dyn Error>> {
        let (_, clock) = loaded[0];
        let (_, reset) = loaded[1];
        let mut usage = SingleChannelBusUsage::new(
            self.common.bus_name(),
            self.common.max_burst_delay(),
            *time_table.get(2).ok_or(
                "trace is too short (less than 3 time indices), cannot calculate clock period",
            )?,
        );
        for (time, value) in clock.iter_changes() {
            if let SignalValue::Binary(v, 1) = value
                && v[0] == 0
            {
                continue;
            }
            let intervals = self.common.intervals();
            if !intervals.is_empty()
                && intervals.iter().all(|&[start, end]| {
                    time_table[time as usize] < start || time_table[time as usize] > end
                })
            {
                continue;
            }
            // We subtract one to use values just before clock signal
            let time = time.saturating_sub(1);
            let reset = reset.get_value_at(
                &reset.get_offset(time).ok_or(format!(
                    "reset value is invalid at {}",
                    time_table[time as usize]
                ))?,
                0,
            );
            let values: Vec<SignalValue> = loaded[2..]
                .iter()
                .map(|(_, s)| {
                    Ok::<_, Box<dyn Error>>(s.get_value_at(
                        &s.get_offset(time).ok_or(format!(
                            "signal does not have value at {}",
                            time_table[time as usize]
                        ))?,
                        0,
                    ))
                })
                .collect::<Result<_, _>>()?;

            if !is_value_of_type(reset, self.common.rst_active_value()) {
                let type_ = self.bus_desc.interpret_cycle(&values, time);
                if let CycleType::Unknown = type_ {
                    let mut state = String::new();
                    self.bus_desc
                        .signals()
                        .iter()
                        .zip(values)
                        .for_each(|(name, value)| state.push_str(&format!("{name}: {value}, ")));
                    eprintln!(
                        "[WARN] bus \"{}\" in unknown state outside reset at time: {} - {}",
                        self.common.bus_name(),
                        time_table[time as usize],
                        state
                    );
                }

                usage.add_cycle(type_);
            } else {
                usage.add_cycle(CycleType::Reset);
            }
        }

        self.result = Some(BusUsage::SingleChannel(usage));
        Ok(())
    }
}

const DEFAULT_YAML: &[&str] = concat_slices!([&str]: &super::COMMON_YAML, &["ready", "valid", "credit", "valid", "htrans", "hready", "psel", "penable", "pready"]);

impl Analyzer for DefaultAnalyzer {
    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }

    fn required_yaml_definitions(&self) -> Vec<&str> {
        Vec::from(DEFAULT_YAML)
    }
}
