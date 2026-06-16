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

The ⚙ Settings button opens a full-screen `Dialog` within the main
window via `window.open_dialog()` (from `gpui_component::WindowExt`).
The dialog is rendered inside the main window's `Root` component —
no separate window is used.

This approach avoids a Wayland/GNOME limitation: applications cannot
programmatically raise/focus their own windows. By keeping settings
as a dialog within the main window, it always appears on top and
closes automatically when the main window closes.

### Dialog structure

The settings dialog contains:

1. **Title** — "Settings" with a close button (✕).
2. **Tab bar** — `TabBar` with "Dictionaries", "Import", and "Download" tabs,
   controlled by `DictState.settings_active_tab` (0, 1, 2).
3. **Content** — renders the active tab.
4. **Footer** — Cancel and Save buttons.

The dialog is opened in `app.rs::cog_button()` and calls
`settings_panel::apply_save()` on Save.

### Dictionaries tab

[gpui/src/components/settings_panel.rs](../gpui/src/components/settings_panel.rs)

Uses plain flex rows (`h_flex`) instead of the `Table` component for
precise column alignment:

| Column | Width | Content |
|--------|-------|---------|
| Checkbox | 28px fixed | Toggle enable/disable |
| Dictionary name | flex-1 (fills remaining) | Display name |
| Arrows | 44px fixed | ▲ ▼ reorder buttons |
| Detail | 28px fixed | ≡ detail button |

Edits mutate `DictState.dictionaries` (a working copy). Save/Cancel
buttons appear in the dialog footer.

The **Save** button calls `settings_panel::apply_save(state, cx)`:

1. Persist the working copy via `settings::update(new)`.
2. `config::reset_pools()` drops all cached DB connections.
3. Spawn re-indexing on the background executor for all enabled MDX
   paths via `formats::detect(...).build_index(false)` + `registry::reload()`.
4. Re-sync `state.dictionaries` from disk.

**Cancel** simply closes the dialog. No revert needed because the
working copy is in-memory only — it's discarded on close.

### Import tab

[gpui/src/components/init_modal.rs](../gpui/src/components/init_modal.rs) — `import_tab_content()`

The same component is shared with the first-run init dialog (see below).
Provides a combined drag-and-drop / click-to-browse drop zone,
a per-session progress bar, and a scrollable file list.

`start_import(paths, state, cx)` handles the import:

1. Validate: only `.mdx` and `.mdd` extensions accepted; others get an
   immediate `Error` row.
2. Copy each valid file to `~/.config/dicto/dicts/`.
3. For `.mdx`: update `settings.toml`, `reset_pools()`, run
   `build_index(false)`, `registry::reload()`, load per-dict CSS.
4. For `.mdd`: copy only — the companion file is auto-discovered by stem.
5. After all files finish, refresh `state.dictionaries` from disk.

### Download tab

[gpui/src/components/download_panel.rs](../gpui/src/components/download_panel.rs)

Fetches a remote catalog (`catalog.json` from `CATALOG_URL` in
[gpui/src/catalog.rs](../gpui/src/catalog.rs)) and displays available
dictionaries. Each entry shows name, description, language pair,
version, size, and license.

**Install states** — determined by `DictCatalogEntry.install_status()`
which checks `~/.config/dicto/dicts/<id>/.version`:

| State | Button | Behavior |
|-------|--------|----------|
| `NotInstalled` | Blue "Download" | Downloads files → adds to settings → indexes → writes `.version` |
| `UpToDate` | Green "✓ Installed" | No action |
| `UpdateAvailable` | Orange "Update" | Re-downloads and re-indexes |

**Download flow** (`start_download` in `download_panel.rs`):

1. Download all files for the entry to `~/.config/dicto/dicts/<id>/`
   on the background executor.
2. A progress timer polls `SharedProgress` (Arc<Mutex>) every 200ms
   and updates `DictDownloadStatus::Downloading` on the UI.
3. After download: add `.mdx` to `settings.toml`, build FST index
   on background executor, reload registry, load stylesheets.
4. Write `.version` file to mark the installed version.
5. Non-`.mdx`/`.mdd` files (CSS, JS, PNG) are downloaded but skipped
   during import — they're companion resources used by the dictionary
   renderer.

**Catalog caching** — the fetched catalog is cached at
`~/.config/dicto/catalog-cache.json` with a 24-hour TTL.

### First-run init dialog

When `~/.config/dicto/dicts/` contains no `.mdx` files at startup,
`DictState.show_init_modal` is set to `true`. On the first render,
`open_get_dictionaries_dialog()` (in `app.rs`) opens a dialog with
Download and Import tabs (Download first). The dialog uses the same
`download_panel::panel_content` and `init_modal::import_tab_content`
components as the settings dialog.

### Dictionary detail dialog

Each dictionary row has a ≡ detail button that opens a nested `Dialog`
(via `window.open_dialog()`). This dialog shows read-only metadata
plus an editable display name:

| Field | Source | Editable |
|-------|--------|----------|
| Display name | `entry.short_name` (auto-derived from MDX title) | Yes — Input + inline Apply button |
| Description | `mdx_header_description()` (cleaned: HTML stripped, CSS discarded) | No |
| Path | `entry.path` | No |
| Encoding | `mdx_header_encoding()` | No |
| Version | `mdx_header_version()` | No |

The Apply button saves the new `short_name` to `DictState.dictionaries`
and closes the dialog. The dialog's close button (✕) dismisses without
saving. No footer buttons — Apply is inline next to the Input field.

### MDX header helpers

The following functions in `mdict_rs::formats::mdict` extract metadata
from the MDX file header:

- `mdx_header_title(path)` — full dictionary title from `<Title>` header
- `mdx_header_description(path)` — raw description (may contain HTML/CSS)
- `mdx_header_encoding(path)` — text encoding (e.g. "UTF-8")
- `mdx_header_version(path)` — format version string (e.g. "2.0")

Description cleaning (`clean_description`) strips:
1. `<style ...>...</style>` blocks (with any attributes like `type="text/css"`)
2. All remaining HTML tags
3. HTML entities (`&amp;`, `&#NNN;`, etc.)
4. CSS-like content (if result still contains `{`, `}`, `:`)
5. Whitespace collapse; truncated to 300 chars

The settings card re-renders on every toggle/reorder because GPUI
rebuilds the element tree from state each frame. Writing directly to
disk on every click would flood the settings file and trigger
reindexes mid-session. Working copy + explicit Save keeps I/O batched.

## Hot reload

Each `MdxDictionary` holds a `RwLock<Option<Arc<FstIndex>>>` that fills
lazily — `open_fst()` memory-maps the `.fst` + `.offsets` files on first
request. `registry::reload()` creates fresh `MdxDictionary` instances,
which naturally drop the old memory maps. `reset_pools()` is a no-op.
Together this means:

- Toggling a dictionary on or off takes effect on the **next** query
  (the new instance opens a fresh memory map against the current `.fst`).
- Reordering takes effect on the next query (the iteration order
  comes from `enabled_mdx()`, which always reads current settings).
- Enabling a dictionary that has no `.fst` yet triggers indexing in
  the background; the first query after indexing completes is when
  the map gets opened. Until then the dict shows up but has zero hits.

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
- Disabling a dictionary doesn't delete its `.fst`/`.offsets` index files.
  Re-enabling is instant. If you actually want to reclaim disk, delete
  the `.mdx.fst`, `.mdx.offsets`, `.mdd.fst`, and `.mdd.offsets` files
  manually after disabling.
