use std::{cell::RefCell, collections::HashMap, f32};

use eframe::{
    egui::{
        self, Color32, FontId, Id, Label, Layout, Rgba, RichText, Stroke, Ui,
        text::{LayoutJob, TextWrapping},
        vec2,
    },
    epaint::Hsva,
};
use egui_plot::{
    Bar, BarChart, ClosestElem, Legend, Line, Plot, PlotItem, PlotPoint, PlotPoints, Polygon, Text,
    uniform_grid_spacer,
};
use wellen::TimescaleUnit;

use crate::{
    analyzer::Analyzer,
    bus::CyclesNum,
    bus_usage::{BusUsage, Statistic},
    surfer_integration,
};

pub fn run_visualization(
    analyzers: Vec<Box<dyn Analyzer>>,
    trace_path: &str,
    time_unit: TimescaleUnit,
) {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "busperf",
        options,
        Box::new(|_| Ok(Box::new(BusperfApp::new(analyzers, trace_path, time_unit)))),
    )
    .expect("Failed to init egui");
}

#[derive(PartialEq)]
enum PlotScale {
    Log,
    Lin,
}

#[derive(PartialEq)]
enum PlotType {
    Pie,
    Buckets(BucketsPlot),
    Timeline(TimelinePlot),
}

#[derive(PartialEq)]
struct BucketsPlot {
    scale: PlotScale,
    selected: Option<(CyclesNum, Id)>,
}

impl BucketsPlot {
    fn new(scale: PlotScale) -> Self {
        Self {
            scale,
            selected: None,
        }
    }
}

#[derive(PartialEq)]
struct TimelinePlot {
    timescale_unit: TimescaleUnit,
    pointer: Option<PlotPoint>,
}

impl TimelinePlot {
    pub fn new(timescale_unit: TimescaleUnit, pointer: Option<PlotPoint>) -> Self {
        Self {
            timescale_unit,
            pointer,
        }
    }
}

impl std::fmt::Display for PlotType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlotType::Pie => write!(f, "Pie"),
            PlotType::Buckets(_) => write!(f, "Buckets"),
            PlotType::Timeline(_) => write!(f, "Timeline"),
        }
    }
}

struct BusperfApp {
    analyzers: Vec<Box<dyn Analyzer>>,
    selected: usize,
    trace_path: String,
    waveform_time_unit: wellen::TimescaleUnit,
    left: PlotType,
    right: PlotType,
}

impl BusperfApp {
    fn new(analyzers: Vec<Box<dyn Analyzer>>, trace_path: &str, time_unit: TimescaleUnit) -> Self {
        let right = if matches!(
            analyzers[0]
                .get_results()
                .expect("Should be already calculated"),
            BusUsage::SingleChannel(_)
        ) {
            PlotType::Pie
        } else {
            PlotType::Timeline(TimelinePlot::new(time_unit, None))
        };
        Self {
            analyzers,
            selected: 0,
            trace_path: trace_path.to_owned(),
            waveform_time_unit: time_unit,
            left: PlotType::Buckets(BucketsPlot::new(PlotScale::Log)),
            right,
        }
    }
}

impl eframe::App for BusperfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::new(egui::panel::Side::Left, "bus_selector").show(ctx, |ui| {
            ui.heading("Bus");
            ui.separator();
            for (i, a) in self.analyzers.iter().enumerate() {
                ui.with_layout(egui::Layout::default().with_cross_justify(true), |ui| {
                    let name = a.get_results().expect("Already calculated").get_name();
                    let mut job = LayoutJob::simple_singleline(
                        name.to_string(),
                        FontId::proportional(12.0),
                        Color32::LIGHT_GRAY,
                    );
                    job.wrap = TextWrapping {
                        max_width: ui.available_width(),
                        max_rows: 1,
                        break_anywhere: true,
                        ..Default::default()
                    };
                    let text = ui.fonts(|f| f.layout_job(job));
                    ui.selectable_value(&mut self.selected, i, text)
                        .on_hover_text(name);
                });
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.selected = (self.selected + 1) % self.analyzers.len();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                if self.selected > 0 {
                    self.selected -= 1;
                } else {
                    self.selected = self.analyzers.len() - 1;
                }
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            let result = self.analyzers[self.selected]
                .get_results()
                .expect("Already calculated");
            ui.heading(RichText::new(result.get_name()).color(Color32::WHITE));
            ui.separator();
            draw_statistics(
                ui,
                result,
                self.selected,
                &self.trace_path,
                self.analyzers[self.selected]
                    .get_signals()
                    .iter()
                    .map(|s| format!("{s}"))
                    .collect(),
                &self.waveform_time_unit,
                (&mut self.left, &mut self.right),
            );
        });
    }
}

