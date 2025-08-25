use axi_wr_analyzer::AXIWrAnalyzer;
use default_analyzer::DefaultAnalyzer;
use python_analyzer::PythonAnalyzer;
use wellen::SignalValue;
use yaml_rust2::Yaml;

use crate::{
    bus::{BusCommon, BusDescription},
    load_signals, BusUsage, CycleType, SimulationData, SingleChannelBusUsage,
};

pub mod axi_wr_analyzer;
pub mod default_analyzer;
pub mod python_analyzer;

pub struct AnalyzerBuilder {}

impl AnalyzerBuilder {
    pub fn build(yaml: (&Yaml, &Yaml), default_max_burst_delay: u32) -> Box<dyn Analyzer> {
        if let Some(custom) = yaml.1["custom_analyzer"].as_str() {
            if custom == "AXIWrAnalyzer" {
                Box::new(AXIWrAnalyzer::new(yaml, default_max_burst_delay))
            } else {
                Box::new(PythonAnalyzer::new(custom))
            }
        } else {
            Box::new(DefaultAnalyzer::from_yaml(yaml, default_max_burst_delay))
        }
    }
}

pub trait Analyzer {
    fn analyze(&mut self, simulation_data: &mut SimulationData, verbose: bool);
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
        // println!("loading took {:?}", start.elapsed());
        print!("{}\t", start.elapsed().as_millis());
    }

    let start = std::time::Instant::now();
    let mut usage = SingleChannelBusUsage::new(&common.bus_name(), common.max_burst_delay());
    for i in clock.iter_changes() {
        if let SignalValue::Binary(v, 1) = i.1 {
            if v[0] == 0 {
                continue;
            }
        }
        // We subtract one to use values just before clock signal
        let time = i.0.saturating_sub(1);
        let reset = reset.get_value_at(&reset.get_offset(time).unwrap(), 0);
        let values: Vec<SignalValue> = loaded[2..]
            .iter()
            .map(|(_, s)| s.get_value_at(&s.get_offset(time).unwrap(), 0))
            .collect();

        // let reset = signals[0];
        if reset.to_bit_string().unwrap() != common.rst_active_value().to_string() {
            usage.add_cycle(bus_desc.interpret_cycle(values, time));
        } else {
            usage.add_cycle(CycleType::Reset);
        }
    }
    usage.end();
    if verbose {
        // println!("calculating took {:?}", start.elapsed());
        println!("{}\t", start.elapsed().as_millis());
    }

    usage
}
