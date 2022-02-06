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
//! ## Create a project using xtask
//!
//! * Create a new directory that will contains the two package of your project
//!     and the workspace's `Cargo.toml`
//!     ```console
//!     mkdir my-project
//!     cd my-project
//!     touch Cargo.toml
//!     ```
//! * Create the project package and the xtask package using `cargo new`:
//!     ```console
//!     cargo new my-project
//!     cargo new xtask
//!     ```
//!
//! * Open the workspace's `Cargo.toml` and add the following:
//!     ```toml
//!     [workspace]
//!     members = [
//!         "my-project",
//!         "xtask",
//!     ]
//!     ```
//!
//! ## Add a command alias
//!
//! Create a `.cargo/config.toml` file and add the following content:
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
//! [here](https://github.com/cargo-xtask/).
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
//! * [`Dist`](crate::dist::Dist) - Generate a distributed package for Wasm
//! * [`Watch`](https://docs.rs/xtask-watch/latest/xtask_watch/struct.Watch.html)
//!     - Re-run a given command when changes are detected
//!     (using [xtask-watch](https://github.com/rustminded/xtask-watch))
//! * [`DevServer`](crate::dev_server::DevServer) - Serve your project at a given IP address.
//!
//! They all implement [`clap::Parser`](https://docs.rs/clap/3.0.14/clap/trait.Parser.html)
//! allowing them to be added easily to an existing CLI implementation and are
//! flexible enough to be customized for most use-cases.
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
//!                 command.arg("check");
//!
//!                 log::info!("Starting to watch");
//!                 watch.run(command)?;
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
//!     provides an implementation of xtask-wasm to build the web app` package,
//!     an "hello world" app using [Yew](https://yew.rs/). This example
//!     demonstrates a simple directory layout and a customized build process
//!     that use the `wasm-opt` feature.
//!
//! The available subcommands are:
//!
//! * Build the web app package.
//!     ```console
//!     cargo xtask build
//!     ```
//!     * Build the web app package, download the
//!         [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools) binary
//!         and optimize the Wasm generated by the build process.
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
//! * `wasm-opt`: enable the [`WasmOpt`](crate::wasm_opt::WasmOpt) struct that helps downloading and using
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
