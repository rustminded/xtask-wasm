use anyhow::{ensure, Context, Result, anyhow};
use std::{
    path::{Path, PathBuf},
    process
};
use lazy_static::lazy_static;


fn wasm_opt_url() -> &'static str {
    lazy_static! {
        static ref WASM_OPT_URL: String = {
            format!(
                "https://github.com/WebAssembly/binaryen/releases/download/version_{version}/bynaryen-version_{version}-{arch}-{os}.tar.gz",
                version = "105",
                arch = platforms::TARGET_ARCH,
                os = platforms::TARGET_OS,
            )
        };
    }

    &WASM_OPT_URL
}

pub fn run(
    binary: Vec<u8>,
    shrink_level: u32,
    optimization_level: u32,
    debug_info: bool,
) -> Result<()> {
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

    ensure!(
        command.output()?.status.success(),
        "command `wasm-opt` failed"
    );

    Ok(())
}

fn download_wasm_opt() -> Result<&'static Path> {
    lazy_static! {
        static ref WASM_OPT_PATH: Result<PathBuf> = {
            fn downloaded_binary_path() -> Result<PathBuf> {
                let cache = binary_install::Cache::at(crate::metadata().target_directory.as_std_path());
                let url = wasm_opt_url();

                #[cfg(target_os = "macos")]
                let binaries = &["wasm-opt", "libbinaryen"];
                #[cfg(not(target_os = "macos"))]
                let binaries = &["wasm-opt"];

                log::info!("Downloading wasm-opt");
                Ok(
                    cache
                        .download(true, "wasm-opt", binaries, &url)
                        .map_err(|err| err.compat())
                        .with_context(|| format!("could not download wasm-opt: {}", url))?
                        .expect("install_permitted is always true; qed")
                        .binary("wasm-opt")
                        .map_err(|err| err.compat())?
                )

            }

            downloaded_binary_path()
        };
    }

    WASM_OPT_PATH.as_deref().map_err(|err| anyhow!("{}", err))
}
