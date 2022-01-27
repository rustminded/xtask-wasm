//! This crate aims to provide an easy and customizable way to help you build
//! Wasm projects by extending them with custom subcommands, based on the
//! [`xtask` concept](https://github.com/matklad/cargo-xtask/), instead of using
//!  external tooling like [`wasm-pack`](https://github.com/rustwasm/wasm-pack).
//!
//! # Setup
//!
//! The best way to add [`xtask-wasm`] to your project is to create a workspace
//! with two packages: your project's package and the xtask package.
//!
//! ## Project with a single package
//!
//! If you project contains only one package, move all the content of the
//! project expect the `.git` directory into a new directory named after
//! the package name at the root of the project.
//!
//! * Create a new package for the xtasks using the following:
//!     ```console
//!     cargo new --bin xtask
//!     ```
//! * Create a new Cargo.toml at the root of the project and add the following:
//!     ```toml
//!     [workspace]
//!     members = [
//!         "project",
//!         "xtask",
//!     ]
//!     ```
//!     Replace `project` by the name of the project package
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
//! Now you can run your xtask package using:
//!
//! ```console
//! cargo xtask
//! ```
//!
//! ## Directory layout example
//!
//! If the name of the project package is `app`, the directory layout should
//! look like this:
//!
//! ```bash
//! project
//! ├── .cargo
//! │   └── config.toml
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
//! You can find more informations about xtask
//! [here](https://github.com/matklad/cargo-xtask/).
//!
//! ## Use xtask-wasm as a dependency
//!
//! Finally, add the following to the xtask package's Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! xtask-wasm = "0.1.0"
//! ```
//!
//! # Usage
//!
//! This library give you 3 types:
//!
//! * [`Dist`] - Generate a distributed package for Wasm
//! * [`Watch`] - Re-run a given command when changes are detected
//! * [`DevServer`] - Serve your project at a given IP address.
//!
//! They all implement [`clap::Parser`] allowing to add them easily to
//! an existing CLI implementation and are flexible enough to be customized for
//! most use-cases.
//!
//! You can find further for each type at their level documentation.
//!
//! # Examples
//!
//! [`examples/demo`](https://github.com/rustminded/xtask-wasm/tree/main/examples/demo)
//! provides an basic implementation of xtask-wasm to
//! build the `webapp` package, an `hello world` app using [Yew](https://yew.rs/).
//! This example demonstrate a simple directory layout and a customized build
//! process that use the [`wasm-opt`] feature.
//!
//! The available subcommands are:
//!
//! * Build the `webapp` package.
//!     ```console
//!     cargo xtask build
//!     ```
//!     * Build the `webapp` package, download the wasm-opt binary and optimize
//!         the Wasm generated by the build process.
//!         ```console
//!         cargo xtask build --optimize
//!         ```
//!
//! * Build the `webapp` package and watch for changes in the workspace root.
//!     ```console
//!     cargo xtask watch
//!     ```
//!
//! * Serve an optimized `webapp` dist on `127.0.0.1:8000` and watch for
//!     changes in the workspace root.
//!     ```console
//!     cargo xtask serve
//!     ```
//!
//! Additional flags can be found using `cargo xtask <subcommand> --help`
//!
//! # Features
//!
//! ## wasm-opt
//!
//! Enable `WasmOpt` that download the [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools)
//! binary and abstract its use to optimize the WASM.
//!
//! This feature can be enabled using the following in the xtask package's
//! Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! xtask-wasm = { version = "0.1.0", features = ["wasm-opt"] }
//! ```

#![deny(missing_docs)]

use lazy_static::lazy_static;
use std::process;

pub use anyhow;
pub use cargo_metadata;
pub use cargo_metadata::camino;
pub use clap;

mod dev_server;
mod dist;
#[cfg(feature = "wasm-opt")]
mod wasm_opt;
mod watch;

pub use dev_server::*;
pub use dist::*;
#[cfg(feature = "wasm-opt")]
pub use wasm_opt::*;
pub use watch::*;

/// Fetch the metadata of the crate.
pub fn metadata() -> &'static cargo_metadata::Metadata {
    lazy_static! {
        static ref METADATA: cargo_metadata::Metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("cannot get crate's metadata");
    }

    &METADATA
}

/// Fetch information of a package in the current crate.
pub fn package(name: &str) -> Option<&cargo_metadata::Package> {
    metadata().packages.iter().find(|x| x.name == name)
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

/// Get the default command for the build in the dist process.
///
/// This is `cargo build --target wasm32-unknown-unknown`.
pub fn default_build_command() -> process::Command {
    let mut command = process::Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}
