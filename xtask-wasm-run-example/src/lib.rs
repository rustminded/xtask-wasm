use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse, parse_macro_input};

/// This macro helps to run an example in the project's `examples/` directory using a development
/// server.
///
/// The macro expands into:
///
/// * A `#[wasm_bindgen(start)]` entry-point (compiled only for `wasm32`) that runs your function
///   body.
/// * A native `main` (compiled for all other targets) with `dist` and `start` subcommands, so
///   `cargo run --example my_example` automatically builds and serves the Wasm bundle without any
///   separate `xtask/` crate.
///
/// # Usage
///
/// ## Minimal example (no arguments)
///
/// When no arguments are passed the macro auto-generates a minimal `index.html` that loads
/// `app.js`. This is the recommended approach for most projects.
///
/// * `examples/my_example.rs`:
///
///   ```rust,ignore
///   use wasm_bindgen::prelude::*;
///
///   #[wasm_bindgen]
///   extern "C" {
///       #[wasm_bindgen(js_namespace = console)]
///       fn log(s: &str);
///   }
///
///   #[xtask_wasm::run_example]
///   fn run_app() {
///       log("Hello from Wasm!");
///   }
///   ```
///
/// * `Cargo.toml` (dev-dependency, wasm32 target only so native builds stay clean):
///
///   ```toml
///   [target.'cfg(target_arch = "wasm32")'.dev-dependencies]
///   xtask-wasm = { version = "*", features = ["run-example"] }
///   ```
///
/// * Run the development server:
///
///   ```console
///   cargo run --example my_example
///   ```
///
/// ## egui / eframe example
///
/// [eframe](https://crates.io/crates/eframe) builds on top of `wasm_bindgen` and uses
/// `web_sys::HtmlCanvasElement` directly — no canvas ID string. The pattern below is compatible
/// with eframe 0.29+.
///
/// * `examples/webapp.rs`:
///
///   ```rust,ignore
///   use eframe::wasm_bindgen::JsCast as _;
///
///   #[xtask_wasm::run_example]
///   fn run() {
///       // Create a <canvas> element dynamically — no index.html canvas needed.
///       let document = web_sys::window().unwrap().document().unwrap();
///       let body = document.body().unwrap();
///       let canvas = document
///           .create_element("canvas").unwrap()
///           .dyn_into::<web_sys::HtmlCanvasElement>().unwrap();
///       canvas.set_id("the_canvas_id");
///       canvas.set_attribute("style", "width:100%;height:100%").unwrap();
///       body.append_child(&canvas).unwrap();
///
///       let runner = eframe::WebRunner::new();
///       // `spawn_local` drives the async start() future on the wasm32 executor.
///       wasm_bindgen_futures::spawn_local(async move {
///           runner
///               .start(canvas, eframe::WebOptions::default(), Box::new(|_cc| {
///                   Ok(Box::new(MyApp::default()))
///               }))
///               .await
///               .expect("failed to start eframe");
///       });
///   }
///   ```
///
/// * `Cargo.toml`:
///
///   ```toml
///   [target.'cfg(target_arch = "wasm32")'.dev-dependencies]
///   xtask-wasm = { version = "*", features = ["run-example"] }
///   eframe = { version = "0.29", default-features = false, features = ["glow"] }
///   wasm-bindgen-futures = "0.4"
///   web-sys = { version = "0.3", features = ["HtmlCanvasElement", "Document", "Window", "Element", "HtmlElement"] }
///   ```
///
/// ## Arguments
///
/// You can give arguments to the macro to customise the example:
///
/// * `app_name` - Override the app name used by [`xtask_wasm::Dist`].
/// * `index` - Provide the full content of a custom `index.html` as a string expression.
/// * `assets_dir` - Path to a custom assets directory.
///
/// > **Warning — `app_name` / `assets_dir` suppress the auto-generated `index.html`.**
/// >
/// > When either `app_name` or `assets_dir` is set (and `index` is not), the macro skips writing
/// > `index.html` into the dist directory. You must then supply your own `index.html` — either via
/// > the `index` argument or by placing it in the `assets_dir`. Forgetting this results in a blank
/// > page with no errors, which can be confusing. If you don't need a custom app name or assets
/// > directory, omit all arguments so the macro generates a working HTML page automatically.
#[proc_macro_attribute]
pub fn run_example(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(item as syn::ItemFn);
    let attr = parse_macro_input!(attr with RunExample::parse);

    attr.generate(item)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

struct RunExample {
    index: Option<syn::Expr>,
    assets_dir: Option<syn::Expr>,
    app_name: Option<syn::Expr>,
}

impl RunExample {
    fn parse(input: parse::ParseStream) -> parse::Result<Self> {
        let mut index = None;
        let mut assets_dir = None;
        let mut app_name = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _eq_token: syn::Token![=] = input.parse()?;
            let expr: syn::Expr = input.parse()?;

            match ident.to_string().as_str() {
                "index" => index = Some(expr),
                "assets_dir" => assets_dir = Some(expr),
                "app_name" => app_name = Some(expr),
                _ => return Err(parse::Error::new(ident.span(), "unrecognized argument")),
            }

            let _comma_token: syn::Token![,] = match input.parse() {
                Ok(x) => x,
                Err(_) if input.is_empty() => break,
                Err(err) => return Err(err),
            };
        }

        Ok(RunExample {
            index,
            assets_dir,
            app_name,
        })
    }

    fn generate(self, item: syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
        let fn_block = item.block;

        let index = if let Some(expr) = &self.index {
            quote_spanned! { expr.span()=> std::fs::write(dist_dir.join("index.html"), #expr)?; }
        } else if self.assets_dir.is_some() || self.app_name.is_some() {
            quote! {}
        } else {
            quote! {
                std::fs::write(
                    dist_dir.join("index.html"),
                    r#"<!DOCTYPE html><html><head><meta charset="utf-8"/><script type="module">import init from "./app.js";init();</script></head><body></body></html>"#,
                )?;
            }
        };

        let app_name = if let Some(expr) = &self.app_name {
            quote_spanned! { expr.span()=> .app_name(#expr) }
        } else {
            quote! {}
        };

        let assets_dir = if let Some(expr) = self.assets_dir {
            quote_spanned! { expr.span()=> .assets_dir(#expr) }
        } else {
            quote! {}
        };

        #[cfg(feature = "wasm-opt")]
        let optimize_wasm = quote! { .optimize_wasm(xtask_wasm::WasmOpt::level(1).shrink(2)) };
        #[cfg(not(feature = "wasm-opt"))]
        let optimize_wasm = quote! {};

        Ok(quote! {
            #[cfg(target_arch = "wasm32")]
            pub mod xtask_wasm_run_example {
                use super::*;
                use xtask_wasm::wasm_bindgen;

                #[xtask_wasm::wasm_bindgen::prelude::wasm_bindgen(start)]
                pub fn run_app() -> Result<(), xtask_wasm::wasm_bindgen::JsValue> {
                    xtask_wasm::console_error_panic_hook::set_once();

                    #fn_block

                    Ok(())
                }
            }

            #[cfg(not(target_arch = "wasm32"))]
            fn main() -> xtask_wasm::anyhow::Result<()> {
                use xtask_wasm::{env_logger, log, clap};

                #[derive(clap::Parser)]
                struct Cli {
                    #[clap(subcommand)]
                    command: Option<Command>,
                }

                #[derive(clap::Parser)]
                enum Command {
                    Dist(xtask_wasm::Dist),
                    Start(xtask_wasm::DevServer),
                }

                env_logger::builder()
                    .filter(Some(module_path!()), log::LevelFilter::Info)
                    .filter(Some("xtask"), log::LevelFilter::Info)
                    .init();

                let cli: Cli = clap::Parser::parse();

                match cli.command {
                    Some(Command::Dist(mut dist)) => {
                        let dist_dir = dist
                            .example(module_path!())
                            #app_name
                            #assets_dir
                            #optimize_wasm
                            .build(env!("CARGO_PKG_NAME"))?;

                        #index

                        Ok(())
                    }
                    Some(Command::Start(dev_server)) => {
                        dev_server.xtask("dist").start()
                    }
                    None => {
                        let dev_server: xtask_wasm::DevServer = clap::Parser::parse();
                        dev_server.xtask("dist").start()
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            fn main() {}
        })
    }
}
