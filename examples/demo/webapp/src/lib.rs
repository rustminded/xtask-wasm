#![cfg(target_arch = "wasm32")]

use egui_screensaver_fractal_clock::FractalClockBackground;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Default)]
struct DemoApp {
    screensaver: FractalClockBackground,
}

impl eframe::App for DemoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(ctx.viewport_rect(), 0.0, egui::Color32::BLACK);
        self.screensaver.paint(&ctx);
    }
}

#[wasm_bindgen(start)]
pub fn run() {
    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");

        let canvas = document
            .create_element("canvas")
            .expect("failed to create canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("created element is not a canvas");

        canvas
            .set_attribute("class", "screensaver")
            .expect("failed to set canvas class");

        document
            .body()
            .expect("no document body")
            .append_child(&canvas)
            .expect("failed to append canvas element");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(DemoApp::default()))),
            )
            .await
            .expect("failed to start eframe");
    });
}
