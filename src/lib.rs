use std::{
    fs::File,
    io::{Read, Write},
    process::Termination,
    sync::{atomic::AtomicU64, Arc},
};

use wellen::{
    viewers::{self, BodyResult},
    Hierarchy, LoadOptions, SignalValue,
};
use yaml_rust2::YamlLoader;

mod bus;

use bus::DelaysNum;
use bus::{BusDescription, CyclesNum};

// #[derive(Debug)]
// pub enum BusDescription {
//     AXI(AXIBus),
//     CreditValid(CreditValidBus),
//     AHB(AHBBus),
// }

pub fn load_bus_descriptions(
    filename: &str,
    default_max_burst_delay: CyclesNum,
) -> Result<Vec<Box<dyn BusDescription>>, Box<dyn std::error::Error>> {
    let mut f = File::open(filename)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let yaml = YamlLoader::load_from_str(&s)?;
    let doc = &yaml[0];
    let mut descs: Vec<Box<dyn BusDescription>> = vec![];
    for i in doc["interfaces"]
        .as_hash()
        .ok_or("YAML should define interfaces")?
        .iter()
    {
        let name = i.0.as_str().ok_or("Invalid bus name")?;
        let scope = i.1["scope"]
            .as_vec()
            .ok_or("Scope should be array of strings")?;
        let scope = scope
            .iter()
            .map(|module| module.as_str().unwrap().to_owned())
            .collect();
        let clk = i.1["clock"]
            .as_str()
            .ok_or("Bus should have clock signal")?;
        let rst = i.1["reset"]
            .as_str()
            .ok_or("Bus should have reset signal")?;
        let rst_type = i.1["reset_type"]
            .as_str()
            .ok_or("Bus should have reset type defined")?;
        let rst_type = if rst_type == "low" {
            0
        } else if rst_type == "high" {
            1
        } else {
            Err("Reset type can be \"high\" or \"low\"")?
        };
        let handshake = i.1["handshake"]
            .as_str()
            .ok_or("Bus should have handshake defined")?;

        match handshake {
            "ReadyValid" => {
                let ready = i.1["ready"]
                    .as_str()
                    .ok_or("ReadyValid bus requires ready signal")?;
                let valid = i.1["valid"]
                    .as_str()
                    .ok_or("ReadyValid bus requires valid signal")?;
                let max_burst_delay = i.1["max_burst_delay"].as_i64();
                let max_burst_delay = if max_burst_delay.is_some() {
                    max_burst_delay.unwrap().try_into().unwrap()
                } else {
                    default_max_burst_delay
                };
                descs.push(Box::new(bus::axi::AXIBus::new(
                    name.to_owned(),
                    scope,
                    clk.to_owned(),
                    rst.to_owned(),
                    rst_type,
                    max_burst_delay,
                    ready.to_owned(),
                    valid.to_owned(),
                )));
            }
            "CreditValid" => {
                let credit = i.1["credit"]
                    .as_str()
                    .ok_or("CreditValid bus requires credit signal")?;
                let valid = i.1["valid"]
                    .as_str()
                    .ok_or("CreditValid bus requires valid signal")?;
                descs.push(Box::new(bus::credit_valid::CreditValidBus::new(
                    name.to_owned(),
                    scope,
                    clk.to_owned(),
                    rst.to_owned(),
                    rst_type.to_owned(),
                    default_max_burst_delay,
                    credit.to_owned(),
                    valid.to_owned(),
                )))
            }
            "AHB" => {
                let htrans = i.1["htrans"]
                    .as_str()
                    .ok_or("AHB bus requires htrans signal")?;
                let hready = i.1["hready"]
                    .as_str()
                    .ok_or("AHB bus requires hready signal")?;
                descs.push(Box::new(bus::ahb::AHBBus::new(
                    name.to_owned(),
                    scope,
                    clk.to_owned(),
                    rst.to_owned(),
                    rst_type.to_owned(),
                    default_max_burst_delay,
                    htrans.to_owned(),
                    hready.to_owned(),
                )))
            }
            _ => Err(format!("Invalid handshake {}", handshake))?,
        }
    }
    Ok(descs)
}

