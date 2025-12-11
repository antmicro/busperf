#[cfg(all(feature = "analyze", feature = "show"))]
use std::{error::Error, io::Write};

#[cfg(all(feature = "analyze", feature = "show"))]
use crate::{
    analyze::{SimulationData, analyzer::Analyzer},
    show::OutputType,
};

#[cfg(feature = "analyze")]
pub mod analyze;

#[cfg(feature = "show")]
pub mod show;

pub mod bus_usage;

#[cfg(all(feature = "analyze", feature = "show"))]
/// Run visualization.
///
/// If any analyzer has not yet been run it will be run. Then visualization of type `type_` will be run.
pub fn run_visualization(
    mut analyzers: Vec<Box<dyn Analyzer>>,
    type_: OutputType,
    out: &mut impl Write,
    simulation_data: &mut SimulationData,
    trace_path: &str,
    verbose: bool,
    skipped_stats: &[String],
) -> Result<(), Box<dyn Error>> {
    use crate::{
        bus_usage::BusData,
        show::{WaveformFile, show_data},
    };

    let usages = analyzers
        .iter_mut()
        .filter_map(|a| {
            if !a.finished_analysis()
                && let Err(e) = a.analyze(simulation_data, verbose)
            {
                use owo_colors::OwoColorize;
                eprintln!(
                    "{} {} {}",
                    "[Error] failed to analyze:".bright_red(),
                    a.bus_name(),
                    e.bright_red()
                );
            }
            a.get_results().cloned().map(|usage| BusData {
                usage,
                signals: a.get_signals().into_iter().cloned().collect(),
            })
        })
        .collect();

    let trace = WaveformFile {
        path: trace_path.to_owned(),
        hash: [0; 32].into(),
        checked: true.into(),
    };
    show_data(usages, trace, type_, out, verbose, skipped_stats)?;
    Ok(())
}
