#![cfg(target_arch = "wasm32")]

mod app;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen(start)]
pub fn run() {
    yew::start_app::<app::Greet>();
    log("Hello from the console!");
}
