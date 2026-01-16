pub mod bus_usage;

/// State in which a bus was in during a clock cycle.
///
/// | busperf        | busy                  | free               | no transaction     | backpressure      |  no data        | unknown        |
/// |----------------|-----------------------|--------------------|--------------------|-------------------|-----------------|----------------|
/// | axi            | ready && valid        | !ready && !valid   | not used           | !ready && valid   | ready && !valid | no used        |
/// | ahb            | seq / no seq          | idle               | not used           | hready            | trans=BUSY      | other          |
/// | credit valid   | credit>0 && valid     | credit>0 && !valid | credit=0 && !valid | not used          | not used        | other          |
/// | apb            | setup or access phase | !psel              | not used           | access && !pready | not used        | other          |
// #[cfg(feature = "python-plugins")]
// use pyo3::prelude::*;
// #[cfg(feature = "python-plugins")]
// #[pyclass]
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

pub type CyclesNum = i32;

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
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

#[cfg(feature = "file-hash")]
use bus_usage::BusData;
#[cfg(feature = "file-hash")]
use flate2::Compression;
#[cfg(feature = "file-hash")]
use std::error::Error;
#[cfg(feature = "file-hash")]
use std::io::Write;
#[cfg(feature = "file-hash")]
#[inline]
pub fn prepare_data(
    usages: Vec<BusData>,
    trace: String,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let hash = calculate_file_hash(&trace)
        .map_err(|e| format!("[ERROR] failed to calculate trace hash: {e}"))?;
    let data = (trace, hash.to_string(), usages);
    let data = bitcode::encode(&data);
    let mut encoder = flate2::write::GzEncoder::new(out, Compression::default());
    encoder
        .write_all(&data)
        .map_err(|e| format!("Write to file failed {e}"))?;
    Ok(())
}

#[cfg(feature = "file-hash")]
pub fn calculate_file_hash(filename: &str) -> Result<blake3::Hash, Box<dyn std::error::Error>> {
    use std::fs::File;

    let file = File::open(filename)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

cfg_if::cfg_if! {
    if #[cfg(feature = "file-hash")] {
    } else {
        pub struct WaveformFile(());
    }
}
