use gpui::{
    AppContext as _, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::{
    h_flex,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::{
    app::DictApp,
    colors,
    components::{init_modal, settings_panel},
    state::DictState,
};

pub fn overlay(
    state: Entity<DictState>,
    window: &mut Window,
    cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    if !state.read(cx).show_settings_modal {
        return div().into_any_element();
    }

    let active_tab = state.read(cx).settings_active_tab;
    let is_importing = state.read(cx).import_files.iter().any(|f| {
        matches!(
            f.status,
            crate::state::ImportStatus::Copying | crate::state::ImportStatus::Indexing
        )
    });

    div()
        .id("settings-modal-backdrop")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.05,
            a: 0.88,
        })
        .child(modal_card(state, active_tab, is_importing, window, cx))
        .into_any_element()
}

fn modal_card(
    state: Entity<DictState>,
    active_tab: usize,
    is_importing: bool,
    _window: &mut Window,
    cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    v_flex()
        .id("settings-modal-card")
        .w(px(560.))
        .min_h(px(600.))
        .max_h(px(600.))
        .rounded(px(12.))
        .bg(colors::surface())
        .border_1()
        .border_color(colors::border())
        .overflow_hidden()
        .child(modal_header(state.clone(), active_tab, is_importing, cx))
        .child(tab_body(state, active_tab, is_importing, cx))
        .into_any_element()
}

fn modal_header(
    state: Entity<DictState>,
    active_tab: usize,
    is_importing: bool,
    cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    let tab_state = state.clone();

    h_flex()
        .w_full()
        .px(px(24.))
        .pt(px(16.))
        .pb(px(0.))
        .items_center()
        .justify_between()
        .child(
            TabBar::new("settings-tabs")
                .underline()
                .selected_index(active_tab)
                .cursor_pointer()
                .on_click(move |&ix, _window, cx| {
                    cx.update_entity(&tab_state, |s, cx| {
                        s.settings_active_tab = ix;
                        cx.notify();
                    });
                })
                .child(Tab::new().label("Dictionaries"))
                .child(Tab::new().label("Import")),
        )
        .child(close_button(state, is_importing, cx))
        .into_any_element()
}

fn close_button(
    state: Entity<DictState>,
    is_importing: bool,
    _cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    if is_importing {
        return div().into_any_element();
    }

    div()
        .id("settings-modal-close")
        .px(px(10.))
        .py(px(4.))
        .rounded(px(6.))
        .text_size(px(12.))
        .text_color(colors::text_secondary())
        .border_1()
        .border_color(colors::border())
        .cursor_pointer()
        .hover(|s| s.bg(colors::bg()).text_color(colors::text()))
        .child("✕")
        .on_click(move |_, _, cx| {
            cx.update_entity(&state, |s, cx| {
                s.show_settings_modal = false;
                s.import_files.clear();
                s.dictionaries = mdict_rs::settings::current().dictionaries;
                cx.notify();
            });
        })
        .into_any_element()
}

fn tab_body(
    state: Entity<DictState>,
    active_tab: usize,
    is_importing: bool,
    cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    v_flex()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .px(px(24.))
        .pt(px(16.))
        .pb(px(20.))
        .child(if active_tab == 0 {
            dicts_tab(state, cx)
        } else {
            import_tab(state, is_importing, cx)
        })
        .into_any_element()
}

fn dicts_tab(state: Entity<DictState>, cx: &mut Context<DictApp>) -> gpui::AnyElement {
    let save_state = state.clone();
    let cancel_state = state.clone();

    v_flex()
        .w_full()
        .gap(px(12.))
        .child(settings_panel::panel_content(state, cx))
        .child(
            h_flex()
                .w_full()
                .justify_end()
                .gap(px(8.))
                .child(
                    div()
                        .id("settings-cancel-btn")
                        .px(px(14.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .text_size(px(13.))
                        .text_color(colors::text_secondary())
                        .border_1()
                        .border_color(colors::border())
                        .bg(colors::bg())
                        .cursor_pointer()
                        .hover(|s| s.bg(colors::surface()))
                        .child("Cancel")
                        .on_click(move |_, _, cx| {
                            settings_panel::revert(&cancel_state, cx);
                            cx.update_entity(&cancel_state, |s, cx| {
                                s.show_settings_modal = false;
                                s.import_files.clear();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    div()
                        .id("settings-save-btn")
                        .px(px(14.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::bg())
                        .bg(colors::primary())
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .child("Save")
                        .on_click(move |_, _, cx| {
                            settings_panel::apply_save(&save_state, cx);
                            cx.update_entity(&save_state, |s, cx| {
                                s.show_settings_modal = false;
                                s.import_files.clear();
                                cx.notify();
                            });
                        }),
                ),
        )
        .into_any_element()
}

fn import_tab(
    state: Entity<DictState>,
    is_importing: bool,
    cx: &mut Context<DictApp>,
) -> gpui::AnyElement {
    init_modal::import_tab_content(state, is_importing, cx)
}
