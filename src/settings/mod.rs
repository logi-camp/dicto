//! User-editable settings persisted to `~/.config/mdict-dict/settings.toml`.
//!
//! Tracks the list of MDX dictionaries (path + enabled flag) in the order
//! the user wants them queried. New `.mdx` files dropped into the mdict
//! directory get appended (enabled by default); removed files are dropped
//! on next save.

use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::{APP_NAME, discover_mdx_files};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub dictionaries: Vec<DictEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    pub path: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub fn settings_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    base.join(APP_NAME).join("settings.toml")
}

fn load_from_disk() -> Settings {
    let path = settings_path();
    if !path.exists() {
        return Settings::default();
    }
    match fs::read_to_string(&path) {
        Ok(s) => match toml::from_str(&s) {
            Ok(parsed) => parsed,
            Err(e) => {
                warn!("settings: parse failed ({e}); using defaults");
                Settings::default()
            }
        },
        Err(e) => {
            warn!("settings: read failed ({e}); using defaults");
            Settings::default()
        }
    }
}

/// Reconcile a stored settings list with what's currently on disk:
/// new files get appended (enabled by default), missing files dropped.
fn merge_with_disk(mut s: Settings) -> Settings {
    let on_disk = discover_mdx_files();
    let known: std::collections::HashSet<String> = on_disk.iter().cloned().collect();

    s.dictionaries.retain(|d| known.contains(&d.path));

    let mut existing: std::collections::HashSet<String> =
        s.dictionaries.iter().map(|d| d.path.clone()).collect();
    for path in on_disk {
        if !existing.contains(&path) {
            s.dictionaries.push(DictEntry {
                path: path.clone(),
                enabled: true,
            });
            existing.insert(path);
        }
    }
    s
}

static SETTINGS: LazyLock<RwLock<Settings>> = LazyLock::new(|| {
    let merged = merge_with_disk(load_from_disk());
    if let Err(e) = save(&merged) {
        warn!("settings: initial save failed: {e}");
    }
    RwLock::new(merged)
});

pub fn current() -> Settings {
    SETTINGS.read().unwrap().clone()
}

/// Replace the settings on disk and in memory. Caller is responsible for
/// telling the rest of the system to react (re-index, rebuild pools).
pub fn update(new: Settings) -> anyhow::Result<()> {
    let cleaned = merge_with_disk(new);
    save(&cleaned)?;
    *SETTINGS.write().unwrap() = cleaned;
    Ok(())
}

fn save(s: &Settings) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = toml::to_string_pretty(s)?;
    fs::write(&path, body)?;
    info!(
        "settings: saved {} entries to {}",
        s.dictionaries.len(),
        path.display()
    );
    Ok(())
}

/// MDX paths the user wants queried, in display order.
pub fn enabled_mdx() -> Vec<String> {
    current()
        .dictionaries
        .into_iter()
        .filter(|d| d.enabled)
        .map(|d| d.path)
        .collect()
}

/// MDD paths corresponding to enabled MDX entries (matched by stem).
pub fn enabled_mdd() -> Vec<String> {
    enabled_mdx()
        .into_iter()
        .map(|mdx| {
            let p = PathBuf::from(&mdx);
            p.with_extension("mdd").to_string_lossy().to_string()
        })
        .filter(|p| PathBuf::from(p).exists())
        .collect()
}
