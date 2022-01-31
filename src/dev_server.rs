use crate::{
    anyhow::{bail, Context, Result},
    camino::Utf8Path,
    clap, Watch,
};
use std::{
    fs,
    io::{prelude::*, BufReader},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    process,
};

#[non_exhaustive]
#[derive(Debug, clap::Parser)]
pub struct DevServer {
    #[clap(long, default_value = "127.0.0.1")]
    pub ip: IpAddr,
    #[clap(long, default_value = "8000")]
    pub port: u16,

    #[clap(flatten)]
    pub watch: Watch,
    #[clap(skip)]
    pub command: Option<process::Command>,
}

impl DevServer {
    pub fn command(mut self, command: process::Command) -> Self {
        self.command = Some(command);
        self
    }

    pub fn start(self, served_path: impl AsRef<Path>) -> Result<()> {
        let watch_process = if let Some(command) = self.command {
            let watch = self.watch.exclude_path(&served_path);
            let handle = std::thread::spawn(|| match watch.run(command) {
                Ok(()) => log::trace!("Starting to watch"),
                Err(err) => log::error!("an error occurred when starting to watch: {}", err),
            });

            Some(handle)
        } else {
            None
        };

        match serve(self.ip, self.port, served_path) {
            Ok(()) => log::trace!("Starting to serve"),
            Err(err) => log::error!("an error occurred when starting to serve: {}", err),
        }

        if let Some(handle) = watch_process {
            handle.join().expect("an error occurred when exiting watch");
        }

        Ok(())
    }
}

fn serve(ip: IpAddr, port: u16, served_path: impl AsRef<Path>) -> Result<()> {
    let address = SocketAddr::new(ip, port);
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
        let full_path_extension = Utf8Path::from_path(&full_path)
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
