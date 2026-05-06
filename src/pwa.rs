use serde::Serialize;
use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};
use xtask_watch::anyhow::{Context, Result};

/// Progressive Web App generation options for [`Dist`](crate::Dist).
///
/// When enabled via [`Dist::pwa`](crate::Dist::pwa), missing PWA files from the `assets_directory`
/// are generated as fallback defaults:
///
/// - `manifest.json` (if absent).
/// - `sw.js` (if absent).
///
/// Existing user-provided assets always win.
#[non_exhaustive]
#[derive(Debug, Serialize)]
pub struct Pwa {
    /// Full application name.
    pub name: String,
    /// Short application name.
    pub short_name: String,
    /// Application description.
    pub description: String,
    /// Start URL for installed launch.
    pub start_url: String,
    /// Navigation scope.
    pub scope: String,
    /// Display mode.
    pub display: PwaDisplayMode,
    /// Theme color.
    pub theme_color: String,
    /// Background color.
    pub background_color: String,
    /// Cache version token.
    #[serde(skip)]
    pub cache_version: String,
    /// Icons
    pub icons: Vec<PwaIcon>,
}

impl Pwa {
    /// Create a new Pwa instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Provide the full application name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Provide the short application name.
    pub fn short_name(mut self, short_name: impl Into<String>) -> Self {
        self.short_name = short_name.into();
        self
    }

    /// Provide the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Provide the start URL.
    pub fn start_url(mut self, url: impl Into<String>) -> Self {
        self.start_url = url.into();
        self
    }

    /// Provide the scope URL.
    pub fn scope(mut self, url: impl Into<String>) -> Self {
        self.scope = url.into();
        self
    }

    /// Provide the display mode.
    pub fn display_mode(mut self, mode: PwaDisplayMode) -> Self {
        self.display = mode;
        self
    }

    /// Provide the theme color.
    pub fn theme_color(mut self, color: impl Into<String>) -> Self {
        self.theme_color = color.into();
        self
    }

    /// Provide the background color.
    pub fn background_color(mut self, color: impl Into<String>) -> Self {
        self.background_color = color.into();
        self
    }

    /// Provide the cache version.
    pub fn cache_version(mut self, version: impl Into<String>) -> Self {
        self.cache_version = version.into();
        self
    }

    /// Provide icons metadata.
    pub fn icons(mut self, icons: Vec<PwaIcon>) -> Self {
        self.icons = icons;
        self
    }

    pub(crate) fn apply(self, dist_dir: &Path) -> Result<()> {
        let manifest_path = dist_dir.join("manifest.json");
        if !manifest_path.exists() {
            let manifest =
                serde_json::to_string_pretty(&self).context("failed to serialize PWA manifest")?;
            fs::write(&manifest_path, manifest)
                .with_context(|| format!("failed to write `{}`", manifest_path.display()))?;
        }

        let sw_path = dist_dir.join("sw.js");
        if !sw_path.exists() {
            let static_resources = static_resources_to_cache(dist_dir)?;
            fs::write(
                &sw_path,
                service_worker_file(
                    self.cache_version.as_ref(),
                    self.name.as_ref(),
                    static_resources.as_ref(),
                )?,
            )
            .with_context(|| format!("failed to write `{}`", sw_path.display()))?;
        }

        Ok(())
    }
}

impl Default for Pwa {
    fn default() -> Self {
        Self {
            name: "app".to_string(),
            short_name: "app".to_string(),
            description: "A web application".to_string(),
            start_url: "./".to_string(),
            scope: "./".to_string(),
            display: PwaDisplayMode::Standalone,
            theme_color: "#000000".to_string(),
            background_color: "#000000".to_string(),
            cache_version: "v1".to_string(),
            icons: Vec::new(),
        }
    }
}

/// Display mode of the generated application.
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PwaDisplayMode {
    /// Hide browser UI and uses the entirety of the available display area.
    Fullscreen,
    /// Opens the app to look and feel like a standalone native app.
    Standalone,
    /// Opens the app to look and feel like a standalone app but with a minimal set of UI
    /// elements for navigation.
    MinimalUI,
    /// Opens the app in a conventional browser tab or new window.
    Browser,
}

