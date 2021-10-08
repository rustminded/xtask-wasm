use anyhow::{bail, ensure, Context, Result};
use std::path::PathBuf;
use std::str::FromStr;
use std::{fs, process};
use structopt::StructOpt;
use wasm_bindgen_cli_support::Bindgen;

#[derive(Debug, StructOpt)]
pub struct BuildArgs {
    profile: Profile,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    Build(BuildArgs),
}

#[derive(Debug, StructOpt, PartialEq)]
pub enum Profile {
    Dev,
    Release,
}

impl FromStr for Profile {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dev" => Ok(Profile::Dev),
            "release" => Ok(Profile::Release),
            _ => Err("Not a profile"),
        }
    }
}

pub fn run(args: BuildArgs, crate_name: &'static str) -> Result<PathBuf> {
    let metadata = match cargo_metadata::MetadataCommand::new().exec() {
        Ok(metadata) => metadata,
        Err(_) => bail!("Cannot get package's metadata"),
    };

    let mut build_process = process::Command::new("cargo");
    build_process
        .current_dir(&metadata.workspace_root)
        .arg("build");

    if args.profile == Profile::Release {
        build_process.arg("--release");
    }

    build_process.args([
        "--target",
        "wasm32-unknown-unknown",
        "--package",
        &crate_name,
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
        .debug(match args.profile {
            Profile::Dev => true,
            Profile::Release => false,
        })
        .generate_output()
        .context("could not generate WASM bindgen file")?;

    let wasm_js = output.js().to_owned();
    let wasm_bin = output.wasm_mut().emit_wasm();

    let build_dir_path = metadata.workspace_root.join("build");
    let static_dir_path = metadata.workspace_root.join("static");

    let wasm_js_path = build_dir_path.join("app.js");
    let wasm_bin_path = build_dir_path.join("app_bg.wasm");

    let _ = fs::create_dir(&build_dir_path);

    fs::write(wasm_js_path, wasm_js).with_context(|| "Cannot write js file")?;
    fs::write(wasm_bin_path, wasm_bin).with_context(|| "Cannot write WASM file")?;

    fs::copy(
        static_dir_path.join("index.html"),
        build_dir_path.join("index.html"),
    )
    .context(format!("could not copy index.html from static directory"))?;

    Ok(PathBuf::from(build_dir_path))
}
