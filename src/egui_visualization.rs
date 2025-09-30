use std::{collections::HashMap, f32};

use eframe::{
    egui::{self, Color32, Id, Label, Layout, PopupAnchor, Rgba, Shape, Stroke, Ui, vec2},
    epaint::Hsva,
};
use egui_plot::{
    Bar, BarChart, ClosestElem, Legend, Line, Plot, PlotItem, PlotPoint, PlotPoints, Polygon,
};
use wellen::TimescaleUnit;

use crate::{BusUsage, analyzer::Analyzer, bus_usage::Statistic, surfer_integration};

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
    selected: Option<(usize, Id)>,
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
            PlotType::Timeline(_) => write!(f, "Timescale"),
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
        Self {
            analyzers,
            selected: 0,
            trace_path: trace_path.to_owned(),
            waveform_time_unit: time_unit,
            left: PlotType::Buckets(BucketsPlot::new(PlotScale::Log)),
            right: PlotType::Timeline(TimelinePlot::new(time_unit, None)),
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
                    let b = ui.button(a.get_results().expect("Already calculated").get_name());
                    if b.clicked() {
                        self.selected = i;
                    }
                    if i == self.selected {
                        b.highlight();
                    }
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
            ui.heading(result.get_name());
            draw_statistics(
                ui,
                result,
                self.selected,
                &self.trace_path,
                self.analyzers[self.selected].get_signals(),
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

fn draw_values(ui: &mut Ui, statistics: &[Statistic]) {
    ui.allocate_ui(vec2(ui.available_size_before_wrap().x, 20.0), |ui| {
        ui.with_layout(
            Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
            |ui| {
                for statistic in statistics.iter() {
                    ui.allocate_ui(vec2(300.0, 40.0), |ui| {
                        egui::Frame::default()
                            .inner_margin(12)
                            .stroke(egui::Stroke::new(2.0, Color32::GRAY))
                            .show(ui, |ui| {
                                let (display, description) = match statistic {
                                    Statistic::Percentage(percentage_statistic) => (
                                        percentage_statistic.display(),
                                        percentage_statistic.description,
                                    ),
                                    Statistic::Bucket(buckets_statistic) => {
                                        (buckets_statistic.display(), buckets_statistic.description)
                                    }
                                    Statistic::Timeline(timeline_statistic) => (
                                        timeline_statistic.display.to_string(),
                                        timeline_statistic.description,
                                    ),
                                };
                                ui.add_sized(
                                    vec2(10.0, 30.0),
                                    Label::new(
                                        egui::RichText::new(display)
                                            .font(egui::FontId::proportional(16.0)),
                                    ),
                                )
                                .on_hover_ui(|ui| {
                                    ui.label(description);
                                });
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
                ui.selectable_value(type_, PlotType::Pie, "Pie");
                ui.selectable_value(
                    type_,
                    PlotType::Buckets(BucketsPlot::new(PlotScale::Log)),
                    "Buckets",
                );
                ui.selectable_value(
                    type_,
                    PlotType::Timeline(TimelinePlot::new(*waveform_time_unit, None)),
                    "Timeline",
                );
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
    if i < 2.0 {
        format!("{}", i)
    } else if i >= 41.0 {
        format!("2^{}+", i)
    } else if i >= 21.0 {
        let i = i as u32 - 20;
        format!("{}-{}M", 1 << (i - 1), 1 << i)
    } else if i >= 11.0 {
        let i = i as u32 - 10;
        format!("{}-{}k", 1 << (i - 1), 1 << i)
    } else {
        let i = i as u32 - 1;
        format!("{}-{}", 1u64 << i, (1 << (i + 1)) - 1)
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
    let salt = scale as *mut PlotScale as usize;
    let mut statistics = statistics
        .iter()
        .filter_map(|s| match s {
            Statistic::Percentage(_) => None,
            Statistic::Bucket(buckets_statistic) => Some(buckets_statistic),
            Statistic::Timeline(_) => None,
        })
        .peekable();
    if statistics.peek().is_none() {
        return;
    }
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
            Plot::new(("buckets", salt))
        }
        .legend(Legend::default())
        .width(width)
        .show(ui, |plot_ui| {
            let mut barcharts = HashMap::new();
            for buckets_statistic in statistics {
                barcharts.insert(Id::new(buckets_statistic.name), buckets_statistic);
                plot_ui.bar_chart(if *scale == PlotScale::Log {
                    BarChart::new(
                        buckets_statistic.name,
                        buckets_statistic
                            .get_buckets()
                            .into_iter()
                            .enumerate()
                            .map(|(i, bucket)| Bar::new(i as f64, bucket as f64))
                            .collect::<Vec<_>>(),
                    )
                    .element_formatter(Box::new(|bar, _| {
                        format!("{}: {}", format_bucket_label(bar.argument), bar.value)
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
                });
            }
            (plot_ui.pointer_coordinate(), barcharts)
        });
        let (coords, barcharts) = response.inner;
        if response.response.secondary_clicked() {
            if let Some(id) = response.hovered_plot_item
                && let Some(coords) = coords
            {
                *selected = Some((coords.x.round() as usize, id));
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
                            PlotScale::Lin => buckets_statistic.get_data_of_value(*selected as u32),
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
    let statistics = statistics.iter().filter_map(|s| match s {
        Statistic::Percentage(_) => None,
        Statistic::Bucket(_) => None,
        Statistic::Timeline(timeline_statistic) => Some(timeline_statistic),
    });
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
            .width(width)
            .show(ui, |plot_ui| {
                for statistic in statistics {
                    plot_ui.line(Line::new(
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
                    ));
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
    name: &'static str,
    value: f32,
    points: Vec<[f64; 2]>,
    color: Color32,
    polygon: Polygon<'a>,
}

impl<'a> PieSlice<'a> {
    fn new(
        polygon: Polygon<'a>,
        points: Vec<[f64; 2]>,
        color: Color32,
        name: &'static str,
        value: f32,
    ) -> Self {
        let color = Rgba::from(color).to_opaque().multiply(0.4);
        Self {
            points,
            color: color.into(),
            polygon,
            name,
            value,
        }
    }
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

impl<'a> PlotItem for PieSlice<'a> {
    fn shapes(&self, ui: &Ui, transform: &egui_plot::PlotTransform, shapes: &mut Vec<egui::Shape>) {
        self.polygon.shapes(ui, transform, shapes);
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
        self.polygon.bounds()
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
        let end = self.points.last().copied().unwrap();

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

    fn on_hover(
        &self,
        plot_area_response: &egui::Response,
        _elem: egui_plot::ClosestElem,
        shapes: &mut Vec<egui::Shape>,
        _cursors: &mut Vec<egui_plot::Cursor>,
        plot: &egui_plot::PlotConfig<'_>,
        _label_formatter: &egui_plot::LabelFormatter<'_>,
    ) {
        shapes.push(Shape::convex_polygon(
            self.points
                .iter()
                .map(|&[x, y]| plot.transform.position_from_point(&PlotPoint { x, y }))
                .collect(),
            self.color,
            Stroke::new(2.0, self.color),
        ));
        let mut tooltip = egui::Tooltip::always_open(
            plot_area_response.ctx.clone(),
            plot_area_response.layer_id,
            plot_area_response.id,
            PopupAnchor::Pointer,
        );

        let tooltip_width = plot_area_response.ctx.style().spacing.tooltip_width;

        tooltip.popup = tooltip.popup.width(tooltip_width);

        tooltip.gap(12.0).show(|ui| {
            ui.set_max_width(tooltip_width);
            ui.label(format!("{}: {} clock cycles", self.name, self.value));
        });
    }
}

fn draw_percentage(ui: &mut Ui, statistics: &[Statistic], width: f32, type_: &PlotType) {
    let salt = type_ as *const PlotType as usize;
    let statistics = statistics.iter().filter_map(|s| match s {
        Statistic::Percentage(percentage) => Some(percentage),
        Statistic::Bucket(_) => None,
        Statistic::Timeline(_) => None,
    });
    Plot::new(("percentage", salt))
        .legend(Legend::default())
        .show_axes(false)
        .allow_scroll(false)
        .width(width)
        .show(ui, |plot_ui| {
            for statistic in statistics {
                let sum: f64 = statistic.data_labels.iter().map(|(d, _)| *d as f64).sum();
                let points_for_circle = 100;
                let max_angle_for_triangle = std::f64::consts::PI * 2.0 / points_for_circle as f64;
                let mut last_angle: f64 = 0.0;
                for (i, (d, l)) in statistic.data_labels.iter().enumerate() {
                    let d = *d as f64;
                    let mut points = vec![[0.0, 0.0]];
                    points.push([last_angle.sin(), last_angle.cos()]);

                    let part_for_stat = d / sum * std::f64::consts::PI * 2.0;
                    let mut tmp = part_for_stat;
                    let mut angle_counter = last_angle;
                    while tmp > max_angle_for_triangle {
                        angle_counter += max_angle_for_triangle;
                        points.push([angle_counter.sin(), angle_counter.cos()]);
                        tmp -= max_angle_for_triangle;
                    }
                    last_angle += part_for_stat;
                    points.push([last_angle.sin(), last_angle.cos()]);

                    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0;
                    let h = i as f32 * golden_ratio;
                    let color = Hsva::new(h, 0.85, 0.5, 1.0);
                    let polygon = Polygon::new(*l, points.clone()).stroke(Stroke::new(1.0, color));
                    plot_ui.add(PieSlice::new(polygon, points, color.into(), l, d as f32));
                }
            }
        });
}
