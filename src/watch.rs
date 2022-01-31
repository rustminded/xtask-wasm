use crate::metadata;
use anyhow::{Context, Result};
use clap::Parser;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    process,
    sync::mpsc,
};

#[non_exhaustive]
#[derive(Debug, Parser)]
pub struct Watch {
    #[clap(long = "watch", short = 'w')]
    pub watch_paths: Vec<PathBuf>,
    #[clap(long = "ignore", short = 'i')]
    pub exclude_paths: Vec<PathBuf>,

    #[clap(skip)]
    pub workspace_exclude_paths: Vec<PathBuf>,
}

impl Watch {
    pub fn watch_path(mut self, path: impl AsRef<Path>) -> Self {
        self.watch_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn watch_paths(mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Self {
        for path in paths {
            self.watch_paths.push(path.as_ref().to_path_buf())
        }
        self
    }

    pub fn exclude_path(mut self, path: impl AsRef<Path>) -> Self {
        self.exclude_paths.push(path.as_ref().to_path_buf());
        self
    }

    pub fn exclude_paths(mut self, paths: impl IntoIterator<Item = impl AsRef<Path>>) -> Self {
        for path in paths {
            self.exclude_paths.push(path.as_ref().to_path_buf());
        }
        self
    }

    pub fn exclude_workspace_path(mut self, path: impl AsRef<Path>) -> Self {
        self.workspace_exclude_paths
            .push(path.as_ref().to_path_buf());
        self
    }

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

    fn is_excluded_path(&self, path: &Path) -> bool {
        if self.exclude_paths.iter().any(|x| path.starts_with(x)) {
            return true;
        }

        if let Ok(stripped_path) = path.strip_prefix(metadata().workspace_root.as_std_path()) {
            if self
                .workspace_exclude_paths
                .iter()
                .any(|x| stripped_path.starts_with(x))
            {
                return true;
            }
        }

        false
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

    pub fn run(self, mut command: process::Command) -> Result<()> {
        let metadata = metadata();
        let watch = self.exclude_path(&metadata.target_directory);

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
        let mut command_start = std::time::Instant::now();

        loop {
            match rx.recv() {
                Ok(notify::RawEvent {
                    path: Some(path), ..
                }) if !watch.is_excluded_path(&path) && !watch.is_hidden_path(&path) => {
                    if command_start.elapsed().as_secs() >= 2 {
                        log::trace!("Detected changes at {}", path.display());
                        #[cfg(unix)]
                        {
                            let now = std::time::Instant::now();

                            unsafe {
                                log::trace!("Killing watch's command process");
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

                        log::info!("Re-running command");
                        child = command.spawn().context("cannot spawn command")?;
                        command_start = std::time::Instant::now();
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn exclude_relative_path() {
        let watch = Watch {
            watch_paths: Vec::new(),
            exclude_paths: Vec::new(),
            workspace_exclude_paths: vec![PathBuf::from("src/watch.rs")],
        };

        assert!(watch.is_excluded_path(
            metadata()
                .workspace_root
                .join("src")
                .join("watch.rs")
                .as_std_path()
        ));
        assert!(!watch.is_excluded_path(metadata().workspace_root.join("src").as_std_path()));
    }
}
