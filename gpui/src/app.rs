use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{Root, TitleBar, h_flex, input::InputState, v_flex};

use crate::colors;
use crate::components::{
    detail_panel, init_modal,
    search_bar::{self, SearchBarProps},
    settings_modal,
    word_list::{self, WordListProps},
};
use crate::state::{DictResult, DictState};

pub struct DictApp {
    pub state: Entity<DictState>,
    input: Entity<InputState>,
}

impl DictApp {
    pub fn new(state: Entity<DictState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_placeholder("Search words...", window, cx);
            s
        });

        let dict_state = state.clone();
        cx.observe(&input, move |this: &mut DictApp, input, cx| {
            let text = input.read(cx).value().to_string();
            if text.is_empty() {
                cx.update_entity(&this.state, |s, cx| {
                    s.suggestions.clear();
                    s.selected_suggestion = None;
                    cx.notify();
                });
                return;
            }

            let dict_state = dict_state.clone();
            cx.spawn(async move |_this, cx| {
                // Debounce: wait for the user to stop typing before fetching
                // suggestions. 150ms is short enough to feel responsive but
                // long enough to avoid hammering the FST on every keystroke.
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;

                let q = text.clone();
                let query_len = q.len();
                let suggestions = cx
                    .background_executor()
                    .spawn(async move { mdict_rs::query::search_suggestions(&q, 50) })
                    .await;

                // Auto-select the first suggestion for queries of 3+ chars.
                // For shorter queries the list is shown but nothing is
                // selected, avoiding jarring previews on single-letter input.
                let auto_select = if query_len >= 3 && !suggestions.is_empty() {
                    Some(0)
                } else {
                    None
                };

                cx.update_entity(&dict_state, |s, cx| {
                    let changed = s.suggestions != suggestions;
                    s.suggestions = suggestions;
                    if changed {
                        s.selected_suggestion = auto_select;
                    }
                    cx.notify();
                });

                // Debounced definition preview: load the definition only
                // after the user pauses typing for 200ms. This prevents
                // rapid-fire definition parsing (which involves HTML
                // parsing, CSS matching, and MDD resource lookups) on
                // every intermediate keystroke.
                if auto_select.is_some() {
                    // Read the first suggestion to preview.
                    let Some(word) =
                        cx.update_entity(&dict_state, |s, _cx| s.suggestions.first().cloned())
                    else {
                        return;
                    };

                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;

                    // Re-read state after the quiet period — if the
                    // user typed more, the suggestions will have
                    // changed and we should skip this stale preview.
                    let should_preview = cx.update_entity(&dict_state, |s, _cx| {
                        s.selected_suggestion == Some(0)
                            && s.result_word.as_deref() != Some(word.as_str())
                    });

                    if should_preview {
                        let word_for_result = word.clone();
                        let results = cx
                            .background_executor()
                            .spawn(async move {
                                mdict_rs::query::query_all(&word)
                                    .into_iter()
                                    .map(|hit| {
                                        let blocks =
                                            crate::html::parse_styled(&hit.definition, &hit.name);
                                        DictResult {
                                            name: hit.name,
                                            blocks,
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .await;

                        cx.update_entity(&dict_state, |s, cx| {
                            if s.selected_suggestion == Some(0) {
                                s.result_word = Some(word_for_result);
                                s.is_searching = false;
                                s.results = results;
                                s.active_result = 0;
                            }
                            cx.notify();
                        });
                    }
                }
            })
            .detach();
        })
        .detach();

        // Focus the search input on startup
        cx.update_entity(&input, |input_state, cx| {
            input_state.focus(window, cx);
        });

        Self { state, input }
    }

    pub fn lookup_word(&mut self, word: String, cx: &mut Context<Self>) {
        if word.is_empty() {
            return;
        }

        cx.update_entity(&self.state, |s, cx| {
            s.result_word = Some(word.clone());
            s.results.clear();
            s.active_result = 0;
            s.is_searching = true;
            cx.notify();
        });

        let dict_state = self.state.clone();
        cx.spawn(async move |_this, cx| {
            let q = word.clone();
            let results = cx
                .background_executor()
                .spawn(async move {
                    mdict_rs::query::query_all(&q)
                        .into_iter()
                        .map(|hit| {
                            let blocks = crate::html::parse_styled(&hit.definition, &hit.name);
                            DictResult {
                                name: hit.name,
                                blocks,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .await;

            cx.update_entity(&dict_state, |s, cx| {
                s.is_searching = false;
                s.results = results;
                s.active_result = 0;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for DictApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Root keeps active dialogs in a list, but doesn't render them
        // automatically — we have to append the dialog layer as a
        // sibling of the main view ourselves.
        let dialog_layer = Root::render_dialog_layer(window, cx);

        let input_handle = self.input.clone();
        let main = v_flex()
            .size_full()
            .bg(colors::bg())
            .child(
                TitleBar::new()
                    .on_close_window(|_, window, _| {
                        window.remove_window();
                    })
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors::primary())
                            .child("Dicto"),
                    ),
            )
            // Search row hosts the cog on its right edge so the button
            // sits outside the title bar's OS-claimed drag region.
            .child(search_bar::search_bar(
                SearchBarProps {
                    input: self.input.clone(),
                    state: self.state.clone(),
                    right_slot: Some(cog_button(self.state.clone())),
                },
                cx,
            ))
            .child(indexing_bar(&self.state, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .child(word_list::word_list(
                        WordListProps {
                            state: self.state.clone(),
                        },
                        cx,
                    ))
                    .child(detail_panel::detail_panel(self.state.clone(), cx)),
            );

        div()
            .size_full()
            .on_key_down(cx.listener(move |_this, event: &KeyDownEvent, window, cx| {
                let m = &event.keystroke.modifiers;
                let key = event.keystroke.key.as_str();

                if m.control && (key == "l" || key == "f") {
                    // Ctrl+L / Ctrl+F: focus search input
                    cx.update_entity(&input_handle, |input, cx| {
                        input.focus(window, cx);
                    });
                } else if key == "escape" {
                    // Escape: clear the search field
                    cx.update_entity(&input_handle, |input, cx| {
                        input.set_value("", window, cx);
                    });
                }
            }))
            .child(main)
            .children(dialog_layer)
            .child(settings_modal::overlay(self.state.clone(), window, cx))
            .child(init_modal::overlay(self.state.clone(), window, cx))
            .into_any_element()
    }
}

/// Slim progress bar shown while background indexing is running.
/// Returns an empty fragment when `indexing_total == 0` so we don't
/// reserve vertical space in the idle state.
fn indexing_bar(state: &Entity<DictState>, cx: &Context<DictApp>) -> gpui::AnyElement {
    let s = state.read(cx);
    if s.indexing_total == 0 {
        return div().into_any_element();
    }

    let done = s.indexing_done;
    let total = s.indexing_total;
    let pct = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let label = match &s.indexing_current {
        Some(name) => format!("Indexing {done}/{total} — {name}"),
        None => format!("Indexing {done}/{total}"),
    };

    v_flex()
        .w_full()
        .px(px(12.))
        .py(px(6.))
        .gap(px(4.))
        .bg(colors::surface())
        .border_b_1()
        .border_color(colors::border())
        .child(
            div()
                .text_size(px(11.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(label)),
        )
        .child(
            div()
                .w_full()
                .h(px(4.))
                .rounded(px(2.))
                .bg(colors::border())
                .child(
                    div()
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(colors::primary())
                        .w(gpui::relative(pct)),
                ),
        )
        .into_any_element()
}

fn cog_button(state: Entity<DictState>) -> gpui::AnyElement {
    div()
        .id("cog-settings-btn")
        .px(px(10.))
        .py(px(4.))
        .mr(px(8.))
        .rounded(px(6.))
        .text_size(px(12.))
        .text_color(colors::text())
        .bg(colors::bg())
        .border_1()
        .border_color(colors::border())
        .cursor_pointer()
        .hover(|s| s.bg(colors::surface()))
        .child(SharedString::from("⚙ Settings"))
        .on_click(move |_, _, cx| {
            cx.update_entity(&state, |s, cx| {
                s.show_settings_modal = true;
                s.settings_active_tab = 0;
                cx.notify();
            });
        })
        .into_any_element()
}
