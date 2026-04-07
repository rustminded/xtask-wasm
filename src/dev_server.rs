use crate::{
    anyhow::{bail, ensure, Context, Result},
    camino::Utf8Path,
    clap, Watch,
    xtask_command,
    Dist,
};
use derive_more::Debug;
use std::{
    ffi, fs,
    io::prelude::*,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
    sync::Arc,
    thread,
};

type RequestHandler = Arc<dyn Fn(Request) -> Result<()> + Send + Sync + 'static>;

/// Abstraction over an HTTP request.
#[non_exhaustive]
pub struct Request<'a> {
    /// TCP stream of the request.
    pub stream: &'a mut TcpStream,
    /// Path of the request.
    pub path: &'a str,
    /// Request header.
    pub header: &'a str,
    /// Path to the distributed directory.
    pub dist_dir: &'a Path,
    /// Path to the file used when the requested file cannot be found for the default request
    /// handler.
    pub not_found_path: Option<&'a Path>,
}

/// A simple HTTP server useful during development.
///
/// It can watch the source code for changes and restart a provided [`command`](Self::command).
///
/// Serve the file from the provided [`dist_dir`](Self::dist_dir) at a given IP address
/// (127.0.0.1:8000 by default). An optional command can be provided to restart the build when
/// changes are detected.
///
/// # Usage
///
/// ```rust,no_run
/// use std::process;
/// use xtask_wasm::{
///     anyhow::Result,
///     clap,
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
///             log::info!("Starting the development server...");
///             dev_server
///                 .xtask("dist")
///                 .start()?;
///         }
///         Opt::Dist => todo!("build project"),
///     }
///
///     Ok(())
/// }
/// ```
///
/// This adds a `start` subcommand that will run `cargo xtask dist`, watching for
/// changes in the workspace and serve the files in the provided dist directory
/// (`target/debug/dist` by default) at the provided IP address (default to `127.0.0.1).
#[non_exhaustive]
#[derive(Debug, clap::Parser)]
#[clap(
    about = "A simple HTTP server useful during development.",
    long_about = "A simple HTTP server useful during development.\n\
        It can watch the source code for changes."
)]
pub struct DevServer {
    /// IP address to bind. Default to `127.0.0.1`.
    #[clap(long, default_value = "127.0.0.1")]
    pub ip: IpAddr,
    /// Port number. Default to `8000`.
    #[clap(long, default_value = "8000")]
    pub port: u16,

    /// Watch object for detecting changes.
    ///
    /// # Note
    ///
    /// Used only if `command` is set.
    #[clap(flatten)]
    pub watch: Watch,

    /// Directory of all generated artifacts.
    #[clap(skip)]
    pub dist_dir: Option<PathBuf>,

    /// Command executed when a change is detected.
    #[clap(skip)]
    pub command: Option<process::Command>,

    /// Use another file path when the URL is not found.
    #[clap(skip)]
    pub not_found_path: Option<PathBuf>,

    /// Pass a custom request handler.
    #[clap(skip)]
    #[debug(skip)]
    request_handler: Option<RequestHandler>,
}

impl DevServer {
    /// Set the dev-server binding address.
    pub fn address(mut self, ip: IpAddr, port: u16) -> Self {
        self.ip = ip;
        self.port = port;

        self
    }

    /// Set the directory for the generated artifacts.
    ///
    /// The default is `target/debug/dist`.
    pub fn dist_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.dist_dir = Some(path.into());
        self
    }

    /// Program of the command that is executed when a change is detected.
    pub fn command(mut self, program: impl AsRef<str>) -> Self {
        let command = std::process::Command::new(program.as_ref());
        self.command = Some(command);
        self
    }

    /// Name of the xtask command that is executed when a change is detected.
    pub fn xtask(mut self, name: impl AsRef<str>) -> Self {
        let mut command = xtask_command();
        command.arg(name.as_ref());
        self.command = Some(command);
        self
    }

    /// Adds an argument to pass to the [`command`](Self::command) executed when changes are
    /// detected.
    ///
    /// This is no-op unless a [`command`](Self::command) is provided.
    pub fn arg<S: AsRef<ffi::OsStr>>(mut self, arg: S) -> Self {
        if let Some(command) = self.command.as_mut() {
            command.arg(arg);
        }
        self
    }

    /// Adds multiple arguments to pass to the [`command`](Self::command) executed when changes are
    /// detected.
    ///
    /// This is no-op unless a [`command`](Self::command) is provided.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<ffi::OsStr>,
    {
        if let Some(command) = self.command.as_mut() {
            command.args(args);
        }
        self
    }

    /// Use another file path when the URL is not found.
    pub fn not_found(mut self, path: impl Into<PathBuf>) -> Self {
        self.not_found_path.replace(path.into());
        self
    }

    /// Pass a custom request handler to the dev server.
    pub fn request_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(Request) -> Result<()> + Send + Sync + 'static,
    {
        self.request_handler.replace(Arc::new(handler));
        self
    }

    /// Start the server, serving the files at [`dist_dir`](Self::dist_dir).
    ///
    /// If `dist_dir` has not been provided, [`default_dist_dir_debug`] will be used.
    pub fn start(self) -> Result<()> {
        let dist_dir = self
            .dist_dir
            .unwrap_or_else(|| {
                Dist::default_debug_dir().as_std_path().to_path_buf()
            });

        let watch_process = if let Some(command) = self.command {
            // NOTE: the path needs to exists in order to be excluded because it is canonicalize
            let _ = std::fs::create_dir_all(&dist_dir);
            let watch = self.watch.exclude_path(&dist_dir);
            let handle = std::thread::spawn(|| match watch.run(command) {
                Ok(()) => log::trace!("Starting to watch"),
                Err(err) => log::error!("an error occurred when starting to watch: {err}"),
            });

            Some(handle)
        } else {
            None
        };

        if let Some(handler) = self.request_handler {
            serve(
                self.ip,
                self.port,
                dist_dir,
                self.not_found_path,
                handler,
            )
            .context("an error occurred when starting to serve")?;
        } else {
            serve(
                self.ip,
                self.port,
                dist_dir,
                self.not_found_path,
                Arc::new(default_request_handler),
            )
            .context("an error occurred when starting to serve")?;
        }

        if let Some(handle) = watch_process {
            handle.join().expect("an error occurred when exiting watch");
        }

        Ok(())
    }
}

