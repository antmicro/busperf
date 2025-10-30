use blake3::Hash;
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

pub struct WaveformFile {
    pub path: String,
    pub hash: Hash,
    pub checked: Cell<bool>,
}

pub fn show_data(
    usages: Vec<BusData>,
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
        Some(&mut std::io::stdout()),
        verbose,
        &[],
    );
}
