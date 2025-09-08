use crate::{
    BusUsage,
    bus::{BusCommon, BusDescription, axi::AXIBus},
    bus_usage::MultiChannelBusUsage,
    load_signals,
};

use super::{Analyzer, analyze_single_bus};

pub struct AXIWrAnalyzer {
    common: BusCommon,
    aw: AXIBus,
    w: AXIBus,
    b: AXIBus,
    b_resp: String,
    result: Option<BusUsage>,
}

impl AXIWrAnalyzer {
    pub fn new(yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml), default_max_burst_delay: u32) -> Self {
        let name = yaml.0.as_str().unwrap();
        let common = BusCommon::from_yaml(name, yaml.1, default_max_burst_delay).unwrap();
        let aw = AXIBus::from_yaml(&yaml.1["aw"]).unwrap();
        let w = AXIBus::from_yaml(&yaml.1["w"]).unwrap();
        let b = AXIBus::from_yaml(&yaml.1["b"]).unwrap();
        let b_resp = yaml.1["b"]["bresp"].as_str().unwrap().to_owned();
        AXIWrAnalyzer {
            common,
            aw,
            w,
            b,
            b_resp,
            result: None,
        }
    }
}

impl Analyzer for AXIWrAnalyzer {
    fn analyze(&mut self, simulation_data: &mut crate::SimulationData, verbose: bool) {
        let mut signals = vec![self.common.clk_name(), self.common.rst_name()];
        signals.append(&mut self.aw.signals());
        signals.append(&mut self.w.signals());
        signals.append(&mut self.b.signals());
        signals.push(&self.b_resp);

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
        let (_, clk) = &loaded[0];
        let (_, _rst) = &loaded[1];
        let (_, _awready) = &loaded[2];
        let (_, awvalid) = &loaded[3];
        let (_, _wready) = &loaded[4];
        let (_, wvalid) = &loaded[5];
        let (_, _bready) = &loaded[6];
        let (_, bvalid) = &loaded[7];
        let (_, b_resp) = &loaded[8];

        let mut next = awvalid.iter_changes().map(|(t, _)| t);
        next.next();
        next.next();
        let last_time = clk.time_indices().last().unwrap();
        let next = next.chain([*last_time, *last_time]);
        let mut usage =
            MultiChannelBusUsage::new(self.common.bus_name(), 10000, 0.0006, 0.00001, *last_time);

        for ((time, value), next) in awvalid.iter_changes().zip(next) {
            if value.to_bit_string().unwrap() != "1" {
                continue;
            }
            let (first_data, _) = wvalid
                .iter_changes()
                .find(|(t, v)| *t >= time && v.to_bit_string().unwrap() == "1")
                .unwrap_or_else(|| panic!("time at error{}", time));
            let (resp_time, _) = bvalid
                .iter_changes()
                .find(|(t, v)| *t > time && v.to_bit_string().unwrap() == "1")
                .unwrap();
            let (last_write, _) = wvalid
                .iter_changes()
                .max_by_key(|(t, v)| {
                    if *t < time || *t > resp_time || v.to_bit_string().unwrap() == "1" {
                        0
                    } else {
                        *t
                    }
                })
                .unwrap();
            let resp = b_resp
                .get_value_at(&b_resp.get_offset(resp_time).unwrap(), 0)
                .to_bit_string()
                .unwrap();
            let delay = next - resp_time;
            usage.add_transaction(time, resp_time, last_write, first_data, &resp, delay);
        }

        usage.channels_usages = [&self.aw, &self.w, &self.b]
            .iter()
            .map(|bus| analyze_single_bus(&self.common, *bus, simulation_data, verbose))
            .collect();
        usage.end(usage.channels_usages[0].reset());

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
