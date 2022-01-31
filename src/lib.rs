use lazy_static::lazy_static;
use std::process;

pub use xtask_watch::{
    anyhow, cargo_metadata, cargo_metadata::camino, clap, metadata, package, Watch,
};

mod build;
mod dev_server;
#[cfg(feature = "wasm-opt")]
mod wasm_opt;

pub use build::*;
pub use dev_server::*;
#[cfg(feature = "wasm-opt")]
pub use wasm_opt::*;

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

pub fn default_build_command() -> process::Command {
    let mut command = process::Command::new("cargo");
    command.args(["build", "--target", "wasm32-unknown-unknown"]);
    command
}
