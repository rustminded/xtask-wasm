use crate::{
    anyhow::{bail, ensure, Context, Result},
    camino::Utf8Path,
    clap, xtask_command, Dist, Watch,
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
use xtask_watch::Lock;

type RequestHandler = Arc<dyn Fn(Request) -> Result<()> + Send + Sync + 'static>;

/// A type that can produce a [`process::Command`] given the final [`DevServer`] configuration.
///
/// Implement this trait to build a command whose arguments or environment depend on the server's
/// configuration — for example to pass `--dist-dir`, `--port`, or other runtime values.
///
/// A blanket implementation is provided for [`process::Command`] itself, so existing call sites
/// that pass a plain command continue to work without any changes.
///
/// # Examples
///
/// ```rust,no_run
/// use std::process;
/// use xtask_wasm::{anyhow::Result, clap, DevServer, Hook};
///
/// struct NotifyOnPort;
///
/// impl Hook for NotifyOnPort {
///     fn build_command(self: Box<Self>, server: &DevServer) -> process::Command {
///         let mut cmd = process::Command::new("notify-send");
///         cmd.arg(format!("dev server on port {}", server.port));
///         cmd
///     }
/// }
///
/// #[derive(clap::Parser)]
/// enum Opt {
///     Start(xtask_wasm::DevServer),
/// }
///
/// fn main() -> Result<()> {
///     let opt: Opt = clap::Parser::parse();
///
///     match opt {
///         Opt::Start(dev_server) => {
///             dev_server
///                 .xtask("dist")
///                 .post(NotifyOnPort)
///                 .start()?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub trait Hook {
    /// Construct the [`process::Command`] to run, using `server` as context.
    fn build_command(self: Box<Self>, server: &DevServer) -> process::Command;
}

impl Hook for process::Command {
    fn build_command(self: Box<Self>, _server: &DevServer) -> process::Command {
        *self
    }
}

