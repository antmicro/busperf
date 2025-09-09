use std::error::Error;

use wellen::Signal;

use crate::{
    BusUsage,
    analyzer::analyze_single_bus,
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
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

pub struct AXIWrAnalyzer {
    common: BusCommon,
    aw: AXIBus,
    w: AXIBus,
    b: AXIBus,
    b_resp: String,
    result: Option<BusUsage>,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
}

fn count_reset(rst: &Signal, active_value: u8) -> u32 {
    let mut last = 0;
    let mut reset = 0;
    for (time, value) in rst.iter_changes() {
        if value.to_bit_string().unwrap() == active_value.to_string() {
            last = time;
        } else {
            reset += time - last;
        }
    }
    reset / 2
}

macro_rules! build_from_yaml {
    ( $( $bus_name:tt $bus_type:ident ),* ; $($signal_name:tt $($signal_init:tt)*),* ) => {
        pub fn build_from_yaml(
            yaml: (&yaml_rust2::Yaml, &yaml_rust2::Yaml),
            default_max_burst_delay: u32,
            window_length: u32,
            x_rate: f32,
            y_rate: f32,
        ) -> Result<Self, Box<dyn Error>> {
            let (name, dict) = yaml;
            let name = name
                .as_str()
                .ok_or("Name of bus should be a valid string")?;
            let common = BusCommon::from_yaml(name, dict, default_max_burst_delay)?;
            $(
                let $bus_name = $bus_type::from_yaml(&dict["$x"])?;
            )*
            $(
                let $signal_name = dict$($signal_init)*;
            )*
            Ok(Self {
                common,
                $($bus_name,)*
                $($signal_name,)*
                result: None,
                window_length,
                x_rate,
                y_rate,
            })
        }
    };
}

impl AXIRdAnalyzer {
    build_from_yaml!(ar AXIBus, r AXIBus; r_resp ["r"]["rresp"].as_str().ok_or("AXI bus should have rresp signal")?.to_owned());
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

        let reset = count_reset(rst, self.common.rst_active_value());
        let mut next_time_iter = arvalid.iter_changes().map(|(t, _)| t);
        next_time_iter.next();
        next_time_iter.next();
        let last_time = clk.time_indices().last().unwrap();
        let next_time_iter = next_time_iter.chain([*last_time, *last_time]);

        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            self.x_rate,
            self.y_rate,
            *last_time,
        );

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

impl AXIWrAnalyzer {
    build_from_yaml!(aw AXIBus, w AXIBus, b AXIBus; b_resp ["b"]["bresp"].as_str().ok_or("AXI bus should have a bresp signal")?.to_owned());
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
        let mut usage = MultiChannelBusUsage::new(
            self.common.bus_name(),
            self.window_length,
            self.x_rate,
            self.y_rate,
            *last_time,
        );

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
