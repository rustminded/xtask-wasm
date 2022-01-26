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
//! * Create a new package for the xtasks using the following:
//!     ```console
//!     cargo new --bin xtask
//!     ```
//! * Create a new Cargo.toml at the root of the project and add the following:
//!     ```toml
//!     [workspace]
//!     members = [
//!         # Replace `project` by the name of the project package
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
//!
//! # Usage
//!
//! This library give you 3 types for your project:
//!
//! * `Build`<todo intra-link> - Build your project,
//! * `Watch`<todo intra-link> - Watches over your project's,
//! * `DevServer`<todo intra-link> - Serve your project.
//!
//! They all implement `clap::Parser` allowing the user of the library to add
//! them easily to an existing CLI system. They come ready out of the box but
//! can be customized in different ways.
//!
//! You can find further for each type at their level documentation.
//!
//! # Examples
//!
//! examples/demo<todo link> provides an basic implementation of xtask-wasm to
//! build the `webapp` package, an `hello world` app using Yew<todo link>.
//! This example demonstrate a simple directory layout and a customized build
//! process that use the `wasm-opt`<todo intra link> feature.
//!
//! The available subcommands are:
//!
//! * `cargo xtask build`
//! * `cargo xtask watch`
//! * `cargo xtask serve`
//!
//! Additional flags can be found using `cargo xtask <subcommand> --help`
//!
//! # Features
//!
//! ## wasm-opt
//!
//! Enable `WasmOpt`<todo (intra?) link> that download the `wasm-opt`<todo link>
//! binary and abstract its use via a builder pattern to optimize the WASM.

// #![deny(missing_docs)]

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

/// Get the default build directory.
///
/// The default for debug build is `target/debug/dist` and `target/release/dist`
/// for the release build.
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

/// Get the default command for the build process.
///
/// This is `cargo build --target wasm32-unknown-unknown`.
pub fn default_build_command() -> process::Command {
    let mut command = process::Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}
