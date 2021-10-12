use anyhow::{bail, ensure, Context, Result};
use std::{fs, process};
use structopt::StructOpt;
use walkdir::WalkDir;
use wasm_bindgen_cli_support::Bindgen;

#[derive(Debug, StructOpt)]
pub struct BuildArgs {
    #[structopt(long)]
    release: bool,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    Build(BuildArgs),
}

pub fn build(
    args: BuildArgs,
    crate_name: &'static str,
    static_dir_path: &'static str,
    build_dir_path: &'static str,
) -> Result<()> {
    let metadata = match cargo_metadata::MetadataCommand::new().exec() {
        Ok(metadata) => metadata,
        Err(_) => bail!("Cannot get package's metadata"),
    };

    let mut build_process = process::Command::new("cargo");
    build_process
        .current_dir(&metadata.workspace_root)
        .arg("build");

    if args.release {
        build_process.arg("--release");
    }

    build_process.args([
        "--target",
        "wasm32-unknown-unknown",
        "--package",
        crate_name,
    ]);

    ensure!(
        build_process
            .status()
            .context("Could not start cargo")?
            .success(),
        "Cargo command failed"
    );

    let input_path = metadata
        .target_directory
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join(&crate_name.replace("-", "_"))
        .with_extension("wasm");

    let mut output = Bindgen::new()
        .input_path(input_path)
        .out_name("app")
        .web(true)
        .expect("web have panic")
        .debug(!args.release)
        .generate_output()
        .context("could not generate WASM bindgen file")?;

    let wasm_js = output.js().to_owned();
    let wasm_bin = output.wasm_mut().emit_wasm();

    let build_dir_path = metadata.workspace_root.join(build_dir_path);
    let static_dir_path = metadata.workspace_root.join(static_dir_path);

    let wasm_js_path = build_dir_path.join("app.js");
    let wasm_bin_path = build_dir_path.join("app_bg.wasm");

    let _ = fs::create_dir(&build_dir_path);

    fs::write(wasm_js_path, wasm_js).with_context(|| "Cannot write js file")?;
    fs::write(wasm_bin_path, wasm_bin).with_context(|| "Cannot write WASM file")?;

    for entry in WalkDir::new(static_dir_path) {
        match entry {
            Ok(value) => {
                let entry_path = value.path();
                if entry_path.is_file() {
                    let destination_filename = entry_path
                        .file_name()
                        .expect("Cannot get filename when iterating on static directory")
                        .to_str()
                        .expect("Cannot convert filename");
                    fs::copy(&entry_path, build_dir_path.join(destination_filename))
                        .context("Could not copy the content of the static directory")?;
                }
            }
            Err(err) => {
                bail!(
                    "An error occurred when iterating on the static directory: {}",
                    err
                );
            }
        }
    }

    Ok(())
}