struct SurferInfo<'a, 'b, 'c> {
    trace_path: &'a str,
    signals: &'b Vec<String>,
    bus_name: &'c str,
}

fn draw_statistics(
    ui: &mut Ui,
    result: &BusUsage,
    id: usize,
    trace_path: &str,
    signals: Vec<String>,
    waveform_time_unit: &TimescaleUnit,
    (left, right): (&mut PlotType, &mut PlotType),
) {
    let statistics = result.get_statistics();
    let surfer_info = SurferInfo {
        trace_path,
        signals: &signals,
        bus_name: result.get_name(),
    };
    draw_values(ui, &statistics);
    let size = ui.available_size();
    ui.horizontal(|ui| {
        ui.set_min_size(size);
        let width = ui.available_width() / 2.0;
        draw_plot(
            ui,
            &statistics,
            id * 2,
            &surfer_info,
            waveform_time_unit,
            left,
            width,
        );
        draw_plot(
            ui,
            &statistics,
            id * 2 + 1,
            &surfer_info,
            waveform_time_unit,
            right,
            width,
        );
    });
}

thread_local! {
    static COLORS: RefCell<HashMap<usize, Color32>> = RefCell::new(HashMap::new());
}

fn set_color(i: usize, color: Color32) {
    COLORS.with_borrow_mut(|colors| {
        colors.insert(i, color);
    })
}

fn get_color(i: usize) -> Color32 {
    COLORS.with_borrow(|colors| {
        if let Some(color) = colors.get(&i) {
            *color
        } else {
            let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
            let h = i as f32 * golden_ratio;
            let color = Hsva::new(h, 0.85, 0.5, 1.0);
            color.into()
        }
    })
}

/// Estimates size that displayed statistic will take up in the UI
fn estimate_size(statistic: &Statistic) -> f32 {
    match statistic {
        Statistic::Percentage(percentage_statistic) => {
            percentage_statistic
                .data_labels
                .iter()
                .map(|&(_, l)| l.len())
                .sum::<usize>() as f32
                * 20.0
        }
        Statistic::Bucket(buckets_statistic) => buckets_statistic.display().len() as f32 * 8.0,
        Statistic::Timeline(timeline_statistic) => timeline_statistic.display.len() as f32 * 10.0,
    }
}

