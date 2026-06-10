use std::sync::{LazyLock, RwLock};

use tracing::warn;

use crate::dictionary::{DictHit, Dictionary};
use crate::formats::detect;
use crate::settings::enabled_mdx;

// ── DictionaryRegistry ────────────────────────────────────────────────────────

pub struct DictionaryRegistry {
    dictionaries: Vec<Box<dyn Dictionary>>,
}

impl DictionaryRegistry {
    pub fn from_settings() -> Self {
        let dicts = enabled_mdx()
            .into_iter()
            .filter_map(|path| {
                let d = detect(&path)?;
                if !d.index_ready() {
                    warn!("registry: index not ready for {path}, skipping");
                    return None;
                }
                Some(d)
            })
            .collect();
        DictionaryRegistry {
            dictionaries: dicts,
        }
    }

    /// Build (or rebuild) indexes for all configured MDX files, then reload.
    pub fn ensure_indexed(&self, force: bool) {
        for d in &self.dictionaries {
            if let Err(e) = d.build_index(force) {
                warn!("registry: indexing failed for {}: {e}", d.name());
            }
        }
    }

    pub fn query_all(&self, word: &str) -> Vec<DictHit> {
        self.dictionaries
            .iter()
            .filter_map(|d| {
                d.lookup(word).map(|def| DictHit {
                    name: d.name().to_string(),
                    definition: def,
                })
            })
            .collect()
    }

    pub fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for d in &self.dictionaries {
            for word in d.suggestions(prefix, limit) {
                if seen.insert(word.clone()) {
                    results.push(word);
                }
                if results.len() >= limit {
                    return results;
                }
            }
        }
        results
    }

    pub fn lookup_resource(&self, path: &str) -> Option<Vec<u8>> {
        self.dictionaries.iter().find_map(|d| d.resource(path))
    }

    pub fn css_for_dict(&self, name: &str) -> Vec<(String, String)> {
        self.dictionaries
            .iter()
            .find(|d| d.name() == name)
            .map(|d| d.css_resources())
            .unwrap_or_default()
    }

    pub fn all_css(&self) -> Vec<(String, Vec<(String, String)>)> {
        self.dictionaries
            .iter()
            .map(|d| (d.name().to_string(), d.css_resources()))
            .collect()
    }
}

// ── global registry ───────────────────────────────────────────────────────────

static REGISTRY: LazyLock<RwLock<DictionaryRegistry>> = LazyLock::new(|| {
    RwLock::new(DictionaryRegistry {
        dictionaries: vec![],
    })
});

/// Initialize or reload the registry from current settings.
/// Must be called after indexing completes.
pub fn reload() {
    *REGISTRY.write().unwrap() = DictionaryRegistry::from_settings();
}

pub fn query_all(word: &str) -> Vec<DictHit> {
    REGISTRY.read().unwrap().query_all(word)
}

pub fn suggestions(prefix: &str, limit: usize) -> Vec<String> {
    REGISTRY.read().unwrap().suggestions(prefix, limit)
}

pub fn lookup_resource(path: &str) -> Option<Vec<u8>> {
    REGISTRY.read().unwrap().lookup_resource(path)
}

pub fn css_for_dict(name: &str) -> Vec<(String, String)> {
    REGISTRY.read().unwrap().css_for_dict(name)
}

pub fn all_css() -> Vec<(String, Vec<(String, String)>)> {
    REGISTRY.read().unwrap().all_css()
}
