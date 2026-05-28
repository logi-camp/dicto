use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, RwLock};

use redb::Database;
use tracing::{info, warn};

pub const APP_NAME: &str = "dicto";

// ── directory helpers ─────────────────────────────────────────────────────────

pub fn discover_mdx_files() -> Vec<String> {
    let config_dir = dirs_config_path();
    let mut files = Vec::new();

    if config_dir.exists() {
        info!("scanning for MDX files in {}", config_dir.display());
        collect_files_with_ext(&config_dir, "mdx", &mut files);
    }

    let fallback = PathBuf::from("./resources/mdx");
    if fallback.exists() && !config_dir.exists() {
        info!("scanning for MDX files in {}", fallback.display());
        collect_files_with_ext(&fallback, "mdx", &mut files);
    }

    if files.is_empty() {
        warn!("no MDX files found in {}", config_dir.display());
    } else {
        info!("found {} MDX file(s)", files.len());
        for f in &files {
            info!("  - {f}");
        }
    }
    files
}

pub fn discover_mdd_files() -> Vec<String> {
    let config_dir = dirs_config_path();
    let mut files = Vec::new();
    if config_dir.exists() {
        collect_files_with_ext(&config_dir, "mdd", &mut files);
    }
    let fallback = PathBuf::from("./resources/mdx");
    if fallback.exists() && !config_dir.exists() {
        collect_files_with_ext(&fallback, "mdd", &mut files);
    }
    if !files.is_empty() {
        info!("found {} MDD file(s)", files.len());
    }
    files
}

fn collect_files_with_ext(dir: &PathBuf, ext: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_ext(&path, ext, out);
        } else if path.extension().map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false) {
            if let Some(s) = path.to_str() {
                out.push(s.to_string());
            }
        }
    }
}

pub fn dirs_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    base.join(APP_NAME).join("dicts")
}

pub fn static_path() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/static"))
}

// ── redb database handles ─────────────────────────────────────────────────────

/// Open or create a redb database at `<source_file>.redb`.
fn open_db(source_file: &str) -> Option<Arc<Database>> {
    let db_path = format!("{source_file}.redb");
    if !PathBuf::from(&db_path).exists() {
        warn!("skipping {source_file}: database {db_path} not found");
        return None;
    }
    match Database::open(&db_path) {
        Ok(db) => Some(Arc::new(db)),
        Err(e) => {
            warn!("skipping {source_file}: failed to open {db_path}: {e}");
            None
        }
    }
}

static MDX_DBS: LazyLock<RwLock<HashMap<String, Arc<Database>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static MDD_DBS: LazyLock<RwLock<HashMap<String, Arc<Database>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn db_for(
    map: &RwLock<HashMap<String, Arc<Database>>>,
    file: &str,
) -> Option<Arc<Database>> {
    if let Some(db) = map.read().unwrap().get(file).cloned() {
        return Some(db);
    }
    let db = open_db(file)?;
    map.write().unwrap().insert(file.to_string(), db.clone());
    Some(db)
}

pub fn get_mdx_db(file: &str) -> anyhow::Result<Arc<Database>> {
    db_for(&MDX_DBS, file)
        .ok_or_else(|| anyhow::anyhow!("no MDX database for {}", file))
}

pub fn get_mdd_db(file: &str) -> anyhow::Result<Arc<Database>> {
    db_for(&MDD_DBS, file)
        .ok_or_else(|| anyhow::anyhow!("no MDD database for {}", file))
}

/// Drop all cached database handles. The next request will reopen them.
pub fn reset_pools() {
    MDX_DBS.write().unwrap().clear();
    MDD_DBS.write().unwrap().clear();
}
