use crate::anyhow::{anyhow, ensure, Context, Result};
use lazy_static::lazy_static;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

lazy_static! {
    static ref WASM_OPT_URL: String = {
        // Any update on the version needs to be done in the README too.
        let version = "110";

        let os = std::env::consts::OS;
        let mut arch = std::env::consts::ARCH;
        if arch == "aarch64" {
            arch = "arm64";
        }

        format!(
                "https://github.com/WebAssembly/binaryen/releases/download/version_{version}/binaryen-version_{version}-{arch}-{os}.tar.gz",
            )
    };
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
                    .with_context(|| {
                        format!("could not download wasm-opt: {}", &WASM_OPT_URL.as_str())
                    })?
                    .expect("install_permitted is always true; qed")
                    .binary("wasm-opt")?)
            }

            downloaded_binary_path()
        };
    }

    WASM_OPT_PATH.as_deref().map_err(|err| anyhow!("{}", err))
}

/// Helper Abstracting the `wasm-opt` binary from
/// [binaryen](https://github.com/WebAssembly/binaryen) for easily optimizing
/// your Wasm binary.
///
/// # Usage
///
/// ```rust,no_run
/// # use xtask_wasm::{anyhow::Result, WasmOpt};
/// # fn main() -> Result<()> {
/// WasmOpt::level(1)
///     .shrink(2)
///     .optimize("app.wasm")?;
/// # Ok(())
/// # }
/// ```
pub struct WasmOpt {
    /// How much to focus on optimizing code.
    pub optimization_level: u32,
    /// How much to focus on shrinking code size.
    pub shrink_level: u32,
    /// Emit names section in Wasm binary.
    pub debug_info: bool,
}

impl WasmOpt {
    /// Set the level of code optimization.
    pub fn level(optimization_level: u32) -> Self {
        Self {
            optimization_level,
            shrink_level: 0,
            debug_info: false,
        }
    }

    /// Set the level of size shrinking.
    pub fn shrink(mut self, shrink_level: u32) -> Self {
        self.shrink_level = shrink_level;
        self
    }

    /// Preserve debug info.
    pub fn debug(mut self) -> Self {
        self.debug_info = true;
        self
    }

    /// Optimize the Wasm binary provided by `binary_path`.
    ///
    /// This function will execute `wasm-opt` over the given Wasm binary,
    /// downloading it if necessary (cached into the `target` directory).
    pub fn optimize(self, binary_path: impl AsRef<Path>) -> Result<Self> {
        let input_path = binary_path.as_ref();
        let output_path = input_path.with_extension("opt");
        let wasm_opt = download_wasm_opt()?;

        let mut command = process::Command::new(wasm_opt);
        command
            .stderr(process::Stdio::inherit())
            .arg(input_path)
            .arg("-o")
            .arg(&output_path)
            .arg("-O")
            .arg("-ol")
            .arg(self.optimization_level.to_string())
            .arg("-s")
            .arg(self.shrink_level.to_string());

        if self.debug_info {
            command.arg("-g");
        }

        #[cfg(target_os = "macos")]
        {
            command.env("DYLD_LIBRARY_PATH", wasm_opt.parent().unwrap());
        }

        log::info!("Optimizing Wasm");
        ensure!(
            command.output()?.status.success(),
            "command `wasm-opt` failed"
        );

        fs::remove_file(input_path)?;
        fs::rename(&output_path, input_path)?;

        log::info!("Wasm optimized");
        Ok(self)
    }
}
