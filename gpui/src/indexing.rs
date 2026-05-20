//! Background indexing & stylesheet loading.
//!
//! `spawn` walks the enabled-MDX list, builds any missing redb indexes off
//! the main thread, updates `DictState` so the UI can show progress, and
//! reloads the registry + CSS as each dictionary completes.

use std::path::{Path, PathBuf};

use gpui::{AppContext as _, AsyncApp, Entity};
use tracing::{info, warn};

use crate::html;
use crate::state::DictState;

/// Spawn background indexing on the AsyncApp's background executor.
/// Cheap to call when everything is already indexed — it just no-ops.
pub fn spawn(state: Entity<DictState>, cx: &mut gpui::App) {
    cx.spawn(async move |cx: &mut AsyncApp| run(state, cx).await).detach();
}

async fn run(state: Entity<DictState>, cx: &mut AsyncApp) {
    // Collect pending dictionaries (paths only — we re-detect inside the
    // background task so the `Box<dyn Dictionary>` doesn't need to cross
    // thread boundaries through this future).
    let pending: Vec<(String, String)> = mdict_rs::settings::enabled_mdx()
        .into_iter()
        .filter_map(|path| {
            let dict = mdict_rs::formats::detect(&path)?;
            if dict.index_ready() {
                return None;
            }
            let name = Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&path)
                .to_string();
            Some((path, name))
        })
        .collect();

    let total = pending.len();
    if total == 0 {
        return;
    }

    let _ = cx.update(|app| app.update_entity(&state, |s, cx| {
        s.indexing_total = total;
        s.indexing_done = 0;
        s.indexing_current = pending.first().map(|(_, n)| n.clone());
        cx.notify();
    }));

    for (i, (path, name)) in pending.into_iter().enumerate() {
        let _ = cx.update(|app| app.update_entity(&state, |s, cx| {
            s.indexing_current = Some(name.clone());
            cx.notify();
        }));

        let path_clone = path.clone();
        let result = cx
            .background_executor()
            .spawn(async move {
                if let Some(dict) = mdict_rs::formats::detect(&path_clone) {
                    dict.build_index(false)
                } else {
                    Ok(())
                }
            })
            .await;

        if let Err(e) = result {
            warn!("indexing failed for {path}: {e}");
        }

        mdict_rs::registry::reload();
        load_stylesheets();

        let _ = cx.update(|app| app.update_entity(&state, |s, cx| {
            s.indexing_done = i + 1;
            cx.notify();
        }));
    }

    let _ = cx.update(|app| app.update_entity(&state, |s, cx| {
        s.indexing_total = 0;
        s.indexing_done = 0;
        s.indexing_current = None;
        cx.notify();
    }));

    info!("background indexing complete");
}

/// Build a per-dictionary stylesheet by combining two sources:
/// (1) any `.css` resources stored *inside* the dict's `.mdd`
/// (2) sibling `.css` files on disk whose stem matches the dict stem
/// (`<stem>.css` or `<stem>_*.css`). Per-dict isolation prevents one
/// dictionary's rules from overriding another's.
pub fn load_stylesheets() {
    use std::collections::HashMap;
    let mut by_dict: HashMap<String, html::Stylesheet> = HashMap::new();

    for mdx in mdict_rs::settings::enabled_mdx() {
        let path = PathBuf::from(&mdx);
        let Some(dir) = path.parent() else { continue };
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };

        let mut sheet = html::Stylesheet::default();

        for (_, body) in mdict_rs::registry::css_for_dict(stem) {
            sheet.extend(html::Stylesheet::parse(&body));
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut css_paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().map_or(false, |e| e.eq_ignore_ascii_case("css"))
                        && css_belongs_to(p, stem)
                })
                .collect();
            css_paths.sort();

            for css in &css_paths {
                if let Ok(body) = std::fs::read_to_string(css) {
                    sheet.extend(html::Stylesheet::parse(&body));
                }
            }
        }

        by_dict.insert(stem.to_string(), sheet);
    }

    html::set_dict_styles(by_dict);
}

fn css_belongs_to(css_path: &Path, dict_stem: &str) -> bool {
    let Some(css_stem) = css_path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    css_stem == dict_stem || css_stem.starts_with(&format!("{dict_stem}_"))
}
