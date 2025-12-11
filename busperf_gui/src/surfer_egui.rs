use crate::egui_visualization::BucketsPlot;
use crate::egui_visualization::TimelinePlot;
use crate::egui_visualization::TimescaleUnit;
use cfg_if::cfg_if;
use eframe::egui::{Id, Response, Ui};
use egui_plot::PlotUi;
use libbusperf::bus_usage::{BucketsStatistic, Statistic};
use std::collections::HashMap;

cfg_if! {
    if #[cfg(feature = "surfer")] {
        type ModalAction = Option<Box<dyn FnOnce(&str)>>;

        struct SurferConnectionUi {
            wrong_checksum_modal: bool,
            file_not_found_modal: bool,
            modal_action: ModalAction,
            warning: String,
        }

        use blake3::Hash;
        struct WaveformFile {
            pub path: String,
            pub hash: Hash,
            pub checked: std::cell::Cell<bool>,
        }

        impl WaveformFile {
            pub fn new(path: String, hash: Option<String>) -> Result<WaveformFile, Box<dyn std::error::Error>> {
                use std::str::FromStr;
                match hash {
                    Some(hash) => Ok(WaveformFile {
                        path,
                        hash: Hash::from_str(&hash).map_err(|_| "invalid file: invalid hash")?,
                        checked: false.into(),
                    }),
                    None => Ok(WaveformFile {
                        path,
                        hash: [0; 32].into(),
                        checked: true.into(),
                    })
                }
            }
        }

        fn ensure_trace_matches(trace: &WaveformFile, ui: &mut SurferConnectionUi) -> bool {
            if trace.checked.get() {
                return true;
            }
            let hash1 = trace.hash;
            if let Ok(hash2) = libbusperf::calculate_file_hash(&trace.path) {
                if hash1 == hash2 {
                    trace.checked.set(true);
                    true
                } else {
                    ui.wrong_checksum_modal = true;
                    false
                }
            } else {
                ui.file_not_found_modal = true;
                false
            }
        }

        use std::cell::RefCell;
        pub struct SurferData {
            trace_path: WaveformFile,
            surfer: RefCell<SurferConnectionUi>,
            signals: Vec<String>,
            bus_name: String,
        }

        impl SurferData {
            pub fn new(trace_path: String, hash: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(SurferData {
                    trace_path: WaveformFile::new(trace_path, hash)?,
                    surfer: RefCell::new(SurferConnectionUi {
                        wrong_checksum_modal: false,
                        file_not_found_modal: false,
                        modal_action: None,
                        warning: String::new(),
                    }),
                    signals: Vec::new(),
                    bus_name: String::new(),
                })
            }
            pub fn set_signals_and_name(&mut self, signals: Vec<String>, bus_name: &str) {
                self.signals = signals;
                self.bus_name = bus_name.to_owned();
            }
            pub fn ui(&mut self, ui: &mut Ui) {
                use eframe::egui::{Sides, Modal};
                if self.surfer.borrow().wrong_checksum_modal {
                    let modal = Modal::new(Id::new("WrongChecksum")).show(ui.ctx(), |ui| {
                        ui.heading("Mismatched file");
                        ui.label("This file's checksum is different from the original waveform's. This means that data was calculated for a different simulation.");

                        Sides::new().show(ui, |_ui| {}, |ui| {
                            if ui.button("Select different file").clicked() {
                                #[cfg(not(target_arch = "wasm32"))]
                                if let Some(path) = rfd::FileDialog::new().pick_file() {
                                    let Ok(path) = path
                                        .into_os_string()
                                        .into_string() else {
                                        return;
                                    };
                                    self.trace_path.path = path;
                                    if ensure_trace_matches(&self.trace_path, &mut *self.surfer.borrow_mut()) {
                                        ui.close();
                                        if let Some(action) = self.surfer.borrow_mut().modal_action.take() {
                                            println!("opening {}", self.trace_path.path);
                                            action(&self.trace_path.path);
                                        }
                                    } else {
                                        self.surfer.borrow_mut().warning = String::from("Invalid file");
                                    }
                                } else {
                                    ui.close();
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                ui.close();
                            }
                            if ui.button("Open anyways").clicked() {
                                ui.close();
                                self.trace_path.checked.set(true);
                                if let Some(action) = self.surfer.borrow_mut().modal_action.take() {
                                    action(&self.trace_path.path);
                                }
                            }
                        })
                    });
                    if modal.should_close() {
                        self.surfer.borrow_mut().wrong_checksum_modal = false;
                    }
                }

                if self.surfer.borrow().file_not_found_modal {
                    let modal = eframe::egui::Modal::new(eframe::egui::Id::new("FileNotFound")).show(ui.ctx(), |ui| {
                        ui.heading("File not found");
                        ui.label("Saved path is not valid.");
                        ui.add_space(10.0);
                        ui.label("Select waveform file to load in surfer.");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.trace_path.path);
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("Select").clicked()
                                && let Some(pathbuf) = rfd::FileDialog::new().pick_file()
                            {
                                if let Some(path) = pathbuf.to_str() {
                                    self.trace_path.path = path.to_string();
                                    self.surfer.borrow_mut().warning = String::new();
                                } else {
                                    self.surfer.borrow_mut().warning = String::from("Non UTF8 in file name");
                                }
                            }
                        });
                        if !self.surfer.borrow().warning.is_empty() {
                            ui.colored_label(eframe::egui::Color32::RED, &self.surfer.borrow().warning);
                        }

                        eframe::egui::Sides::new().show(
                            ui,
                            |_ui| {},
                            |ui| {
                                if ui.button("Ok").clicked() {
                                    if ensure_trace_matches(&self.trace_path, &mut self.surfer.borrow_mut()) {
                                        ui.close();
                                        if let Some(action) = self.surfer.borrow_mut().modal_action.take() {
                                            action(&self.trace_path.path);
                                        }
                                    } else {
                                        self.surfer.borrow_mut().warning = String::from("Invalid file");
                                    }
                                }
                            },
                        );
                    });
                    if modal.should_close() {
                        self.surfer.borrow_mut().file_not_found_modal = false;
                    }
                }
            }
        }
    } else {
        pub struct SurferData(());

        impl SurferData {
            pub fn new(_trace_path: String, _hash: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(SurferData(()))
            }

            pub fn ui(&self, _ui: &mut Ui) {}
            pub fn set_signals_and_name(&mut self, _signals: Vec<String>, _bus_name: &str) {}
        }
    }
}

