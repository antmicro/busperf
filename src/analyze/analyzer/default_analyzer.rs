use constcat::concat_slices;
use wellen::{SignalValue, TimeTable};

use crate::CycleType;
use crate::bus_usage::{BusUsage, SingleChannelBusUsage};
use crate::{
    CyclesNum,
    analyze::{
        SimulationData,
        analyzer::private::AnalyzerInternal,
        bus::{BusCommon, BusDescription, BusDescriptionBuilder, SignalPath, is_value_of_type},
        load_signals,
    },
};

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
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        let name = name
            .into_string()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, &dict, default_max_burst_delay)?;
        let bus_desc = BusDescriptionBuilder::build(dict, common.module_scope())?;
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
        loaded: Vec<(wellen::SignalRef, wellen::Signal)>,
        time_table: &TimeTable,
    ) {
        let (_, clock) = &loaded[0];
        let (_, reset) = &loaded[1];
        let clock_period = time_table[2];
        let mut usage = SingleChannelBusUsage::new(
            self.common.bus_name(),
            self.common.max_burst_delay(),
            clock_period,
        );
        for (time, value) in clock.iter_changes() {
            if let SignalValue::Binary(v, 1) = value
                && v[0] == 0
            {
                continue;
            }
            // We subtract one to use values just before clock signal
            let time = time.saturating_sub(1);
            let reset =
                reset.get_value_at(&reset.get_offset(time).expect("Value should be valid"), 0);
            let values: Vec<SignalValue> = loaded[2..]
                .iter()
                .map(|(_, s)| {
                    s.get_value_at(&s.get_offset(time).expect("Value should be valid"), 0)
                })
                .collect();

            if !is_value_of_type(reset, self.common.rst_active_value()) {
                let type_ = self.bus_desc.interpret_cycle(&values, time);
                if let CycleType::Unknown = type_ {
                    let mut state = String::from("");
                    self.bus_desc
                        .signals()
                        .iter()
                        .zip(values)
                        .for_each(|(name, value)| {
                            state.push_str(&format!("{}: {}, ", name, value))
                        });
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
    }
}

const DEFAULT_YAML: &[&str] =
    concat_slices!([&str]: &super::COMMON_YAML, &["ready", "valid", "handshake"]);

impl Analyzer for DefaultAnalyzer {
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool) {
        let usage = analyze_single_bus(&self.common, &*self.bus_desc, simulation_data, verbose);
        self.result = Some(BusUsage::SingleChannel(usage));
    }

    fn get_results(&self) -> Option<&BusUsage> {
        self.result.as_ref()
    }

    fn required_yaml_definitions(&self) -> Vec<&str> {
        Vec::from(DEFAULT_YAML)
    }
}

pub fn analyze_single_bus(
    common: &BusCommon,
    bus_desc: &dyn BusDescription,
    simulation_data: &mut SimulationData,
    verbose: bool,
) -> SingleChannelBusUsage {
    let mut signals = vec![common.clk_path(), common.rst_path()];
    signals.append(&mut bus_desc.signals());

    let start = std::time::Instant::now();
    let loaded = load_signals(simulation_data, &signals);
    let (_, clock) = &loaded[0];
    let (_, reset) = &loaded[1];
    if verbose {
        println!(
            "Loading signals for {} took {:?}",
            common.bus_name(),
            start.elapsed()
        );
    }

    let start = std::time::Instant::now();
    let mut usage = SingleChannelBusUsage::new(
        common.bus_name(),
        common.max_burst_delay(),
        simulation_data.body.time_table[2],
    );
    for (time, value) in clock.iter_changes() {
        if let SignalValue::Binary(v, 1) = value
            && v[0] == 0
        {
            continue;
        }
        let intervals = common.intervals();
        let time_table = &simulation_data.body.time_table;
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
            &reset
                .get_offset(time)
                .expect("Value should be valid at that time"),
            0,
        );
        let values: Vec<SignalValue> = loaded[2..]
            .iter()
            .map(|(_, s)| {
                s.get_value_at(
                    &s.get_offset(time)
                        .expect("Value should be valid at that time"),
                    0,
                )
            })
            .collect();

        if !is_value_of_type(reset, common.rst_active_value()) {
            let type_ = bus_desc.interpret_cycle(&values, time);
            if let CycleType::Unknown = type_ {
                let mut state = String::new();
                bus_desc
                    .signals()
                    .iter()
                    .zip(values)
                    .for_each(|(name, value)| state.push_str(&format!("{name}: {value}, ")));
                eprintln!(
                    "[WARN] bus \"{}\" in unknown state outside reset at time: {} - {}",
                    common.bus_name(),
                    simulation_data.body.time_table[time as usize],
                    state
                );
            }

            usage.add_cycle(type_);
        } else {
            usage.add_cycle(CycleType::Reset);
        }
    }
    if verbose {
        println!(
            "Calculating statistics for {} took {:?}",
            common.bus_name(),
            start.elapsed()
        );
    }

    usage
}
