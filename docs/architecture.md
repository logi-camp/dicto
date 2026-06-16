# Architecture

mdict-rs is two crates in one workspace:

- **`mdict-rs`** (library + `mdict-rs` web bin) — pure-Rust MDX/MDD
  parser, redb indexer, query pipeline, settings store.
- **`mdict-gpui`** (under [gpui/](../gpui/)) — desktop UI built on
  [GPUI](https://github.com/zed-industries/zed) and
  [gpui-component](https://github.com/longbridge/gpui-component).
  Depends on the library; never bypasses it for file access.

A second binary, `mdict-rs`, is an axum-based web server that exposes
the same query layer over HTTP. It exists for historical reasons and
to keep the library honest about its dependencies.

## Data flow (single lookup)

```
┌──────────────────────────────────────────────────────────────────┐
│  GPUI app (mdict-gpui)                                           │
│                                                                  │
│   SearchBar.on_change ─► 150ms debounce                          │
│                          ─► query::search_suggestions(prefix, 50) │
│                          ─► returns Vec<String>, dedup'd         │
│                                                                  │
│   Auto-select first suggestion (if query >= 3 chars):            │
│                          ─► 200ms debounce                       │
│                          ─► query::query_all(first_match)        │
│                          ─► parse_styled + render preview        │
│                                                                  │
│   WordList click / Enter ─► query::query_all(word)               │
│       ─► returns Vec<DictHit { name, definition: html }>         │
│       ─► html::parse_styled(html, name)                          │
│            (parser uses the per-dict Stylesheet)                 │
│       ─► state.results = Vec<DictResult { name, blocks }>        │
│                                                                  │
│   DetailPanel render:                                            │
│       ─► render_blocks(&state.results[active].blocks)            │
│                                                                  │
│   Keyboard shortcuts:                                            │
│       Ctrl+L / Ctrl+F ─► focus search input                     │
│       Escape           ─► clear search input                    │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  Library (mdict-rs)                                              │
│                                                                  │
│   query::query_all                                               │
│     ─► settings::enabled_mdx()  (filtered + ordered list)        │
│     ─► for each enabled mdx:                                     │
│           MdxDictionary::open_fst() → Arc<FstIndex>              │
│           FstIndex::get_record(word) → definition bytes          │
│           if @@@LINK=…  re-resolve in same dict (max 5 hops)     │
│                                                                  │
│   query::lookup_resource(path)                                   │
│     ─► settings::enabled_mdd() + candidate_keys(path)            │
│     ─► returns Vec<u8> (image, audio, font, css…)                │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  FST indexes built at startup                                    │
│                                                                  │
│   <file>.mdx.fst      FST map: lowercased key → row index u64   │
│   <file>.mdx.offsets  flat array of 24-byte offset records      │
│   <file>.mdd.fst      same for MDD resources                    │
│   <file>.mdd.offsets                                            │
│                                                                  │
│   Each MdxDictionary holds RwLock<Option<Arc<FstIndex>>>;        │
│   lazy-opened (memory-mapped) on first request.                  │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  Indexing (build_mdx_index / build_mdd_index)                    │
│   parses the .mdx/.mdd, walks every record, writes .fst +       │
│   .offsets pair. Runs at startup for any dict whose .fst is      │
│   missing or whose .offsets has zero records.                    │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  On disk                                                         │
│   ~/.config/dicto/dicts/<id>/<filename>.mdx                  │
│   ~/.config/dicto/dicts/<id>/<filename>.mdd                  │
│   ~/.config/dicto/dicts/<id>/.version                        │
│   ~/.config/dicto/settings.toml                             │
│   ~/.config/dicto/catalog-cache.json                        │
└──────────────────────────────────────────────────────────────────┘
```

## Crate layout

```
src/
├── lib.rs                  pub modules below
├── main.rs                 web bin entry (axum, port 8181)
├── handlers/mod.rs         HTTP handlers (/query, /lucky)
├── config/mod.rs           dirs, connection pools, reset_pools()
├── mdict/
│   ├── mod.rs              re-exports
│   ├── header.rs           parse the encrypted/encoded header
│   ├── keyblock.rs         the V1/V2 key block parser
│   ├── recordblock.rs      compressed record block reader
│   ├── mdx.rs              .mdx → Iterator<Record>
│   └── mdd.rs              .mdd → Iterator<Resource>
├── indexing/mod.rs         build_index / build_index_mdd / re-index
├── query/mod.rs            query, query_all, search_suggestions,
│                            lookup_resource, css_resources_in_mdd,
│                            redirect resolution
├── settings/mod.rs         load/save settings.toml, enabled_mdx/mdd
├── util/mod.rs             fast_decrypt (the MDict cipher)
└── lucky/mod.rs            randomized word source for /lucky

gpui/
├── Cargo.toml              gpui + gpui-component + rodio
├── src/
│   ├── main.rs             window setup, tray, indexing kick-off,
│   │                       per-dict CSS loader
│   ├── app.rs              DictApp (Render), lookup_word(),
│   │                       cog button (opens settings dialog),
│   │                       init dialog (first-run Get Dictionaries),
│   │                       indexing_bar, global keyboard shortcuts
│   │                       (Ctrl+L/F, Esc), auto-select first
│   │                       suggestion with debounced preview
│   ├── state.rs            DictState — see "State fields" below
│   ├── catalog.rs          DictCatalogEntry model, catalog fetch/cache,
│   │                       InstallStatus (NotInstalled/UpToDate/Update),
│   │                       download URL construction
│   ├── download.rs         File download with SHA256 verification,
│   │                       SharedProgress for UI polling
│   ├── audio.rs            rodio + ffmpeg fallback for Speex
│   ├── colors.rs           theme color refs (bg, surface, primary,
│   │                       text, border, success, error, update)
│   ├── components/
│   │   ├── detail_panel.rs heading + TabBar + scroll body
│   │   ├── download_panel.rs catalog list, download/installed/update
│   │   │                       buttons, progress polling, post-download
│   │   │                       settings update + indexing
│   │   ├── word_list.rs    left pane
│   │   ├── search_bar.rs   input + right_slot for the cog
│   │   ├── settings_panel.rs dict list UI + apply_save;
│   │   │                       detail dialog (per-dict metadata);
│   │   │                       uses flex rows for column alignment
│   │   ├── settings_window.rs header_tabs_for_dialog (shared
│   │   │                       TabBar for settings dialog)
│   │   └── init_modal.rs   import_tab_content shared by settings
│   │                       and init dialog; drag/drop + file picker
│   └── html/
│       ├── mod.rs          STYLESHEETS map, parse_styled
│       ├── css.rs          CSS subset parser + matcher
│       ├── parser.rs       HTML tokenizer + element-stack builder
│       └── render.rs       Block → GPUI element tree
```

## State fields (DictState)

`gpui/src/state.rs` — the single `Entity<DictState>` shared across all components.

| Field | Type | Purpose |
|-------|------|---------|
| `word_list_scroll` | `gpui::ScrollHandle` | Scroll handle for the word list panel; used to auto-scroll to the selected item during keyboard navigation. |
| `results` | `Vec<DictResult>` | Parsed hits for the current lookup word. |
| `active_result` | `usize` | Index into `results` shown in DetailPanel. |
| `result_word` | `Option<String>` | The word that produced `results`. |
| `is_searching` | `bool` | True while background query is in flight. |
| `suggestions` | `Vec<String>` | Current autocomplete list. |
| `selected_suggestion` | `Option<usize>` | Keyboard-selected row in WordList. |
| `dictionaries` | `Vec<DictEntry>` | Working copy for the settings Dictionaries tab. |
| `indexing_total` / `indexing_done` / `indexing_current` | `usize` / `usize` / `Option<String>` | Background indexing progress shown in `indexing_bar`. |
| `settings_active_tab` | `usize` | 0 = Dictionaries, 1 = Import, 2 = Download (settings dialog). |
| `show_init_modal` | `bool` | True when `~/.config/dicto/dicts/` contains no `.mdx` files at startup. Opens the Get Dictionaries dialog. |
| `import_files` | `Vec<ImportFile>` | Files queued/in-progress/completed in the current import session. |
| `catalog` | `CatalogState` | Idle → Loading → Loaded { base_url, entries } or Error. Fetched from remote catalog.json. |
| `download_status` | `DictDownloadStatus` | Idle / Downloading { progress, speed, current_file } / Done / Error. |
| `download_active_id` | `Option<String>` | ID of the dictionary currently being downloaded. |
| `init_modal_tab` | `usize` | 0 = Download, 1 = Import (first-run dialog). |

`ImportFile.status` cycles through `Pending → Copying → Indexing → Done` (or `Error(String)` on failure).

## Threading & async

- GPUI's `Render` impl runs on the main UI thread. We never block it.
- All DB I/O is dispatched via `cx.background_executor().spawn(...)`
  — there's only one `cx` and one async runtime; no tokio inside the
  GUI binary.
- HTML parsing is also moved to the background executor; the UI
  thread only receives the parsed `Vec<Block>` and renders it.
- Audio plays in `std::thread::spawn` — short-lived, one stream per
  click. Rodio's `OutputStream` is dropped after the clip ends.
- Indexing on startup (in `gpui/src/main.rs::main`) is synchronous —
  the window doesn't open until DBs are ready. Re-indexing triggered
  by saving settings runs on the background executor.

## Persistence summary

| Where | What |
|-------|------|
| `~/.config/dicto/dicts/<id>/*.mdx` (and `.mdd`) | Downloaded dictionaries, one folder per catalog entry. |
| `~/.config/dicto/dicts/<id>/.version` | Installed version string (e.g. `"1.0"`), used for update detection. |
| `~/.config/dicto/dicts/<stem>.mdx` (and `.mdd`) | Manually-imported dictionaries (flat in dicts/). |
| `~/.config/dicto/dicts/*.mdx.fst` + `*.mdx.offsets` (and `.mdd.*`) | FST index + offset table built by `indexing`. |
| `~/.config/dicto/settings.toml` | Enabled list + display order. |
| `~/.config/dicto/catalog-cache.json` | Cached remote catalog (24h TTL). |
| `/tmp/mdict-rs-cache/` | Decoded images + transcoded WAV clips. Wiped on reboot; safe to delete any time. |

## Where to start when something's wrong

| Symptom | Doc |
|---------|-----|
| Parsing panics, wrong characters, missing entries | [mdict-format.md](mdict-format.md) |
| Definition text smushed, colors wrong, missing styles | [rendering.md](rendering.md) |
| Sound clip silent or noisy logs | [rendering.md](rendering.md) (Audio section) |
| Wrong dictionary selected, settings not applied | [settings.md](settings.md) |
| Click does nothing, click goes through, dialog never opens | [rendering.md](rendering.md) (GPUI gotchas) |