/// Icon metadata for a generated web app manifest.
#[derive(Debug, Serialize)]
pub struct PwaIcon {
    /// Source of the icon (relative to the `dist` directory).
    pub src: String,
    /// MIME type of the icon.
    #[serde(rename = "type")]
    pub mime_type: String,
    /// Pixel size descriptor (e.g. 192x192).
    pub sizes: String,
}

pub(crate) fn service_worker_file(
    cache_version: &str,
    app_name: &str,
    static_resources: &[String],
) -> Result<String> {
    let mut out = String::new();

    writeln!(out, "const VERSION= {cache_version:?};").expect("can write String");
    writeln!(out, "const APP_NAME = {app_name:?};").expect("can write String");
    writeln!(out, "const CACHE_NAME = `${{APP_NAME}}-${{VERSION}}`;").expect("can write String");

    writeln!(out, "const APP_STATIC_RESOURCES = [").expect("can write String");
    for resource in static_resources {
        writeln!(out, "  {resource:?},").expect("can write String");
    }
    writeln!(out, "];").expect("can write String");
    writeln!(out).expect("can write String");

    write!(
        out,
        r#"self.addEventListener("install", (event) => {{
  event.waitUntil(
    (async () => {{
      const cache = await caches.open(CACHE_NAME);
      await cache.addAll(APP_STATIC_RESOURCES);
      await self.skipWaiting();
    }})(),
  );
}});

self.addEventListener("activate", (event) => {{
  event.waitUntil(
    (async () => {{
      const names = await caches.keys();
      await Promise.all(
        names.map((name) => {{
          if (name.startsWith(APP_NAME) && name !== CACHE_NAME) {{
            return caches.delete(name);
          }}
          return Promise.resolve(false);
        }}),
      );
      await clients.claim();
    }})(),
  );
}});

self.addEventListener("fetch", (event) => {{
  if (event.request.method !== "GET") return;

  event.respondWith(
    (async () => {{
      const cache = await caches.open(CACHE_NAME);
      if (event.request.mode === "navigate") {{
        try {{
          return await fetch(event.request);
        }} catch {{
          const cachedIndex = await cache.match("./index.html");
          if (cachedIndex) return cachedIndex;
          throw new Error("Offline and no cached index.html");
        }}
      }}
      const cachedResponse = await cache.match(event.request);
      if (cachedResponse) return cachedResponse;

      try {{
        const networkResponse = await fetch(event.request);
        const url = new URL(event.request.url);
        if (url.origin === self.location.origin && networkResponse.ok) {{
          cache.put(event.request, networkResponse.clone());
        }}
        return networkResponse;
      }} catch {{
        const fallback = await cache.match(event.request, {{ ignoreSearch: true }});
        if (fallback) return fallback;
        throw new Error(`Offline and no cache match: ${{event.request.url}}`);
      }}
    }})(),
  );
}});"#
    )
    .expect("can write String");

    Ok(out)
}

fn static_resources_to_cache(dist_dir: &Path) -> Result<Vec<String>> {
    fn should_exclude(path: &Path) -> bool {
        if path.file_name().and_then(|x| x.to_str()) == Some("sw.js") {
            return true;
        }
        matches!(
            path.extension().and_then(|x| x.to_str()),
            Some("map") | Some("txt")
        )
    }

    let mut resources = Vec::new();
    resources.push("./".to_string());
    let walker = walkdir::WalkDir::new(dist_dir);
    for entry in walker {
        let entry = entry
            .with_context(|| format!("failed to walk into directory `{}`", dist_dir.display()))?;
        let source = entry.path();

        if !source.is_file() || should_exclude(source) {
            continue;
        }

        let relative: PathBuf = source
            .strip_prefix(dist_dir)
            .with_context(|| {
                format!(
                    "cannot strip dist prefix `{}` from `{}`",
                    dist_dir.display(),
                    source.display(),
                )
            })?
            .to_path_buf();

        let web_path = format!("./{}", relative.to_string_lossy().replace('\\', "/"));
        resources.push(web_path);
    }

    resources.sort();
    resources.dedup();
    Ok(resources)
}
