use crate::{
    anyhow::{bail, Context, Result},
    camino::Utf8Path,
    clap, Watch,
};
use std::{
    env, ffi, fs,
    io::{prelude::*, BufReader},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    path::Path,
    process,
};

/// A simple HTTP server useful during development.
///
/// It can watch the source code for changes and restart a provided build
/// command.
///
/// Get the files at `watch_path` and serve them at a given IP address
/// (127.0.0.1:8000 by default). An optional command can be provided to restart
/// the build when changes are detected.
///
/// # Usage
///
/// ```rust,no_run
/// use std::process;
/// use xtask_wasm::{
///     anyhow::Result,
///     clap,
///     default_dist_dir,
/// };
///
/// #[derive(clap::Parser)]
/// enum Opt {
///     Start(xtask_wasm::DevServer),
///     Dist,
/// }
///
/// fn main() -> Result<()> {
///     let opt: Opt = clap::Parser::parse();
///
///     match opt {
///         Opt::Start(mut dev_server) => {
///             let mut command = process::Command::new("cargo");
///             command.args(["xtask", "dist"]);
///
///             log::info!("Starting the dev server");
///             dev_server.command(command).start(default_dist_dir(false))?;
///         }
///         Opt::Dist => todo!("build project"),
///     }
///
///     Ok(())
/// }
/// ```
///
/// Add a `start` subcommand that will run `cargo xtask dist`, watching for
/// changes in the workspace and serve the files in the default dist directory
/// (`target/debug/dist` for non-release) at a given IP address.
#[non_exhaustive]
#[derive(Debug, clap::Parser)]
pub struct DevServer {
    /// IP address to bind. Default to `127.0.0.1`.
    #[clap(long, default_value = "127.0.0.1")]
    pub ip: IpAddr,
    /// Port number. Default to `8000`.
    #[clap(long, default_value = "8000")]
    pub port: u16,

    /// Command executed when a change is detected.
    #[clap(skip)]
    pub command: Option<process::Command>,
    /// Watch object for detecting changes.
    ///
    /// # Note
    ///
    /// Used only if `command` is set.
    #[clap(flatten)]
    pub watch: Watch,
}

impl DevServer {
    /// Set the command that is executed when a change is detected.
    pub fn command(mut self, command: process::Command) -> Self {
        self.command = Some(command);
        self
    }

    /// Adds an argument to pass to the command executed when changes are detected.
    ///
    /// This will use the xtask command by default.
    pub fn arg<S: AsRef<ffi::OsStr>>(mut self, arg: S) -> Self {
        self.set_xtask_command().arg(arg);
        self
    }

    /// Adds multiple arguments to pass to the command executed when changes are detected.
    ///
    /// This will use the xtask command by default.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<ffi::OsStr>,
    {
        self.set_xtask_command().args(args);
        self
    }

    fn set_xtask_command(&mut self) -> &mut process::Command {
        if self.command.is_none() {
            self.command = Some(process::Command::new(env::args_os().next().unwrap()));
        }
        self.command.as_mut().unwrap()
    }

    /// Start the server, serving the files at `served_path`.
    ///
    /// [`crate::default_dist_dir`] should be used to get the dist directory that needs
    /// to be served.
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

        serve(self.ip, self.port, served_path)
            .context("an error occurred when starting to serve")?;

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

fn respond_to_request(stream: &mut TcpStream, dist_dir_path: impl AsRef<Path>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    let requested_path = request
        .split_whitespace()
        .nth(1)
        .context("Could not find path in request")?;

    let rel_path = Path::new(requested_path.trim_matches('/'));
    let mut full_path = dist_dir_path.as_ref().join(rel_path);

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
