use crate::metadata;
use anyhow::{Context, Result};
use clap::Parser;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    process,
    sync::mpsc,
    time::{Duration, Instant},
};

/// Watches over your project's source code, relaunching the given command when
/// changes are detected.
///
/// # Usage
///
/// ```rust,no_run
/// # use std::process;
/// # use xtask_wasm::{anyhow::Result, clap};
/// #
/// # #[derive(clap::Parser)]
/// # struct Opt {
/// #   #[clap(subcommand)]
/// #   cmd: Command,
/// # }
/// #
/// #[derive(clap::Parser)]
/// enum Command {
///     Watch(xtask_wasm::Watch),
/// }
///
/// fn main() -> Result<()> {
///     let opt: Opt = clap::Parser::parse();
///
///     match opt.cmd {
///         Command::Watch(watch) => {
///             let mut command = process::Command::new("cargo");
///             command.args(["xtask", "dist"]);
///
///             println!("Starting to watch");
///             watch
///                 .exclude_workspace_path("dist")
///                 .run(command)?;
///         }
///     }
///
///     Ok(())
/// }
/// ```
///
/// Add a `watch` subcommand that will run `cargo xtask dist`, monitoring for
/// changes in the workspace (expect for hidden files, workspace's target
/// directory and the *dist* directory). If a valid change is detected, the
/// `cargo xtask dist` command will be relaunched with a debounce of 2 seconds
/// to avoid relaunching recursively on multiple files for example.
#[non_exhaustive]
#[derive(Debug, Parser)]
pub struct Watch {
    /// Watch specific file(s) or folder(s). The default is the workspace root
    #[clap(long = "watch", short = 'w')]
    pub watch_paths: Vec<PathBuf>,
    /// Paths that will be excluded
    #[clap(long = "ignore", short = 'i')]
    pub exclude_paths: Vec<PathBuf>,

    /// Paths, relative to the workspace root, that will be excluded
    #[clap(skip)]
    pub workspace_exclude_paths: Vec<PathBuf>,
    /// Set the debounce duration after relaunching a command.
    /// The default is 2 seconds
    #[clap(skip)]
    pub debounce: Option<Duration>,
}

impl Watch {
    /// Adds a path that will be monitored by the watch process.
    pub fn watch_path(mut self, path: impl AsRef<Path>) -> Self {
        self.watch_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds multiple paths that will be monitored by the watch process.
    pub fn watch_paths(mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Self {
        for path in paths {
            self.watch_paths.push(path.as_ref().to_path_buf())
        }
        self
    }

    /// Adds a path that will not be monitored by the watch process.
    pub fn exclude_path(mut self, path: impl AsRef<Path>) -> Self {
        self.exclude_paths.push(path.as_ref().to_path_buf());
        self
    }

    /// Adds multiple paths that will not be monitored by the watch process.
    pub fn exclude_paths(mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Self {
        for path in paths {
            self.exclude_paths.push(path.as_ref().to_path_buf());
        }
        self
    }

    /// Adds a path, relative to the workspace, that will not be monitored by
    /// the watch process.
    pub fn exclude_workspace_path(mut self, path: impl AsRef<Path>) -> Self {
        self.workspace_exclude_paths
            .push(path.as_ref().to_path_buf());
        self
    }

    /// Adds multiple paths, relative to the workspace, that will not be
    /// monitored by the watch process.
    pub fn exclude_workspace_paths(
        mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Self {
        for path in paths {
            self.workspace_exclude_paths
                .push(path.as_ref().to_path_buf());
        }
        self
    }

    /// Set the debounce duration after relaunching the command
    pub fn debounce(mut self, duration: Duration) -> Self {
        self.debounce = Some(duration);
        self
    }

    fn is_excluded_path(&self, path: &Path) -> bool {
        if path.starts_with(metadata().workspace_root.as_std_path()) {
            path.strip_prefix(metadata().workspace_root.as_std_path())
                .expect("path starts with workspace root; qed");
            self.workspace_exclude_paths
                .iter()
                .any(|x| path.starts_with(x))
        } else {
            self.exclude_paths.iter().any(|x| path.starts_with(x))
        }
    }

    fn is_hidden_path(&self, path: &Path) -> bool {
        if self.watch_paths.is_empty() {
            path.strip_prefix(&metadata().workspace_root)
                .expect("cannot strip prefix")
                .iter()
                .any(|x| {
                    x.to_str()
                        .expect("path contains non Utf-8 characters")
                        .starts_with('.')
                })
        } else {
            self.watch_paths.iter().any(|x| {
                path.strip_prefix(x)
                    .expect("cannot strip prefix")
                    .iter()
                    .any(|x| {
                        x.to_str()
                            .expect("path contains non Utf-8 characters")
                            .starts_with('.')
                    })
            })
        }
    }

    /// Run the given `command`, monitoring the watched paths and relaunch the
    /// command when changes are detected.
    ///
    /// Workspace's `target` directory and hidden paths are excluded by default.
    pub fn run(self, mut command: process::Command) -> Result<()> {
        let metadata = metadata();
        let watch = self.exclude_workspace_path(&metadata.target_directory);

        let (tx, rx) = mpsc::channel();
        let mut watcher: RecommendedWatcher =
            notify::Watcher::new_raw(tx).context("could not initialize watcher")?;

        if watch.watch_paths.is_empty() {
            log::trace!("Watching {}", &metadata.workspace_root);
            watcher
                .watch(&metadata.workspace_root, RecursiveMode::Recursive)
                .context("cannot watch this crate")?;
        } else {
            for path in &watch.watch_paths {
                match watcher.watch(&path, RecursiveMode::Recursive) {
                    Ok(()) => log::trace!("Watching {}", path.display()),
                    Err(err) => log::error!("cannot watch {}: {}", path.display(), err),
                }
            }
        }

        let mut child = command.spawn().context("cannot spawn command")?;
        let mut command_start = Instant::now();

        loop {
            match rx.recv() {
                Ok(notify::RawEvent {
                    path: Some(path), ..
                }) if !watch.is_excluded_path(&path) && !watch.is_hidden_path(&path) => {
                    if command_start.elapsed()
                        >= watch.debounce.unwrap_or_else(|| Duration::from_secs(2))
                    {
                        log::trace!("Detected changes at {}", path.display());
                        #[cfg(unix)]
                        {
                            let now = Instant::now();

                            unsafe {
                                log::trace!("Killing watch's command process");
                                libc::kill(
                                    child.id().try_into().expect("cannot get process id"),
                                    libc::SIGTERM,
                                );
                            }

                            while now.elapsed().as_secs() < 2 {
                                std::thread::sleep(Duration::from_millis(200));
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

                        log::info!("Re-running command");
                        child = command.spawn().context("cannot spawn command")?;
                        command_start = Instant::now();
                    } else {
                        log::trace!("Ignoring changes at {}", path.display());
                    }
                }
                Ok(_) => {}
                Err(err) => log::error!("watch error: {}", err),
            }
        }
    }
}