pub struct SimulationData {
    hierarchy: Hierarchy,
    body: BodyResult,
}

pub fn load_simulation_trace(filename: &str) -> SimulationData {
    let load_options = LoadOptions {
        multi_thread: true,
        remove_scopes_with_empty_name: false,
    };
    let header =
        viewers::read_header_from_file(filename, &load_options).expect("Failed to load file.");
    let hierarchy = header.hierarchy;
    let body = viewers::read_body(header.body, &hierarchy, Some(Arc::new(AtomicU64::new(0))))
        .expect("Failed to load body.");

    SimulationData { hierarchy, body }
}

// fn load_signals<const N: usize>(
//     simulation_data: &mut SimulationData,
//     scope_name: &Vec<String>,
//     names: &[&str; N],
// ) -> [(wellen::SignalRef, wellen::Signal); N] {
//     let hierarchy = &simulation_data.hierarchy;
//     let body = &mut simulation_data.body;
//     let signal_refs = names.map(|r| {
//         hierarchy[hierarchy
//             .lookup_var(scope_name, &r.to_owned())
//             .expect(&format!("{} signal does not exist", &r))]
//         .signal_ref()
//     });

//     let mut loaded = body.source.load_signals(&signal_refs, &hierarchy, true);
//     loaded.sort_by_key(|(signal_ref, _)| signal_refs.iter().position(|s| s == signal_ref).unwrap());
//     loaded.try_into().unwrap()
// }

fn load_signals(
    simulation_data: &mut SimulationData,
    scope_name: &Vec<String>,
    names: &Vec<&str>,
) -> Vec<(wellen::SignalRef, wellen::Signal)> {
    let hierarchy = &simulation_data.hierarchy;
    let scope_name: Vec<&str> = scope_name.iter().map(|s| s.as_str()).collect();
    let body = &mut simulation_data.body;
    let signal_refs: Vec<wellen::SignalRef> = names
        .into_iter()
        .map(|r| {
            hierarchy[hierarchy
                .lookup_var(&scope_name, r)
                .expect(&format!("{} signal does not exist", &r))]
            .signal_ref()
        })
        .collect();

    let mut loaded = body.source.load_signals(&signal_refs, &hierarchy, true);
    loaded.sort_by_key(|(signal_ref, _)| signal_refs.iter().position(|s| s == signal_ref).unwrap());
    loaded.try_into().unwrap()
}

#[derive(PartialEq, Debug)]
pub struct BusUsage<'a> {
    pub bus_name: &'a str,
    busy: CyclesNum,
    backpressure: CyclesNum,
    no_data: CyclesNum,
    no_transaction: CyclesNum,
    free: CyclesNum,
    transaction_delays: Vec<CyclesNum>,
    current_delay: usize,
    transaction_delay_buckets: Vec<DelaysNum>,
    burst_lengths: Vec<CyclesNum>,
    burst_length_buckets: Vec<u32>,
    burst_delays: CyclesNum,
    current_burst: usize,
    max_burst_delay: CyclesNum,
}

pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
}

impl<'a> BusUsage<'a> {
    fn new(name: &str, max_burst_delay: CyclesNum) -> BusUsage {
        BusUsage {
            bus_name: name,
            busy: 0,
            backpressure: 0,
            no_data: 0,
            no_transaction: 0,
            free: 0,
            transaction_delays: vec![0],
            current_delay: 0,
            transaction_delay_buckets: vec![],
            burst_lengths: vec![0],
            current_burst: 0,
            burst_length_buckets: vec![],
            burst_delays: 0,
            max_burst_delay,
        }
    }

    fn add_cycle(&mut self, t: CycleType) {
        if let CycleType::Busy = t {
            self.add_busy_cycle();
        } else {
            self.add_wasted_cycle(t);
        }
    }

