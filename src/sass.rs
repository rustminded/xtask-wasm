use crate::anyhow::{Context, Result};
use crate::dist::Transformer;
use std::{fs, path::Path};

/// A [`Transformer`] that compiles SASS/SCSS files to CSS.
///
/// Files whose names begin with `_` are treated as partials and skipped (not emitted
/// to the dist directory). All other `.sass` and `.scss` files are compiled to `.css`.
/// Non-SASS files are not claimed and fall through to the default plain-copy behaviour.
///
/// Add this transformer to [`Dist`] to enable SASS/SCSS compilation:
///
/// ```rust,no_run
/// use xtask_wasm::{anyhow::Result, clap, SassTransformer};
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
///             dist.transformer(SassTransformer::default())
///                 .build("my-project")?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// To customise the compilation options, construct `SassTransformer` directly:
///
/// ```rust,no_run
/// use xtask_wasm::{anyhow::Result, clap, SassTransformer};
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
///             dist.transformer(SassTransformer {
///                     options: sass_rs::Options {
///                         output_style: sass_rs::OutputStyle::Compressed,
///                         ..Default::default()
///                     },
///                 })
///                 .build("my-project")?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// [`Dist`]: crate::Dist
pub struct SassTransformer {
    /// Options forwarded to [`sass_rs::compile_file`].
    pub options: sass_rs::Options,
}

impl Default for SassTransformer {
    fn default() -> Self {
        SassTransformer {
            options: sass_rs::Options::default(),
        }
    }
}

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
