use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(message: &str);
}

#[xtask_wasm::run_example(static_dir = "webapp/static", app_name = "web_app")]
fn run_app() {
    log("Hello World!");
}