fn draw_values(ui: &mut Ui, statistics: &[Statistic]) {
    ui.allocate_ui(vec2(ui.available_size_before_wrap().x, 20.0), |ui| {
        ui.with_layout(
            Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                for (stat_id, statistic) in statistics.iter().enumerate() {
                    let size = estimate_size(statistic);
                    ui.allocate_ui(vec2(size, 40.0), |ui| {
                        let frame =
                            egui::Frame::default()
                                .inner_margin(12)
                                .stroke(egui::Stroke::new(2.0, Color32::GRAY))
                                .show(ui, |ui| {
                                    match statistic {
                                        Statistic::Percentage(percentage_statistic) => {
                                            ui.add_sized(
                                                vec2(10.0, 30.0),
                                                Label::new(
                                                    egui::RichText::new(format!(
                                                        "{}:",
                                                        percentage_statistic.name
                                                    ))
                                                    .font(FontId::proportional(16.0)),
                                                ),
                                            );
                                            for (i, (d, l)) in
                                                percentage_statistic.data_labels.iter().enumerate()
                                            {
                                                // Because percentage statistics require more than one color we offset by some big number
                                                let color_id = (stat_id + 1) * 10000 + i;
                                                let mut color = get_color(color_id);
                                                egui::Frame::default()
                                                    .inner_margin(5)
                                                    .stroke(egui::Stroke::new(1.0, color))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(format!("{l}: {d}"))
                                                                .font(FontId::proportional(14.0))
                                                                .color(color),
                                                        )
                                                        .context_menu(
                                                            |ui| {
                                                                println!("FOO");
                                                                egui::widgets::color_picker::color_picker_color32(
                                                                    ui,
                                                                    &mut color,
                                                                    egui::color_picker::Alpha::Opaque,
                                                                );
                                                                set_color(color_id, color);
                                                            },
                                                        );
                                                    });
                                            }
                                            percentage_statistic.description
                                        }
                                        Statistic::Bucket(buckets_statistic) => {
                                            ui.add_sized(
                                                vec2(10.0, 30.0),
                                                Label::new(
                                                    egui::RichText::new(
                                                        buckets_statistic.display(),
                                                    )
                                                    .font(egui::FontId::proportional(16.0))
                                                    .color(get_color(stat_id)),
                                                ),
                                            )
                                            .context_menu(|ui| {
                                                let mut color = get_color(stat_id);
                                                egui::widgets::color_picker::color_picker_color32(
                                                    ui,
                                                    &mut color,
                                                    egui::color_picker::Alpha::Opaque,
                                                );
                                                set_color(stat_id, color);
                                            });
                                            buckets_statistic.description
                                        }
                                        Statistic::Timeline(timeline_statistic) => {
                                            ui.add_sized(
                                                vec2(10.0, 30.0),
                                                Label::new(
                                                    egui::RichText::new(
                                                        timeline_statistic.display.to_string(),
                                                    )
                                                    .font(egui::FontId::proportional(16.0))
                                                    .color(get_color(stat_id)),
                                                ),
                                            )
                                            .context_menu(|ui| {
                                                let mut color = get_color(stat_id);
                                                egui::widgets::color_picker::color_picker_color32(
                                                    ui,
                                                    &mut color,
                                                    egui::color_picker::Alpha::Opaque,
                                                );
                                                set_color(stat_id, color);
                                            });
                                            timeline_statistic.description
                                        }
                                    }
                                });
                        frame.response.on_hover_ui_at_pointer(|ui| {
                            ui.label(frame.inner);
                        });
                    });
                }
            },
        );
    });
}

fn draw_plot(
    ui: &mut Ui,
    statistics: &[Statistic],
    id: usize,
    surfer_info: &SurferInfo,
    waveform_time_unit: &TimescaleUnit,
    type_: &mut PlotType,
    width: f32,
) {
    ui.vertical(|ui| {
        let salt = type_ as *mut PlotType as u32;
        egui::ComboBox::new(salt, "Plot type")
            .selected_text(type_.to_string())
            .show_ui(ui, |ui| {
                let is_pie: Box<dyn Fn(&Statistic) -> bool> =
                    Box::new(|s| matches!(&s, Statistic::Percentage(_)));
                let is_bucket: Box<dyn Fn(&Statistic) -> bool> =
                    Box::new(|s| matches!(&s, Statistic::Bucket(_)));
                let is_timeline: Box<dyn Fn(&Statistic) -> bool> =
                    Box::new(|s| matches!(&s, Statistic::Timeline(_)));

                type ItemCreation = Box<dyn Fn(&mut Ui, bool, &mut PlotType)>;
                let pie_button: ItemCreation = Box::new(|ui: &mut Ui, active, type_| {
                    ui.selectable_value(
                        type_,
                        PlotType::Pie,
                        if active { "Pie" } else { "Pie - no data" },
                    );
                });
                let bucket_button: ItemCreation = Box::new(|ui: &mut Ui, active, type_| {
                    ui.selectable_value(
                        type_,
                        PlotType::Buckets(BucketsPlot::new(PlotScale::Log)),
                        if active {
                            "Buckets"
                        } else {
                            "Buckets - no data"
                        },
                    );
                });
                let waveform_time_unit = *waveform_time_unit;
                let timeline_buttom: ItemCreation = Box::new(move |ui: &mut Ui, active, type_| {
                    ui.selectable_value(
                        type_,
                        PlotType::Timeline(TimelinePlot::new(waveform_time_unit, None)),
                        if active {
                            "Timeline"
                        } else {
                            "Timeline - no data"
                        },
                    );
                });
                for (item, cond) in [
                    (pie_button, is_pie),
                    (bucket_button, is_bucket),
                    (timeline_buttom, is_timeline),
                ] {
                    if statistics.iter().any(cond) {
                        item(ui, true, type_);
                    } else {
                        ui.add_enabled_ui(false, |ui| {
                            item(ui, false, type_);
                        });
                    }
                }
            });
        match type_ {
            PlotType::Pie => draw_percentage(ui, statistics, width, type_),
            PlotType::Buckets(buckets) => {
                draw_buckets(ui, statistics, id, surfer_info, buckets, width)
            }
            PlotType::Timeline(timeline) => draw_timeline(
                ui,
                statistics,
                id,
                surfer_info,
                waveform_time_unit,
                timeline,
                width,
            ),
        };
    });
}

