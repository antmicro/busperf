use default_analyzer::DefaultAnalyzer;
use python_analyzer::PythonAnalyzer;
use wellen::{Signal, SignalRef, SignalValue, TimeTable};
use yaml_rust2::Yaml;

use crate::{
    BusUsage, CycleType, SimulationData, SingleChannelBusUsage,
    analyzer::axi_analyzer::{AXIRdAnalyzer, AXIWrAnalyzer},
    bus::{BusCommon, BusDescription, CyclesNum, SignalPath, is_value_of_type},
    load_signals,
};

pub mod axi_analyzer;
pub mod default_analyzer;
pub mod python_analyzer;

pub struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(
        yaml: (Yaml, Yaml),
        default_max_burst_delay: CyclesNum,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Box<dyn Analyzer>, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        Ok(if let Some(custom) = dict["custom_analyzer"].as_str() {
            match custom {
                "AXIWrAnalyzer" => Box::new(AXIWrAnalyzer::build_from_yaml(
                    (name, dict),
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                "AXIRdAnalyzer" => Box::new(AXIRdAnalyzer::build_from_yaml(
                    (name, dict),
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                _ => {
                    let common = BusCommon::from_yaml(
                        name.into_string().ok_or("Bus should have a valid name")?,
                        &dict,
                        default_max_burst_delay,
                    )?;
                    Box::new(PythonAnalyzer::new(
                        custom,
                        common,
                        &dict,
                        window_length,
                        x_rate,
                        y_rate,
                    )?)
                }
            }
        } else {
            Box::new(DefaultAnalyzer::from_yaml(
                (name, dict),
                default_max_burst_delay,
            )?)
        })
    }
}

pub trait AnalyzerInternal {
    fn bus_name(&self) -> &str;
    fn get_signals(&self) -> Vec<&SignalPath>;
    fn calculate(&mut self, loaded: Vec<(SignalRef, Signal)>, time_table: &TimeTable);
}

pub trait Analyzer: AnalyzerInternal {
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool) {
        let start = std::time::Instant::now();
        let signal_paths = self.get_signals();
        let loaded = load_signals(simulation_data, &signal_paths);
        if verbose {
            println!(
                "Loading signals for {} took {:?}",
                self.bus_name(),
                start.elapsed()
            );
        }

        let start = std::time::Instant::now();
        self.calculate(loaded, &simulation_data.body.time_table);
        if verbose {
            println!(
                "Calculating statistics for {} took {:?}",
                self.bus_name(),
                start.elapsed()
            );
        }
    }
    fn get_results(&self) -> Option<&BusUsage>;
    fn finished_analysis(&self) -> bool {
        self.get_results().is_some()
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
