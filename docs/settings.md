# Settings & per-dictionary configuration

mdict-rs reads its user settings from a single TOML file at
`~/.config/dicto/settings.toml` (or the XDG-equivalent on
non-Linux). The file is auto-managed by the app — you can hand-edit
it but you usually don't need to.

## On-disk schema

```toml
[[dictionaries]]
path = "/home/you/.config/dicto/dicts/LDOCE5.mdx"
enabled = true

[[dictionaries]]
path = "/home/you/.config/dicto/dicts/Cambridge Advanced Learner's Dictionary 3th.mdx"
enabled = true

[[dictionaries]]
path = "/home/you/.config/dicto/dicts/Longman_Phrasal_Verbs_Dictionary.mdx"
enabled = false
```

Order in the file is the query/display order. Disabled entries are
preserved but skipped during query and indexing.

## Discovery rules

[src/settings/mod.rs](../src/settings/mod.rs)

At every read, `merge_with_disk()` reconciles the stored list with
what's actually on disk:

1. Drop entries whose `.mdx` file no longer exists.
2. Append entries for newly-discovered `.mdx` files (enabled by
   default).
3. Save the reconciled list back to disk.

This means dropping a new dictionary file in
`~/.config/dicto/dicts/` and restarting the app is enough to
pick it up — no need to edit the TOML.

## Public API

```rust
pub fn current() -> Settings;          // read snapshot
pub fn update(new: Settings);          // write + merge + persist
pub fn enabled_mdx() -> Vec<String>;   // paths in display order
pub fn enabled_mdd() -> Vec<String>;   // same, MDD siblings only if present
```

`enabled_mdd()` pairs each enabled MDX with the `.mdd` of the same
stem in the same directory. If the MDD isn't there, the dict still
works for text lookups but has no images / pronunciations.

## Settings UI (GPUI)

The ⚙ Settings button opens a custom full-screen overlay modal
(`settings_modal::overlay`) controlled by `DictState.show_settings_modal`.
The modal is a fixed 560 × 600 px card rendered as a sibling of the
main view — no `window.open_dialog()` is used, which lets the import
callbacks close the modal via `cx.update_entity()` without needing a
`Window` reference.

### Dictionaries tab

[gpui/src/components/settings_panel.rs](../gpui/src/components/settings_panel.rs)

A scrollable list of dictionary rows (`overflow_y_scrollbar`, `h(320)`):

- Checkbox-style toggle (the colored box; click to enable/disable).
- Dictionary stem name + full path.
- `▲` / `▼` reorder buttons (disabled at list boundaries).

Edits mutate `DictState.dictionaries` (a working copy). Save/Cancel
buttons appear inline below the list.

The **Save** button calls `settings_panel::apply_save(state, cx)`:

1. Persist the working copy via `settings::update(new)`.
2. `config::reset_pools()` drops all cached DB connections.
3. Spawn re-indexing on the background executor for all enabled MDX
   paths via `formats::detect(...).build_index(false)` + `registry::reload()`.
4. Re-sync `state.dictionaries` from disk.

**Cancel** calls `settings_panel::revert(state, cx)` (reloads working
copy from disk) then closes.

Both buttons also clear `state.import_files` on close.

### Import tab

[gpui/src/components/init_modal.rs](../gpui/src/components/init_modal.rs) — `import_tab_content()`

The same component is shared with the first-run init modal (see below).
Provides a combined drag-and-drop / click-to-browse drop zone,
a per-session progress bar, and a scrollable file list. The ✕ close
button is hidden while an import is in progress.

### First-run init modal

When `~/.config/dicto/dicts/` contains no `.mdx` files at startup,
`DictState.show_init_modal` is set to `true` and the init overlay
(`init_modal::overlay`) covers the whole window, prompting the user
to import dictionaries before the app is usable.

`start_import(paths, state, cx)` handles both entry points (init modal
and Settings → Import tab):

1. Validate: only `.mdx` and `.mdd` extensions accepted; others get an
   immediate `Error` row.
2. Copy each valid file to `~/.config/dicto/dicts/`.
3. For `.mdx`: update `settings.toml`, `reset_pools()`, run
   `build_index(false)`, `registry::reload()`, load per-dict CSS.
4. For `.mdd`: copy only — the companion file is auto-discovered by stem.
5. After all files finish, refresh `state.dictionaries` from disk.
   The modal stays open so the user can review results; it closes
   only when the user clicks Done/✕, which also clears `state.import_files`.

### Why a working copy

The settings card re-renders on every toggle/reorder because GPUI
rebuilds the element tree from state each frame. Writing directly to
disk on every click would flood the settings file and trigger
reindexes mid-session. Working copy + explicit Save keeps I/O batched.

## Hot reload

The library's database handle map is a `RwLock<HashMap<file, Arc<Database>>>`
that fills lazily — `db_for(file)` opens a redb `Database` on first
request. `reset_pools()` empties both maps (`MDX_DBS`, `MDD_DBS`).
Together this means:

- Toggling a dictionary on or off takes effect on the **next** query
  (the handle is recreated against the current `.redb`).
- Reordering takes effect on the next query (the iteration order
  comes from `enabled_mdx()`, which always reads current settings).
- Enabling a dictionary that has no `.redb` yet triggers indexing in
  the background; the first query after indexing completes is when
  the handle gets opened. Until then the dict shows up but has zero
  hits.

No restart needed for any of these.

## CSS pairing

Per-dictionary stylesheets (loaded at startup, see
[rendering.md](rendering.md)) follow these rules:

- For dictionary `<dir>/<stem>.mdx`, CSS comes from:
  1. Every `<path LIKE '%.css'>` row in the matching MDD index.
  2. Sibling files on disk whose stem equals `<stem>` or starts
     with `<stem>_`.

Other `.css` files in the directory are ignored — class names like
`.hw`, `.hwd`, `.pos` are reused with different meanings across
dictionaries, so a global stylesheet would silently break some of
them.

If you want to tweak how a dictionary renders without re-packing its
MDD:

1. Identify the dictionary's stem (e.g. `LDOCE5`).
2. Drop a `LDOCE5_overrides.css` next to `LDOCE5.mdx`.
3. The next launch picks it up; later-loaded rules override earlier
   ones, so you can patch individual properties without reproducing
   the whole stylesheet.

## Gotchas

- The settings file path uses `dirs::config_dir()` (Linux:
  `$XDG_CONFIG_HOME` or `~/.config`). The CSS loader in the GPUI
  binary doesn't depend on the `dirs` crate, so it falls back to
  `$XDG_CONFIG_HOME` then `$HOME/.config` manually. Both should end
  up at the same path on a normal Linux setup; on macOS / Windows
  the two could disagree. Keep that in mind if porting.
- The dialog reads `mdict_rs::settings::current()` on Cancel to
  revert. If the settings file was edited externally between Save
  cycles, Cancel will revert to the *file*, not to the values the
  dialog initially loaded.
- Indexing a newly-enabled large dictionary takes minutes (LDOCE5 is
  ~80 MB MDX → redb, and the MDD is ~1 GB). The Settings
  Save closes the modal immediately and the indexer continues in the
  background. The `indexing_bar` in the main window shows per-file
  progress (`indexing_done / indexing_total — current_name`). The
  Import tab shows its own per-file progress bar during the copy +
  index pipeline.
- Disabling a dictionary doesn't delete its `.redb` files. Re-enabling
  is instant. If you actually want to reclaim disk, delete the
  `.mdx.redb` and `.mdd.redb` files manually after disabling.