fn format_bucket_label(i: f64) -> String {
    match i {
        ..-40.0 => format!("-2^{}+", -i),
        -40.0..-20.0 => {
            let i = -(i as i32) - 20;
            format!("- {}-{}M", 1 << i, 1 << (i - 1))
        }
        -20.0..-10.0 => {
            let i = -(i as i32) - 10;
            format!("- {}-{}k", 1 << i, 1 << (i - 1))
        }
        -10.0..-1.0 => {
            let i = -(i as i32) - 1;
            format!("- {}-{}", (1 << (i + 1)) - 1, 1 << i)
        }
        -1.0..2.0 => format!("{i}"),
        2.0..11.0 => {
            let i = i as u32 - 1;
            format!("{}-{}", 1 << i, (1 << (i + 1)) - 1)
        }
        11.0..21.0 => {
            let i = i as u32 - 10;
            format!("{}-{}k", 1 << (i - 1), 1 << i)
        }
        21.0..41.0 => {
            let i = i as u32 - 20;
            format!("{}-{}M", 1 << (i - 1), 1 << i)
        }
        41.0.. => format!("2^{}+", i),
        _ => panic!("how did we get here {i}"),
    }
}

fn draw_buckets(
    ui: &mut Ui,
    statistics: &[Statistic],
    id: usize,
    surfer_info: &SurferInfo,
    buckets: &mut BucketsPlot,
    width: f32,
) {
    let BucketsPlot { scale, selected } = buckets;
    let statistics = statistics.iter().enumerate().filter_map(|(i, s)| match s {
        Statistic::Percentage(_) => None,
        Statistic::Bucket(buckets_statistic) => Some((i, buckets_statistic)),
        Statistic::Timeline(_) => None,
    });
    let statistics_num = statistics.clone().count();
    if statistics_num == 0 {
        ui.label(
            egui::RichText::new("There are no statistics to display on a barchart for this bus.")
                .color(Color32::RED),
        );
    }
    let grid_spacer = |input| {
        uniform_grid_spacer(|input| {
            let (min, max) = input.bounds;
            let view = (max - min).max(100.0) as usize;
            let multiplier = view.ilog10() as i32;
            [
                10.0f64.powi(multiplier),
                10.0f64.powi(multiplier - 1),
                10.0f64.powi(multiplier - 2),
            ]
        })(input)
    };
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label("Scale: ");
            ui.radio_value(scale, PlotScale::Log, "log");
            ui.radio_value(scale, PlotScale::Lin, "lin");
        });
        let response = if *scale == PlotScale::Log {
            Plot::new(("buckets", id)).x_axis_formatter(|marker, _| {
                let i = marker.value;
                format_bucket_label(i)
            })
        } else {
            Plot::new(("buckets", id))
        }
        .legend(Legend::default())
        .show_x(false)
        .x_axis_label("Value")
        .y_axis_label("Number of occurences")
        .cursor_color(Color32::TRANSPARENT)
        .x_grid_spacer(grid_spacer)
        .y_grid_spacer(grid_spacer)
        .width(width)
        .show(ui, |plot_ui| {
            let mut barcharts = HashMap::new();
            for (i, (stat_id, buckets_statistic)) in statistics.into_iter().enumerate() {
                barcharts.insert(Id::new(buckets_statistic.name), buckets_statistic);
                plot_ui.bar_chart(
                    if *scale == PlotScale::Log {
                        BarChart::new(
                            buckets_statistic.name,
                            buckets_statistic
                                .get_buckets()
                                .into_iter()
                                .map(|(bucket, value)| {
                                    let i = i as f64;
                                    let bar_width = 0.5 / statistics_num as f64;
                                    let start = bucket as f64 - 0.25 + 0.5 * bar_width;
                                    let offset = i * bar_width;
                                    Bar::new(start + offset, value as f64).width(bar_width)
                                })
                                .collect::<Vec<_>>(),
                        )
                        .element_formatter(Box::new(|bar, _| {
                            format!(
                                "{}: {}",
                                format_bucket_label(bar.argument.round()),
                                bar.value
                            )
                        }))
                    } else {
                        BarChart::new(
                            buckets_statistic.name,
                            buckets_statistic
                                .get_data()
                                .into_iter()
                                .map(|(k, v)| Bar::new(k as f64, v as f64))
                                .collect::<Vec<_>>(),
                        )
                    }
                    .color(get_color(stat_id)),
                );
            }
            (plot_ui.pointer_coordinate(), barcharts)
        });
        let (coords, barcharts) = response.inner;
        if response.response.secondary_clicked() {
            if let Some(id) = response.hovered_plot_item
                && let Some(coords) = coords
            {
                *selected = Some((coords.x.round() as i32, id));
            } else {
                *selected = None;
            }
        }

        response.response.context_menu(|ui| {
            match selected {
                Some((selected, id)) => {
                    if ui.button("open in surfer").clicked() {
                        let buckets_statistic = barcharts[id];
                        let data = match scale {
                            PlotScale::Log => buckets_statistic.get_data_for_bucket(*selected),
                            PlotScale::Lin => buckets_statistic.get_data_of_value(*selected),
                        };
                        surfer_integration::open_and_mark_periods(
                            surfer_info.trace_path,
                            surfer_info.signals.clone(),
                            &data
                                .iter()
                                .map(|period| (period.start(), period.end()))
                                .collect::<Vec<_>>(),
                            &format!("{} {}", buckets_statistic.name, surfer_info.bus_name),
                            buckets_statistic.color,
                        );
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
    });
}

fn waveform_to_plot_time(
    value: f64,
    waveform_time_unit: &TimescaleUnit,
    plot_time_unit: &TimescaleUnit,
) -> f64 {
    let diff = plot_time_unit
        .to_exponent()
        .expect("Should always be valid") as i32
        - waveform_time_unit
            .to_exponent()
            .expect("Should always be valid") as i32;
    if diff > 0 {
        value / 10.0f64.powi(diff.abs())
    } else {
        value * 10.0f64.powi(diff.abs())
    }
}

fn plot_to_waveform_time(
    value: f64,
    waveform_time_unit: &TimescaleUnit,
    plot_time_unit: &TimescaleUnit,
) -> f64 {
    let diff = waveform_time_unit
        .to_exponent()
        .expect("Should always be valid") as i32
        - plot_time_unit
            .to_exponent()
            .expect("Should always be valid") as i32;
    if diff > 0 {
        value / 10.0f64.powi(diff.abs())
    } else {
        value * 10.0f64.powi(diff.abs())
    }
}

fn draw_timeline(
    ui: &mut Ui,
    statistics: &[Statistic],
    id: usize,
    surfer_info: &SurferInfo,
    waveform_time_unit: &TimescaleUnit,
    timeline: &mut TimelinePlot,
    width: f32,
) {
    let TimelinePlot {
        timescale_unit: plot_time_unit,
        pointer: coords,
    } = timeline;
    let mut statistics = statistics
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Statistic::Percentage(_) => None,
            Statistic::Bucket(_) => None,
            Statistic::Timeline(timeline_statistic) => Some((i, timeline_statistic)),
        })
        .peekable();
    if statistics.peek().is_none() {
        ui.label(
            egui::RichText::new("There are no statistics to display on a timeline for this bus.")
                .color(Color32::RED),
        );
    }
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label("Time unit: ");
            ui.radio_value(plot_time_unit, TimescaleUnit::Seconds, "s");
            ui.radio_value(plot_time_unit, TimescaleUnit::MilliSeconds, "ms");
            ui.radio_value(plot_time_unit, TimescaleUnit::MicroSeconds, "us");
            ui.radio_value(plot_time_unit, TimescaleUnit::NanoSeconds, "ns");
            ui.radio_value(plot_time_unit, TimescaleUnit::PicoSeconds, "ps");
        });
        Plot::new(("timeline", id))
            .legend(Legend::default())
            .x_axis_label("Time")
            .y_axis_label("Value")
            .width(width)
            .show(ui, |plot_ui| {
                for (stat_id, statistic) in statistics {
                    plot_ui.line(
                        Line::new(
                            statistic.name,
                            PlotPoints::from(
                                statistic
                                    .values
                                    .iter()
                                    .map(|&[x, y]| {
                                        [
                                            waveform_to_plot_time(
                                                x,
                                                waveform_time_unit,
                                                plot_time_unit,
                                            ),
                                            y,
                                        ]
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                        )
                        .color(get_color(stat_id)),
                    );
                }
                if plot_ui.response().secondary_clicked() {
                    *coords = plot_ui.pointer_coordinate();
                }
                plot_ui.response().context_menu(|ui| {
                    if ui.button("open in surfer").clicked() {
                        crate::surfer_integration::open_at_time(
                            surfer_info.trace_path,
                            surfer_info.signals.clone(),
                            plot_to_waveform_time(
                                coords.expect("Should be set by right click").x,
                                waveform_time_unit,
                                plot_time_unit,
                            ),
                        );
                    }
                });
            });
    });
}

