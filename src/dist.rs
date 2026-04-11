use crate::{
    anyhow::{bail, ensure, Context, Result},
    camino, clap, default_build_command, metadata,
};
use derive_more::Debug;
use std::{fs, path::PathBuf, process};
use wasm_bindgen_cli_support::Bindgen;

/// A type that can transform or copy a single asset file during [`Dist::build`].
///
/// Implement this trait to customise how individual files in the assets directory are
/// processed before they land in the dist directory — for example to compile SASS to
/// CSS, minify JavaScript, or generate additional output files from a source file.
///
/// Return `Ok(true)` if the file was handled (the transformer wrote its own output).
/// Return `Ok(false)` to fall through to the next transformer, or to the default
/// plain-copy behaviour if no transformer claims the file.
///
/// A blanket implementation is provided for `()` (no-op, always returns `Ok(false)`),
/// so the trait is easy to stub out in tests.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::Path;
/// use xtask_wasm::{anyhow::Result, Transformer};
///
/// struct UppercaseText;
///
/// impl Transformer for UppercaseText {
///     fn transform(&self, source: &Path, dest: &Path) -> Result<bool> {
///         if source.extension().and_then(|e| e.to_str()) == Some("txt") {
///             let content = std::fs::read_to_string(source)?;
///             std::fs::write(dest, content.to_uppercase())?;
///             return Ok(true);
///         }
///         Ok(false)
///     }
/// }
///
/// xtask_wasm::Dist::default()
///     .transformer(UppercaseText)
///     .build("my-project")
///     .unwrap();
/// ```
pub trait Transformer {
    /// Process a single asset file.
    ///
    /// `source` is the absolute path to the file in the assets directory.
    /// `dest` is the intended output path inside the dist directory, preserving
    /// the same relative path as `source` (the implementor may change the extension).
    ///
    /// Return `Ok(true)` if the file was handled, `Ok(false)` to defer.
    fn transform(&self, source: &Path, dest: &Path) -> Result<bool>;
}

use std::path::Path;

impl Transformer for () {
    fn transform(&self, _source: &Path, _dest: &Path) -> Result<bool> {
        Ok(false)
    }
}

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
///                 .assets_dir("my-project/assets")
///                 .app_name("my-project")
///                 .build("my-project")?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// In this example, we added a `dist` subcommand to build and package the
/// `my-project` crate. It will run the [`default_build_command`](crate::default_build_command)
/// at the workspace root, copy the content of the `my-project/assets` directory,
/// generate JS bindings and output two files: `my-project.js` and `my-project.wasm`
/// into the dist directory.
#[non_exhaustive]
#[derive(Debug, clap::Parser)]
#[clap(
    about = "Generate the distributed package.",
    long_about = "Generate the distributed package.\n\
        It will build and package the project for WASM."
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
    pub dist_dir: Option<PathBuf>,
    /// Directory of all static assets artifacts.
    ///
    /// Default to `assets` in the package root when it exists.
    #[clap(skip)]
    pub assets_dir: Option<PathBuf>,
    /// Set the resulting app name, default to `app`.
    #[clap(skip)]
    pub app_name: Option<String>,
    /// Transformers applied to each file in the assets directory during the build.
    ///
    /// Each transformer is called in order for every file; the first one that returns
    /// `Ok(true)` claims the file and the rest are skipped. Files not claimed by any
    /// transformer are copied verbatim into the dist directory.
    ///
    /// When the `sass` feature is enabled, a [`SassTransformer`] is included by default.
    #[clap(skip)]
    #[debug(skip)]
    pub transformers: Vec<Box<dyn Transformer>>,
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
    pub fn dist_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.dist_dir = Some(path.into());
        self
    }

    /// Set the directory for the static assets artifacts (like `index.html`).
    ///
    /// Default to `assets` in the package root when it exists.
    pub fn assets_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.assets_dir = Some(path.into());
        self
    }

    /// Set the resulting package name.
    ///
    /// The default is `app`.
    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = Some(app_name.into());
        self
    }

    /// Add a transformer for the asset copy step.
    ///
    /// Transformers are called in the order they are added. See [`Transformer`] for details.
    pub fn transformer(mut self, transformer: impl Transformer + 'static) -> Self {
        self.transformers.push(Box::new(transformer));
        self
    }

    /// Set the example to build.
    pub fn example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }

    /// Get the default dist directory for debug builds.
    pub fn default_debug_dir() -> camino::Utf8PathBuf {
        metadata().target_directory.join("debug").join("dist")
    }

    /// Get the default dist directory for release builds.
    pub fn default_release_dir() -> camino::Utf8PathBuf {
        metadata().target_directory.join("release").join("dist")
    }

    /// Build the given package for Wasm.
    ///
    /// This will generate JS bindings via [`wasm-bindgen`](https://docs.rs/wasm-bindgen/latest/wasm_bindgen/)
    /// and copy files from a given assets directory if any to finally return
    /// the path of the generated artifacts.
    #[cfg_attr(
        feature = "wasm-opt",
        doc = "Wasm optimizations can be achieved using [`WasmOpt`](crate::WasmOpt) if the feature `wasm-opt` is enabled."
    )]
    #[cfg_attr(
        not(feature = "wasm-opt"),
        doc = "Wasm optimizations can be achieved using `WasmOpt` if the feature `wasm-opt` is enabled."
    )]
    pub fn build(self, package_name: &str) -> Result<PathBuf> {
        log::trace!("Getting package's metadata");
        let metadata = metadata();

        let dist_dir = self.dist_dir.unwrap_or_else(|| {
            if self.release {
                Self::default_release_dir().into()
            } else {
                Self::default_debug_dir().into()
            }
        });

        log::trace!("Initializing dist process");
        let mut build_command = self.build_command;

        build_command.current_dir(&metadata.workspace_root);

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
                .join(example.replace('-', "_"))
                .with_extension("wasm")
        } else {
            build_dir
                .join(package_name.replace('-', "_"))
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
            .omit_default_module_path(false)
            .input_path(input_path)
            .out_name(&app_name)
            .web(true)
            .expect("web have panic")
            .debug(!self.release)
            .generate_output()
            .context("could not generate Wasm bindgen file")?;

        if dist_dir.exists() {
            log::trace!("Removing already existing dist directory");
            fs::remove_dir_all(&dist_dir)?;
        }

        log::trace!("Writing outputs to dist directory");
        output.emit(&dist_dir)?;

        let assets_dir = if let Some(assets_dir) = self.assets_dir {
            assets_dir
        } else if let Some(package) = metadata
            .packages
            .iter()
            .find(|package| package.name == package_name)
        {
            package
                .manifest_path
                .parent()
                .context("package manifest has no parent directory")?
                .join("assets")
                .as_std_path()
                .to_path_buf()
        } else {
            bail!("failed to determine assets directory path: package `{package_name}` not found");
        };

        if assets_dir.exists() {
            log::trace!("Copying assets directory into dist directory");
            copy_assets(&assets_dir, &dist_dir, &self.transformers)?;
        }

        log::info!("Successfully built in {}", dist_dir.display());

        Ok(dist_dir)
    }
}

