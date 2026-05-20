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

## Settings dialog (GPUI)

[gpui/src/components/settings_panel.rs](../gpui/src/components/settings_panel.rs)

Opens as a gpui-component `Dialog` overlay (not an inline panel)
triggered by the cog button in the search-bar row.

The dialog body is a fixed-height (`h(360)`) scrollable list of rows:

- Checkbox-style toggle (the colored box; click to enable/disable).
- Dictionary name + path.
- `↑` / `↓` buttons (disabled at boundaries) to reorder.

Edits mutate `DictState.dictionaries` (a working copy). The dialog's
Save button calls `settings_panel::apply_save(state, cx)`:

1. Persist the working copy via `settings::update(new)`.
2. `config::reset_pools()` drops all cached DB connections.
3. Spawn re-indexing on the background executor for any newly
   enabled dictionary whose `.db` is missing.
4. Re-sync `state.dictionaries` from the persisted settings (in case
   `merge_with_disk` reordered or normalized anything).

Cancel reverts the working copy from disk and closes the dialog.

### Why a working copy

The dialog re-renders every time state changes (toggle, reorder).
Each render rebuilds the dialog content from `state.dictionaries`.
If we wrote directly to disk on every click we'd get a flurry of
saves and reindexes for an interactive session. Working copy +
explicit Save keeps things sane.

## Hot reload

The library's connection pool is a `RwLock<HashMap<file, Pool>>`
that fills lazily — `pool_for(file)` creates a pool on first
request. `reset_pools()` empties the map. Together this means:

- Toggling a dictionary on or off takes effect on the **next** query
  (the pool is recreated against the current `.db`).
- Reordering takes effect on the next query (the iteration order
  comes from `enabled_mdx()`, which always reads current settings).
- Enabling a dictionary that has no `.db` yet triggers indexing in
  the background; the first query after indexing completes is when
  the pool gets built. Until then the dict shows up but has zero
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
  ~80 MB MDX → 580 MB SQLite, and the MDD is ~1 GB). The dialog
  Save closes the modal immediately and the indexer continues in
  the background — there's no progress UI yet, only `tracing::info`
  lines.
- Disabling a dictionary doesn't delete its `.db` files. Re-enabling
  is instant. If you actually want to reclaim disk, delete the
  `.mdx.db` and `.mdd.db` files manually after disabling.
