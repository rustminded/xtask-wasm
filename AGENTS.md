# AGENTS.md — xtask-wasm

Guidelines for agentic coding tools working in this repository.

---

## Repository Overview

`xtask-wasm` is a Rust library crate providing customizable xtask subcommands (`Dist`,
`Watch`, `DevServer`) for building WebAssembly projects without external tools like
`wasm-pack`. It is part of a Cargo workspace with one other member:
`xtask-wasm-run-example` (a proc-macro crate).

```
xtask-wasm/
├── src/                    # Library source (lib.rs, dist.rs, dev_server.rs, sass.rs, wasm_opt.rs)
├── xtask-wasm-run-example/ # Proc-macro workspace member
├── examples/demo/          # Demo workspace (separate Cargo workspace)
│   ├── webapp/             # Example wasm app
│   └── xtask/              # Example xtask binary using xtask-wasm
└── .github/workflows/      # CI definitions
```

---

## Build, Lint, and Test Commands

All commands are run from the repository root unless noted otherwise.

### Standard check/build
```bash
cargo check --workspace --all-features
cargo build --workspace --all-features
```

### Run all tests
```bash
cargo test --workspace --all-features
```

### Run a single test
```bash
# By test name (substring match)
cargo test --workspace --all-features <test_name>

# By test name in a specific crate
cargo test -p xtask-wasm --all-features <test_name>
```

### Formatting
```bash
# Check (CI uses this)
cargo fmt --all -- --check

# Apply
cargo fmt --all
```

### Linting (clippy)
```bash
# CI command — all warnings are errors
cargo clippy --all --tests --all-features -- -D warnings

# Local (softer, same flags)
cargo clippy --all --tests --all-features
```

### Check the demo workspace
```bash
# Must be run from the demo directory
cargo check -p xtask              # from examples/demo/
```

### CI matrix
CI runs on ubuntu/windows/macos against both stable and the MSRV declared in
`Cargo.toml` (`rust-version`). Always verify `cargo fmt` and `cargo clippy` pass before
committing.

---

## Code Style Guidelines

### Formatting

- Use `cargo fmt` (default `rustfmt` settings — no `rustfmt.toml` in this repo).
- 4-space indentation; no tabs.
- Trailing commas in multi-line struct literals and function call arguments.
- Method chains are broken across lines with a leading `.`:
  ```rust
  dist
      .assets_dir("static")
      .app_name("my-app")
      .build("my-app")?;
  ```

### Imports

- Group imports using nested paths where possible:
  ```rust
  use std::{fs, path::PathBuf, process};
  ```
- Standard library imports first, then external crates, then `crate::` / `super::`.
- Feature-gated imports use `#[cfg(...)]` attribute blocks, not inline `cfg!()` in use
  statements.
- Re-exports from `xtask_watch` (`anyhow`, `camino`, `clap`, etc.) are accessed via
  `crate::` — do not add duplicate direct dependencies when the re-export is sufficient.

### Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Types / traits | `PascalCase` | `Dist`, `DevServer`, `Transformer` |
| Functions / methods | `snake_case` | `build_command`, `copy_assets` |
| Fields / variables | `snake_case` | `dist_dir`, `app_name` |
| Constants / statics | `SCREAMING_SNAKE_CASE` | `WASM_OPT_URL` |
| Modules | `snake_case`, match filename | `dev_server`, `wasm_opt` |

### Error Handling

- All fallible functions return `anyhow::Result<T>` (re-exported as `crate::Result`).
- Prefer `context("…")` / `with_context(|| format!("…"))` from `anyhow` to annotate
  errors with actionable messages.
- Use `ensure!(condition, "message")` for precondition checks.
- Use `bail!("message")` for early exit with an error.
- `unwrap()` is acceptable only when the invariant is guaranteed by surrounding logic
  (e.g. `strip_prefix(…).unwrap()` inside a `WalkDir` iterator where prefix is known).
- `expect("message")` is acceptable where a panic would indicate a programmer bug; write
  a clear message describing what was expected.
- Do **not** introduce custom error types — stay with `anyhow` throughout.

### Types and Traits

- All public API structs (`Dist`, `DevServer`, `Request`) are marked `#[non_exhaustive]`
  to preserve semver compatibility.
- All public API structs derive `clap::Parser`; fields not from the CLI use
  `#[clap(skip)]`.
- Use `derive_more::Debug` (not `std::fmt::Debug`) so `#[debug(skip)]` can be applied
  to fields containing non-`Debug` types (e.g. `Box<dyn Transformer>`,
  `Arc<dyn Fn(…)>`).
- `impl Default` is written manually for structs that cannot derive it (e.g. anything
  containing `process::Command`).
- Builder pattern: methods take `mut self` and return `Self` for chaining.
- Use `Vec<Box<dyn Trait>>` for heterogeneous collections of hooks/transformers.
- Use `Arc<dyn Fn(…) + Send + Sync + 'static>` for shared callable fields.
- Use `lazy_static!` for lazily initialized statics.

### Documentation

- `#![deny(missing_docs)]` is active in `lib.rs` — **every public item must have a doc
  comment**.
- Use `///` for item-level docs; `//!` for module/crate-level docs.
- Include `# Examples` sections (with ` ```rust,no_run ``` `) for all significant public
  API items.
- Document `# Panics` sections when `expect(…)` can be triggered by incorrect user
  input.
- Feature-gated public items carry `#[cfg_attr(docsrs, doc(cfg(feature = "…")))]`.

### Conditional Compilation

- Feature flags: `run-example`, `sass`, `wasm-opt`. Add new opt-in functionality as
  optional features.
- Target-gated deps use `[target.'cfg(…)'.dependencies]` in `Cargo.toml`.
- Host-only (non-wasm) code is gated behind `#[cfg(not(target_arch = "wasm32"))]`.

### Logging

- Use the `log` crate macros: `log::trace!`, `log::debug!`, `log::info!`, `log::warn!`,
  `log::error!`.
- Do not use `println!` / `eprintln!` for diagnostic output; use `log::` macros.

---

## Dependency Philosophy

- Keep dependencies minimal; prefer what is already pulled in transitively (e.g. via
  `xtask-watch`).
- Avoid duplicating dependencies that are already re-exported from `xtask-watch`.
- `wasm-bindgen-cli-support` is used as a library instead of shelling out to
  `wasm-bindgen` — preserve this property.

---

## MSRV Policy

The minimum supported Rust version is declared in the root `Cargo.toml` under
`rust-version`. CI verifies both stable and MSRV. Do not use language or library
features introduced after that version without updating `rust-version` and the changelog.

---

## Changelog

All user-visible changes must be recorded in `CHANGELOG.md` under the `[Unreleased]`
section. Follow the Keep a Changelog format already in use.