impl Default for Dist {
    fn default() -> Dist {
        Dist {
            quiet: Default::default(),
            jobs: Default::default(),
            profile: Default::default(),
            release: Default::default(),
            features: Default::default(),
            all_features: Default::default(),
            no_default_features: Default::default(),
            verbose: Default::default(),
            color: Default::default(),
            frozen: Default::default(),
            locked: Default::default(),
            offline: Default::default(),
            ignore_rust_version: Default::default(),
            example: Default::default(),
            build_command: default_build_command(),
            dist_dir: Default::default(),
            assets_dir: Default::default(),
            app_name: Default::default(),
            transformers: vec![
                #[cfg(feature = "sass")]
                Box::new(SassTransformer::default()),
            ],
        }
    }
}

fn copy_assets(
    assets_dir: &Path,
    dist_dir: &Path,
    transformers: &[Box<dyn Transformer>],
) -> Result<()> {
    let walker = walkdir::WalkDir::new(assets_dir);
    for entry in walker {
        let entry = entry
            .with_context(|| format!("cannot walk into directory `{}`", assets_dir.display()))?;
        let source = entry.path();
        let dest = dist_dir.join(source.strip_prefix(assets_dir).unwrap());

        if !source.is_file() {
            continue;
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create directory `{}`", parent.display()))?;
        }

        let handled = transformers
            .iter()
            .map(|t| t.transform(source, &dest))
            .find(|r| r.as_ref().map_or(false, |&v| v))
            .transpose()?
            .unwrap_or(false);

        if !handled {
            fs::copy(source, &dest).with_context(|| {
                format!("cannot copy `{}` to `{}`", source.display(), dest.display())
            })?;
        }
    }

    Ok(())
}

/// A [`Transformer`] that compiles SASS/SCSS files to CSS.
///
/// Files whose names begin with `_` are treated as partials and skipped (not emitted
/// to the dist directory). All other `.sass` and `.scss` files are compiled to `.css`.
/// Non-SASS files are not claimed and fall through to the default plain-copy behaviour.
///
/// `SassTransformer` is included automatically in [`Dist::default`] when the `sass`
/// feature is enabled. To customise the compilation options, replace the default
/// transformer:
///
/// ```rust,no_run
/// # #[cfg(feature = "sass")]
/// # {
/// use xtask_wasm::{Dist, SassTransformer};
///
/// Dist::default()
///     .transformer(SassTransformer {
///         options: sass_rs::Options {
///             output_style: sass_rs::OutputStyle::Compressed,
///             ..Default::default()
///         },
///     })
///     .build("my-project")
///     .unwrap();
/// # }
/// ```
#[cfg(feature = "sass")]
pub struct SassTransformer {
    /// Options forwarded to [`sass_rs::compile_file`].
    pub options: sass_rs::Options,
}

#[cfg(feature = "sass")]
impl Default for SassTransformer {
    fn default() -> Self {
        SassTransformer {
            options: sass_rs::Options::default(),
        }
    }
}

#[cfg(feature = "sass")]
impl Transformer for SassTransformer {
    fn transform(&self, source: &Path, dest: &Path) -> Result<bool> {
        fn is_sass(path: &Path) -> bool {
            matches!(
                path.extension()
                    .and_then(|x| x.to_str().map(|x| x.to_lowercase()))
                    .as_deref(),
                Some("sass") | Some("scss")
            )
        }

        fn is_partial(path: &Path) -> bool {
            path.file_name()
                .expect("WalkDir does not yield paths ending with `..` or `.`")
                .to_str()
                .map(|x| x.starts_with('_'))
                .unwrap_or(false)
        }

        if !is_sass(source) {
            return Ok(false);
        }

        // Partials are silently skipped — claiming the file prevents the plain-copy
        // fallback from copying the raw .scss into dist.
        if is_partial(source) {
            return Ok(true);
        }

        let dest = dest.with_extension("css");
        let css = sass_rs::compile_file(source, self.options.clone())
            .expect("could not compile SASS file");
        fs::write(&dest, css)
            .with_context(|| format!("could not write CSS to `{}`", dest.display()))?;

        Ok(true)
    }
}
