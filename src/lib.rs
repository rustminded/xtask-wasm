use anyhow::{ensure, Context, Result};
use cargo_metadata::{camino::Utf8PathBuf, Metadata};
use std::{fs, process};
use wasm_bindgen_cli_support::Bindgen;

pub fn get_metadata() -> Result<Metadata> {
    cargo_metadata::MetadataCommand::new()
        .exec()
        .context("Cannot get metadata")
}

pub fn build(metadata: Metadata, package_name: &str) -> Result<Utf8PathBuf> {
    let mut command = process::Command::new("cargo");
    command.current_dir(&metadata.workspace_root).args([
        "build",
        "--target",
        "wasm32-unknown-unknown",
        "--package",
        package_name,
    ]);

    ensure!(
        command.status().context("Could not start cargo")?.success(),
        "Cargo command failed"
    );

    let input_path = metadata
        .target_directory
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join(package_name.replace("-", "_"))
        .with_extension("wasm");

    let mut output = Bindgen::new()
        .input_path(input_path)
        .out_name("app")
        .web(true)
        .expect("web have panic")
        .debug(true)
        .generate_output()
        .context("could not generate WASM bindgen file")?;

    let wasm_js = output.js().to_owned();
    let wasm_bin = output.wasm_mut().emit_wasm();

    let build_dir_path = metadata.workspace_root.join("build");
    let static_dir_path = metadata.workspace_root.join("static");

    let wasm_js_path = build_dir_path.join("app.js");
    let wasm_bin_path = build_dir_path.join("app.wasm");

    let _ = fs::create_dir(&build_dir_path);

    fs::write(wasm_js_path, wasm_js).with_context(|| "Cannot write js file")?;
    fs::write(wasm_bin_path, wasm_bin).with_context(|| "Cannot write WASM file")?;

    fs::copy(
        static_dir_path.join("index.html"),
        build_dir_path.join("index.html"),
    )
    .context(format!("could not copy index.html from static directory"))?;

    Ok(build_dir_path)
}
