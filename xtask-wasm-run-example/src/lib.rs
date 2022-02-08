use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse, parse_macro_input};

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
}

impl RunExample {
    fn parse(input: parse::ParseStream) -> parse::Result<Self> {
        let mut index = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            let _eq_token: syn::Token![=] = input.parse()?;
            let expr: syn::Expr = input.parse()?;

            match ident.to_string().as_str() {
                "index" => index = Some(expr),
                _ => return Err(parse::Error::new(ident.span(), "unrecognized argument")),
            }

            let _comma_token: syn::Token![,] = match input.parse() {
                Ok(x) => x,
                Err(_) if input.is_empty() => break,
                Err(err) => return Err(err),
            };
        }

        Ok(RunExample { index })
    }

    fn generate(self, item: syn::ItemFn) -> syn::Result<proc_macro2::TokenStream> {
        let fn_block = item.block;
        let index = if let Some(expr) = self.index {
            let span = expr.span();
            quote_spanned! {span=> #expr }
        } else {
            quote! { r#"<!DOCTYPE html><html><head><meta charset="utf-8"/><script type="module">import init from "/app.js";init(new URL('app.wasm', import.meta.url));</script></head><body></body></html>"# }
        };

        Ok(quote! {
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
                let mut dist_command = std::process::Command::new(std::env::args().next().unwrap());
                dist_command.arg("dist");

                match cli.command {
                    Some(Command::Dist(mut dist)) => {
                        let xtask_wasm::DistResult { dist_dir, .. } = dist
                            .example(module_path!())
                            .app_name("app")
                            .run(env!("CARGO_PKG_NAME"))?;
                        std::fs::write(dist_dir.join("index.html"), #index)?;
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
        })
    }
}
