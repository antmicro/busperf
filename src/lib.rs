use crate::{
    analyze::{AnalyzersGraph, SimulationData, analyze_all},
    show::OutputType,
};
use std::{error::Error, io::Write};

pub mod analyze;
pub mod show;

/// Run visualization.
///
/// If any analyzer has not yet been run it will be run. Then visualization of type `type_` will be run.
pub fn run_visualization(
    analyzers: AnalyzersGraph,
    type_: OutputType,
    out: &mut impl Write,
    simulation_data: &mut SimulationData,
    trace_path: String,
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    use crate::show::show_data;

    let result = analyze_all(analyzers, simulation_data, verbose);
    let usages: Vec<_> = result
        .into_iter()
        .filter_map(|u| match u {
            Ok(usage) => Some(usage),
            Err(e) => {
                use owo_colors::OwoColorize;
                eprintln!(
                    "{} {}",
                    "[Error] Failed to analyze".bright_red(),
                    e.bright_red()
                );
                None
            }
        })
        .collect();
    show_data(usages, trace_path, None, type_, out, verbose, skipped_stats)?;
    Ok(())
}
