use mdict_rs::settings::DictEntry;

use crate::html::Block;

#[derive(Debug, Clone)]
pub struct DictResult {
    pub name: String,
    pub blocks: Vec<Block>,
}

pub struct DictState {
    /// One entry per dictionary that had a hit for the current word, in
    /// settings order. Parsed blocks are cached so the detail panel
    /// never re-parses HTML on render.
    pub results: Vec<DictResult>,
    pub active_result: usize,

    pub result_word: Option<String>,
    pub is_searching: bool,
    pub suggestions: Vec<String>,
    pub selected_suggestion: Option<usize>,

    /// Working copy of the dictionary list used by the settings dialog.
    /// Edits mutate this in place; Save persists it, Cancel reloads from disk.
    pub dictionaries: Vec<DictEntry>,

    /// Background-indexing progress. `indexing_total == 0` means idle.
    pub indexing_total: usize,
    pub indexing_done: usize,
    pub indexing_current: Option<String>,
}

impl DictState {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            active_result: 0,
            result_word: None,
            is_searching: false,
            suggestions: Vec::new(),
            selected_suggestion: None,
            dictionaries: mdict_rs::settings::current().dictionaries,
            indexing_total: 0,
            indexing_done: 0,
            indexing_current: None,
        }
    }
}
