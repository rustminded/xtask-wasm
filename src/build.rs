use crate::{default_build_command, default_build_dir, metadata};
use anyhow::{ensure, Context, Result};
use clap::Parser;
use std::{fs, path::PathBuf, process};
use wasm_bindgen_cli_support::Bindgen;

#[non_exhaustive]
#[derive(Debug, Parser)]
pub struct Build {
    /// No output printed to stdout
    #[clap(short, long)]
    pub quiet: bool,
    /// Number of parallel jobs, defaults to # of CPUs
    #[clap(short, long)]
    pub jobs: Option<String>,
    /// Build artifacts with the specified profile
    #[clap(long)]
    pub profile: Option<String>,
    /// Build artifacts in release mode, with optimizations
    #[clap(long)]
    pub release: bool,
    /// Space or comma separated list of features to activate
    #[clap(long)]
    pub features: Vec<String>,
    /// Activate all available features
    #[clap(long)]
    pub all_features: bool,
    /// Do not activate the `default` features
    #[clap(long)]
    pub no_default_features: bool,
    /// Use verbose output
    #[clap(short, long)]
    pub verbose: bool,
    /// Coloring: auto, always, never
    #[clap(long)]
    pub color: Option<String>,
    /// Require Cargo.lock and cache are up to date
    #[clap(long)]
    pub frozen: bool,
    /// Require Cargo.lock is up to date
    #[clap(long)]
    pub locked: bool,
    /// Run without accessing the network
    #[clap(long)]
    pub offline: bool,
    /// Ignore `rust-version` specification in packages
    #[clap(long)]
    pub ignore_rust_version: bool,

    /// Command passed to the build process
    #[clap(skip = default_build_command())]
    pub command: process::Command,
    /// Directory of all generated artifacts
    #[clap(skip)]
    pub build_dir_path: Option<PathBuf>,
    /// Directory of all static artifacts
    #[clap(skip)]
    pub static_dir_path: Option<PathBuf>,
    /// Set the resulting app name, default to "app"
    #[clap(skip)]
    pub app_name: Option<String>,
    /// Set the command's current directory as the workspace root
    #[clap(skip = true)]
    pub run_in_workspace: bool,
}

impl Build {
    pub fn command(mut self, command: process::Command) -> Self {
        self.command = command;
        self
    }

    pub fn build_dir_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.build_dir_path = Some(path.into());
        self
    }

    pub fn static_dir_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.static_dir_path = Some(path.into());
        self
    }

    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    pub fn run_in_workspace(mut self, res: bool) -> Self {
        self.run_in_workspace = res;
        self
    }

    pub fn run(self, crate_name: &str) -> Result<BuildResult> {
        log::trace!("Getting package's metadata");
        let metadata = metadata();

        let build_dir_path = self
            .build_dir_path
            .unwrap_or_else(|| default_build_dir(self.release).as_std_path().to_path_buf());

        log::trace!("Initializing build process");
        let mut build_process = self.command;

        if self.run_in_workspace {
            build_process.current_dir(&metadata.workspace_root);
        }

        if self.quiet {
            build_process.arg("--quiet");
        }

        if let Some(number) = self.jobs {
            build_process.args(["--jobs", &number]);
        }

        if let Some(profile) = self.profile {
            build_process.args(["--profile", &profile]);
        }

        if self.release {
            build_process.arg("--release");
        }

        for feature in &self.features {
            build_process.args(["--features", feature]);
        }

        if self.all_features {
            build_process.arg("--all-features");
        }

        if self.no_default_features {
            build_process.arg("--no-default-features");
        }

        if self.verbose {
            build_process.arg("--verbose");
        }

        if let Some(color) = self.color {
            build_process.args(["--color", &color]);
        }

        if self.frozen {
            build_process.arg("--frozen");
        }

        if self.locked {
            build_process.arg("--locked");
        }

        if self.offline {
            build_process.arg("--offline");
        }

        if self.ignore_rust_version {
            build_process.arg("--ignore-rust-version");
        }

        build_process.args(["--package", crate_name]);

        let input_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(if self.release { "release" } else { "debug" })
            .join(&crate_name.replace("-", "_"))
            .with_extension("wasm");

        if input_path.exists() {
            log::trace!("Removing existing target directory");
            fs::remove_file(&input_path).context("cannot remove existing target")?;
        }

        log::trace!("Spawning build process");
        ensure!(
            build_process
                .status()
                .context("could not start cargo")?
                .success(),
            "cargo command failed"
        );

        let app_name = self.app_name.unwrap_or_else(|| "app".to_string());

        log::trace!("Generating wasm output");
        let mut output = Bindgen::new()
            .input_path(input_path)
            .out_name(&app_name)
            .web(true)
            .expect("web have panic")
            .debug(!self.release)
            .generate_output()
            .context("could not generate WASM bindgen file")?;

        let wasm_js = output.js().to_owned();
        let wasm_bin = output.wasm_mut().emit_wasm();

        let wasm_js_path = build_dir_path.join(&app_name).with_extension("js");
        let wasm_bin_path = build_dir_path.join(&app_name).with_extension("wasm");

        if build_dir_path.exists() {
            log::trace!("Removing already existing build directory");
            fs::remove_dir_all(&build_dir_path)?;
        }

        log::trace!("Creating new build directory");
        fs::create_dir_all(&build_dir_path).context("cannot create build directory")?;

        log::trace!("Writing files into build directory");
        fs::write(&wasm_js_path, wasm_js).with_context(|| "cannot write js file")?;
        fs::write(&wasm_bin_path, wasm_bin).with_context(|| "cannot write WASM file")?;

        let mut copy_options = fs_extra::dir::CopyOptions::new();
        copy_options.overwrite = true;
        copy_options.content_only = true;

        if let Some(static_dir) = self.static_dir_path {
            log::trace!("Copying static directory into build directory");
            fs_extra::dir::copy(static_dir, &build_dir_path, &copy_options)
                .context("cannot copy static directory")?;
        }

        log::info!("Successfully built in {}", build_dir_path.display());

        Ok(BuildResult {
            build_dir: build_dir_path,
            js: wasm_js_path,
            wasm: wasm_bin_path,
        })
    }
}

/// Provides paths of the generated artifacts from a
/// build process.
pub struct BuildResult {
    /// Directory containing the artifacts
    pub build_dir: PathBuf,
    /// js output generated from wasm_bindgen
    pub js: PathBuf,
    /// wasm output generated from wasm_bindgen
    pub wasm: PathBuf,
}
