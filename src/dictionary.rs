/// A single dictionary's hit for a word lookup.
#[derive(Debug, Clone)]
pub struct DictHit {
    /// Human-readable dictionary name (file stem).
    pub name: String,
    /// HTML or plain-text definition.
    pub definition: String,
}

/// Core interface every dictionary format must implement.
///
/// All methods that touch the index or source file are fallible.
/// A format that doesn't support resources (e.g. a plain word list)
/// simply returns `Ok(None)` from `resource` and an empty vec from
/// `css_resources`.
pub trait Dictionary: Send + Sync {
    /// Human-readable name shown in the UI (typically the file stem).
    fn name(&self) -> &str;

    /// Look up a word. Returns the HTML/text definition or `None` if not
    /// found. Redirect resolution (e.g. `@@@LINK=`) is handled internally.
    fn lookup(&self, word: &str) -> Option<String>;

    /// Prefix search. Returns up to `limit` matching headwords.
    fn suggestions(&self, prefix: &str, limit: usize) -> Vec<String>;

    /// Fetch a binary resource (image, audio, font) by virtual path.
    /// Returns `None` if this dictionary has no such resource.
    fn resource(&self, path: &str) -> Option<Vec<u8>>;

    /// CSS stylesheets bundled with this dictionary as `(filename, body)` pairs.
    fn css_resources(&self) -> Vec<(String, String)>;

    /// Build (or rebuild) the on-disk search index for this dictionary.
    /// `force = true` removes and recreates an existing index.
    fn build_index(&self, force: bool) -> anyhow::Result<()>;

    /// True if a complete, usable index already exists on disk.
    fn index_ready(&self) -> bool;
}
