#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

#[cfg(not(target_arch = "wasm32"))]
pub use xtask_watch::{
    anyhow, cargo_metadata, cargo_metadata::camino, clap, metadata, package, xtask_command, Watch,
    WatchLock, WatchLockGuard,
};

#[cfg(not(target_arch = "wasm32"))]
mod dev_server;
#[cfg(not(target_arch = "wasm32"))]
mod dist;
#[cfg(all(not(target_arch = "wasm32"), feature = "sass"))]
mod sass;
#[cfg(all(not(target_arch = "wasm32"), feature = "wasm-opt"))]
mod wasm_opt;

#[cfg(not(target_arch = "wasm32"))]
pub use dev_server::*;
#[cfg(not(target_arch = "wasm32"))]
pub use dist::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "sass"))]
#[cfg_attr(docsrs, doc(cfg(feature = "sass")))]
pub use sass::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "wasm-opt"))]
#[cfg_attr(docsrs, doc(cfg(feature = "wasm-opt")))]
pub use wasm_opt::*;

#[cfg(all(not(target_arch = "wasm32"), feature = "sass"))]
#[cfg_attr(docsrs, doc(cfg(feature = "sass")))]
pub use sass_rs;

#[cfg(all(not(target_arch = "wasm32"), feature = "run-example"))]
#[cfg_attr(docsrs, doc(cfg(feature = "run-example")))]
pub use env_logger;

#[cfg(all(not(target_arch = "wasm32"), feature = "run-example"))]
#[cfg_attr(docsrs, doc(cfg(feature = "run-example")))]
pub use log;

/// Get the default command for the build in the dist process.
///
/// This is `cargo build --target wasm32-unknown-unknown`.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_build_command() -> Command {
    let mut command = Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}

#[cfg(all(target_arch = "wasm32", feature = "run-example"))]
pub use console_error_panic_hook;

#[cfg(all(target_arch = "wasm32", feature = "run-example"))]
pub use wasm_bindgen;

#[cfg(feature = "run-example")]
pub use xtask_wasm_run_example::*;
