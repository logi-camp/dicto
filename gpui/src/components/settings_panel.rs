//! Settings panel rendered as a Dialog modal.
//!
//! The cog button in the title bar acts as the Dialog trigger; the
//! dialog itself owns its open/close state via gpui-component's
//! overlay system. The dialog body is a scrollable list of
//! dictionaries with a checkbox-style toggle and up/down reorder
//! buttons.

use std::path::Path;

use gpui::{
    div, px, AppContext as _, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use gpui_component::{
    dialog::DialogContent,
    scroll::ScrollableElement,
    h_flex, v_flex,
};
use mdict_rs::settings::DictEntry;
use tracing::{info, warn};

use crate::{colors, state::DictState};

/// Build the contents shown inside the settings Dialog.
pub fn dialog_content(
    state: Entity<DictState>,
    content: DialogContent,
    _window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> DialogContent {
    let snapshot = state.read(cx).dictionaries.clone();

    let header = div()
        .text_size(px(12.))
        .text_color(colors::text_secondary())
        .child(SharedString::from(
            "Toggle dictionaries on or off, and reorder them. The top-most enabled \
             dictionary is queried first.",
        ));

    // `overflow_y_scrollbar` wraps the v_flex in a `size_full` container,
    // so we need to give it a concrete height — `max_h` alone doesn't
    // bound the wrapper. A fixed-pixel height is fine for a settings
    // dialog where we control the overall size.
    let mut list = v_flex()
        .id("settings-dict-list")
        .w_full()
        .gap(px(6.))
        .h(px(360.))
        .overflow_y_scrollbar();

    if snapshot.is_empty() {
        list = list.child(
            div()
                .text_size(px(13.))
                .text_color(colors::text_secondary())
                .py(px(20.))
                .child(SharedString::from(
                    "No .mdx files found in ~/.config/mdict-dict/mdict/. Drop dictionary \
                     files there and reopen this dialog.",
                )),
        );
    } else {
        let total = snapshot.len();
        for (i, entry) in snapshot.iter().enumerate() {
            list = list.child(row(i, entry, total, state.clone()));
        }
    }

    content.child(v_flex().w_full().gap(px(10.)).child(header).child(list))
}

fn row(idx: usize, entry: &DictEntry, total: usize, state: Entity<DictState>) -> gpui::AnyElement {
    let display = Path::new(&entry.path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&entry.path)
        .to_string();
    let path_text = entry.path.clone();
    let enabled = entry.enabled;

    let toggle_state = state.clone();
    let up_state = state.clone();
    let down_state = state.clone();

    h_flex()
        .id(SharedString::from(format!("dict-row-{idx}")))
        .w_full()
        .gap(px(8.))
        .items_center()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(6.))
        .bg(colors::bg())
        .border_1()
        .border_color(colors::border())
        .child(
            div()
                .id(SharedString::from(format!("dict-toggle-{idx}")))
                .flex()
                .items_center()
                .justify_center()
                .w(px(18.))
                .h(px(18.))
                .rounded(px(4.))
                .border_1()
                .border_color(if enabled { colors::primary() } else { colors::border() })
                .bg(if enabled { colors::primary() } else { colors::bg() })
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(colors::bg())
                .child(if enabled { "✓" } else { "" })
                .on_click(move |_, _, cx| {
                    cx.update_entity(&toggle_state, |s, cx| {
                        if let Some(d) = s.dictionaries.get_mut(idx) {
                            d.enabled = !d.enabled;
                        }
                        cx.notify();
                    });
                }),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w(px(0.))
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text())
                        .child(SharedString::from(display)),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(colors::text_secondary())
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(SharedString::from(path_text)),
                ),
        )
        .child(arrow_button("up", idx, idx == 0, move |cx| {
            cx.update_entity(&up_state, |s, cx| {
                if idx > 0 && idx < s.dictionaries.len() {
                    s.dictionaries.swap(idx, idx - 1);
                    cx.notify();
                }
            });
        }))
        .child(arrow_button("down", idx, idx + 1 >= total, move |cx| {
            cx.update_entity(&down_state, |s, cx| {
                if idx + 1 < s.dictionaries.len() {
                    s.dictionaries.swap(idx, idx + 1);
                    cx.notify();
                }
            });
        }))
        .into_any_element()
}

fn arrow_button(
    kind: &'static str,
    idx: usize,
    disabled: bool,
    on_click: impl Fn(&mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let glyph = if kind == "up" { "▲" } else { "▼" };
    let id = SharedString::from(format!("arrow-{kind}-{idx}"));

    let mut el = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(24.))
        .h(px(24.))
        .rounded(px(4.))
        .border_1()
        .border_color(colors::border())
        .text_size(px(10.))
        .text_color(if disabled { colors::text_secondary() } else { colors::text() })
        .bg(colors::bg())
        .child(glyph);

    if !disabled {
        el = el
            .cursor_pointer()
            .hover(|s| s.bg(colors::surface()))
            .on_click(move |_, _, cx| on_click(cx));
    }
    el.into_any_element()
}

/// Persist the edited dictionary list, drop stale pools, and re-index
/// any newly enabled dictionary in the background.
pub fn apply_save(state: &Entity<DictState>, cx: &mut gpui::App) {
    let edited = state.read(cx).dictionaries.clone();
    let executor = cx.background_executor().clone();

    let new_settings = mdict_rs::settings::Settings {
        dictionaries: edited,
    };
    if let Err(e) = mdict_rs::settings::update(new_settings) {
        warn!("settings: save failed: {e}");
        return;
    }
    mdict_rs::config::reset_pools();
    info!("settings: pools reset, kicking off background reindex");

    let mdx = mdict_rs::settings::enabled_mdx();
    executor
        .spawn(async move {
            for path in mdx {
                if let Some(dict) = mdict_rs::formats::detect(&path) {
                    if let Err(e) = dict.build_index(false) {
                        warn!("reindex failed for {path}: {e}");
                    }
                }
            }
            mdict_rs::registry::reload();
            info!("settings: background reindex complete");
        })
        .detach();

    // Re-sync working copy from disk so any merge-with-disk normalization
    // (newly discovered files, dropped missing files) shows up next open.
    cx.update_entity(state, |s, cx| {
        s.dictionaries = mdict_rs::settings::current().dictionaries;
        cx.notify();
    });
}

/// Restore the working copy from disk (used by the dialog's Cancel button).
pub fn revert(state: &Entity<DictState>, cx: &mut gpui::App) {
    cx.update_entity(state, |s, cx| {
        s.dictionaries = mdict_rs::settings::current().dictionaries;
        cx.notify();
    });
}