    fn add_busy_cycle(&mut self) {
        self.busy += 1;
        self.burst_lengths[self.current_burst] += 1;

        let transaction_delay = self.transaction_delays[self.current_delay];
        if transaction_delay > 0 {
            if transaction_delay > self.max_burst_delay {
                self.transaction_delays.push(0);
                self.current_delay += 1;
                let bucket = transaction_delay.ilog2() as usize;
                if self.transaction_delay_buckets.len() <= bucket {
                    self.transaction_delay_buckets.resize(bucket + 1, 0);
                }
                self.transaction_delay_buckets[bucket] += 1;
            } else {
                self.burst_delays += transaction_delay;
            }
            self.transaction_delays[self.current_delay] = 0;
        }
    }

    fn add_wasted_cycle(&mut self, t: CycleType) {
        match t {
            CycleType::Free => self.free += 1,
            CycleType::NoTransaction => self.no_transaction += 1,
            CycleType::Backpressure => self.backpressure += 1,
            CycleType::NoData => self.no_data += 1,
            CycleType::Busy => unreachable!(),
        }
        self.transaction_delays[self.current_delay] += 1;
        let transaction_delay = self.transaction_delays[self.current_delay];
        let burst_length = self.burst_lengths[self.current_burst];
        if transaction_delay > self.max_burst_delay {
            if burst_length > transaction_delay - 1 {
                let actual_length = burst_length - self.max_burst_delay;
                self.burst_lengths[self.current_burst] -= self.max_burst_delay;
                self.burst_lengths.push(0);
                self.current_burst += 1;
                let bucket = actual_length.ilog2() as usize;
                if self.burst_length_buckets.len() <= bucket {
                    self.burst_length_buckets.resize(bucket + 1, 0);
                }
                self.burst_length_buckets[bucket] += 1;
            }
            self.burst_lengths[self.current_burst] = 0;
        } else {
            self.burst_lengths[self.current_burst] += 1;
        }
    }

    fn end(&mut self) {
        let burst_length = self.burst_lengths[self.current_burst];
        if burst_length > 0 {
            let bucket = burst_length.ilog2() as usize;
            if self.burst_length_buckets.len() <= bucket {
                self.burst_length_buckets.resize(bucket + 1, 0);
            }
            self.burst_length_buckets[bucket] += 1;
        } else {
            self.burst_lengths.pop();
        }
        let transaction_delay = self.transaction_delays[self.current_delay];
        if transaction_delay > 0 {
            let bucket = transaction_delay.ilog2() as usize;
            if self.transaction_delay_buckets.len() <= bucket {
                self.transaction_delay_buckets.resize(bucket + 1, 0);
            }
            self.transaction_delay_buckets[bucket] += 1;
        } else {
            self.transaction_delays.pop();
        }
    }

    fn get_data(&self, delays_num: usize, bursts_num: usize, verbose: bool) -> Vec<String> {
        let time =
            (self.busy + self.backpressure + self.no_data + self.free + self.no_transaction) as f32;
        let mut v = vec![
            self.bus_name.to_string(),
            format!("{}({:.2})", self.busy, self.busy as f32 / time * 100.0),
            format!(
                "{}({:.2})",
                self.no_transaction,
                self.no_transaction as f32 / time * 100.0
            ),
            format!(
                "{}({:.2})",
                self.backpressure,
                self.backpressure as f32 / time * 100.0
            ),
            format!(
                "{}({:.2})",
                self.no_data,
                self.no_data as f32 / time * 100.0
            ),
            format!("{}({:.2})", self.free, self.free as f32 / time * 100.0),
        ];

        v.push(if verbose {
            format!("{:?}", self.transaction_delays)
        } else {
            String::from("")
        });

        for num in self.transaction_delay_buckets.iter() {
            v.push(num.to_string());
        }
        for _ in 0..delays_num - self.transaction_delay_buckets.len() {
            v.push(String::from("0"));
        }

        v.push(if verbose {
            format!("{:?}", self.burst_lengths)
        } else {
            String::from("")
        });

        for num in self.burst_length_buckets.iter() {
            v.push(num.to_string());
        }
        for _ in 0..bursts_num - self.burst_length_buckets.len() {
            v.push(String::from("0"));
        }
        v
    }

