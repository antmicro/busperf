use blake3::Hash;
use flate2::Compression;
use std::io::Read;
use std::str::FromStr;
use std::{cell::Cell, io::Write};

use crate::bus_usage::BusData;

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
) {
    match type_ {
        OutputType::Pretty => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::print_statistics(out, &usages, verbose, skipped_stats);
        }
        OutputType::Csv => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_csv(out, &usages, verbose, skipped_stats)
        }
        OutputType::Md => {
            let usages = usages.iter().map(|u| &u.usage).collect::<Vec<_>>();
            text_output::generate_md_table(out, &usages, verbose, skipped_stats)
        }
        OutputType::Rendered => {
            egui_visualization::run_visualization(usages, trace, wellen::TimescaleUnit::PicoSeconds)
        }
        OutputType::Data => {
            save_data(usages, trace, out);
        }
        #[cfg(feature = "generate-html")]
        OutputType::Html => {
            generate_html(usages, trace, out);
        }
    }
}

pub fn visualization_from_file(filename: &str, output_type: OutputType, verbose: bool) {
    let data = std::fs::read(filename).expect("Failed to load file");
    let mut decoder = flate2::read::GzDecoder::new(&*data);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).expect("Failed decompression");
    let config = bincode::config::standard();
    let data: (String, String, Vec<BusData>) = bincode::decode_from_slice(&buf, config)
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
        &mut std::io::stdout(),
        verbose,
        &[],
    );
}

#[inline]
fn prepare_data(usages: Vec<BusData>, trace: WaveformFile, out: &mut impl Write) {
    let data = (trace.path, trace.hash.to_string(), usages);
    let config = bincode::config::standard();
    let data = bincode::encode_to_vec(data, config).expect("Serialization failed");
    let mut encoder = flate2::write::GzEncoder::new(out, Compression::default());
    encoder.write_all(&data).expect("Write to file failed");
}

fn save_data(usages: Vec<BusData>, trace: WaveformFile, out: &mut impl Write) {
    prepare_data(usages, trace, out);
}

#[cfg(feature = "generate-html")]
fn generate_html(usages: Vec<BusData>, trace: WaveformFile, out: &mut impl Write) {
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

    out.write_all(html.as_bytes()).expect("Failed to write")
}
