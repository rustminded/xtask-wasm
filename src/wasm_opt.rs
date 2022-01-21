use anyhow::{anyhow, ensure, Context, Result};
use lazy_static::lazy_static;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

lazy_static! {
    static ref WASM_OPT_URL: String = {
        format!(
                "https://github.com/WebAssembly/binaryen/releases/download/version_{version}/binaryen-version_{version}-{arch}-{os}.tar.gz",
                version = "105",
                arch = platforms::TARGET_ARCH,
                os = platforms::TARGET_OS,
            )
    };
}

pub fn run(
    binary_path: impl AsRef<Path>,
    shrink_level: u32,
    optimization_level: u32,
    debug_info: bool,
) -> Result<()> {
    let input_path = binary_path.as_ref();
    let output_path = input_path.with_extension("opt");
    let wasm_opt = download_wasm_opt()?;

    let mut command = process::Command::new(&wasm_opt);
    command
        .stderr(process::Stdio::inherit())
        .arg(input_path)
        .arg("-o")
        .arg(&output_path)
        .arg("-O")
        .arg("-ol")
        .arg(optimization_level.to_string())
        .arg("-s")
        .arg(shrink_level.to_string());

    if debug_info {
        command.arg("-g");
    }

    #[cfg(target_os = "macos")]
    {
        command.env("DYLD_LIBRARY_PATH", wasm_opt.parent().unwrap());
    }

    log::info!("Optimizing WASM");
    ensure!(
        command.output()?.status.success(),
        "command `wasm-opt` failed"
    );

    fs::remove_file(&input_path)?;
    fs::rename(&output_path, &input_path)?;

    log::info!("WASM optimized");
    Ok(())
}

fn download_wasm_opt() -> Result<&'static Path> {
    lazy_static! {
        static ref WASM_OPT_PATH: Result<PathBuf> = {
            fn downloaded_binary_path() -> Result<PathBuf> {
                let cache =
                    binary_install::Cache::at(crate::metadata().target_directory.as_std_path());

                #[cfg(target_os = "macos")]
                let binaries = &["wasm-opt", "libbinaryen"];
                #[cfg(not(target_os = "macos"))]
                let binaries = &["wasm-opt"];

                log::info!("Downloading wasm-opt");
                Ok(cache
                    .download(true, "wasm-opt", binaries, &WASM_OPT_URL)
                    .map_err(|err| err.compat())
                    .with_context(|| format!("could not download wasm-opt: {}", &WASM_OPT_URL.as_str()))?
                    .expect("install_permitted is always true; qed")
                    .binary("wasm-opt")
                    .map_err(|err| err.compat())?)
            }

            downloaded_binary_path()
        };
    }

    WASM_OPT_PATH.as_deref().map_err(|err| anyhow!("{}", err))
}