/// Abstraction over an HTTP request.
#[derive(Debug)]
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
/// changes are detected using [`command`](Self::command), [`xtask`](Self::xtask) or
/// [`cargo`](Self::cargo).
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
///         Opt::Dist => todo!("build project"),
///         Opt::Start(dev_server) => {
///             log::info!("Starting the development server...");
///             dev_server
///                 .xtask("dist")
///                 .start()?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// This adds a `start` subcommand that will run `cargo xtask dist`, watching for
/// changes in the workspace and serve the files in the default dist directory
/// (`target/debug/dist`) at the default IP address.
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

    /// Watch configuration for detecting file-system changes.
    ///
    /// Controls which paths are watched, debounce timing, and other watch
    /// behaviour. Watching is only active when at least one of `pre_hooks`,
    /// `command`, or `post_hooks` is set; if none are provided the watch
    /// thread is not started.
    #[clap(flatten)]
    pub watch: Watch,

    /// Directory of all generated artifacts.
    #[clap(skip)]
    pub dist_dir: Option<PathBuf>,

    /// Commands executed before the main command when a change is detected.
    #[clap(skip)]
    #[debug(skip)]
    pub pre_hooks: Vec<Box<dyn Hook>>,

    /// Main command executed when a change is detected.
    #[clap(skip)]
    pub command: Option<process::Command>,

    /// Commands executed after the main command when a change is detected.
    #[clap(skip)]
    #[debug(skip)]
    pub post_hooks: Vec<Box<dyn Hook>>,

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

    /// Add a command to execute before the main command when a change is detected.
    pub fn pre(mut self, command: impl Hook + 'static) -> Self {
        self.pre_hooks.push(Box::new(command));
        self
    }

    /// Add multiple commands to execute before the main command when a change is detected.
    pub fn pres(mut self, commands: impl IntoIterator<Item = impl Hook + 'static>) -> Self {
        self.pre_hooks
            .extend(commands.into_iter().map(|c| Box::new(c) as Box<dyn Hook>));
        self
    }

    /// Add a command to execute after the main command when a change is detected.
    pub fn post(mut self, command: impl Hook + 'static) -> Self {
        self.post_hooks.push(Box::new(command));
        self
    }

    /// Add multiple commands to execute after the main command when a change is detected.
    pub fn posts(mut self, commands: impl IntoIterator<Item = impl Hook + 'static>) -> Self {
        self.post_hooks
            .extend(commands.into_iter().map(|c| Box::new(c) as Box<dyn Hook>));
        self
    }

    /// Main command executed when a change is detected.
    ///
    /// See [`xtask`](Self::xtask) if you want to use an `xtask` command.
    pub fn command(mut self, command: process::Command) -> Self {
        self.command = Some(command);
        self
    }

    /// Name of the main xtask command that is executed when a change is detected.
    ///
    /// See [`command`](Self::command) to use an arbitrary command.
    pub fn xtask(mut self, name: impl AsRef<str>) -> Self {
        let mut command = xtask_command();
        command.arg(name.as_ref());
        self.command = Some(command);
        self
    }

    /// Cargo subcommand executed as the main command when a change is detected.
    ///
    /// See [`xtask`](Self::xtask) for xtask commands or [`command`](Self::command) for arbitrary
    /// commands.
    pub fn cargo(mut self, subcommand: impl AsRef<str>) -> Self {
        let mut command = process::Command::new("cargo");
        command.arg(subcommand.as_ref());
        self.command = Some(command);
        self
    }

    /// Adds an argument to the main command executed when changes are detected.
    ///
    /// # Panics
    ///
    /// Panics if called before [`command`](Self::command), [`xtask`](Self::xtask) or
    /// [`cargo`](Self::cargo).
    pub fn arg<S: AsRef<ffi::OsStr>>(mut self, arg: S) -> Self {
        self.command
            .as_mut()
            .expect("`arg` called without a command set; call `command`, `xtask` or `cargo` first")
            .arg(arg);
        self
    }

    /// Adds multiple arguments to the main command executed when changes are detected.
    ///
    /// # Panics
    ///
    /// Panics if called before [`command`](Self::command), [`xtask`](Self::xtask) or
    /// [`cargo`](Self::cargo).
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<ffi::OsStr>,
    {
        self.command
            .as_mut()
            .expect("`args` called without a command set; call `command`, `xtask` or `cargo` first")
            .args(args);
        self
    }

    /// Inserts or updates an environment variable for the main command executed when changes are
    /// detected.
    ///
    /// # Panics
    ///
    /// Panics if called before [`command`](Self::command), [`xtask`](Self::xtask) or
    /// [`cargo`](Self::cargo).
    pub fn env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<ffi::OsStr>,
        V: AsRef<ffi::OsStr>,
    {
        self.command
            .as_mut()
            .expect("`env` called without a command set; call `command`, `xtask` or `cargo` first")
            .env(key, val);
        self
    }

    /// Inserts or updates multiple environment variables for the main command executed when
    /// changes are detected.
    ///
    /// # Panics
    ///
    /// Panics if called before [`command`](Self::command), [`xtask`](Self::xtask) or
    /// [`cargo`](Self::cargo).
    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<ffi::OsStr>,
        V: AsRef<ffi::OsStr>,
    {
        self.command
            .as_mut()
            .expect("`envs` called without a command set; call `command`, `xtask` or `cargo` first")
            .envs(vars);
        self
    }

    /// Use another file path when the URL is not found.
    pub fn not_found_path(mut self, path: impl Into<PathBuf>) -> Self {
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
    /// If `dist_dir` has not been provided, [`Dist::default_debug_dir`] will be used.
    pub fn start(mut self) -> Result<()> {
        // Resolve dist_dir early so Hooks can observe the final value via &self.
        if self.dist_dir.is_none() {
            self.dist_dir = Some(Dist::default_debug_dir().into());
        }
        let dist_dir = self.dist_dir.clone().unwrap();

        // Shared critical section between build execution and request serving.
        let section_lock = Lock::new();

        let watch_process = {
            // mem::take so we can pass &self to build_command while the fields are empty.
            let pre_hooks = std::mem::take(&mut self.pre_hooks);
            let post_hooks = std::mem::take(&mut self.post_hooks);
            let main_command = self.command.take();

            let mut commands: Vec<process::Command> = pre_hooks
                .into_iter()
                .map(|p| p.build_command(&self))
                .collect();
            if let Some(command) = main_command {
                commands.push(command);
            }
            commands.extend(post_hooks.into_iter().map(|p| p.build_command(&self)));

            if !commands.is_empty() {
                // NOTE: the path needs to exists in order to be excluded because it is canonicalize
                std::fs::create_dir_all(&dist_dir).with_context(|| {
                    format!("cannot create dist directory `{}`", dist_dir.display())
                })?;
                let watch = self.watch.exclude_path(&dist_dir);

                let section_lock_watch = section_lock.clone();
                let handle = std::thread::spawn(move || {
                    match watch.run_with_lock(commands, section_lock_watch) {
                        Ok(()) => log::trace!("Starting to watch"),
                        Err(err) => log::error!("an error occurred when starting to watch: {err}"),
                    }
                });

                Some(handle)
            } else {
                None
            }
        };

        if let Some(handler) = self.request_handler {
            serve(
                self.ip,
                self.port,
                dist_dir,
                self.not_found_path,
                handler,
                section_lock,
            )
            .context("an error occurred when starting to serve")?;
        } else {
            serve(
                self.ip,
                self.port,
                dist_dir,
                self.not_found_path,
                Arc::new(default_request_handler),
                section_lock,
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
            pre_hooks: Default::default(),
            command: None,
            post_hooks: Default::default(),
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
    section_lock: Lock,
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
        let section_lock = section_lock.clone();
        thread::spawn(move || {
            let header = warn_not_fail!(read_header(&stream));
            let _guard = match section_lock.read() {
                Ok(guard) => guard,
                Err(err) => {
                    let _ = stream.write("HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n".as_bytes());
                    log::error!("could not acquire read lock: {err}");
                    return;
                }
            };
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
