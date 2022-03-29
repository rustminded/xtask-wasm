use crate::{
    anyhow::{ensure, Context, Result},
    camino, clap, default_build_command, metadata,
};
use lazy_static::lazy_static;
use std::{fs, path::PathBuf, process};
use wasm_bindgen_cli_support::Bindgen;

/// A helper to generate the distributed package.
///
/// # Usage
///
/// ```rust,no_run
/// use std::process;
/// use xtask_wasm::{anyhow::Result, clap};
///
/// #[derive(clap::Parser)]
/// enum Opt {
///     Dist(xtask_wasm::Dist),
/// }
///
/// fn main() -> Result<()> {
///     let opt: Opt = clap::Parser::parse();
///
///     match opt {
///         Opt::Dist(dist) => {
///             log::info!("Generating package...");
///
///             dist
///                 .static_dir_path("my-project/static")
///                 .app_name("my-project")
///                 .run_in_workspace(true)
///                 .run("my-project")?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// In this example, we added a `dist` subcommand to build and package the
/// `my-project` crate. It will run the [`default_build_command`](crate::default_build_command)
/// at the workspace root, copy the content of the `project/static` directory,
/// generate JS bindings and output two files: `project.js` and `project.wasm`
/// into the dist directory.
#[non_exhaustive]
#[derive(Debug, clap::Parser)]
#[clap(
    about = "Generate the distributed package.",
    long_about = "Generate the distributed package.\n\
        It will build and package the project for WASM.",
)]
pub struct Dist {
    /// No output printed to stdout.
    #[clap(short, long)]
    pub quiet: bool,
    /// Number of parallel jobs, defaults to # of CPUs.
    #[clap(short, long)]
    pub jobs: Option<String>,
    /// Build artifacts with the specified profile.
    #[clap(long)]
    pub profile: Option<String>,
    /// Build artifacts in release mode, with optimizations.
    #[clap(long)]
    pub release: bool,
    /// Space or comma separated list of features to activate.
    #[clap(long)]
    pub features: Vec<String>,
    /// Activate all available features.
    #[clap(long)]
    pub all_features: bool,
    /// Do not activate the `default` features.
    #[clap(long)]
    pub no_default_features: bool,
    /// Use verbose output
    #[clap(short, long)]
    pub verbose: bool,
    /// Coloring: auto, always, never.
    #[clap(long)]
    pub color: Option<String>,
    /// Require Cargo.lock and cache are up to date.
    #[clap(long)]
    pub frozen: bool,
    /// Require Cargo.lock is up to date.
    #[clap(long)]
    pub locked: bool,
    /// Run without accessing the network.
    #[clap(long)]
    pub offline: bool,
    /// Ignore `rust-version` specification in packages.
    #[clap(long)]
    pub ignore_rust_version: bool,
    /// Name of the example target to run.
    #[clap(long)]
    pub example: Option<String>,

    /// Command passed to the build process.
    #[clap(skip = default_build_command())]
    pub build_command: process::Command,
    /// Directory of all generated artifacts.
    #[clap(skip)]
    pub dist_dir_path: Option<PathBuf>,
    /// Directory of all static artifacts.
    #[clap(skip)]
    pub static_dir_path: Option<PathBuf>,
    /// Set the resulting app name, default to `app`.
    #[clap(skip)]
    pub app_name: Option<String>,
    /// Set the command's current directory as the workspace root.
    #[clap(skip = true)]
    pub run_in_workspace: bool,
    /// Output style for SASS/SCSS
    #[cfg(feature = "sass")]
    #[clap(skip)]
    pub sass_options: sass_rs::Options,
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

    #[cfg(feature = "sass")]
    /// Set the output style for SCSS/SASS
    pub fn sass_options(mut self, output_style: sass_rs::Options) -> Self {
        self.sass_options = output_style;
        self
    }

