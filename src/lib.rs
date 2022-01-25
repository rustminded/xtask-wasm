use lazy_static::lazy_static;
use std::process;

mod build;
mod dev_server;
#[cfg(feature = "wasm-opt")]
mod wasm_opt;
mod watch;

pub use build::*;
pub use dev_server::*;
#[cfg(feature = "wasm-opt")]
pub use wasm_opt::WasmOpt;
pub use watch::*;

pub use anyhow;
pub use cargo_metadata;
pub use cargo_metadata::camino;

pub fn metadata() -> &'static cargo_metadata::Metadata {
    lazy_static! {
        static ref METADATA: cargo_metadata::Metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .expect("cannot get crate's metadata");
    }

    &METADATA
}

pub fn package(name: &str) -> Option<&cargo_metadata::Package> {
    metadata().packages.iter().find(|x| x.name == name)
}

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
