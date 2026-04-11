# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.1] - 2026-04-11

### Changed

- `#[run_example]` now automatically applies
  `WasmOpt::level(1).shrink(2)` on release builds when the `wasm-opt`
  feature is enabled, matching the recommended manual setup.

## [0.6.0] - 2026-04-11

### Added

- `DevServer::xtask(name)` and `DevServer::cargo(subcommand)` convenience
  builders for setting the main watch command. These replace the old implicit
  behavior where `arg()` would silently create an xtask command if none was set.
- `DevServer::arg()`, `DevServer::args()`, `DevServer::env()`,
  `DevServer::envs()` restored as builder methods on `DevServer` for passing
  extra arguments/environment to the `xtask()` and `cargo()` shorthands (which
  build the command internally). These now panic with a clear message if called
  before a command is set — a programmer error, not a runtime condition.
- `DevServer::dist_dir(path)` builder to explicitly set the directory served by
  the dev server (previously had to be passed to `start()`).
- `Hook` trait for `DevServer` pre/post commands. Lets you construct a
  `process::Command` with access to the final server configuration (e.g.
  resolved `port`, `dist_dir`). A blanket impl is provided for
  `process::Command` itself.
- `Transformer` trait for `Dist` asset processing. Transformers are tried in
  order per file; the first to return `Ok(true)` claims the file, unclaimed
  files are plain-copied. Errors are propagated immediately via `?`.
- `Dist::transformer(impl Transformer)` builder to register asset transformers.
- `SassTransformer` struct (behind the `sass` feature) implementing `Transformer`
  to compile SASS/SCSS files to CSS.
- `Dist::optimize_wasm(WasmOpt)` builder (behind the `wasm-opt` feature) to
  integrate wasm-opt directly into the `build()` pipeline. Automatically skipped
  for debug builds.
- `Dist::default_debug_dir()` and `Dist::default_release_dir()` associated
  functions returning `camino::Utf8PathBuf`.
- "Why xtask-wasm?" section added to the README and crate-level documentation.

### Changed

- **BREAKING** `DevServer::start()` no longer takes a `dist_dir` path argument.
  The directory is inferred from `Dist::default_debug_dir()` or set via the new
  `DevServer::dist_dir()` builder.
- **BREAKING** `DevServer::command()` now accepts a `process::Command` instead
  of a program name string, consistent with `pre()` and `post()`.
- **BREAKING** `DevServer::not_found()` renamed to `DevServer::not_found_path()`.
- **BREAKING** `Dist::run()` renamed to `Dist::build()`.
- **BREAKING** `Dist::dist_dir_path()` renamed to `Dist::dist_dir()`.
- **BREAKING** `Dist::static_dir_path()` renamed to `Dist::assets_dir()`. The
  assets directory is now auto-discovered at `<package_root>/assets` when not
  explicitly set. The copy step is silently skipped (with a `log::debug!`) if
  the directory does not exist, allowing users with custom asset pipelines to
  call `build()` without a pre-existing assets directory on disk.
- **BREAKING** `run_in_workspace` field and `use_workspace_root()` /
  `use_current_dir()` methods removed from `Dist`. The build command always runs
  from the workspace root, which was the correct value in virtually all
  use-cases. The `false` branch was unreliable anyway since Cargo crate
  resolution requires being inside the workspace.
- **BREAKING** `SassTransformer` is now opt-in. Previously `Dist::default()`
  would auto-seed a `SassTransformer` when the `sass` feature was enabled,
  meaning any user adding their own `SassTransformer` would silently end up with
  two in the pipeline. Use `.transformer(SassTransformer::default())` to opt in.
- **BREAKING** `Request::dist_dir_path` field renamed to `Request::dist_dir`.
- `Dist::default_debug_dir()` and `Dist::default_release_dir()` now return an
  owned `camino::Utf8PathBuf` instead of `&'static camino::Utf8Path`, removing
  the `lazy_static` machinery and the `.as_std_path().to_path_buf()` boilerplate
  at call sites.
- `walkdir` promoted from a `sass`-feature-gated dependency to a regular
  dependency (used unconditionally by `copy_assets`).
- `cfg_not_wasm32!` / `cfg_wasm32!` / `cfg_sass!` / `cfg_wasm_opt!` /
  `cfg_run_example!` macros removed. These wrapped `mod` declarations inside
  macro bodies, making those modules invisible to `cargo fmt` and silently
  skipping format checks on the bulk of the codebase. Replaced with plain
  `#[cfg(...)]` attributes.
- MSRV bumped to 1.88.
- `run_example` macro: `static_dir` argument renamed to `assets_dir`.
- `run_example` macro: default `index.html` is no longer generated when
  `app_name` is set. The default template hardcodes `app.js`/`app_bg.wasm`;
  with a custom app name those filenames are wrong. Users setting `app_name`
  should provide their own `index.html` via the `index` argument or in their
  assets directory.

### Removed

- **BREAKING** Free functions `default_dist_dir(release: bool)`,
  `default_dist_dir_debug()`, and `default_dist_dir_release()` removed in favor
  of `Dist::default_debug_dir()` and `Dist::default_release_dir()`.
- **BREAKING** `Dist::sass_options()` removed. Configure SASS compilation
  options via `SassTransformer { options: ... }` passed to `Dist::transformer()`.

### Fixed

- `SassTransformer`: SASS compilation errors are now propagated as `Err` instead
  of panicking.
- `copy_assets`: transformer errors are now propagated immediately. The previous
  iterator chain used `find()` with `map_or(false, ...)` which treated `Err` as
  `false` and silently fell through to the plain-copy fallback.
- Auto-discovery was looking for a `public/` directory while the rest of the
  codebase used `assets`, silently breaking auto-discovery for anyone following
  the documented naming convention.

[Unreleased]: https://github.com/rustminded/xtask-wasm/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/rustminded/xtask-wasm/compare/v0.5.3...v0.6.0
