use anyhow::{bail, ensure, Context, Result};
use std::path::Path;
use std::{fs, process};
use structopt::StructOpt;
use wasm_bindgen_cli_support::Bindgen;

#[derive(Debug, StructOpt)]
pub struct Build {
    #[structopt(long)]
    release: bool,
    #[structopt(short, long)]
    quiet: bool,
}

impl Build {
    pub fn run(
        &self,
        crate_name: &'static str,
        static_dir_path: impl AsRef<Path>,
        build_dir_path: impl AsRef<Path>,
    ) -> Result<()> {
        let metadata = match cargo_metadata::MetadataCommand::new().exec() {
            Ok(metadata) => metadata,
            Err(_) => bail!("Cannot get package's metadata"),
        };

        let mut build_process = process::Command::new("cargo");
        build_process
            .current_dir(&metadata.workspace_root)
            .arg("build");

        if self.release {
            build_process.arg("--release");
        }

        if self.quiet {
            build_process.arg("--quiet");
        }

        build_process.args([
            "--target",
            "wasm32-unknown-unknown",
            "--package",
            crate_name,
        ]);

        ensure!(
            build_process
                .status()
                .context("Could not start cargo")?
                .success(),
            "Cargo command failed"
        );

        if !self.quiet {
            println!("Generating build...")
        }

        let input_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join("debug")
            .join(&crate_name.replace("-", "_"))
            .with_extension("wasm");

        let mut output = Bindgen::new()
            .input_path(input_path)
            .out_name("app")
            .web(true)
            .expect("web have panic")
            .debug(!self.release)
            .generate_output()
            .context("could not generate WASM bindgen file")?;

        let wasm_js = output.js().to_owned();
        let wasm_bin = output.wasm_mut().emit_wasm();

        let build_dir_path = build_dir_path.as_ref();

        let wasm_js_path = build_dir_path.join("app.js");
        let wasm_bin_path = build_dir_path.join("app_bg.wasm");

        if build_dir_path.exists() {
            fs::remove_dir_all(&build_dir_path)?;
        }

        let _ = fs::create_dir(&build_dir_path);

        fs::write(wasm_js_path, wasm_js).with_context(|| "Cannot write js file")?;
        fs::write(wasm_bin_path, wasm_bin).with_context(|| "Cannot write WASM file")?;

        let mut copy_options = fs_extra::dir::CopyOptions::new();
        copy_options.overwrite = true;
        copy_options.content_only = true;

        fs_extra::dir::copy(static_dir_path, build_dir_path, &copy_options)
            .context("Cannot copy static directory")?;

        Ok(())
    }
}

use std::io::prelude::*;
use std::net::TcpListener;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, StructOpt)]
pub struct DevServer {
    #[structopt(long, default_value = "127.0.0.1")]
    ip: IpAddr,
    #[structopt(long, default_value = "8000")]
    port: u16,
}

impl DevServer {
    pub fn serve(&self, build_dir_path: impl AsRef<Path>) -> Result<()> {
        let address = SocketAddr::new(self.ip, self.port);
        let listener = TcpListener::bind(&address).context("Cannot bind to the given address")?;
        let build_dir_path = build_dir_path.as_ref();
        let index = build_dir_path.join("index.html");

        println!("{}", &address);

        for stream in listener.incoming() {
            let mut stream = stream.context("Error in the incoming stream")?;
            let mut buffer = [0; 1024];

            stream
                .read(&mut buffer)
                .context("Cannot read from the stream")?;

            let contents = fs::read_to_string(&index).expect("Cannot read index content");

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                contents.len(),
                contents
            );

            stream
                .write(response.as_bytes())
                .context("Cannot write response")?;
            stream.flush()?;
        }

        Ok(())
    }
}