    /// Set the example to build.
    pub fn example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }

    /// Build the given package for Wasm.
    ///
    /// This will generate JS bindings via [`wasm-bindgen`](https://docs.rs/wasm-bindgen/latest/wasm_bindgen/)
    /// and copy files from a given static directory if any to finally return
    /// the paths of the generated artifacts with [`DistResult`].
    ///
    /// Wasm optimizations can be achieved using [`crate::WasmOpt`] if the
    /// feature `wasm-opt` is enabled.
    pub fn run(self, package_name: &str) -> Result<DistResult> {
        log::trace!("Getting package's metadata");
        let metadata = metadata();

        let dist_dir_path = self
            .dist_dir_path
            .unwrap_or_else(|| default_dist_dir(self.release).as_std_path().to_path_buf());

        log::trace!("Initializing dist process");
        let mut build_command = self.build_command;

        if self.run_in_workspace {
            build_command.current_dir(&metadata.workspace_root);
        }

        if self.quiet {
            build_command.arg("--quiet");
        }

        if let Some(number) = self.jobs {
            build_command.args(["--jobs", &number]);
        }

        if let Some(profile) = self.profile {
            build_command.args(["--profile", &profile]);
        }

        if self.release {
            build_command.arg("--release");
        }

        for feature in &self.features {
            build_command.args(["--features", feature]);
        }

        if self.all_features {
            build_command.arg("--all-features");
        }

        if self.no_default_features {
            build_command.arg("--no-default-features");
        }

        if self.verbose {
            build_command.arg("--verbose");
        }

        if let Some(color) = self.color {
            build_command.args(["--color", &color]);
        }

        if self.frozen {
            build_command.arg("--frozen");
        }

        if self.locked {
            build_command.arg("--locked");
        }

        if self.offline {
            build_command.arg("--offline");
        }

        if self.ignore_rust_version {
            build_command.arg("--ignore-rust-version");
        }

        build_command.args(["--package", package_name]);

        if let Some(example) = &self.example {
            build_command.args(["--example", example]);
        }

        let build_dir = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(if self.release { "release" } else { "debug" });
        let input_path = if let Some(example) = &self.example {
            build_dir
                .join("examples")
                .join(&example.replace('-', "_"))
                .with_extension("wasm")
        } else {
            build_dir
                .join(&package_name.replace('-', "_"))
                .with_extension("wasm")
        };

        if input_path.exists() {
            log::trace!("Removing existing target directory");
            fs::remove_file(&input_path).context("cannot remove existing target")?;
        }

        log::trace!("Spawning build process");
        ensure!(
            build_command
                .status()
                .context("could not start cargo")?
                .success(),
            "cargo command failed"
        );

        let app_name = self.app_name.unwrap_or_else(|| "app".to_string());

        log::trace!("Generating Wasm output");
        let mut output = Bindgen::new()
            .input_path(input_path)
            .out_name(&app_name)
            .web(true)
            .expect("web have panic")
            .debug(!self.release)
            .generate_output()
            .context("could not generate Wasm bindgen file")?;

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

        log::trace!("Writing Wasm output into dist directory");
        fs::write(&wasm_js_path, wasm_js).context("cannot write js file")?;
        fs::write(&wasm_bin_path, wasm_bin).context("cannot write Wasm file")?;

        if let Some(static_dir) = self.static_dir_path {
            #[cfg(feature = "sass")]
            {
                log::trace!("Generating CSS files from SASS/SCSS");
                sass(&static_dir, &dist_dir_path, &self.sass_options)?;
            }

            #[cfg(not(feature = "sass"))]
            {
                let mut copy_options = fs_extra::dir::CopyOptions::new();
                copy_options.overwrite = true;
                copy_options.content_only = true;

                log::trace!("Copying static directory into dist directory");
                fs_extra::dir::copy(static_dir, &dist_dir_path, &copy_options)
                    .context("cannot copy static directory")?;
            }
        }

        log::info!("Successfully built in {}", dist_dir_path.display());

        Ok(DistResult {
            dist_dir: dist_dir_path,
            js: wasm_js_path,
            wasm: wasm_bin_path,
        })
    }
}

#[cfg(feature = "sass")]
fn sass(
    static_dir: &std::path::Path,
    dist_dir: &std::path::Path,
    options: &sass_rs::Options
) -> Result<()> {
    fn is_sass(path: &std::path::Path) -> bool {
        matches!(
            path.extension().and_then(|x| x.to_str().map(|x| x.to_lowercase())).as_deref(),
            Some("sass") | Some("scss")
        )
    }

    fn should_ignore(path: &std::path::Path) -> bool {
        path
            .file_name()
            .expect("WalkDir does not yield paths ending with `..`  or `.`")
            .to_str()
            .map(|x| x.starts_with("_"))
            .unwrap_or(false)
    }

    log::trace!("Generating dist artifacts");
    let walker = walkdir::WalkDir::new(&static_dir);
    for entry in walker {
        let entry = entry.with_context(|| format!("cannot walk into directory `{}`", &static_dir.display()))?;
        let source = entry.path();
        let dest = dist_dir.join(source.strip_prefix(&static_dir).unwrap());
        let _ = fs::create_dir_all(dest.parent().unwrap());

        if !source.is_file() {
            continue
        } else if is_sass(source) {
            if !should_ignore(source) {
                let dest = dest
                    .with_extension("css");

                let css = sass_rs::compile_file(source, options.clone())
                    .expect("could not convert SASS/ file");
                fs::write(&dest, css).with_context(|| format!("could not write CSS to file `{}`", dest.display()))?;
            }
        } else {
            fs::copy(source, &dest).with_context(|| format!("cannot move `{}` to `{}`", source.display(), dest.display()))?;
        }
    }

    Ok(())
}

/// Provides paths of the generated dist artifacts.
pub struct DistResult {
    /// Directory containing the generated artifacts.
    pub dist_dir: PathBuf,
    /// JS output generated by wasm-bindgen.
    pub js: PathBuf,
    /// Wasm output generated by wasm-bindgen.
    pub wasm: PathBuf,
}

/// Get the default dist directory.
///
/// The default for debug build is `target/debug/dist` and `target/release/dist`
/// for the release build.
pub fn default_dist_dir(release: bool) -> &'static camino::Utf8Path {
    lazy_static! {
        static ref DEFAULT_RELEASE_PATH: camino::Utf8PathBuf =
            metadata().target_directory.join("release").join("dist");
        static ref DEFAULT_DEBUG_PATH: camino::Utf8PathBuf =
            metadata().target_directory.join("debug").join("dist");
    }

    if release {
        &DEFAULT_RELEASE_PATH
    } else {
        &DEFAULT_DEBUG_PATH
    }
}
