use std::{
    fs::File,
    io::{Read, Write},
    sync::{Arc, atomic::AtomicU64},
};

use analyzer::{Analyzer, AnalyzerBuilder};
use wellen::{
    Hierarchy, LoadOptions,
    viewers::{self, BodyResult},
};
use yaml_rust2::YamlLoader;

mod analyzer;
mod bus;
pub mod bus_usage;
mod plugins;

pub use bus_usage::BusUsage;
pub use bus_usage::SingleChannelBusUsage;

mod egui_visualization;
mod surfer_integration;
mod text_output;

use bus::CyclesNum;

pub fn load_bus_analyzers(
    filename: &str,
    default_max_burst_delay: CyclesNum,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
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
        match AnalyzerBuilder::build(i, default_max_burst_delay, window_length, x_rate, y_rate) {
            Ok(analyzer) => analyzers.push(analyzer),
            Err(e) => {
                match i.0.as_str() {
                    Some(name) => eprintln!("Failed to load {}, {:?}", name, e),
                    None => eprintln!("Failed to load bus which does not have a name: {:?}", e),
                };
            }
        }
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
        println!("Loading trace took {:?}", start.elapsed());
    }
    SimulationData { hierarchy, body }
}

fn load_signals(
    simulation_data: &mut SimulationData,
    scope_name: &[String],
    names: &Vec<&str>,
) -> Vec<(wellen::SignalRef, wellen::Signal)> {
    let hierarchy = &simulation_data.hierarchy;
    let scope_name: Vec<&str> = scope_name.iter().map(|s| s.as_str()).collect();
    let body = &mut simulation_data.body;
    let signal_refs: Vec<wellen::SignalRef> = names
        .iter()
        .map(|r| {
            hierarchy[hierarchy
                .lookup_var(&scope_name, r)
                .unwrap_or_else(|| panic!("{} signal does not exist", &r))]
            .signal_ref()
        })
        .collect();

    let mut loaded = body.source.load_signals(&signal_refs, hierarchy, true);
    loaded.sort_by_key(|(signal_ref, _)| {
        signal_refs
            .iter()
            .position(|s| s == signal_ref)
            .expect("There should be one loaded signal for each signal_ref")
    });
    loaded
}

pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
    Reset,
    Unknown,
}

pub enum OutputType {
    Pretty,
    Csv,
    Md,
    Rendered,
}

impl TryFrom<&str> for OutputType {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "text" => Ok(Self::Pretty),
            "csv" => Ok(Self::Csv),
            "md" => Ok(Self::Md),
            "gui" => Ok(Self::Rendered),
            _ => Err("Expected one of [csv, md, gui, text]"),
        }
    }
}

pub fn show_data(
    mut analyzers: Vec<Box<dyn Analyzer>>,
    type_: OutputType,
    out: Option<&mut impl Write>,
    simulation_data: &mut SimulationData,
    trace_path: &str,
    verbose: bool,
) {
    for a in analyzers.iter_mut() {
        if !a.finished_analysis() {
            a.analyze(simulation_data, verbose);
        }
    }

    match type_ {
        OutputType::Pretty => {
            text_output::print_statistics(out.unwrap(), &analyzers, verbose);
        }
        OutputType::Csv => text_output::generate_csv(out.unwrap(), &analyzers, verbose),
        OutputType::Md => text_output::generate_md_table(out.unwrap(), &analyzers, verbose),
        OutputType::Rendered => egui_visualization::run_visualization(
            analyzers,
            trace_path,
            simulation_data
                .hierarchy
                .timescale()
                .unwrap_or(wellen::Timescale {
                    factor: 1,
                    unit: wellen::TimescaleUnit::Seconds,
                })
                .unit,
        ),
    }
}
