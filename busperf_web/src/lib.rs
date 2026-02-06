#![cfg(not(target_arch = "wasm32"))]
#![cfg(feature = "build_wasm")]

use libbusperf::bus_usage::BusData;
use libbusperf::{Timescale, prepare_data};
use std::error::Error;
use std::io::Write;

/// Generate html containing wasm of busperf_web and calculated bus statistics.
pub fn generate_html(
    timescale: Timescale,
    usages: Vec<BusData>,
    trace: String,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    use base64::prelude::*;

    let mut busperf_data = Vec::new();
    prepare_data(timescale, usages, trace, &mut busperf_data)?;

    let busperf_data = BASE64_STANDARD.encode(busperf_data);

    let js = include_str!(concat!(env!("OUT_DIR"), "/busperf_web.js"));

    let wasm = include_bytes!(concat!(env!("OUT_DIR"), "/busperf_web_bg.wasm"));
    let wasm = BASE64_STANDARD.encode(wasm);

    let html = String::from(include_str!("../template.html"));
    let html = html.replace("JAVASCRIPT_HERE", js);
    let html = html.replace("WASM_HERE", &wasm);
    let html = html.replace("DATA_HERE", &busperf_data);

    out.write_all(html.as_bytes())
        .map_err(|e| format!("Failed to write {e}"))?;
    Ok(())
}
