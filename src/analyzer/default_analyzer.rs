use wellen::SignalValue;

use crate::{
    bus::{BusCommon, BusDescription, BusDescriptionBuilder},
    bus_usage::SingleChannelBusUsage,
    load_signals, BusUsage, CycleType,
};

use super::Analyzer;

pub struct DefaultAnalyzer {
    common: BusCommon,
    bus_desc: Box<dyn BusDescription>,
    result: Option<BusUsage>,
}

impl DefaultAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
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

    fn analyze_internal(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let bus_desc = &self.bus_desc;
        let mut signals = vec![self.common.clk_name(), self.common.rst_name()];
        signals.append(&mut bus_desc.signals());

        let start = std::time::Instant::now();
        let loaded = load_signals(simulation_data, &self.common.module_scope(), &signals);
        let (_, clock) = &loaded[0];
        let (_, reset) = &loaded[1];
        if verbose {
            // println!("loading took {:?}", start.elapsed());
            print!("{}\t", start.elapsed().as_millis());
        }

        let start = std::time::Instant::now();
        let mut usage =
            SingleChannelBusUsage::new(&self.common.bus_name(), self.common.max_burst_delay());
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
            if reset.to_bit_string().unwrap() != self.common.rst_active_value().to_string() {
                usage.add_cycle(bus_desc.interpret_cycle(values, time));
            } else {
                usage.add_cycle(CycleType::NoTransaction);
            }
        }
        usage.end();
        if verbose {
            // println!("calculating took {:?}", start.elapsed());
            println!("{}\t", start.elapsed().as_millis());
        }
        self.result = Some(BusUsage::SingleChannel(usage));
    }
}

impl Analyzer for DefaultAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        self.analyze_internal(simulation_data, verbose);
    }

    fn get_results(&self) -> &crate::BusUsage {
        self.result.as_ref().unwrap()
    }
}
