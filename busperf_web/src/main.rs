use std::{cell::RefCell, rc::Rc};

use busperf::show::egui_visualization::BusperfApp;
use eframe::egui::{self, Ui};

struct Test {
    bp: Rc<RefCell<Option<BusperfApp>>>,
}

impl Test {
    fn new() -> Self {
        Self {
            bp: Rc::new(RefCell::new(None)),
        }
    }
}

use wasm_bindgen::prelude::*;

static mut DATA: Vec<u8> = Vec::new();

#[wasm_bindgen]
pub fn set_busperf_data(data: Vec<u8>) {
    unsafe {
        DATA = data;
    }
}

impl eframe::App for Test {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        if let Some(file) = ctx.input(|input| input.raw.dropped_files.first().map(|f| f.clone())) {
            let name = &file.name;
            web_sys::console::log_1(&name.into());
            if let Some(data) = file.bytes
                && let Ok(a) = BusperfApp::build_from_bytes(&*data)
            {
                *self.bp.borrow_mut() = Some(a);
            }
        }
        if let Some(app) = &mut *self.bp.borrow_mut() {
            app.update(ctx, frame);
        } else {
            unsafe {
                #[allow(static_mut_refs)]
                let foo = if !DATA.is_empty() { true } else { false };
                if foo {
                    web_sys::console::log_1(&"LOADED DATA".into());
                    #[allow(static_mut_refs)]
                    if let Ok(a) = BusperfApp::build_from_bytes(&DATA) {
                        *self.bp.borrow_mut() = Some(a);
                    }
                    ctx.request_repaint();
                }
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(ui.available_height() / 3.0);
                ui.with_layout(
                    egui::Layout::default()
                        .with_cross_align(egui::Align::Center)
                        .with_main_align(egui::Align::Center),
                    |ui| {
                        ui.label(egui::RichText::new("Select or drop a busperf file").size(25.0));
                        ui.add_space(50.0);
                        if ui
                            .add_sized([100.0, 60.0], |ui: &mut Ui| {
                                ui.button(egui::RichText::new("Select").size(20.0))
                            })
                            .clicked()
                        {
                            let ctx = ctx.clone();
                            let app = self.bp.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Some(file) = rfd::AsyncFileDialog::new().pick_file().await {
                                    let data = file.read().await;
                                    unsafe {
                                        DATA = data;
                                    }
                                    // if let Ok(a) = BusperfApp::build_from_bytes(&data) {
                                    //     *app.borrow_mut() = Some(a);
                                    // }
                                    // ctx.request_repaint();
                                };
                            });
                        }
                    },
                );
            });
        }
    }
}
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|_| Ok(Box::new(Test::new()))))
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
