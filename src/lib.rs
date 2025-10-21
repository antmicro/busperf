use std::{
    cell::Cell,
    fs::File,
    io::{Read, Write},
    str::FromStr,
    sync::{Arc, atomic::AtomicU64},
};

use analyzer::{Analyzer, AnalyzerBuilder};
use blake3::Hash;
use flate2::Compression;
use wellen::{
    Hierarchy, LoadOptions,
    viewers::{self, BodyResult},
};
use yaml_rust2::YamlLoader;

pub mod analyzer;
mod bus;
pub mod bus_usage;
mod plugins;

mod egui_visualization;
mod surfer_integration;
mod text_output;

use bus::CyclesNum;

use crate::{bus::SignalPath, bus_usage::BusUsageFull};

/// Loads descriptions of the buses from yaml file with given name.
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
    let mut yaml = YamlLoader::load_from_str(&s)?;
    let doc = yaml.remove(0);
    let doc = doc
        .into_hash()
        .ok_or("Yaml should not be empty")?
        .remove(&yaml_rust2::Yaml::from_str("interfaces"))
        .ok_or("Yaml should define interfaces")?
        .into_hash()
        .ok_or("Invalid yaml format")?;
    let mut analyzers: Vec<Box<dyn Analyzer>> = vec![];
    for (name, dict) in doc {
        let n = name
            .as_str()
            .ok_or("Each bus should have a name")?
            .to_owned();
        match AnalyzerBuilder::build(
            (name, dict),
            default_max_burst_delay,
            window_length,
            x_rate,
            y_rate,
        ) {
            Ok(analyzer) => analyzers.push(analyzer),
            Err(e) => {
                eprintln!("Failed to load {}, {:?}", n, e)
            }
        }
    }
    Ok(analyzers)
}

pub struct SimulationData {
    hierarchy: Hierarchy,
    body: BodyResult,
}

/// Loads waveform file.
///
/// * `filename` - path to file.
/// * `verbose` - prints how long it took to load.
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
    signal_paths: &Vec<&SignalPath>,
) -> Vec<(wellen::SignalRef, wellen::Signal)> {
    let hierarchy = &simulation_data.hierarchy;
    let body = &mut simulation_data.body;
    let signal_refs: Vec<wellen::SignalRef> = signal_paths
        .iter()
        .map(|path| {
            hierarchy[hierarchy
                .lookup_var(&path.scope, &path.name)
                .unwrap_or_else(|| panic!("signal \"{}\" does not exist", path))]
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

/// State in which a bus was in during a clock cycle.
///
/// | busperf        | busy                  | free               | no transaction     | backpressure      |  no data        | unknown        |
/// |----------------|-----------------------|--------------------|--------------------|-------------------|-----------------|----------------|
/// | axi            | ready && valid        | !ready && !valid   | not used           | !ready && valid   | ready && !valid | no used        |
/// | ahb            | seq / no seq          | idle               | not used           | hready            | trans=BUSY      | other          |
/// | credit valid   | credit>0 && valid     | credit>0 && !valid | credit=0 && !valid | not used          | not used        | other          |
/// | apb            | setup or access phase | !psel              | not used           | access && !pready | not used        | other          |
pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
    Reset,
    Unknown,
}

/// Type of visualization of data.
#[derive(Clone)]
pub enum OutputType {
    /// Pretty printed text
    Pretty,
    Csv,
    Md,
    /// GUI
    Rendered,
}

/// * "text" -> Pretty
/// * "csv" -> Csv
/// * "md" -> Md
/// * "gui" -> Rendered
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

/// Run visualization.
///
/// If any analyzer has not yet been run it will be run. Then visualization of type `type_` will be run.
pub fn run_visualization(
    mut analyzers: Vec<Box<dyn Analyzer>>,
    type_: OutputType,
    out: Option<&mut impl Write>,
    simulation_data: &mut SimulationData,
    trace_path: &str,
    verbose: bool,
    skipped_stats: &[String],
) {
    let usages = analyzers
        .iter_mut()
        .map(|a| {
            if !a.finished_analysis() {
                a.analyze(simulation_data, verbose);
            }
            BusUsageFull::new(
                a.get_results().cloned().expect("Was just calculated"),
                a.get_signals().into_iter().cloned().collect(),
            )
        })
        .collect();

    let trace = WaveformFile {
        path: trace_path.to_owned(),
        hash: [0; 32].into(),
        checked: true.into(),
    };
    show_data(usages, trace, type_, out, verbose, skipped_stats);
}

struct WaveformFile {
    path: String,
    hash: Hash,
    checked: Cell<bool>,
}

fn show_data(
    usages: Vec<BusUsageFull>,
    trace: WaveformFile,
    type_: OutputType,
    out: Option<&mut impl Write>,
    verbose: bool,
    skipped_stats: &[String],
) {
    match type_ {
        OutputType::Pretty => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::print_statistics(out.unwrap(), &usages, verbose, skipped_stats);
        }
        OutputType::Csv => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_csv(out.unwrap(), &usages, verbose, skipped_stats)
        }
        OutputType::Md => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_md_table(out.unwrap(), &usages, verbose, skipped_stats)
        }
        OutputType::Rendered => {
            egui_visualization::run_visualization(usages, trace, wellen::TimescaleUnit::PicoSeconds)
        }
    }
}

fn calculate_file_hash(filename: &str) -> Result<Hash, Box<dyn std::error::Error>> {
    let file = File::open(filename)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

pub fn save_data(analyzers: &[Box<dyn Analyzer>], filename: &str, trace_path: &str) {
    let mut file = File::create(filename).expect("Failed to create output file");
    let hash = calculate_file_hash(trace_path)
        .expect("File already checked")
        .to_string();

    let data = (
        trace_path,
        hash,
        analyzers
            .iter()
            .filter_map(|a| {
                a.get_results().map(|r| {
                    BusUsageFull::new(r.clone(), a.get_signals().into_iter().cloned().collect())
                })
            })
            .collect::<Vec<_>>(),
    );

    let config = bincode::config::standard();
    let data = bincode::encode_to_vec(data, config).expect("Serialization failed");
    let mut encoder = flate2::write::GzEncoder::new(&mut file, Compression::default());
    encoder.write_all(&data).expect("Write to file failed");
}

pub fn visualization_from_file(filename: &str, output_type: OutputType, verbose: bool) {
    let data = std::fs::read(filename).expect("Failed to load file");
    let mut decoder = flate2::read::GzDecoder::new(&*data);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).expect("Failed decompression");
    let config = bincode::config::standard();
    let data: (String, String, Vec<BusUsageFull>) = bincode::decode_from_slice(&buf, config)
        .expect("Invalid file data")
        .0;
    let (waveform_path, hash, usages) = data;
    let hash = Hash::from_str(&hash).expect("Invalid hash value");
    let trace = WaveformFile {
        path: waveform_path,
        hash,
        checked: false.into(),
    };
    show_data(
        usages,
        trace,
        output_type,
        Some(&mut std::io::stdout()),
        verbose,
        &[],
    );
}
