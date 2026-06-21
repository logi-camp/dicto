use gpui::{
    AppContext as _, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Styled, px,
};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
};

use crate::{colors, state::DictState};

/// Props for the search bar component.
pub struct SearchBarProps {
    pub input: gpui::Entity<InputState>,
    pub state: gpui::Entity<DictState>,
    /// Optional element placed to the right of the input (e.g. settings button).
    pub right_slot: Option<gpui::AnyElement>,
}

/// Renders the search input bar with keyboard navigation (Up/Down/Enter).
pub fn search_bar(
    props: SearchBarProps,
    cx: &mut gpui::Context<crate::app::DictApp>,
) -> impl IntoElement {
    let input_el = Input::new(&props.input).appearance(true).cleanable(true);
    let state = props.state.clone();

    let mut row = h_flex()
        .id("search-bar")
        .w_full()
        .px(px(12.))
        .py(px(8.))
        .gap(px(8.))
        .items_center()
        .bg(colors::bg())
        .border_b_1()
        .border_color(colors::border())
        .child(h_flex().flex_1().w_full().child(input_el));

    if let Some(extra) = props.right_slot {
        row = row.child(extra);
    }

    row.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
        match event.keystroke.key.as_str() {
            "down" => navigate_down(&state, this, cx),
            "up" => navigate_up(&state, this, cx),
            _ => {}
        }
    }))
}

fn navigate_down(
    state: &gpui::Entity<DictState>,
    this: &mut crate::app::DictApp,
    cx: &mut gpui::Context<crate::app::DictApp>,
) {
    let s = state.read(cx);
    let len = s.suggestions.len();
    if len == 0 {
        return;
    }
    let max = len.saturating_sub(1);
    let current = s.selected_suggestion.unwrap_or(usize::MAX);
    let next = if current >= max { 0 } else { current + 1 };
    let word = s.suggestions.get(next).cloned();

    cx.update_entity(state, |s, cx| {
        s.selected_suggestion = Some(next);
        s.word_list_scroll.scroll_to_item(next);
        cx.notify();
    });
    if let Some(w) = word {
        this.lookup_word(w, cx);
    }
}

fn navigate_up(
    state: &gpui::Entity<DictState>,
    this: &mut crate::app::DictApp,
    cx: &mut gpui::Context<crate::app::DictApp>,
) {
    let s = state.read(cx);
    let len = s.suggestions.len();
    if len == 0 {
        return;
    }
    let max = len.saturating_sub(1);
    let current = s.selected_suggestion.unwrap_or(0);
    let prev = if current == 0 { max } else { current - 1 };
    let word = s.suggestions.get(prev).cloned();

    cx.update_entity(state, |s, cx| {
        s.selected_suggestion = Some(prev);
        s.word_list_scroll.scroll_to_item(prev);
        cx.notify();
    });
    if let Some(w) = word {
        this.lookup_word(w, cx);
    }
}
