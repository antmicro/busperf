use blake3::Hash;
use flate2::Compression;
use std::error::Error;
use std::io::Read;
use std::str::FromStr;
use std::{cell::Cell, io::Write};

use crate::bus_usage::BusData;
use crate::calculate_file_hash;

pub mod egui_visualization;
mod surfer_integration;
mod text_output;

/// Type of visualization of data.
#[derive(Clone)]
pub enum OutputType {
    /// Pretty printed text
    Pretty,
    Csv,
    Md,
    /// GUI
    Rendered,
    /// Busperf data - binary format
    Data,
    /// Busperf web with embedded data in one html file
    #[cfg(feature = "generate-html")]
    Html,
}

pub struct WaveformFile {
    pub path: String,
    pub hash: Hash,
    pub checked: Cell<bool>,
}

pub fn show_data(
    usages: Vec<BusData>,
    trace: WaveformFile,
    type_: OutputType,
    out: &mut impl Write,
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    match type_ {
        OutputType::Pretty => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::print_statistics(out, &usages, verbose, skipped_stats)
        }
        OutputType::Csv => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_csv(out, &usages, verbose, skipped_stats)
        }
        OutputType::Md => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_md_table(out, &usages, verbose, skipped_stats)
        }
        #[cfg(not(target_arch = "wasm32"))]
        OutputType::Rendered => {
            egui_visualization::run_visualization(usages, trace, wellen::TimescaleUnit::PicoSeconds)
        }
        #[cfg(target_arch = "wasm32")]
        OutputType::Rendered => Ok(()),
        OutputType::Data => save_data(usages, trace, out),
        #[cfg(feature = "generate-html")]
        OutputType::Html => generate_html(usages, trace, out),
    }
}

pub fn visualization_from_file(
    filename: &str,
    output_type: OutputType,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let data = std::fs::read(filename).map_err(|e| format!("Failed to load file {e}"))?;
    let mut decoder = flate2::read::GzDecoder::new(&*data);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(|_| "Invalid file")?;
    let config = bincode::config::standard();
    let (data, _): ((String, String, Vec<BusData>), _) =
        bincode::decode_from_slice(&buf, config).map_err(|_| "Invalid file data")?;
    let (waveform_path, hash, usages) = data;
    let hash = Hash::from_str(&hash).map_err(|_| "Invalid file: bad hash value")?;
    let trace = WaveformFile {
        path: waveform_path,
        hash,
        checked: false.into(),
    };
    show_data(
        usages,
        trace,
        output_type,
        &mut std::io::stdout(),
        verbose,
        &[],
    )?;
    Ok(())
}

#[inline]
fn prepare_data(
    usages: Vec<BusData>,
    trace: WaveformFile,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let hash = calculate_file_hash(&trace.path)
        .map_err(|e| format!("[ERROR] failed to calculate trace hash: {e}"))?;
    let data = (trace.path, hash.to_string(), usages);
    let config = bincode::config::standard();
    let data = bincode::encode_to_vec(data, config).map_err(|_| "Serialization failed")?;
    let mut encoder = flate2::write::GzEncoder::new(out, Compression::default());
    encoder
        .write_all(&data)
        .map_err(|e| format!("Write to file failed {e}"))?;
    Ok(())
}

fn save_data(
    usages: Vec<BusData>,
    trace: WaveformFile,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    prepare_data(usages, trace, out)
}

#[cfg(feature = "generate-html")]
fn generate_html(
    usages: Vec<BusData>,
    trace: WaveformFile,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    use base64::prelude::*;

    let mut busperf_data = Vec::new();
    prepare_data(usages, trace, &mut busperf_data);

    let busperf_data = BASE64_STANDARD.encode(busperf_data);

    let js = include_str!("../target_wasm/busperf_web.js");

    let wasm = include_bytes!("../target_wasm/busperf_web_bg.wasm");
    let wasm = BASE64_STANDARD.encode(wasm);

    let html = String::from(include_str!("../template.html"));
    let html = html.replace("JAVASCRIPT_HERE", &js);
    let html = html.replace("WASM_HERE", &wasm);
    let html = html.replace("DATA_HERE", &busperf_data);

    out.write_all(html.as_bytes())
        .map_err(|e| format!("Failed to write {e}"))?;
    Ok(())
}
