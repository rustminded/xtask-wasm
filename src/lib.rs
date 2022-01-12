use std::io::{prelude::*, BufReader};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::{fs, process};

use anyhow::{bail, ensure, Context, Result};
use lazy_static::lazy_static;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use structopt::StructOpt;
use wasm_bindgen_cli_support::Bindgen;

pub use camino;

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

pub fn default_build_dir(release: bool) -> &'static camino::Utf8PathBuf {
    if release {
        lazy_static! {
            static ref DEFAULT_RELEASE_PATH: camino::Utf8PathBuf =
                metadata().target_directory.join("release").join("dist");
        }

        &DEFAULT_RELEASE_PATH
    } else {
        lazy_static! {
            static ref DEFAULT_DEBUG_PATH: camino::Utf8PathBuf =
                metadata().target_directory.join("debug").join("dist");
        }

        &DEFAULT_DEBUG_PATH
    }
}

#[non_exhaustive]
#[derive(Debug, StructOpt)]
pub struct Build {
    #[structopt(long)]
    pub release: bool,

    #[structopt(skip = default_build_command())]
    pub command: process::Command,
    #[structopt(skip)]
    pub build_dir_path: Option<PathBuf>,
    #[structopt(skip = true)]
    pub run_in_workspace: bool,
}

fn default_build_command() -> process::Command {
    let mut command = process::Command::new("cargo");

    command.args(["build", "--target", "wasm32-unknown-unknown"]);

    command
}

impl Build {
    pub fn execute(self, crate_name: &str, static_dir_path: impl AsRef<Path>) -> Result<PathBuf> {
        log::trace!("Getting package's metadata");
        let metadata = metadata();

        let build_dir_path = self
            .build_dir_path
            .unwrap_or_else(|| default_build_dir(self.release).clone().into_std_path_buf());

        log::trace!("Initializing build process");
        let mut build_process = self.command;

        if self.run_in_workspace {
            build_process.current_dir(&metadata.workspace_root);
        }

        if self.release {
            build_process.arg("--release");
        }

        build_process.args(["--package", crate_name]);

        let input_path = metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(if self.release { "release" } else { "debug" })
            .join(&crate_name.replace("-", "_"))
            .with_extension("wasm");

        if input_path.exists() {
            log::trace!("Removing existing target directory");
            fs::remove_file(&input_path).context("cannot remove existing target")?;
        }

        log::trace!("Spawning build process");
        ensure!(
            build_process
                .status()
                .context("could not start cargo")?
                .success(),
            "cargo command failed"
        );

        log::trace!("Generating wasm output");
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

        let wasm_js_path = build_dir_path.join("app.js");
        let wasm_bin_path = build_dir_path.join("app_bg.wasm");

        if build_dir_path.exists() {
            log::trace!("Removing already existing build directory");
            fs::remove_dir_all(&build_dir_path)?;
        }

        log::trace!("Creating new build directory");
        fs::create_dir_all(&build_dir_path).context("cannot create build directory")?;

        log::trace!("Writing files into build directory");
        fs::write(wasm_js_path, wasm_js).with_context(|| "cannot write js file")?;
        fs::write(wasm_bin_path, wasm_bin).with_context(|| "cannot write WASM file")?;

        let mut copy_options = fs_extra::dir::CopyOptions::new();
        copy_options.overwrite = true;
        copy_options.content_only = true;

        log::trace!("Copying static directory into build directory");
        fs_extra::dir::copy(&static_dir_path, &build_dir_path, &copy_options)
            .context("cannot copy static directory")?;

        log::info!("Builded successfully at {}", build_dir_path.display());

        Ok(build_dir_path)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, StructOpt)]
pub struct Watch {
    #[structopt(long = "watch", short = "w")]
    pub watch_paths: Vec<PathBuf>,
    #[structopt(long = "ignore", short = "i")]
    pub exclude_paths: Vec<PathBuf>,

    #[structopt(skip)]
    pub workspace_exclude_paths: Vec<PathBuf>,
}

impl Watch {
    pub fn exclude_path(&mut self, path: impl AsRef<Path>) {
        self.exclude_paths.push(path.as_ref().to_path_buf())
    }

