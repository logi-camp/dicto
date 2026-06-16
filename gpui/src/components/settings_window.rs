use gpui::{AppContext as _, Entity, IntoElement, ParentElement, Styled};
use gpui_component::{
    h_flex,
    tab::{Tab, TabBar},
};

use crate::state::DictState;

/// Dialog-compatible header tabs (takes &mut App).
pub fn header_tabs_for_dialog(
    state: Entity<DictState>,
    active_tab: usize,
    _cx: &mut gpui::App,
) -> gpui::AnyElement {
    let tab_state = state.clone();

    h_flex()
        .w_full()
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
                .child(Tab::new().label("Import"))
                .child(Tab::new().label("Download")),
        )
        .into_any_element()
}
