use crate::{default_build_command, default_dist_dir, metadata};
use anyhow::{ensure, Context, Result};
use clap::Parser;
use std::{fs, path::PathBuf, process};
use wasm_bindgen_cli_support::Bindgen;

/// An helper to generate the distributed package
///
/// This structs provide a customizable way to assemble and generate a package
/// for Wasm.
///
/// # Usage
///
/// ```rust,no_run
/// # use std::process;
/// # use xtask_wasm::{anyhow::Result, clap};
/// #
/// # #[derive(clap::Parser)]
/// # struct Opt {
/// #     #[clap(subcommand)]
/// #     cmd: Command,
/// # }
/// #
/// #[derive(clap::Parser)]
/// enum Command {
///     Dist(xtask_wasm::Dist),
/// }
///
/// fn main() -> Result<()> {
///     let opt: Opt = clap::Parser::parse();
///
///     match opt.cmd {
///         Command::Dist(dist) => {
///             let dist = dist
///                 .dist_dir_path("dist")
///                 .static_dir_path("project/static")
///                 .app_name("project")
///                 .run_in_workspace(true)
///                 .run("project")?;
///
///             println!("Built at {}", dist.dist_dir.display());
///         }
///     }
///
///     Ok(())
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Parser)]
pub struct Dist {
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
    pub build_command: process::Command,
    /// Directory of all generated artifacts
    #[clap(skip)]
    pub dist_dir_path: Option<PathBuf>,
    /// Directory of all static artifacts
    #[clap(skip)]
    pub static_dir_path: Option<PathBuf>,
    /// Set the resulting app name, default to `app`
    #[clap(skip)]
    pub app_name: Option<String>,
    /// Set the command's current directory as the workspace root
    #[clap(skip = true)]
    pub run_in_workspace: bool,
}

impl Dist {
    /// Set the command used by the build process.
    ///
    /// The default command is the result of the [`default_build_command`].
    pub fn build_command(mut self, command: process::Command) -> Self {
        self.build_command = command;
        self
    }

    /// Set the directory for the generated artifacts.
    ///
    /// The default for debug build is `target/debug/dist` and
    /// `target/release/dist` for the release build.
    pub fn dist_dir_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.dist_dir_path = Some(path.into());
        self
    }

    /// Set the directory for the static artifacts (like `index.html`).
    pub fn static_dir_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.static_dir_path = Some(path.into());
        self
    }

    /// Set the resulting package name.
    ///
    /// The default is `app`.
    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    /// Set the dist process current directory as the workspace root.
    pub fn run_in_workspace(mut self, res: bool) -> Self {
        self.run_in_workspace = res;
        self
    }

    /// Build the given package for Wasm.
    ///
    /// This will generate JS bindings via [`wasm-bindgen`](https://docs.rs/wasm-bindgen/latest/wasm_bindgen/)
    /// and copy files from a given static directory if any to finally return
    /// the paths of the generated artifacts with [`DistResult`].
    pub fn run(self, package_name: &str) -> Result<DistResult> {
        log::trace!("Getting package's metadata");
        let metadata = metadata();

        let dist_dir_path = self
            .dist_dir_path
            .unwrap_or_else(|| default_dist_dir(self.release).as_std_path().to_path_buf());

        log::trace!("Initializing dist process");
        let mut build_process = self.build_command;

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

        build_process.args(["--package", package_name]);

        let input_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(if self.release { "release" } else { "debug" })
            .join(&package_name.replace("-", "_"))
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

        let wasm_js_path = dist_dir_path.join(&app_name).with_extension("js");
        let wasm_bin_path = dist_dir_path.join(&app_name).with_extension("wasm");

        if dist_dir_path.exists() {
            log::trace!("Removing already existing dist directory");
            fs::remove_dir_all(&dist_dir_path)?;
        }

        log::trace!("Creating new dist directory");
        fs::create_dir_all(&dist_dir_path).context("cannot create build directory")?;

        log::trace!("Writing files into dist directory");
        fs::write(&wasm_js_path, wasm_js).with_context(|| "cannot write js file")?;
        fs::write(&wasm_bin_path, wasm_bin).with_context(|| "cannot write WASM file")?;

        let mut copy_options = fs_extra::dir::CopyOptions::new();
        copy_options.overwrite = true;
        copy_options.content_only = true;

        if let Some(static_dir) = self.static_dir_path {
            log::trace!("Copying static directory into dist directory");
            fs_extra::dir::copy(static_dir, &dist_dir_path, &copy_options)
                .context("cannot copy static directory")?;
        }

        log::info!("Successfully built in {}", dist_dir_path.display());

        Ok(DistResult {
            dist_dir: dist_dir_path,
            js: wasm_js_path,
            wasm: wasm_bin_path,
        })
    }
}

/// Provides paths of the generated dist artifacts.
pub struct DistResult {
    /// Directory containing the generated artifacts
    pub dist_dir: PathBuf,
    /// js output generated by wasm_bindgen
    pub js: PathBuf,
    /// wasm output generated by wasm_bindgen
    pub wasm: PathBuf,
}