struct PieSlice<'a> {
    points: Vec<[f64; 2]>,
    polygon: Polygon<'a>,
    text: Text,
}

impl<'a> PieSlice<'a> {
    fn new(
        i: usize,
        stat_id: usize,
        last_angle: &mut f64,
        value: f64,
        sum: f64,
        name: &'static str,
    ) -> Self {
        let points_for_circle = 100;
        let max_angle_for_triangle = std::f64::consts::PI * 2.0 / points_for_circle as f64;

        let mut points = vec![[0.0, 0.0]];
        points.push([last_angle.sin(), last_angle.cos()]);

        let part_for_stat = value / sum * std::f64::consts::PI * 2.0;
        let mut tmp = part_for_stat;
        let mut angle_counter = *last_angle;
        while tmp > max_angle_for_triangle {
            angle_counter += max_angle_for_triangle;
            points.push([angle_counter.sin(), angle_counter.cos()]);
            tmp -= max_angle_for_triangle;
        }
        *last_angle += part_for_stat;
        points.push([last_angle.sin(), last_angle.cos()]);

        // Because percentage statistics require more than one color we offset by some big number
        let color = get_color((stat_id + 1) * 10000 + i);

        let x = points.iter().map(|[x, _]| x).sum::<f64>()
            / (points.len() as f64 * 1.2 + 0.1 * (i % 3) as f64);
        let y = points.iter().map(|[_, y]| y).sum::<f64>()
            / (points.len() as f64 * 1.2 + 0.1 * (i % 3) as f64);
        let text = Text::new(
            format!("{} name", name),
            PlotPoint { x, y },
            egui::RichText::new(format!("{}: {} clock cycles", name, value))
                .font(egui::FontId::proportional(16.0)),
        )
        .color(Color32::from_gray(220));
        let polygon = Polygon::new(name, points.clone()).stroke(Stroke::new(1.0, color));

        let color = Rgba::from(color).to_opaque().multiply(0.4);
        let polygon = polygon.fill_color(color.to_opaque().multiply(0.5));
        Self {
            points,
            polygon,
            text,
        }
    }
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

impl<'a> PlotItem for PieSlice<'a> {
    fn shapes(&self, ui: &Ui, transform: &egui_plot::PlotTransform, shapes: &mut Vec<egui::Shape>) {
        self.polygon.shapes(ui, transform, shapes);
        self.text.shapes(ui, transform, shapes);
    }

