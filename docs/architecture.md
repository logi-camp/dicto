# Architecture

mdict-rs is two crates in one workspace:

- **`mdict-rs`** (library + `mdict-rs` web bin) — pure-Rust MDX/MDD
  parser, SQLite indexer, query pipeline, settings store.
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
│   SearchBar.on_change ─► query::search_suggestions(prefix, 50)   │
│                          ─► returns Vec<String>, dedup'd         │
│                                                                  │
│   WordList click  ─► query::query_all(word)                      │
│       ─► returns Vec<DictHit { name, definition: html }>         │
│       ─► html::parse_styled(html, name)                          │
│            (parser uses the per-dict Stylesheet)                 │
│       ─► state.results = Vec<DictResult { name, blocks }>        │
│                                                                  │
│   DetailPanel render:                                            │
│       ─► render_blocks(&state.results[active].blocks)            │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  Library (mdict-rs)                                              │
│                                                                  │
│   query::query_all                                               │
│     ─► settings::enabled_mdx()  (filtered + ordered list)        │
│     ─► for each enabled mdx:                                     │
│           config::get_db_connection(file)                        │
│           SELECT def FROM MDX_INDEX WHERE text = :word           │
│           if @@@LINK=…  re-resolve in same dict (max 5 hops)     │
│                                                                  │
│   query::lookup_resource(path)                                   │
│     ─► settings::enabled_mdd() + candidate_keys(path)            │
│     ─► returns Vec<u8> (image, audio, font, css…)                │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  SQLite mirrors built at startup                                 │
│                                                                  │
│   <file>.mdx.db    table MDX_INDEX(text TEXT PRIMARY KEY,        │
│                                    def  TEXT NOT NULL)           │
│   <file>.mdd.db    table MDD_INDEX(path TEXT PRIMARY KEY,        │
│                                    data BLOB NOT NULL)           │
│                                                                  │
│   Pools: RwLock<HashMap<file, r2d2::Pool>>, lazy-filled on first │
│   request; reset_pools() drops them after settings changes.      │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  Indexing (mdx_to_sqlite / mdd_to_sqlite)                        │
│   parses the .mdx/.mdd, walks every record, writes to its DB.    │
│   Runs at startup for any dict whose DB is missing or empty.     │
└──────────────────────────────────────────────────────────────────┘
                            ▲
                            │
┌──────────────────────────────────────────────────────────────────┐
│  On disk                                                         │
│   ~/.config/dicto/dicts/<DictName>.mdx                      │
│   ~/.config/dicto/dicts/<DictName>.mdd                      │
│   ~/.config/dicto/settings.toml                             │
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
├── indexing/mod.rs         mdx_to_sqlite / mdd_to_sqlite / re-index
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
│   │                       cog button + Dialog wiring
│   ├── state.rs            DictState (results, suggestions,
│   │                       dictionaries snapshot)
│   ├── audio.rs            rodio + ffmpeg fallback for Speex
│   ├── colors.rs           theme color refs
│   ├── components/
│   │   ├── detail_panel.rs heading + TabBar + scroll body
│   │   ├── word_list.rs    left pane
│   │   ├── search_bar.rs   input + right_slot for the cog
│   │   └── settings_panel.rs Dialog body, save/cancel/reindex
│   └── html/
│       ├── mod.rs          STYLESHEETS map, parse_styled
│       ├── css.rs          CSS subset parser + matcher
│       ├── parser.rs       HTML tokenizer + element-stack builder
│       └── render.rs       Block → GPUI element tree
```

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
| `~/.config/dicto/dicts/*.mdx` (and `.mdd`) | Source dictionaries, user-managed. |
| `~/.config/dicto/dicts/*.mdx.db` (and `.mdd.db`) | SQLite mirrors built by `indexing`. |
| `~/.config/dicto/settings.toml` | Enabled list + display order. |
| `/tmp/mdict-rs-cache/` | Decoded images + transcoded WAV clips. Wiped on reboot; safe to delete any time. |

## Where to start when something's wrong

| Symptom | Doc |
|---------|-----|
| Parsing panics, wrong characters, missing entries | [mdict-format.md](mdict-format.md) |
| Definition text smushed, colors wrong, missing styles | [rendering.md](rendering.md) |
| Sound clip silent or noisy logs | [rendering.md](rendering.md) (Audio section) |
| Wrong dictionary selected, settings not applied | [settings.md](settings.md) |
| Click does nothing, click goes through, dialog never opens | [rendering.md](rendering.md) (GPUI gotchas) |
