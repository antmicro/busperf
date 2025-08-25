use std::{
    fs::File,
    io::{Read, Write},
    sync::{atomic::AtomicU64, Arc},
};

use analyzer::{Analyzer, AnalyzerBuilder};
use bus_usage::{get_header, get_header_multi, MultiChannelBusUsage};
use wellen::{
    viewers::{self, BodyResult},
    Hierarchy, LoadOptions,
};
use yaml_rust2::YamlLoader;

mod analyzer;
mod bus;
mod bus_usage;

pub use bus_usage::BusUsage;
pub use bus_usage::SingleChannelBusUsage;

use bus::CyclesNum;
use bus::DelaysNum;

pub fn load_bus_analyzers(
    filename: &str,
    default_max_burst_delay: CyclesNum,
) -> Result<Vec<Box<dyn Analyzer>>, Box<dyn std::error::Error>> {
    let mut f = File::open(filename)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let yaml = YamlLoader::load_from_str(&s)?;
    let doc = &yaml[0];
    let mut analyzers: Vec<Box<dyn Analyzer>> = vec![];
    for i in doc["interfaces"]
        .as_hash()
        .ok_or("YAML should define interfaces")?
        .iter()
    {
        let analyzer: Box<dyn Analyzer> = AnalyzerBuilder::build(i, default_max_burst_delay);
        analyzers.push(analyzer);
    }
    Ok(analyzers)
}

pub struct SimulationData {
    hierarchy: Hierarchy,
    body: BodyResult,
}

pub fn load_simulation_trace(filename: &str, verbose: bool) -> SimulationData {
    let start = std::time::Instant::now();
    let load_options = LoadOptions {
        multi_thread: true,
        remove_scopes_with_empty_name: false,
    };
    let header =
        viewers::read_header_from_file(filename, &load_options).expect("Failed to load file.");
    let hierarchy = header.hierarchy;
    let body = viewers::read_body(header.body, &hierarchy, Some(Arc::new(AtomicU64::new(0))))
        .expect("Failed to load body.");
    if verbose {
        // println!("loading trace took {}", start.elapsed().as_millis());
        println!("{}", start.elapsed().as_millis());
    }
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

pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
}

fn generate_tabled<O>(
    header: &Vec<String>,
    data: &Vec<Vec<String>>,
    verbose: bool,
    style: O,
) -> tabled::Table
where
    O: tabled::settings::TableOption<
        tabled::grid::records::vec_records::VecRecords<
            tabled::grid::records::vec_records::Text<String>,
        >,
        tabled::grid::config::ColoredConfig,
        tabled::grid::dimension::CompleteDimension,
    >,
{
    let mut builder = tabled::builder::Builder::new();
    builder.push_record(header);
    for u in data {
        builder.push_record(u);
    }
    let mut t = builder.build();
    t.with(style);
    t
}

fn print_statistics_internal<O>(
    write: &mut impl Write,
    usages: &[&BusUsage],
    verbose: bool,
    style: O,
) where
    O: tabled::settings::TableOption<
            tabled::grid::records::vec_records::VecRecords<
                tabled::grid::records::vec_records::Text<String>,
            >,
            tabled::grid::config::ColoredConfig,
            tabled::grid::dimension::CompleteDimension,
        > + Clone,
{
    let single_usages: Vec<&SingleChannelBusUsage> = usages
        .iter()
        .filter_map(|u| match u {
            BusUsage::SingleChannel(single) => Some(single),
            _ => None,
        })
        .collect();
    if single_usages.len() > 0 {
        let (header, delays, bursts) = get_header(&single_usages);
        let data = single_usages
            .iter()
            .map(|u| u.get_data(delays, bursts, verbose))
            .collect();
        writeln!(
            write,
            "{}",
            generate_tabled(&header, &data, verbose, style.clone())
        )
        .unwrap();
    }

    let multi_usage: Vec<&MultiChannelBusUsage> = usages
        .iter()
        .filter_map(|u| match u {
            BusUsage::MultiChannel(multi) => Some(multi),
            _ => None,
        })
        .collect();
    if multi_usage.len() > 0 {
        let (header, c2c, c2d, ld2c, delays) = get_header_multi(&multi_usage);
        let data = multi_usage
            .iter()
            .map(|u| u.get_data(verbose, c2c, c2d, ld2c, delays))
            .collect();
        writeln!(write, "{}", generate_tabled(&header, &data, verbose, style)).unwrap();
    }
}
pub fn print_statistics(write: &mut impl Write, usages: &[&BusUsage], verbose: bool) {
    print_statistics_internal(write, usages, verbose, tabled::settings::Style::rounded());
}

pub fn generate_md_table(write: &mut impl Write, usages: &[&BusUsage], verbose: bool) {
    print_statistics_internal(write, usages, verbose, tabled::settings::Style::markdown());
}

pub fn generate_csv(write: &mut impl Write, usages: &[&BusUsage], verbose: bool) {
    let mut wtr = csv::Writer::from_writer(write);
    let usages: Vec<&SingleChannelBusUsage> = usages
        .iter()
        .filter_map(|u| match u {
            BusUsage::SingleChannel(single) => Some(single),
            _ => None,
        })
        .collect();
    let (header, delays, bursts) = get_header(&usages);
    wtr.write_record(header).unwrap();
    for u in usages {
        wtr.write_record(u.get_data(delays, bursts, verbose))
            .unwrap();
    }
    wtr.flush().unwrap();
}