    fn initialize(&mut self, x_range: std::ops::RangeInclusive<f64>) {
        self.polygon.initialize(x_range);
    }

    fn color(&self) -> Color32 {
        self.polygon.color()
    }

    fn geometry(&self) -> egui_plot::PlotGeometry<'_> {
        self.polygon.geometry()
    }

    fn bounds(&self) -> egui_plot::PlotBounds {
        let mut bounds = self.polygon.bounds();
        bounds.merge(&self.text.bounds());
        bounds
    }

    fn base(&self) -> &egui_plot::PlotItemBase {
        self.polygon.base()
    }

    fn base_mut(&mut self) -> &mut egui_plot::PlotItemBase {
        self.polygon.base_mut()
    }

    fn find_closest(
        &self,
        point: egui::Pos2,
        transform: &egui_plot::PlotTransform,
    ) -> Option<egui_plot::ClosestElem> {
        let point = transform.value_from_position(point);
        if point.x * point.x + point.y * point.y > 1.0 {
            return None;
        }
        let point = [point.x, point.y];
        let start = self.points[1];
        let end = self
            .points
            .last()
            .copied()
            .expect("There should be always some points");

        let start_to_point = dot(start, point);
        let end_to_point = dot(end, point);
        let start_to_end = dot(start, end);
        if start_to_end <= 0.0 {
            if start_to_point < 0.001 && end_to_point > -0.01 {
                Some(ClosestElem {
                    index: 0,
                    dist_sq: start_to_point.max(1.0 - end_to_point) as f32,
                })
            } else {
                None
            }
        } else if start_to_point < 0.001 || end_to_point > -0.01 {
            Some(ClosestElem {
                index: 0,
                dist_sq: start_to_point.max(1.0 - end_to_point) as f32,
            })
        } else {
            None
        }
    }
}

