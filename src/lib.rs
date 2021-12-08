use std::io::{prelude::*, BufReader};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::{fs, process};

use anyhow::{bail, ensure, Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use structopt::StructOpt;
use wasm_bindgen_cli_support::Bindgen;

#[derive(Debug, StructOpt)]
pub struct Build {
    #[structopt(long)]
    release: bool,
}

impl Build {
    pub fn execute(
        &self,
        crate_name: &'static str,
        static_dir_path: impl AsRef<Path>,
        build_dir_path: impl AsRef<Path>,
    ) -> Result<()> {
        log::trace!("Build: get package's metadata");
        let metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .context("cannot get package's metadata")?;

        log::trace!("Build: Initialize build process");
        let mut build_process = process::Command::new("cargo");
        build_process
            .current_dir(&metadata.workspace_root)
            .arg("build");

        if self.release {
            build_process.arg("--release");
        }

        build_process.args([
            "--target",
            "wasm32-unknown-unknown",
            "--package",
            crate_name,
        ]);

        let input_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(if self.release { "release" } else { "debug" })
            .join(&crate_name.replace("-", "_"))
            .with_extension("wasm");

        if input_path.exists() {
            log::trace!("Build: Removing existing target directory");
            fs::remove_file(&input_path).context("cannot remove existing target")?;
        }

        log::trace!("Build: Spawning build process");
        ensure!(
            build_process
                .status()
                .context("could not start cargo")?
                .success(),
            "cargo command failed"
        );

        log::trace!("Build: Generating wasm output");
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
            log::trace!("Removing already existing build directory");
            fs::remove_dir_all(&build_dir_path)?;
        }

        log::trace!("Build: Creating new build directory");
        fs::create_dir(&build_dir_path).context("cannot create build directory")?;

        log::trace!("Build: Writing files into build directory");
        fs::write(wasm_js_path, wasm_js).with_context(|| "cannot write js file")?;
        fs::write(wasm_bin_path, wasm_bin).with_context(|| "cannot write WASM file")?;

        let mut copy_options = fs_extra::dir::CopyOptions::new();
        copy_options.overwrite = true;
        copy_options.content_only = true;

        log::trace!("Build: Copying static directory into build directory");
        fs_extra::dir::copy(static_dir_path, build_dir_path, &copy_options)
            .context("cannot copy static directory")?;

        Ok(())
    }
}

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
        let listener = TcpListener::bind(&address).context("cannot bind to the given address")?;

        log::info!("Development server at: http://{}", &address);

        for mut stream in listener.incoming().filter_map(|x| x.ok()) {
            respond_to_request(&mut stream, &build_dir_path).unwrap_or_else(|e| {
                let _ = stream.write("HTTP/1.1 400 BAD REQUEST\r\n\r\n".as_bytes());
                log::error!("an error occurred: {}", e);
            });
        }

        Ok(())
    }

    pub fn watch(
        self,
        build_dir_path: impl AsRef<Path>,
        command: process::Command,
        watch: Watch,
    ) -> Result<()> {
        let build_dir_pathbuf = build_dir_path.as_ref().to_owned();

        let handle = std::thread::spawn(move || match self.serve(build_dir_pathbuf) {
            Ok(()) => {}
            Err(err) => log::error!("an error occurred when starting the dev server: {}", err),
        });

        match watch.execute(command, build_dir_path) {
            Ok(()) => {}
            Err(err) => log::error!("an error occurred when starting to watch: {}", err),
        }

        match handle.join() {
            Ok(()) => {}
            Err(err) => log::error!("problem waiting end of the watch: {:?}", err),
        }

        Ok(())
    }
}

fn respond_to_request(stream: &mut TcpStream, build_dir_path: impl AsRef<Path>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    let requested_path = request
        .split_whitespace()
        .nth(1)
        .context("Could not find path in request")?;

    let rel_path = Path::new(requested_path.trim_matches('/'));

    let mut full_path = build_dir_path.as_ref().join(rel_path);

    if full_path.is_dir() {
        if full_path.join("index.html").exists() {
            full_path = full_path.join("index.html")
        } else if full_path.join("index.htm").exists() {
            full_path = full_path.join("index.htm")
        } else {
            bail!("no index.html in {}", full_path.display());
        }
    }

    let stream = reader.get_mut();

    if full_path.is_file() {
        let full_path_extension = cargo_metadata::camino::Utf8Path::from_path(&full_path)
            .context("request path contains non-utf8 characters")?
            .extension();

        let content_type = match full_path_extension {
            Some("html") => "text/html;charset=utf-8",
            Some("css") => "text/css;charset=utf-8",
            Some("js") => "application/javascript",
            Some("wasm") => "application/wasm",
            _ => "application/octet-stream",
        };

        stream
            .write(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n",
                    full_path.metadata()?.len(),
                    content_type,
                )
                .as_bytes(),
            )
            .context("cannot write response")?;

        std::io::copy(&mut fs::File::open(&full_path)?, stream)?;
    } else {
        stream
            .write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes())
            .context("cannot write response")?;
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Watch {
    #[structopt(long, short = "w")]
    watch_paths: Vec<PathBuf>,
}

impl Watch {
    pub fn new(watch_paths: Vec<impl AsRef<Path>>) -> Self {
        Self {
            watch_paths: watch_paths
                .iter()
                .map(|path| path.as_ref().to_owned())
                .collect(),
        }
    }
    pub fn execute(
        &self,
        mut command: process::Command,
        build_dir_path: impl AsRef<Path>,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let mut watcher: RecommendedWatcher =
            notify::Watcher::new(tx, std::time::Duration::from_secs(2))
                .context("could not initialize watcher")?;

        let metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .context("cannot get package's metadata")?;
        let target_path = metadata.target_directory.as_std_path();
        let build_path = &metadata.workspace_root.as_std_path().join(build_dir_path);

        watcher
            .watch(&metadata.workspace_root, RecursiveMode::Recursive)
            .context("cannot watch this crate")?;

        for path in &self.watch_paths {
            match watcher.watch(path, RecursiveMode::Recursive) {
                Ok(()) => {}
                Err(err) => log::error!("cannot watch \"{:?}\": {}", path, err),
            }
        }

        let mut child = command.spawn().context("cannot spawn command")?;

        loop {
            use notify::DebouncedEvent::*;

            let message = rx.recv();

            match &message {
                Ok(Create(path)) | Ok(Write(path)) | Ok(Remove(path)) | Ok(Rename(_, path))
                    if !path.starts_with(build_path)
                        && !path.starts_with(target_path)
                        && !path
                            .file_name()
                            .and_then(|x| x.to_str())
                            .map(|x| x.starts_with('.'))
                            .unwrap_or(false) =>
                {
                    #[cfg(unix)]
                    {
                        let now = std::time::Instant::now();

                        unsafe {
                            libc::kill(
                                child.id().try_into().expect("cannot get process id"),
                                libc::SIGTERM,
                            );
                        }

                        while now.elapsed().as_secs() < 2 {
                            std::thread::sleep(std::time::Duration::from_millis(200));
                            if let Ok(Some(_)) = child.try_wait() {
                                break;
                            }
                        }
                    }

                    match child.try_wait() {
                        Ok(Some(_)) => {}
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }

                    child = command.spawn().context("cannot spawn command")?;
                }
                Ok(_) => {}
                Err(err) => log::error!("watch error: {}", err),
            };
        }
    }
}
