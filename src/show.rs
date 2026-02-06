//! Module containing different ways of displaying data calculated by Busperf.
use std::error::Error;
use std::io::Read;
use std::io::Write;

use libbusperf::bus_usage::BusData;
use libbusperf::{Timescale, prepare_data};

mod text_output;

/// Type of visualization of data.
#[derive(Clone)]
pub enum OutputType {
    /// Pretty printed text
    Pretty,
    Csv,
    Md,
    /// GUI
    #[cfg(feature = "gui")]
    Rendered,
    /// Busperf data - binary format
    Data,
    /// Busperf web with embedded data in one html file
    #[cfg(feature = "generate-html")]
    Html,
}

/// Output data from `usages` in format defined by `type_` into `out`.
#[allow(clippy::too_many_arguments)]
pub fn show_data(
    usages: Vec<BusData>,
    timescale: Timescale,
    trace_path: String,
    _hash: Option<String>,
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
        #[cfg(feature = "gui")]
        OutputType::Rendered => busperf_gui::run_egui(usages, trace_path, _hash, timescale),
        OutputType::Data => save_data(timescale, usages, trace_path, out),
        #[cfg(feature = "generate-html")]
        OutputType::Html => busperf_web::generate_html(timescale, usages, trace_path, out),
    }
}

/// Show data from a binary busperf data file.
pub fn visualization_from_file(
    filename: &str,
    output_type: OutputType,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let data = std::fs::read(filename).map_err(|e| format!("Failed to load file {e}"))?;
    let mut decoder = flate2::read::GzDecoder::new(&*data);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf).map_err(|_| "Invalid file")?;
    let data: (String, String, Timescale, Vec<BusData>) =
        bitcode::decode(&buf).map_err(|_| "Invalid file data")?;
    let (waveform_path, hash, timescale, usages) = data;
    show_data(
        usages,
        timescale,
        waveform_path,
        Some(hash),
        output_type,
        &mut std::io::stdout(),
        verbose,
        &[],
    )?;
    Ok(())
}

/// Save data into binary format.
fn save_data(
    timescale: Timescale,
    usages: Vec<BusData>,
    trace: String,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    prepare_data(timescale, usages, trace, out)
}