fn draw_percentage(ui: &mut Ui, statistics: &[Statistic], width: f32, type_: &PlotType) {
    let salt = type_ as *const PlotType as usize;
    let mut statistics = statistics
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Statistic::Percentage(percentage) => Some((i, percentage)),
            Statistic::Bucket(_) => None,
            Statistic::Timeline(_) => None,
        })
        .peekable();
    if statistics.peek().is_none() {
        ui.label(
            egui::RichText::new("There are no statistics to display on a piechart for this bus.")
                .color(Color32::RED),
        );
    }
    Plot::new(("percentage", salt))
        .legend(Legend::default())
        .show_axes(false)
        .show_grid(false)
        .allow_scroll(false)
        .allow_zoom(false)
        .allow_boxed_zoom(false)
        .allow_drag(false)
        .cursor_color(Color32::TRANSPARENT)
        .show_x(false)
        .show_y(false)
        .width(width)
        .show(ui, |plot_ui| {
            for (stat_id, statistic) in statistics {
                let sum: f64 = statistic.data_labels.iter().map(|(d, _)| *d as f64).sum();
                let mut last_angle: f64 = 0.0;
                for (i, (d, l)) in statistic.data_labels.iter().enumerate() {
                    if *d > 0.0 {
                        plot_ui.add(PieSlice::new(
                            i,
                            stat_id,
                            &mut last_angle,
                            *d as f64,
                            sum,
                            l,
                        ));
                    }
                }
            }
        });
}