cfg_if! {
    if #[cfg(feature = "surfer")] {
        use crate::surfer_integration;
        use eframe::egui::{self, containers::menu::MenuConfig};

        fn plot_to_waveform_time(
            value: f64,
            waveform_time_unit: &TimescaleUnit,
            plot_time_unit: &TimescaleUnit,
        ) -> f64 {
            let diff = i32::from(waveform_time_unit)
                - i32::from(plot_time_unit);
            if diff > 0 {
                value / 10.0f64.powi(diff.abs())
            } else {
                value * 10.0f64.powi(diff.abs())
            }
        }

        pub fn surfer_ui_timeline(plot_ui: &PlotUi, surfer_info: &SurferData, waveform_time_unit: &TimescaleUnit, timeline: &mut TimelinePlot, all_statistics: &[Statistic]) {
            let TimelinePlot {
                timescale_unit: plot_time_unit,
                pointer: coords,
                period_start,
                period_end,
            } = timeline;
            plot_ui.response().context_menu(|ui| {
                ui.menu_button("open in surfer", |ui| {
                    if ui.button("mark this time").clicked()
                        && ensure_trace_matches(
                            &surfer_info.trace_path,
                            &mut surfer_info.surfer.borrow_mut(),
                        )
                    {
                        surfer_integration::open_at_time(
                            &surfer_info.trace_path.path,
                            surfer_info.signals.clone(),
                            plot_to_waveform_time(
                                coords.expect("Should be set by right click").x,
                                waveform_time_unit,
                                plot_time_unit,
                            ),
                        );
                    }
                    ui.menu_button("mark statistic", |ui| {
                        for s in all_statistics {
                            if let Statistic::Bucket(s) = s {
                                ui.menu_button(s.name, |ui| {
                                    if ui.button("before this point").clicked() {
                                        let time = coords.expect("Is set by right click").x;
                                        let periods = s
                                            .data
                                            .iter()
                                            .rev()
                                            .filter(|period| (period.end() as f64) < time)
                                            .map(|period| (period.start(), period.end()))
                                            .take(10)
                                            .collect::<Vec<_>>();
                                        surfer_integration::open_and_mark_periods(
                                            &surfer_info.trace_path.path,
                                            surfer_info.signals.clone(),
                                            &periods,
                                            &format!("{} {}", s.name, surfer_info.bus_name),
                                            s.color,
                                        );
                                        if let Some(&(start, _)) = periods.last() {
                                            surfer_integration::zoom_to_range(
                                                start,
                                                time as u64,
                                            );
                                        }
                                    }
                                    if ui.button("after this point").clicked() {
                                        let time = coords.expect("Is set by right click").x;
                                        let periods = s
                                            .data
                                            .iter()
                                            .filter(|period| period.start() as f64 > time)
                                            .map(|period| (period.start(), period.end()))
                                            .take(10)
                                            .collect::<Vec<_>>();
                                        surfer_integration::open_and_mark_periods(
                                            &surfer_info.trace_path.path,
                                            surfer_info.signals.clone(),
                                            &periods,
                                            &format!("{} {}", s.name, surfer_info.bus_name),
                                            s.color,
                                        );
                                        if let Some(&(_, end)) = periods.last() {
                                            surfer_integration::zoom_to_range(time as u64, end);
                                        }
                                    }
                                    let menu = egui::containers::menu::SubMenuButton::new(
                                        "custom period",
                                    )
                                    .config(MenuConfig::new().close_behavior(
                                        egui::PopupCloseBehavior::CloseOnClickOutside,
                                    ));
                                    menu.ui(ui, |ui| {
                                        ui.add(egui::DragValue::new(period_start).speed(0.1));
                                        ui.add(egui::DragValue::new(period_end).speed(0.1));
                                        if ui.button("open").clicked() {
                                            let period_start = plot_to_waveform_time(
                                                *period_start,
                                                waveform_time_unit,
                                                plot_time_unit,
                                            );
                                            let period_end = plot_to_waveform_time(
                                                *period_end,
                                                waveform_time_unit,
                                                plot_time_unit,
                                            );
                                            surfer_integration::open_and_mark_periods(
                                                &surfer_info.trace_path.path,
                                                surfer_info.signals.clone(),
                                                &s.data
                                                    .iter()
                                                    .filter(|period| {
                                                        period.start() as f64 > period_start
                                                            && (period.end() as f64)
                                                                < period_end
                                                    })
                                                    .map(|period| {
                                                        (period.start(), period.end())
                                                    })
                                                    .collect::<Vec<_>>(),
                                                &format!("{} {}", s.name, surfer_info.bus_name),
                                                s.color,
                                            );
                                            surfer_integration::zoom_to_range(
                                                period_start as u64,
                                                period_end as u64,
                                            );
                                            ui.close();
                                        }
                                    });
                                });
                            }
                        }
                    });
                });
            });
        }
    } else {
        pub fn surfer_ui_timeline(_plot_ui: &PlotUi, _surfer_info: &SurferData, _waveform_time_unit: &TimescaleUnit, _timeline: &mut TimelinePlot, _all_statistics: &[Statistic]) {}
    }
}

