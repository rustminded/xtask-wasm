use quote::quote;
use syn::{parse, parse_macro_input};

/// This macro helps to run an example in the project's `examples/` directory using a development
/// server.
///
/// # Usage
///
/// * In the file `examples/my_example.rs`, create your example:
///
///   ```rust,ignore
///   use wasm_bindgen::prelude::*;
///
///   #[wasm_bindgen]
///   extern "C" {
///       #[wasm_bindgen(js_namespace = console)]
///       fn log(message: &str);
///   }
///
///   #[xtask_wasm::run_example]
///   fn run_app() {
///       log::("Hello World!");
///   }
///   ```
///
/// * In the project's `Cargo.toml`:
///
///   ```toml
///   [dev-dependencies]
///   xtask-wasm = { version = "*", features = ["run-example"] }
///   ```
///
/// * Then to run the development server with the example:
///
///     ```console
///     cargo run --example my_example
///     ```
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
    static_dir: Option<syn::Expr>,
    app_name: Option<syn::Expr>,
}

impl RunExample {
    fn parse(input: parse::ParseStream) -> parse::Result<Self> {
        let mut index = None;
        let mut static_dir = None;
        let mut app_name = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _eq_token: syn::Token![=] = input.parse()?;
            let expr: syn::Expr = input.parse()?;

            match ident.to_string().as_str() {
                "index" => index = Some(expr),
                "static_dir" => static_dir = Some(expr),
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
            static_dir,
            app_name,
        })
    }

    fn generate(self, item: syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
        let fn_block = item.block;

        let index = if let Some(expr) = &self.index {
            quote! {
                std::fs::write(
                    dist_dir.join("index.html"),
                    #expr
                )
            }
        } else if let Some(_) = &self.static_dir {
            quote! {}
        } else {
            quote! {
                std::fs::write(
                    dist_dir.join("index.html"),
                    r#"<!DOCTYPE html><html><head><meta charset="utf-8"/><script type="module">import init from "/app.js";init(new URL('app.wasm', import.meta.url));</script></head><body></body></html>"#
                )
            }
        };

        let app_name = if let Some(expr) = &self.app_name {
            quote! { .app_name(#expr) }
        } else {
            quote! {}
        };

        let static_dir = if let Some(expr) = self.static_dir {
            quote! { .static_dir_path(#expr) }
        } else {
            quote! {}
        };

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
                let mut dist_command = xtask_wasm::xtask_command();
                dist_command.arg("dist");

                match cli.command {
                    Some(Command::Dist(mut dist)) => {
                        let xtask_wasm::DistResult { dist_dir, .. } = dist
                            .example(module_path!())
                            #app_name
                            #static_dir
                            .run(env!("CARGO_PKG_NAME"))?;

                            #index

                        Ok(())
                    }
                    Some(Command::Start(dev_server)) => {
                        let served_path = xtask_wasm::default_dist_dir(false);
                        dev_server.command(dist_command).start(served_path)
                    }
                    None => {
                        let dev_server: xtask_wasm::DevServer = clap::Parser::parse();
                        let served_path = xtask_wasm::default_dist_dir(false);
                        dev_server.command(dist_command).start(served_path)
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            fn main() {}
        })
    }
}
