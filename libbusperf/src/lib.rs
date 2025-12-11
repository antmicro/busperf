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

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
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
pub fn calculate_file_hash(filename: &str) -> Result<blake3::Hash, Box<dyn std::error::Error>> {
    use std::fs::File;

    let file = File::open(filename)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize())
}

cfg_if::cfg_if! {
    if #[cfg(feature = "file-hash")] {
        use blake3::Hash;
        pub struct WaveformFile {
            pub path: String,
            pub hash: Hash,
            pub checked: std::cell::Cell<bool>,
        }

        impl WaveformFile {
            pub fn new(path: String, hash: String) -> Result<WaveformFile, Box<dyn std::error::Error>> {
                use std::str::FromStr;
                Ok(WaveformFile {
                    path,
                    hash: Hash::from_str(&hash).map_err(|_| "invalid file: invalid hash")?,
                    checked: true.into(),
                })
            }
        }
    } else {
        pub struct WaveformFile(());
    }
}