cfg_if! {
    if #[cfg(feature = "surfer")] {
        use crate::egui_visualization::PlotScale;
        pub fn surfer_ui_buckets(response: &Response, buckets: &BucketsPlot, barcharts: HashMap<Id, &BucketsStatistic>, surfer: &SurferData) {
            let BucketsPlot { scale, selected } = buckets;
            response.context_menu(|ui| {
                    match selected {
                        Some((selected, id)) => {
                            if ui.button("open in surfer").clicked() {
                                let buckets_statistic = barcharts[id];
                                let data = match scale {
                                    PlotScale::Log => buckets_statistic.get_data_for_bucket(*selected),
                                    PlotScale::Lin => buckets_statistic.get_data_of_value(*selected),
                                };
                                let signals = surfer.signals.clone();
                                let periods = data
                                    .iter()
                                    .map(|period| (period.start(), period.end()))
                                    .collect::<Vec<_>>();
                                let suffix = format!("{} {}", buckets_statistic.name, surfer.bus_name);
                                let color = buckets_statistic.color;
                                let action = move |trace_path: &str| {
                                    let periods = periods;
                                    surfer_integration::open_and_mark_periods(
                                        trace_path, signals, &periods, &suffix, color,
                                    );
                                };
                                if ensure_trace_matches(
                                    &surfer.trace_path,
                                    &mut surfer.surfer.borrow_mut(),
                                ) {
                                    action(&surfer.trace_path.path);
                                } else {
                                    surfer.surfer.borrow_mut().modal_action = Some(Box::new(action));
                                }
                            }
                        }
                        None => {
                            // if the user clicks outside of barchart, we still create a button
                            // that will immediately get destroyed to not break the UI layout
                            let _ = ui.button("open in surfer");
                            ui.close()
                        }
                    }
                });

        }
    } else {
        pub fn surfer_ui_buckets(_response: &Response, _buckets: &BucketsPlot, _barcharts: HashMap<Id, &BucketsStatistic>, _surfer: &SurferData) {}
    }
}
