//! This crate aims to provide an easy and customizable way to help you build
//! Wasm projects by extending them with custom commands, based on the xtask<todo link>
//! concept, instead of using external tooling like `wasm-pack`.
//!
//! # Setup
//!
//! The best way to expend your project with xtask is to create a workspace for
//! your project.
//!
//! ## Project without workspace
//!
//! If you project contains only one package, move all the content of the
//! project expect the `.git` directory into a new directory named after
//! the package name at the root of the project.
//!
//! Create a new package for the xtasks using the following:
//!
//! ```console
//! cargo new --bin xtask
//! ```
//!
//! Create a new Cargo.toml at the root of the project and add the following:
//!
//! ```toml
//! [workspace]
//! members = [
//!     # Replace `project` by the name of the project package
//!     "project",
//!     "xtask",
//! ]
//! ```
//!
//! ## Project with a workspace
//!
//! If your project already use a workspace:
//! * Create a new package:
//!     ```console
//!     cargo new --bin xtask
//!     ```
//! * Add the new package to your workspace's Cargo.toml with the workspace
//!     members field
//!
//! ## Add a command alias
//!
//! Create a `.cargo` directory at the workspace root and add a file named
//! `config.toml` with the following content:
//!
//! ```toml
//! [alias]
//! xtask = "run --package xtask --"
//! ```
//!
//! ## Directory layout example
//!
//! If the name of my project is `app`, the directory layout should look like
//! this:
//!
//! ```bash
//! ├── Cargo.toml
//! ├── app
//! │   ├── Cargo.toml
//! │   └── src
//! │       └── ...
//! └── xtask
//!     ├── Cargo.toml
//!     └── src
//!         └── main.rs
//! ```
//!
//! You can find more informations about the xtask concept here<todo link>.
//!
//! ## Use xtask-wasm as a dependency
//!
//! Finally, add the following to the xtask's Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! xtask-wasm = "0.1.0"
//! ```
//!
//! wasm-opt<todo link> (`Loads WebAssembly and runs Binaryen IR passes on it.`)
//! is disabled by default, but can be enabled with the `wasm-opt` feature:
//!
//! ```toml
//! [dependencies]
//! xtask-wasm = { version = "0.1.0", features = ["wasm-opt"] }
//! ```

use lazy_static::lazy_static;
use std::process;

pub use anyhow;
pub use cargo_metadata;
pub use cargo_metadata::camino;
pub use clap;

mod build;
mod dev_server;
#[cfg(feature = "wasm-opt")]
mod wasm_opt;
mod watch;

pub use build::*;
pub use dev_server::*;
#[cfg(feature = "wasm-opt")]
pub use wasm_opt::*;
pub use watch::*;

/// Allows you to fetch the metadata of the crate
pub fn metadata() -> &'static cargo_metadata::Metadata {
    lazy_static! {
        static ref METADATA: cargo_metadata::Metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("cannot get crate's metadata");
    }

    &METADATA
}

/// Allows you to fetch informations of a package in the current crate
pub fn package(name: &str) -> Option<&cargo_metadata::Package> {
    metadata().packages.iter().find(|x| x.name == name)
}

/// Lazily return the default build directory.
///
/// Default for `target/debug/dist` in debug mode and `target/release/dist` in release mode
pub fn default_build_dir(release: bool) -> &'static camino::Utf8Path {
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

/// Lazily return the default command of the build process.
///
/// Equivalent to `cargo build --target wasm32-unknown-unknown`
pub fn default_build_command() -> process::Command {
    let mut command = process::Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}
