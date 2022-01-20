use anyhow::{ensure, Context, Result};
use std::{path::PathBuf, process};

#[allow(unreachable_code)]
pub fn run(
    binary: Vec<u8>,
    shrink_level: u32,
    optimization_level: u32,
    debug_info: bool,
) -> Result<Vec<u8>> {
    return {
        let wasm_opt = download_wasm_opt()?;

        let mut command = process::Command::new(&wasm_opt);
        command
            .stderr(process::Stdio::inherit())
            .args(&["-o", "-", "-O"])
            .args(&["-ol", &optimization_level.to_string()])
            .args(&["-s", &shrink_level.to_string()]);

        if debug_info {
            command.arg("-g");
        }

        #[cfg(target_os = "macos")]
        {
            command.env("DYLD_LIBRARY_PATH", wasm_opt.parent().unwrap());
        }

        #[cfg(windows)]
        let delete_guard = {
            use std::io::Write;

            let tmp = tempfile::NamedTempFile::new()?;
            tmp.as_file().write_all(&binary)?;
            command.arg(tmp.path());
            tmp
        };

        #[cfg(unix)]
        {
            use std::io::{Seek, SeekFrom, Write};

            let mut file = tempfile::tempfile()?;
            file.write_all(&binary)?;
            file.seek(SeekFrom::Start(0))?;
            command.stdin(file);
        }

        let output = command.output()?;
        ensure!(output.status.success(), "command `wasm-opt` failed");

        Ok(output.stdout)
    };

    log::warn!("No optimization has been done on the WASM");
    Ok(binary)
}

fn download_wasm_opt() -> Result<PathBuf> {
    let cache = binary_install::Cache::at(crate::metadata().target_directory.as_std_path());
    let url = format!(
        "https://github.com/WebAssembly/binaryen/releases/download/version_{version}/binaryen-version_{version}-{arch}-{os}.tar.gz",
        version = "105",
        arch = platforms::TARGET_ARCH,
        os = platforms::TARGET_OS,
    );

    #[cfg(target_os = "macos")]
    let binaries = &["wasm-opt", "libbinaryen"];
    #[cfg(not(target_os = "macos"))]
    let binaries = &["wasm-opt"];

    log::info!("Downloading wasm-opt");
    Ok(cache
        .download(true, "wasm-opt", binaries, &url)
        .map_err(|err| err.compat())
        .with_context(|| format!("could not download binaryen: {}", url))?
        .expect("cannot install binaryen")
        .binary("wasm-opt")
        .map_err(|err| err.compat())?)
}
