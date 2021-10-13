use anyhow::{bail, ensure, Context, Result};
use log::{info, warn};
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
            info!("Generating build...")
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
use std::net::{IpAddr, SocketAddr};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;

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

        println!("Development server at: http://{}", &address);

        for stream in listener.incoming().filter_map(|x| x.ok()) {
            respond_to_request(stream, build_dir_path.to_path_buf()).unwrap_or_else(|e| {
                warn!("An error occurred: {}", e);
            });
        }

        Ok(())
    }
}

fn respond_to_request(mut stream: TcpStream, build_dir_path: PathBuf) -> Result<()> {
    let mut buffer = [0; 4096];

    stream
        .read(&mut buffer)
        .context("Cannot read from the stream")?;

    let request = String::from_utf8(buffer.to_vec())?;

    let requested_path = Path::new(
        request
            .split_whitespace()
            .nth(1)
            .expect("No path in the request"),
    );
    let response_path = if requested_path.ends_with("/") {
        build_dir_path.join("index.html")
    } else {
        build_dir_path.join(requested_path.strip_prefix("/").unwrap())
    };

    if response_path.exists() {
        let content = fs::read(&response_path).context("Cannot read from file")?;

        let content_type = if response_path.ends_with("html") {
            "content-type: text/html;charset=utf-8"
        } else if response_path.ends_with("js") {
            "content-type: application/javascript"
        } else if response_path.ends_with("wasm") {
            "content-type: application/wasm"
        } else if response_path.ends_with("css") {
            "content-type: text/css;charset=utf-8"
        } else {
            Default::default()
        };

        stream
            .write(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{}\r\n",
                    content.len(),
                    content_type,
                )
                .as_bytes(),
            )
            .context("Cannot write response")?;
        stream.write(&content).context("Cannot write content")?;
    } else {
        stream
            .write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes())
            .context("Cannot write response")?;
    };

    Ok(())
}
