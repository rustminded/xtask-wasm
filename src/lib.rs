//! This crate aims to provide an easy and customizable way to help you build
//! Wasm projects by extending them with custom subcommands, based on the
//! [`xtask` concept](https://github.com/matklad/cargo-xtask/), instead of using
//! external tooling like [`wasm-pack`](https://github.com/rustwasm/wasm-pack).
//!
//! # Minimum Supported Rust Version
//!
//! This crate requires **Rust 1.58.1** at a minimum because there is a security
//! issue on a function we use in std in previous version.
//!
//! # Setup
//!
//! The best way to add xtask-wasm to your project is to create a workspace
//! with two packages: your project's package and the xtask package.
//!
//! ## Create or update a project using xtask
//!
//! * New project: You can create a new project using xtask, you can use the
//!     following:
//!     ```console
//!     mkdir project_name
//!     cd project_name
//!     cargo new project
//!     cargo new xtask
//!     touch Cargo.toml
//!     ```
//!     Open the workspace's `Cargo.toml` and add the following:
//!     ```toml
//!     [workspace]
//!     default-members = ["my-project"]
//!     members = [
//!         "project",
//!         "xtask",
//!     ]
//!     ```
//!
//! * Project with a single package: If you project contains only one package,
//!     move all the content of the project except the `.git` directory into a
//!     new directory named after the package name at the root of the project.
//!     * Create a new package for the xtasks using the following:
//!         ```console
//!         cargo new xtask
//!         ```
//!     * Create a new `Cargo.toml` at the root of the project and add the following:
//!         ```toml
//!         [workspace]
//!         default-members = ["my-project"]
//!         members = [
//!             "my-project",
//!             "xtask",
//!         ]
//!         ```
//!         Replace `my-project` by the name of the project package.
//!
//! * Project with a workspace: If your project already uses a workspace,
//!     * Create a new package:
//!         ```console
//!         cargo new xtask
//!         ```
//!     * Add the new package to your workspace's `Cargo.toml` like this:
//!         ```toml
//!         [workspace]
//!         default-members = [..]
//!         members = [
//!             ..
//!             "xtask",
//!         ]
//!         ```
//!
//! ## Add a command alias
//!
//! Create the file `.cargo/config.toml` if it doesn't already exit and add the
//! following content:
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
//! If the name of the project package is `my-project`, the directory layout should
//! look like this:
//!
//! ```console
//! project
//! ├── .cargo
//! │   └── config.toml
//! ├── Cargo.toml
//! ├── my-project
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
//! Finally, add the following to the xtask package's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! xtask-wasm = "0.1.0"
//! ```
//!
//! # Usage
//!
//! This library gives you 3 [clap](https://docs.rs/clap/latest/clap/) structs:
//!
//! * [`Dist`] - Generate a distributed package for Wasm
//! * [`Watch`] - Re-run a given command when changes are detected (using
//!     [xtask-watch](https://github.com/rustminded/xtask-watch))
//! * [`DevServer`] - Serve your project at a given IP address.
//!
//! They all implement [`clap::Parser`] allowing to add them easily to
//! an existing CLI implementation and are flexible enough to be customized for
//! most use-cases.
//!
//! You can find further information for each type at their documentation level.
//!
//! # Examples
//!
//! * A basic implementation could look like this:
//!     ```rust,no_run
//!     use std::process::Command;
//!     use xtask_wasm::{anyhow::Result, clap};
//!
//!     #[derive(clap::Parser)]
//!     enum Opt {
//!         Dist(xtask_wasm::Dist),
//!         Watch(xtask_wasm::Watch),
//!         Serve(xtask_wasm::DevServer),
//!     }
//!
//!
//!     fn main() -> Result<()> {
//!         let opt: Opt = clap::Parser::parse();
//!
//!         match opt {
//!             Opt::Dist(dist) => {
//!                 let dist = dist
//!                     .dist_dir_path("dist")
//!                     .static_dir_path("project/static")
//!                     .app_name("project")
//!                     .run_in_workspace(true)
//!                     .run("project")?;
//!
//!                 log::info!("Built at {}", dist.dist_dir.display());
//!
//!             }
//!             Opt::Watch(watch) => {
//!                 let mut command = Command::new("cargo");
//!                 command.args(["xtask", "dist"]);
//!
//!                 log::info!("Starting to watch");
//!                 watch.exclude_workspace_path("dist").run(command)?;
//!             }
//!             Opt::Serve(mut dev_server) => {
//!                 let mut command = Command::new("cargo");
//!                 command.args(["xtask", "dist"]);
//!
//!                 log::info!("Starting the dev server");
//!                 dev_server.command(command).start("dist")?;
//!             }
//!         }
//!
//!         Ok(())
//!     }
//!     ```
//!
//! * [`examples/demo`](https://github.com/rustminded/xtask-wasm/tree/main/examples/demo)
//!     provides an implementation of xtask-wasm to build the `webapp` package, an
//!     "hello world" app using [Yew](https://yew.rs/). This example
//!     demonstrates a simple directory layout and a customized build process
//!     that use the [`wasm-opt`] feature.
//!
//! The available subcommands are:
//!
//! * Build the web app package.
//!     ```console
//!     cargo xtask build
//!     ```
//!     * Build the `webapp` package, download the wasm-opt binary and optimize
//!         the Wasm generated by the build process.
//!         ```console
//!         cargo xtask build --optimize
//!         ```
//!
//! * Build the web app package and watch for changes in the workspace root.
//!     ```console
//!     cargo xtask watch
//!     ```
//!
//! * Serve an optimized web app dist on `127.0.0.1:8000` and watch for
//!     changes in the workspace root.
//!     ```console
//!     cargo xtask serve
//!     ```
//!
//! Additional flags can be found using `cargo xtask <subcommand> --help`
//!
//! # Features
//!
//! * `wasm-opt`: enable the [`WasmOpt`] struct that helps downloading and using
//!     [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools) very easily.

#![deny(missing_docs)]

use std::process::Command;

pub use xtask_watch::{
    anyhow, cargo_metadata, cargo_metadata::camino, clap, metadata, package, Watch,
};

mod dev_server;
mod dist;
#[cfg(feature = "wasm-opt")]
mod wasm_opt;

pub use dev_server::*;
pub use dist::*;
#[cfg(feature = "wasm-opt")]
pub use wasm_opt::*;

/// Get the default command for the build in the dist process.
///
/// This is `cargo build --target wasm32-unknown-unknown`.
pub fn default_build_command() -> Command {
    let mut command = Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}
