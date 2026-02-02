pub use crate::{
    analyze::{AnalyzersConfig, analyze_all, load_bus_analyzers, load_simulation_trace},
    show::{OutputType, show_data, visualization_from_file},
};
use std::{error::Error, io::Write};

mod analyze;
mod show;

/// Performs whole analysis process.
///
/// Loads the bus description and simulation trace from provided files. Calculates statistics and outputs them in requested format to out
/// Returns `Ok(())` when every step succedes. In case some analyzers fail it outputs the successful ones.
pub fn analyze_and_show_results(
    simulation_trace_file: &str,
    bus_description_file: &str,
    config: AnalyzersConfig,
    type_: OutputType,
    verbose: bool,
    out: &mut impl Write,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    let analyzers = load_bus_analyzers(bus_description_file, &config)
        .map_err(|e| format!("[ERROR] Invalid bus decription: {e}"))?;
    let mut simulation_data = load_simulation_trace(simulation_trace_file, verbose)
        .map_err(|e| format!("[ERROR] Invalid simulation trace: {e}"))?;
    let result = analyze_all(analyzers, &mut simulation_data, verbose);
    let mut failed = vec![];
    let usages: Vec<_> = result
        .into_iter()
        .filter_map(|u| match u {
            Ok(usage) => Some(usage),
            Err(e) => {
                use owo_colors::OwoColorize;
                failed.push(format!("Failed to analyze: {e}"));
                eprintln!(
                    "{} {}",
                    "[Error] Failed to analyze".bright_red(),
                    e.bright_red()
                );
                None
            }
        })
        .collect();
    show_data(
        usages,
        simulation_trace_file.to_owned(),
        None,
        type_,
        out,
        verbose,
        skipped_stats,
    )?;
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("\n").into())
    }
}
