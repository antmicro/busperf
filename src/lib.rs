use blake3::Hash;
use std::fs::File;
#[cfg(all(feature = "analyze", feature = "show"))]
use std::io::Write;

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

/// State in which a bus was in during a clock cycle.
///
/// | busperf        | busy                  | free               | no transaction     | backpressure      |  no data        | unknown        |
/// |----------------|-----------------------|--------------------|--------------------|-------------------|-----------------|----------------|
/// | axi            | ready && valid        | !ready && !valid   | not used           | !ready && valid   | ready && !valid | no used        |
/// | ahb            | seq / no seq          | idle               | not used           | hready            | trans=BUSY      | other          |
/// | credit valid   | credit>0 && valid     | credit>0 && !valid | credit=0 && !valid | not used          | not used        | other          |
/// | apb            | setup or access phase | !psel              | not used           | access && !pready | not used        | other          |
#[cfg(feature = "python-plugins")]
use pyo3::prelude::*;
#[cfg(feature = "python-plugins")]
#[pyclass]
#[derive(Clone, Copy)]
pub enum CycleType {
    Busy,
    Free,
    NoTransaction,
    Backpressure,
    NoData,
    Reset,
    Unknown,
}

fn calculate_file_hash(filename: &str) -> Result<Hash, Box<dyn std::error::Error>> {
    let file = File::open(filename)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}
pub type CyclesNum = i32;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct SignalPath {
    pub scope: Vec<String>,
    pub name: String,
}

impl std::fmt::Display for SignalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for s in self.scope.iter() {
            write!(f, "{}.", s)?;
        }
        write!(f, "{}", self.name)?;
        Ok(())
    }
}

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
) {
    use crate::{
        bus_usage::BusData,
        show::{WaveformFile, show_data},
    };

    let usages = analyzers
        .iter_mut()
        .map(|a| -> BusData {
            if !a.finished_analysis() {
                a.analyze(simulation_data, verbose);
            }
            BusData {
                usage: a.get_results().cloned().expect("Has just been calculated"),
                signals: a.get_signals().into_iter().cloned().collect(),
            }
        })
        .collect();

    let trace = WaveformFile {
        path: trace_path.to_owned(),
        hash: [0; 32].into(),
        checked: true.into(),
    };
    show_data(usages, trace, type_, out, verbose, skipped_stats);
}
