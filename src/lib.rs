#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! This crate aims to provide an easy and customizable way to help you build
//! Wasm projects by extending them with custom subcommands, based on the
//! [`xtask` concept](https://github.com/matklad/cargo-xtask/), instead of using
//! external tooling like [`wasm-pack`](https://github.com/rustwasm/wasm-pack).
//!
//! # Minimum Supported Rust Version
//!
//! This crate requires **Rust 1.58.1** at a minimum because there is a security
//! issue on a function we use from std in previous version
//! (see [cve-2022-21658](https://groups.google.com/g/rustlang-security-announcements/c/R1fZFDhnJVQ)).
//!
//! # Setup
//!
//! The best way to add xtask-wasm to your project is to create a workspace
//! with two packages: your project's package and the xtask package.
//!
//! ## Create a project using xtask
//!
//! * Create a new directory that will contains the two package of your project
//!   and the workspace's `Cargo.toml`:
//!
//!   ```console
//!   mkdir my-project
//!   cd my-project
//!   touch Cargo.toml
//!   ```
//!
//! * Create the project package and the xtask package using `cargo new`:
//!
//!   ```console
//!   cargo new my-project
//!   cargo new xtask
//!   ```
//!
//! * Open the workspace's `Cargo.toml` and add the following:
//!
//!   ```toml
//!   [workspace]
//!   default-members = ["my-project"]
//!   members = [
//!       "my-project",
//!       "xtask",
//!   ]
//!   ```
//!
//! * Create a `.cargo/config.toml` file and add the following content:
//!
//!   ```toml
//!   [alias]
//!   xtask = "run --package xtask --"
//!   ```
//!
//! The directory layout should look like this:
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
//! And now you can run your xtask package using:
//!
//! ```console
//! cargo xtask
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
//! This library gives you three structs:
//!
//! * [`Dist`](crate::dist::Dist) - Generate a distributed package for Wasm.
//! * [`Watch`](https://docs.rs/xtask-watch/latest/xtask_watch/struct.Watch.html) -
//!   Re-run a given command when changes are detected
//!   (using [xtask-watch](https://github.com/rustminded/xtask-watch)).
//! * [`DevServer`](crate::dev_server::DevServer) - Serve your project at a given IP address.
//!
//! They all implement [`clap::Parser`](https://docs.rs/clap/latest/clap/trait.Parser.html)
//! allowing them to be added easily to an existing CLI implementation and are
//! flexible enough to be customized for most use-cases.
//!
//! You can find further information for each type at their documentation level.
//!
//! # Examples
//!
//! ## A basic implementation
//!
//! ```rust,no_run
//! use std::process::Command;
//! use xtask_wasm::{anyhow::Result, clap, default_dist_dir};
//!
//! #[derive(clap::Parser)]
//! enum Opt {
//!     Dist(xtask_wasm::Dist),
//!     Watch(xtask_wasm::Watch),
//!     Start(xtask_wasm::DevServer),
//! }
//!
//!
//! fn main() -> Result<()> {
//!     let opt: Opt = clap::Parser::parse();
//!
//!     match opt {
//!         Opt::Dist(dist) => {
//!             log::info!("Generating package...");
//!
//!             dist
//!                 .dist_dir_path("dist")
//!                 .static_dir_path("my-project/static")
//!                 .app_name("my-project")
//!                 .run_in_workspace(true)
//!                 .run("my-project")?;
//!         }
//!         Opt::Watch(watch) => {
//!             log::info!("Watching for changes and check...");
//!
//!             let mut command = Command::new("cargo");
//!             command.arg("check");
//!
//!             watch.run(command)?;
//!         }
//!         Opt::Start(mut dev_server) => {
//!             log::info!("Starting the development server...");
//!
//!             dev_server.arg("dist").start(default_dist_dir(false))?;
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## [`examples/demo`](https://github.com/rustminded/xtask-wasm/tree/main/examples/demo)
//!
//! Provides a basic implementation of xtask-wasm to generate the web app
//! package, an "hello world" app using [Yew](https://yew.rs/). This example
//! demonstrates a simple directory layout and a customized dist process
//! that use the `wasm-opt` feature.
//!
//! The available subcommands are:
//!
//! * Build the web app package.
//!
//!   ```console
//!   cargo xtask dist
//!   ```
//!   * Build the web app package, download the [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools)
//!     binary and optimize the Wasm generated by the dist process.
//!
//!     ```console
//!     cargo xtask dist --optimize
//!     ```
//!
//! * Build the web app package and watch for changes in the workspace root.
//!
//!   ```console
//!   cargo xtask watch
//!   ```
//!
//! * Serve an optimized web app dist on `127.0.0.1:8000` and watch for
//!   changes in the workspace root.
//!
//!   ```console
//!   cargo xtask start
//!   ```
//!
//! Additional flags can be found using `cargo xtask <subcommand> --help`.
//!
//! This example also demonstrates the use of the `run-example` feature that allows you to use the
//! following:
//!
//! ```console
//! cargo run --example run_example
//! ```
//!
//! This command will run the code in `examples/run_example` using the development server.
//!
//! # Features
//!
//! * `wasm-opt`: enable the [`WasmOpt`](crate::wasm_opt::WasmOpt) struct that helps downloading
//!     and using [`wasm-opt`](https://github.com/WebAssembly/binaryen#tools) very easily.
//! * `run-example`: a helper to run examples from `examples/` directory using a development
//!     server.
//! * `sass`: allow the use of SASS/SCSS in your project.
//!
//! # Troubleshooting
//!
//! When using the re-export of [`clap`](https://docs.rs/clap/latest/clap), you
//! might encounter this error:
//!
//! ```console
//! error[E0433]: failed to resolve: use of undeclared crate or module `clap`
//!  --> xtask/src/main.rs:4:10
//!   |
//! 4 | #[derive(Parser)]
//!   |          ^^^^^^ use of undeclared crate or module `clap`
//!   |
//!   = note: this error originates in the derive macro `Parser` (in Nightly builds, run with -Z macro-backtrace for more info)
//! ```
//!
//! This occurs because you need to import clap in the scope too. This error can
//! be resolved like this:
//!
//! ```rust
//! use xtask_wasm::clap;
//!
//! #[derive(clap::Parser)]
//! struct MyStruct {}
//! ```
//!
//! Or like this:
//!
//! ```rust
//! use xtask_wasm::{clap, clap::Parser};
//!
//! #[derive(Parser)]
//! struct MyStruct {}
//! ```

#[macro_use]
mod cfg;

cfg_not_wasm32! {
    use std::process::Command;

    pub use xtask_watch::{
        anyhow, cargo_metadata, cargo_metadata::camino, clap, metadata, package, xtask_command,
        Watch,
    };

    mod dev_server;
    mod dist;

    pub use dev_server::*;
    pub use dist::*;

    cfg_run_example! {
        pub use env_logger;
        pub use log;
    }

    cfg_wasm_opt! {
        mod wasm_opt;
        pub use wasm_opt::*;
    }

    cfg_sass! {
        pub use sass_rs;
    }

    /// Get the default command for the build in the dist process.
    ///
    /// This is `cargo build --target wasm32-unknown-unknown`.
    pub fn default_build_command() -> Command {
        let mut command = Command::new("cargo");
        command.args(["build", "--target", "wasm32-unknown-unknown"]);
        command
    }
}

cfg_wasm32! {
    cfg_run_example! {
        pub use console_error_panic_hook;
        pub use wasm_bindgen;
    }
}

cfg_run_example! {
    pub use xtask_wasm_run_example::*;
}
