use std::path::PathBuf;

use mdict_rs::settings::DictEntry;

use crate::catalog::DictCatalogEntry;
use crate::html::Block;

#[derive(Debug, Clone)]
pub struct DictResult {
    /// Short name for tab labels.
    pub short_name: String,
    pub blocks: Vec<Block>,
}

pub struct ImportFile {
    pub path: PathBuf,
    pub name: String,
    pub status: ImportStatus,
}

pub enum ImportStatus {
    Pending,
    Copying,
    Indexing,
    Done,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum CatalogState {
    Idle,
    Loading,
    Loaded {
        base_url: String,
        entries: Vec<DictCatalogEntry>,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum DictDownloadStatus {
    Idle,
    Downloading {
        progress: f32,
        speed: String,
        current_file: String,
    },
    Done,
    Error(String),
}

pub struct DictState {
    /// Scroll handle for the word list panel, used to auto-scroll to the
    /// selected item during keyboard navigation.
    pub word_list_scroll: gpui::ScrollHandle,
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

    /// True when the dicts directory was empty at startup — shows import modal.
    pub show_import_modal: bool,
    /// Files being imported via the init/settings modal.
    pub import_files: Vec<ImportFile>,

    /// Active tab in the settings dialog: 0 = Dictionaries, 1 = Import, 2 = Download.
    pub settings_active_tab: usize,

    pub catalog: CatalogState,
    pub download_status: DictDownloadStatus,
    pub download_active_id: Option<String>,
    pub import_modal_tab: usize,
}

impl DictState {
    pub fn new() -> Self {
        Self {
            word_list_scroll: gpui::ScrollHandle::new(),
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
            show_import_modal: mdict_rs::config::discover_mdx_files().is_empty(),
            import_files: Vec::new(),
            settings_active_tab: 0,
            catalog: CatalogState::Idle,
            download_status: DictDownloadStatus::Idle,
            download_active_id: None,
            import_modal_tab: 0,
        }
    }
}