impl Default for DevServer {
    fn default() -> DevServer {
        DevServer {
            ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8000,
            watch: Default::default(),
            dist_dir: None,
            command: None,
            not_found_path: None,
            request_handler: None,
        }
    }
}

fn serve(
    ip: IpAddr,
    port: u16,
    dist_dir: PathBuf,
    not_found_path: Option<PathBuf>,
    handler: RequestHandler,
) -> Result<()> {
    let address = SocketAddr::new(ip, port);
    let listener = TcpListener::bind(address).context("cannot bind to the given address")?;

    log::info!("Development server running at: http://{}", &address);

    macro_rules! warn_not_fail {
        ($expr:expr) => {{
            match $expr {
                Ok(res) => res,
                Err(err) => {
                    log::warn!("Malformed request's header: {}", err);
                    return;
                }
            }
        }};
    }

    for mut stream in listener.incoming().filter_map(Result::ok) {
        let handler = handler.clone();
        let dist_dir = dist_dir.clone();
        let not_found_path = not_found_path.clone();
        thread::spawn(move || {
            let header = warn_not_fail!(read_header(&stream));
            let request = Request {
                stream: &mut stream,
                header: header.as_ref(),
                path: warn_not_fail!(parse_request_path(&header)),
                dist_dir: dist_dir.as_ref(),
                not_found_path: not_found_path.as_deref(),
            };

            (handler)(request).unwrap_or_else(|e| {
                let _ = stream.write("HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n".as_bytes());
                log::error!("an error occurred: {e}");
            });
        });
    }

    Ok(())
}

fn read_header(mut stream: &TcpStream) -> Result<String> {
    let mut header = Vec::with_capacity(64 * 1024);
    let mut peek_buffer = [0u8; 4096];

    loop {
        let n = stream.peek(&mut peek_buffer)?;
        ensure!(n > 0, "Unexpected EOF");

        let data = &mut peek_buffer[..n];
        if let Some(i) = data.windows(4).position(|x| x == b"\r\n\r\n") {
            let data = &mut peek_buffer[..(i + 4)];
            stream.read_exact(data)?;
            header.extend(&*data);
            break;
        } else {
            stream.read_exact(data)?;
            header.extend(&*data);
        }
    }

    Ok(String::from_utf8(header)?)
}

fn parse_request_path(header: &str) -> Result<&str> {
    let content = header.split('\r').next().unwrap();
    let requested_path = content
        .split_whitespace()
        .nth(1)
        .context("could not find path in request")?;
    Ok(requested_path
        .split_once('?')
        .map(|(prefix, _suffix)| prefix)
        .unwrap_or(requested_path))
}

/// Default request handler
pub fn default_request_handler(request: Request) -> Result<()> {
    let requested_path = request.path;

    log::debug!("<-- {requested_path}");

    let rel_path = Path::new(requested_path.trim_matches('/'));
    let mut full_path = request.dist_dir.join(rel_path);

    if full_path.is_dir() {
        if full_path.join("index.html").exists() {
            full_path = full_path.join("index.html")
        } else if full_path.join("index.htm").exists() {
            full_path = full_path.join("index.htm")
        } else {
            bail!("no index.html in {}", full_path.display());
        }
    }

    if let Some(path) = request.not_found_path {
        if !full_path.is_file() {
            full_path = request.dist_dir.join(path);
        }
    }

    if full_path.is_file() {
        log::debug!("--> {}", full_path.display());
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

        request
            .stream
            .write(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n",
                    full_path.metadata()?.len(),
                    content_type,
                )
                .as_bytes(),
            )
            .context("cannot write response")?;

        std::io::copy(&mut fs::File::open(&full_path)?, request.stream)?;
    } else {
        log::error!("--> {} (404 NOT FOUND)", full_path.display());
        request
            .stream
            .write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes())
            .context("cannot write response")?;
    }

    Ok(())
}
