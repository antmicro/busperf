use default_analyzer::DefaultAnalyzer;
use python_analyzer::PythonAnalyzer;
use wellen::{Signal, SignalRef, SignalValue};
use yaml_rust2::Yaml;

use crate::{
    BusUsage, CycleType, SimulationData, SingleChannelBusUsage,
    analyzer::axi_analyzer::{AXIRdAnalyzer, AXIWrAnalyzer},
    bus::{BusCommon, BusDescription},
    load_signals,
};

pub mod axi_analyzer;
pub mod default_analyzer;
pub mod python_analyzer;

pub struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(
        yaml: (&Yaml, &Yaml),
        default_max_burst_delay: u32,
        window_length: u32,
        x_rate: f32,
        y_rate: f32,
    ) -> Result<Box<dyn Analyzer>, Box<dyn std::error::Error>> {
        let (name, dict) = yaml;
        Ok(if let Some(custom) = dict["custom_analyzer"].as_str() {
            match custom {
                "AXIWrAnalyzer" => Box::new(AXIWrAnalyzer::build_from_yaml(
                    yaml,
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                "AXIRdAnalyzer" => Box::new(AXIRdAnalyzer::build_from_yaml(
                    yaml,
                    default_max_burst_delay,
                    window_length,
                    x_rate,
                    y_rate,
                )?),
                _ => {
                    let common = BusCommon::from_yaml(
                        name.as_str().ok_or("Bus should have a valid name")?,
                        dict,
                        default_max_burst_delay,
                    )?;
                    Box::new(PythonAnalyzer::new(custom, common, dict)?)
                }
            }
        } else {
            Box::new(DefaultAnalyzer::from_yaml(yaml, default_max_burst_delay)?)
        })
    }
}

pub trait AnalyzerInternal {
    fn bus_name(&self) -> &str;
    fn load_signals(&self, simulation_data: &mut SimulationData) -> Vec<(SignalRef, Signal)>;
    fn calculate(&mut self, loaded: Vec<(SignalRef, Signal)>);
}

pub trait Analyzer: AnalyzerInternal {
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool) {
        let start = std::time::Instant::now();
        let loaded = self.load_signals(simulation_data);
        if verbose {
            println!("Loading {} took {:?}", self.bus_name(), start.elapsed());
        }

        let start = std::time::Instant::now();
        self.calculate(loaded);
        if verbose {
            println!("Calculating {} took {:?}", self.bus_name(), start.elapsed());
        }
    }
    fn get_results(&self) -> &BusUsage;
}

pub fn analyze_single_bus(
    common: &BusCommon,
    bus_desc: &dyn BusDescription,
    simulation_data: &mut SimulationData,
    verbose: bool,
) -> SingleChannelBusUsage {
    let mut signals = vec![common.clk_name(), common.rst_name()];
    signals.append(&mut bus_desc.signals());

    let start = std::time::Instant::now();
    let loaded = load_signals(simulation_data, common.module_scope(), &signals);
    let (_, clock) = &loaded[0];
    let (_, reset) = &loaded[1];
    if verbose {
        println!("Loading took {:?}", start.elapsed());
    }

    let start = std::time::Instant::now();
    let mut usage = SingleChannelBusUsage::new(common.bus_name(), common.max_burst_delay());
    for (time, value) in clock.iter_changes() {
        if let SignalValue::Binary(v, 1) = value
            && v[0] == 0
        {
            continue;
        }
        // We subtract one to use values just before clock signal
        let time = time.saturating_sub(1);
        let reset = reset.get_value_at(&reset.get_offset(time).unwrap(), 0);
        let values: Vec<SignalValue> = loaded[2..]
            .iter()
            .map(|(_, s)| s.get_value_at(&s.get_offset(time).unwrap(), 0))
            .collect();

        if reset.to_bit_string().unwrap() != common.rst_active_value().to_string() {
            let type_ = bus_desc.interpret_cycle(&values, time);
            if let CycleType::Unknown = type_ {
                let mut state = String::from("");
                bus_desc
                    .signals()
                    .iter()
                    .zip(values)
                    .for_each(|(name, value)| state.push_str(&format!("{}: {}, ", name, value)));
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
    usage.end();
    if verbose {
        println!("calculating took {:?}", start.elapsed());
    }

    usage
}