    pub fn exclude_paths(&mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) {
        for path in paths {
            self.exclude_path(path)
        }
    }

    pub fn exclude_workspace_path(&mut self, path: impl AsRef<Path>) {
        let metadata = metadata();

        self.workspace_exclude_paths
            .push(metadata.workspace_root.as_std_path().join(path))
    }

    pub fn exclude_workspace_paths(&mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) {
        for path in paths {
            self.exclude_workspace_path(path)
        }
    }

    pub fn watch_path(&mut self, path: impl AsRef<Path>) {
        self.watch_paths.push(path.as_ref().to_path_buf())
    }

    pub fn watch_paths(&mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) {
        for path in paths {
            self.watch_path(path)
        }
    }

    fn is_excluded_path(&mut self, path: &Path) -> bool {
        self.exclude_paths.iter().any(|x| path.starts_with(x))
            || self
                .workspace_exclude_paths
                .iter()
                .any(|x| path.starts_with(x))
    }

    fn is_hidden_path(&mut self, path: &Path) -> bool {
        path.file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.starts_with('.'))
            .unwrap_or(false)
    }

    pub fn execute(&mut self, mut command: process::Command) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let mut watcher: RecommendedWatcher =
            notify::Watcher::new(tx, std::time::Duration::from_secs(2))
                .context("could not initialize watcher")?;

        let metadata = metadata();

        self.exclude_path(&metadata.target_directory);

        if self.watch_paths.is_empty() {
            log::trace!("Watching {}", &metadata.workspace_root);
            watcher
                .watch(&metadata.workspace_root, RecursiveMode::Recursive)
                .context("cannot watch this crate")?;
        } else {
            for path in &self.watch_paths {
                match watcher.watch(&path, RecursiveMode::Recursive) {
                    Ok(()) => log::trace!("Watching {}", path.display()),
                    Err(err) => log::error!("cannot watch {}: {}", path.display(), err),
                }
            }
        }

        let mut child = command.spawn().context("cannot spawn command")?;

        loop {
            use notify::DebouncedEvent::*;

            let message = rx.recv();

            match &message {
                Ok(Create(path)) | Ok(Write(path)) | Ok(Remove(path)) | Ok(Rename(_, path))
                    if !self.is_excluded_path(path) && !self.is_hidden_path(path) =>
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

#[non_exhaustive]
#[derive(Debug, StructOpt)]
pub struct DevServer {
    #[structopt(long, default_value = "127.0.0.1")]
    pub ip: IpAddr,
    #[structopt(long, default_value = "8000")]
    pub port: u16,

    #[structopt(flatten)]
    pub watch: Watch,
    #[structopt(skip)]
    pub served_path: Option<PathBuf>,
    #[structopt(skip)]
    pub release: bool,
}

impl DevServer {
    pub fn serve(&self) -> Result<()> {
        let served_path = self
            .served_path
            .clone()
            .unwrap_or_else(|| default_build_dir(self.release).clone().into_std_path_buf());

        let address = SocketAddr::new(self.ip, self.port);
        let listener = TcpListener::bind(&address).context("cannot bind to the given address")?;

        log::info!("Development server running at: http://{}", &address);

        for mut stream in listener.incoming().filter_map(|x| x.ok()) {
            respond_to_request(&mut stream, &served_path).unwrap_or_else(|e| {
                let _ = stream.write("HTTP/1.1 400 BAD REQUEST\r\n\r\n".as_bytes());
                log::error!("an error occurred: {}", e);
            });
        }

        Ok(())
    }

    pub fn serve_and_watch(self, command: process::Command) -> Result<()> {
        let mut watch = self.watch.clone();

        if let Some(path) = &self.served_path {
            watch.exclude_workspace_path(path);
        }

        let handle = std::thread::spawn(move || match self.serve() {
            Ok(()) => log::trace!("Starting server"),
            Err(err) => log::error!("an error occurred when starting the dev server: {}", err),
        });

        match watch.execute(command) {
            Ok(()) => log::trace!("Starting to watch"),
            Err(err) => log::error!("an error occurred when starting to watch: {}", err),
        }

        match handle.join() {
            Ok(()) => log::trace!("Ending watch"),
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
        let full_path_extension = camino::Utf8Path::from_path(&full_path)
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
