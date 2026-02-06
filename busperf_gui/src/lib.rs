pub mod egui_visualization;
mod surfer_egui;

#[cfg(feature = "surfer")]
mod surfer_integration;

/// Run the egui GUI and show passed bus statistics.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_egui(
    usages: Vec<libbusperf::bus_usage::BusData>,
    trace_path: String,
    hash: Option<String>,
    time_unit: libbusperf::Timescale,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = eframe::NativeOptions::default();
    let surfer_data = surfer_egui::SurferData::new(trace_path, hash)?;
    if usages.is_empty() {
        Err("No results to show")?;
    }
    eframe::run_native(
        "busperf",
        options,
        Box::new(|_| {
            Ok(Box::new(egui_visualization::BusperfApp::new(
                usages,
                surfer_data,
                time_unit,
            )))
        }),
    )
    .map_err(|e| format!("[Error] failed to run egui {}", e))?;
    Ok(())
}
