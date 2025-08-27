use pyo3::buffer::Element;

use crate::{
    analyzer::default_analyzer::DefaultAnalyzer,
    bus::{axi::AXIBus, BusCommon, BusDescription, BusDescriptionBuilder},
    bus_usage::MultiChannelBusUsage,
    load_signals, BusUsage,
};

use super::{analyze_single_bus, Analyzer};

pub struct AXIRdAnalyzer {
    common: BusCommon,
    ar: AXIBus,
    r: AXIBus,
    r_resp: String,
    result: Option<BusUsage>,
}

impl AXIRdAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
        let name = yaml.0.as_str().unwrap();
        let common = BusCommon::from_yaml(name, yaml.1, default_max_burst_delay).unwrap();
        let ar = AXIBus::from_yaml(&yaml.1["ar"]).unwrap();
        let r = AXIBus::from_yaml(&yaml.1["r"]).unwrap();
        let r_resp = yaml.1["r"]["r_resp"].as_str().unwrap().to_owned();
        AXIRdAnalyzer {
            common,
            ar,
            r,
            r_resp,
            result: None,
        }
    }
}

impl Analyzer for AXIRdAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let mut signals = vec![self.common.clk_name(), self.common.rst_name()];
        signals.append(&mut self.ar.signals());
        signals.append(&mut self.r.signals());
        signals.push(&self.r_resp);

        let start = std::time::Instant::now();
        let loaded = load_signals(simulation_data, self.common.module_scope(), &signals);
        if verbose {
            println!(
                "Loading {} took {:?}",
                self.common.bus_name(),
                start.elapsed()
            );
        }

        let start = std::time::Instant::now();
        let mut usage = MultiChannelBusUsage::new(self.common.bus_name(), 10000, 0.0006, 0.00001);
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, arready) = &loaded[2];
        let (_, arvalid) = &loaded[3];
        let (_, rready) = &loaded[4];
        let (_, rvalid) = &loaded[5];
        let (_, r_resp) = &loaded[6];

        let mut last = 0;
        let mut reset = 0;
        for (time, value) in rst.iter_changes() {
            if value.to_bit_string().unwrap() == self.common.rst_active_value().to_string() {
                last = time;
            } else {
                reset += time - last;
            }
        }
        reset = reset / 2;

        let mut next = arvalid.iter_changes().map(|(t, _)| t);
        next.next();
        next.next();
        let last_time = clk.time_indices().last().unwrap();
        let next = next.chain([*last_time, *last_time]);

        for ((time, value), next) in arvalid.iter_changes().zip(next) {
            if value.to_bit_string().unwrap() != "1" {
                continue;
            }
            let (first_data, _) = rvalid
                .iter_changes()
                .find(|(t, v)| *t >= time && v.to_bit_string().unwrap() == "1")
                .expect(&format!("time at error{}", time));
            let resp_time = first_data;
            let last_write = first_data;
            let resp = r_resp
                .get_value_at(&r_resp.get_offset(resp_time).unwrap(), 0)
                .to_bit_string()
                .unwrap();
            let delay = next - resp_time;
            usage.add_transaction(time, resp_time, last_write, first_data, &resp, delay);
        }

        // usage.channels_usages = [&self.ar, &self.r]
        //     .iter()
        //     .map(|bus| analyze_single_bus(&self.common, *bus, simulation_data, verbose))
        //     .collect();
        // usage.end(usage.channels_usages[0].reset());
        usage.end(reset);

        if verbose {
            println!(
                "Calculating {} took {:?}",
                self.common.bus_name(),
                start.elapsed()
            );
        }

        self.result = Some(BusUsage::MultiChannel(usage));
    }

    fn get_results(&self) -> &crate::BusUsage {
        self.result.as_ref().unwrap()
    }
}