    pub fn literal(
        bus_name: &str,
        busy: CyclesNum,
        backpressure: CyclesNum,
        no_data: CyclesNum,
        no_transaction: CyclesNum,
        free: CyclesNum,
        transaction_delays: Vec<CyclesNum>,
        current_delay: usize,
        transaction_delay_buckets: Vec<DelaysNum>,
        burst_lengths: Vec<CyclesNum>,
        burst_length_buckets: Vec<u32>,
        burst_delays: CyclesNum,
        current_burst: usize,
        max_burst_delay: CyclesNum,
    ) -> BusUsage {
        BusUsage {
            bus_name,
            busy,
            backpressure,
            no_data,
            no_transaction,
            free,
            transaction_delays,
            current_delay,
            transaction_delay_buckets,
            burst_lengths,
            burst_length_buckets,
            burst_delays,
            current_burst,
            max_burst_delay,
        }
    }
}

fn get_header(usages: &[BusUsage]) -> (Vec<String>, usize, usize) {
    let delays = usages
        .iter()
        .map(|u| u.transaction_delay_buckets.len())
        .max()
        .unwrap();
    let bursts = usages
        .iter()
        .map(|u| u.burst_length_buckets.len())
        .max()
        .unwrap();

    let mut v = vec![
        String::from("bus_name"),
        String::from("busy(%)"),
        String::from("no transaction(%)"),
        String::from("backpressure(%)"),
        String::from("no data to send(%)"),
        String::from("free(%)"),
        String::from("delays between transactions "),
    ];
    for i in 0..delays {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1).to_string());
    }
    v.push(format!("burst lengths"));
    for i in 0..bursts {
        v.push(format!("{}-{}", 1 << i, (1 << (i + 1)) - 1));
    }
    (v, delays, bursts)
}

fn generate_tabled<O>(usages: &[BusUsage], verbose: bool, style: O) -> tabled::Table
where
    O: tabled::settings::TableOption<
        tabled::grid::records::vec_records::VecRecords<
            tabled::grid::records::vec_records::Text<String>,
        >,
        tabled::grid::config::ColoredConfig,
        tabled::grid::dimension::CompleteDimension,
    >,
{
    let (header, delays, bursts) = get_header(&usages);
    let mut builder = tabled::builder::Builder::new();
    builder.push_record(header);
    for u in usages {
        builder.push_record(u.get_data(delays, bursts, verbose));
    }
    let mut t = builder.build();
    t.with(style);
    t
}

pub fn print_statistics(write: &mut impl Write, usages: &[BusUsage], verbose: bool) {
    writeln!(
        write,
        "{}",
        generate_tabled(usages, verbose, tabled::settings::Style::rounded())
    )
    .unwrap();
}

pub fn generate_md_table(write: &mut impl Write, usages: &[BusUsage], verbose: bool) {
    writeln!(
        write,
        "{}",
        generate_tabled(usages, verbose, tabled::settings::Style::markdown())
    )
    .unwrap();
}

pub fn generate_csv(write: &mut impl Write, usages: &[BusUsage], verbose: bool) {
    let mut wtr = csv::Writer::from_writer(write);
    let (header, delays, bursts) = get_header(&usages);
    wtr.write_record(header).unwrap();
    for u in usages {
        wtr.write_record(u.get_data(delays, bursts, verbose))
            .unwrap();
    }
    wtr.flush().unwrap();
}

pub fn calculate_usage<'a>(
    simulation_data: &mut SimulationData,
    bus_desc: &'a dyn BusDescription,
) -> BusUsage<'a> {
    // match bus_desc {
    //     BusDescription::AXI(axi) => calculate_ready_valid_bus(simulation_data, axi),
    //     BusDescription::CreditValid(credit_valid_bus) => {
    //         calculate_credit_valid_bus(simulation_data, credit_valid_bus)
    //     }
    //     BusDescription::AHB(ahbbus) => calculate_ahb_bus(simulation_data, ahbbus),
    // }
    let mut signals = vec![bus_desc.common().clk_name(), bus_desc.common().rst_name()];
    signals.append(&mut bus_desc.signals());

    let loaded = load_signals(simulation_data, &bus_desc.common().module_scope(), &signals);
    let (_, clock) = &loaded[0];
    let (_, reset) = &loaded[1];

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
    usage
}
