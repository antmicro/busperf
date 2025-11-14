use std::{
    fs::File,
    io::{Read, Write},
    sync::{Arc, atomic::AtomicU64},
};

use flate2::Compression;
use wellen::{
    Hierarchy, LoadOptions,
    viewers::{self, BodyResult},
};
use yaml_rust2::YamlLoader;

use crate::{CyclesNum, bus_usage::BusData, calculate_file_hash};
use analyzer::{Analyzer, AnalyzerBuilder};
use bus::SignalPath;

pub mod analyzer;
mod bus;
#[cfg(feature = "python-plugins")]
mod plugins;

/// Loads descriptions of the buses from yaml file with given name.
pub fn load_bus_analyzers(
    filename: &str,
    default_max_burst_delay: CyclesNum,
    window_length: u32,
    x_rate: f32,
    y_rate: f32,
    plugins_path: &str,
) -> Result<Vec<Box<dyn Analyzer>>, Box<dyn std::error::Error>> {
    let mut f = File::open(filename)?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    let mut yaml = YamlLoader::load_from_str(&s)?;
    let mut doc = yaml
        .remove(0)
        .into_hash()
        .ok_or("Yaml should not be empty")?;
    let interfaces = doc
        .remove(&yaml_rust2::Yaml::from_str("interfaces"))
        .ok_or("Yaml should define interfaces")?
        .into_hash()
        .ok_or("Invalid yaml format")?;
    let unused = doc
        .into_iter()
        .filter_map(|(name, _)| {
            if let Some(s) = name.into_string()
                && s != "scopes"
                && s != "common_clk_rst_ifs"
            {
                Some(s)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !unused.is_empty() {
        Err(format!(
            "Yaml can only have interfaces, scopes(optional) and common_clk_rst_ifs(optional) in top level, but has extra: {}",
            unused.join(", ")
        ))?;
    }
    let mut analyzers: Vec<Box<dyn Analyzer>> = vec![];
    for (name, dict) in interfaces {
        let n = name
            .as_str()
            .ok_or("Each bus should have a name")?
            .to_owned();
        analyzers.push(
            AnalyzerBuilder::build(
                (name, dict),
                default_max_burst_delay,
                window_length,
                x_rate,
                y_rate,
                plugins_path,
            )
            .map_err(|e| format!("bus {n}, {e}"))?,
        );
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
                    BusData::new(r.clone(), a.get_signals().into_iter().cloned().collect())
                })
            })
            .collect::<Vec<_>>(),
    );

    let config = bincode::config::standard();
    let data = bincode::encode_to_vec(data, config).expect("Serialization failed");
    let mut encoder = flate2::write::GzEncoder::new(&mut file, Compression::default());
    encoder.write_all(&data).expect("Write to file failed");
}
