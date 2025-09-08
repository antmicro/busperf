use std::error::Error;

use crate::{
    BusUsage,
    bus::{BusCommon, BusDescription, axi::AXIBus},
    bus_usage::MultiChannelBusUsage,
    load_signals,
};

use super::Analyzer;

pub struct AXIRdAnalyzer {
    common: BusCommon,
    ar: AXIBus,
    r: AXIBus,
    r_resp: String,
    result: Option<BusUsage>,
}

impl AXIRdAnalyzer {
    pub fn build_from_yaml(
        yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
        default_max_burst_delay: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let (name, dict) = yaml;
        let name = name
            .as_str()
            .ok_or("Name of bus should be a valid string")?;
        let common = BusCommon::from_yaml(name, dict, default_max_burst_delay)?;
        let ar = AXIBus::from_yaml(&dict["ar"])?;
        let r = AXIBus::from_yaml(&dict["r"])?;
        let r_resp = dict["r"]["rresp"]
            .as_str()
            .ok_or("AXI bus should have rresp signal")?
            .to_owned();
        Ok(AXIRdAnalyzer {
            common,
            ar,
            r,
            r_resp,
            result: None,
        })
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
        let (_, clk) = &loaded[0];
        let (_, rst) = &loaded[1];
        let (_, _arready) = &loaded[2];
        let (_, arvalid) = &loaded[3];
        let (_, _rready) = &loaded[4];
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
        reset /= 2;
        let mut next_time_iter = arvalid.iter_changes().map(|(t, _)| t);
        next_time_iter.next();
        next_time_iter.next();
        let last_time = clk.time_indices().last().unwrap();
        let next_time_iter = next_time_iter.chain([*last_time, *last_time]);

        let mut usage =
            MultiChannelBusUsage::new(self.common.bus_name(), 10000, 0.0006, 0.00001, *last_time);

        let mut rst = rst.iter_changes().filter_map(|(t, v)| {
            if v.to_bit_string().unwrap() == self.common.rst_active_value().to_string() {
                Some(t)
            } else {
                None
            }
        });
        let mut next_reset = rst.next().unwrap_or(*last_time);
        for ((time, value), next) in arvalid.iter_changes().zip(next_time_iter) {
            if value.to_bit_string().unwrap() != "1" {
                continue;
            }
            while next_reset < time {
                next_reset = rst.next().unwrap_or(*last_time);
            }
            let (first_data, _) = rvalid
                .iter_changes()
                .find(|(t, v)| *t >= time && v.to_bit_string().unwrap() == "1")
                .unwrap_or_else(|| panic!("time at error{}", time));
            let resp_time = first_data;
            if next_reset < resp_time {
                continue;
            }
            let last_write = first_data;
            let resp = r_resp
                .get_value_at(&r_resp.get_offset(resp_time).unwrap(), 0)
                .to_bit_string()
                .unwrap();
            let delay = next - resp_time;
            usage.add_transaction(time, resp_time, last_write, first_data, &resp, delay);
        }

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
