use wellen::SignalValue;

use crate::{
    bus::{BusDescription, BusDescriptionBuilder},
    load_signals, BusUsage, CycleType,
};

use super::Analyzer;

pub struct DefaultAnalyzer {
    bus_desc: Box<dyn BusDescription>,
    result: Option<BusUsage>,
}

impl DefaultAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
        let bus_desc = BusDescriptionBuilder::build(yaml, default_max_burst_delay)
            .expect("Failed to load bus");
        DefaultAnalyzer {
            bus_desc,
            result: None,
        }
    }

    fn analyze_internal(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let bus_desc = &self.bus_desc;
        let mut signals = vec![bus_desc.common().clk_name(), bus_desc.common().rst_name()];
        signals.append(&mut bus_desc.signals());

        let start = std::time::Instant::now();
        let loaded = load_signals(simulation_data, &bus_desc.common().module_scope(), &signals);
        let (_, clock) = &loaded[0];
        let (_, reset) = &loaded[1];
        if verbose {
            // println!("loading took {:?}", start.elapsed());
            print!("{}\t", start.elapsed().as_millis());
        }

        let start = std::time::Instant::now();
        let mut usage = BusUsage::new(&bus_desc.bus_name(), bus_desc.common().max_burst_delay());
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
            if reset.to_bit_string().unwrap() != bus_desc.common().rst_active_value().to_string() {
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
        self.result = Some(usage);
    }
}

impl Analyzer for DefaultAnalyzer {
    fn load_buses(
        &self,
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Vec<Box<dyn crate::bus::BusDescription>>, Box<dyn std::error::Error>> {
        todo!();
    }

    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        self.analyze_internal(simulation_data, verbose);
    }

    fn get_results(&self) -> &crate::BusUsage {
        self.result.as_ref().unwrap()
    }
}
