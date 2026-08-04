//! Quick Translate popup window.
//!
//! A small floating window that appears when the quick-translate hotkey is
//! pressed. Shows the selected text and its translation.

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};
use gpui::prelude::FluentBuilder as _;
use gpui_component::{h_flex, scroll::ScrollableElement, v_flex};

use crate::{colors, state::DictState};

/// State of the translation popup.
#[derive(Debug, Clone)]
pub enum PopupState {
    /// Selection captured; waiting for the user to click Translate.
    Idle { original: String },
    /// Loading the translation.
    Loading { original: String },
    /// Translation succeeded.
    Ready {
        original: String,
        translation: String,
        provider: String,
        model: String,
    },
    /// Translation failed.
    Error { original: String, error: String },
}

/// Build the popup view content.
///
/// `settings` carries the full quick-translate config (provider, model,
/// target_lang, TTS) so the inline Options panel can render and mutate it.
/// `state_entity` is the shared `DictState` so selectors/Translate can persist
/// and kick off translations. `options_open` toggles the inline Options panel.
pub fn translate_popup(
    state: &PopupState,
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state_entity: &Entity<DictState>,
    options_open: bool,
    playback_source: (crate::playback::PlaybackState, Option<std::time::Duration>),
    playback_translation: (crate::playback::PlaybackState, Option<std::time::Duration>),
    on_toggle_options: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let target_lang = settings.target_lang.clone();
    let tts = settings.tts.clone();
    let card = v_flex()
        .id("qt-popup-card")
        .w(px(460.))
        .min_h(px(120.))
        .max_h(px(560.))
        .p(px(14.))
        .bg(colors::surface())
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border())
        // Swallow clicks on the card itself so only the backdrop dismisses.
        .on_mouse_down(gpui::MouseButton::Left, |_, _window, cx| {
            cx.stop_propagation();
        });

    // The header row: "ORIGINAL" label + a Speak button for the source text.
    let original_header = |original: &str,
                           tts: &mdict_rs::settings::TtsSettings,
                           state: Entity<DictState>,
                           pb: (crate::playback::PlaybackState, Option<std::time::Duration>)| {
        h_flex()
            .justify_between()
            .items_center()
            .child(section_label("Original"))
            .child(speak_button(
                Slot::Source,
                original.to_string(),
                source_voice_lang(),
                tts.clone(),
                state,
                pb,
            ))
    };

    // The inline Options panel (provider/model/target-lang/TTS selectors).
    let options_panel = popup_options_panel(settings, state_entity.clone());

    let content = match state {
        PopupState::Idle { original } => v_flex()
            .gap(px(8.))
            .child(original_header(original, &tts, state_entity.clone(), playback_source.clone()))
            .child(scrollable_text(original, false))
            .child(divider())
            .child(translate_button(original.clone(), state_entity.clone()))
            .child(options_toggle(options_open, on_toggle_options))
            .when(options_open, |this| this.child(options_panel)),

        PopupState::Loading { original } => v_flex()
            .gap(px(8.))
            .child(original_header(original, &tts, state_entity.clone(), playback_source.clone()))
            .child(scrollable_text(original, false))
            .child(divider())
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(spinner_dots())
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(colors::text_secondary())
                            .child(SharedString::from("Translating…")),
                    ),
            )
            .child(options_toggle(options_open, on_toggle_options))
            .when(options_open, |this| this.child(options_panel)),

        PopupState::Ready {
            original,
            translation,
            provider,
            model,
        } => {
            let translation_for_btn = translation.clone();
            let lang_for_btn = target_lang.to_string();
            v_flex()
                .gap(px(8.))
                .child(original_header(original, &tts, state_entity.clone(), playback_source.clone()))
                .child(scrollable_text(original, false))
                .child(divider())
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(section_label("Translation"))
                        .child(speak_button(Slot::Translation, translation_for_btn, lang_for_btn, tts.clone(), state_entity.clone(), playback_translation.clone())),
                )
                .child(scrollable_text(translation, true))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(colors::text_secondary())
                        .child(SharedString::from(format!("via {} · {}", provider, model))),
                )
                .child(translate_button(original.clone(), state_entity.clone()))
                .child(options_toggle(options_open, on_toggle_options))
                .when(options_open, |this| this.child(options_panel))
        }

        PopupState::Error { original, error } => v_flex()
            .gap(px(8.))
            .child(original_header(original, &tts, state_entity.clone(), playback_source.clone()))
            .child(scrollable_text(original, false))
            .child(divider())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(colors::error())
                    .child(SharedString::from(format!("Translation failed: {}", error))),
            )
            .child(translate_button(original.clone(), state_entity.clone()))
            .child(options_toggle(options_open, on_toggle_options))
            .when(options_open, |this| this.child(options_panel)),
    };

    // Wrap content in a scroll container so the popup never overflows its
    // max_h when the Options panel (or long text) pushes content past it.
    // Two-layer pattern: outer = flex sizing (bounded by the card's max_h),
    // inner = h_full + overflow_y_scroll. min_h(0.) lets flex shrink it below
    // its content's natural size so scrolling actually engages.
    let scroll_content = div()
        .id("qt-popup-scroll")
        .w_full()
        .h_full()
        .overflow_y_scrollbar()
        .child(content);

    card.child(
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .child(scroll_content),
    )
    .into_any_element()
}

/// A compact "⚙ Options" toggle button shown at the bottom of every popup state.
fn options_toggle(
    open: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let label = if open { "▾ Hide options" } else { "▸ Options" };
    div()
        .id("qt-popup-options-toggle")
        .cursor_pointer()
        .text_size(px(11.))
        .text_color(colors::text_secondary())
        .child(SharedString::from(label))
        .on_click(on_click)
        .into_any_element()
}

/// The inline Options panel: compact selectors for provider, model, target
/// language, and TTS. Each selector mutates `DictState`, persists, and — for
/// translation-affecting changes — reloads the translator.
fn popup_options_panel(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use mdict_rs::settings::LlmProvider;

    v_flex()
        .mt(px(2.))
        .p(px(10.))
        .gap(px(8.))
        .bg(colors::bg())
        .rounded(px(6.))
        .border_1()
        .border_color(colors::border())
        .child(popup_field("Provider", popup_provider_buttons(settings, state.clone())))
        .child(popup_field("Model", popup_model_buttons(settings, state.clone())))
        .child(popup_field("Target", popup_target_lang_buttons(settings, state.clone())))
        .child(divider())
        .child(popup_field("TTS", popup_tts_preset_buttons(settings, state.clone())))
        .child(popup_field("Voice", popup_tts_voice_buttons(settings, state)))
        .into_any_element()
}

/// Label (small, above) + content; compact for the 460px popup.
fn popup_field(label: &str, content: gpui::AnyElement) -> gpui::AnyElement {
    v_flex()
        .gap(px(3.))
        .child(
            div()
                .text_size(px(10.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(label.to_uppercase())),
        )
        .child(content)
        .into_any_element()
}

/// Compact selectable chip; smaller than the settings panel's provider_button.
fn chip<F>(label: &str, selected: bool, on_click: F) -> gpui::AnyElement
where
    F: Fn(&mut gpui::App) + 'static,
{
    let base = div()
        .id(SharedString::from(format!("qt-chip-{label}")))
        .px(px(8.))
        .py(px(3.))
        .rounded(px(4.))
        .cursor_pointer()
        .text_size(px(11.))
        .on_click(move |_, _, cx| on_click(cx));

    if selected {
        base.bg(colors::primary())
            .text_color(colors::bg())
            .child(SharedString::from(label))
            .into_any_element()
    } else {
        base.bg(colors::surface())
            .text_color(colors::text_secondary())
            .border_1()
            .border_color(colors::border())
            .child(SharedString::from(label))
            .into_any_element()
    }
}

fn wrap_chips(children: Vec<gpui::AnyElement>) -> gpui::AnyElement {
    let mut row = h_flex().gap(px(4.)).flex_wrap();
    for c in children {
        row = row.child(c);
    }
    row.into_any_element()
}

fn popup_provider_buttons(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use mdict_rs::settings::LlmProvider;

    let is_anthropic = matches!(settings.llm_provider, LlmProvider::Anthropic);
    let is_openai = matches!(settings.llm_provider, LlmProvider::OpenAiCompatible);

    wrap_chips(vec![
        chip("Anthropic", is_anthropic, {
            let s = state.clone();
            move |cx| {
                s.update(cx, |st, cx| {
                    st.quick_translate.llm_provider = LlmProvider::Anthropic;
                    st.save_settings(cx);
                    st.reload_translator(cx);
                });
            }
        }),
        chip("OpenAI", is_openai, {
            let s = state;
            move |cx| {
                s.update(cx, |st, cx| {
                    st.quick_translate.llm_provider = LlmProvider::OpenAiCompatible;
                    st.save_settings(cx);
                    st.reload_translator(cx);
                });
            }
        }),
    ])
}

fn popup_model_buttons(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use crate::components::qt_catalog;

    let models = qt_catalog::models_for(settings.llm_provider);
    let current = settings.model.as_str();
    let in_list = models.iter().any(|(id, _)| *id == current);

    let mut chips: Vec<gpui::AnyElement> = Vec::new();
    for (id, label) in models {
        let selected = *id == current;
        let id_owned = id.to_string();
        chips.push(chip(label, selected, {
            let s = state.clone();
            move |cx| {
                s.update(cx, |st, cx| {
                    st.quick_translate.model = id_owned.clone();
                    st.save_settings(cx);
                    st.reload_translator(cx);
                });
            }
        }));
    }
    if !in_list && !current.is_empty() {
        chips.push(chip(&format!("Custom: {current}"), true, |_| {}));
    }
    wrap_chips(chips)
}

fn popup_target_lang_buttons(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use crate::components::qt_catalog;

    let current = settings.target_lang.as_str();
    let in_list = qt_catalog::TARGET_LANGS.iter().any(|l| *l == current);

    let mut chips: Vec<gpui::AnyElement> = Vec::new();
    for lang in qt_catalog::TARGET_LANGS {
        let selected = *lang == current;
        let l = lang.to_string();
        chips.push(chip(lang, selected, {
            let s = state.clone();
            move |cx| {
                s.update(cx, |st, cx| {
                    st.quick_translate.target_lang = l.clone();
                    st.save_settings(cx);
                    // Sync the engine's settings copy so the popup re-renders
                    // with the new selection (the popup reads from the engine).
                    st.reload_translator(cx);
                });
            }
        }));
    }
    chips.push(chip("Custom…", !in_list, {
        let s = state;
        move |cx| {
            s.update(cx, |st, cx| {
                if qt_catalog::TARGET_LANGS
                    .iter()
                    .any(|l| *l == st.quick_translate.target_lang.as_str())
                    || st.quick_translate.target_lang.is_empty()
                {
                    st.quick_translate.target_lang = String::new();
                    st.save_settings(cx);
                    st.reload_translator(cx);
                }
            });
        }
    }));
    wrap_chips(chips)
}

fn popup_tts_preset_buttons(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use crate::components::qt_catalog;

    let active = qt_catalog::find_tts_preset(&settings.tts.model, &settings.tts.api_base_url);
    let mut chips: Vec<gpui::AnyElement> = Vec::new();
    for (i, preset) in qt_catalog::TTS_PRESETS.iter().enumerate() {
        let selected = active == Some(i);
        let model = preset.model.to_string();
        let base_url = preset.base_url.to_string();
        let first_voice = preset
            .voices
            .first()
            .map(|v| v.to_string())
            .unwrap_or_default();
        chips.push(chip(preset.label, selected, {
            let s = state.clone();
            move |cx| {
                s.update(cx, |st, cx| {
                    st.quick_translate.tts.model = model.clone();
                    st.quick_translate.tts.api_base_url = base_url.clone();
                    st.quick_translate.tts.voice = first_voice.clone();
                    st.save_settings(cx);
                    // Sync the engine's settings copy so the popup re-renders.
                    st.reload_translator(cx);
                });
            }
        }));
    }
    if active.is_none() && !settings.tts.model.is_empty() {
        chips.push(chip(&format!("Custom: {}", settings.tts.model), true, |_| {}));
    }
    wrap_chips(chips)
}

fn popup_tts_voice_buttons(
    settings: &mdict_rs::settings::QuickTranslateSettings,
    state: Entity<DictState>,
) -> gpui::AnyElement {
    use crate::components::qt_catalog;

    let active = qt_catalog::find_tts_preset(&settings.tts.model, &settings.tts.api_base_url);
    let mut chips: Vec<gpui::AnyElement> = Vec::new();

    if let Some(idx) = active {
        let preset = &qt_catalog::TTS_PRESETS[idx];
        let current = settings.tts.voice.as_str();
        let in_list = preset.voices.iter().any(|v| *v == current);
        for voice in preset.voices {
            let selected = *voice == current;
            let v = voice.to_string();
            chips.push(chip(voice, selected, {
                let s = state.clone();
                move |cx| {
                    s.update(cx, |st, cx| {
                        st.quick_translate.tts.voice = v.clone();
                        st.save_settings(cx);
                        // Sync the engine's settings copy so the popup re-renders.
                        st.reload_translator(cx);
                    });
                }
            }));
        }
        if !in_list && !current.is_empty() {
            chips.push(chip(&format!("Custom: {current}"), true, |_| {}));
        }
    } else {
        chips.push(
            div()
                .text_size(px(11.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(settings.tts.voice.clone()))
                .into_any_element(),
        );
    }
    wrap_chips(chips)
}

fn section_label(text: &str) -> gpui::AnyElement {
    div()
        .text_size(px(11.))
        .text_color(colors::text_secondary())
        .child(SharedString::from(text.to_uppercase()))
        .into_any_element()
}

/// Scrollable text block. `emphasize` bumps the font size/weight for the
/// translation. The outer flex container has a bounded max height so
/// `overflow_y_scrollbar` (which wraps in a `size_full` div) actually scrolls.
fn scrollable_text(text: &str, emphasize: bool) -> gpui::AnyElement {
    let max_h = if emphasize { px(280.) } else { px(120.) };
    let mut block = div()
        .id(if emphasize { "qt-translation-scroll" } else { "qt-original-scroll" })
        .max_h(max_h)
        .min_h(px(0.))
        .w_full()
        .text_color(colors::text())
        .overflow_y_scrollbar();
    if emphasize {
        block = block.text_size(px(14.)).font_weight(gpui::FontWeight::MEDIUM);
    } else {
        block = block.text_size(px(13.));
    }
    block
        .child(SharedString::from(text.to_string()))
        .into_any_element()
}

fn divider() -> gpui::AnyElement {
    div().h(px(1.)).bg(colors::border()).into_any_element()
}

/// Language hint for speaking the *source* text. We don't reliably know the
/// source language, so let the platform TTS pick its default voice.
fn source_voice_lang() -> String {
    String::new()
}

/// The primary action button: kicks off a translation via the engine. Shown in
/// the `Idle` state.
fn translate_button(original: String, state: Entity<DictState>) -> gpui::AnyElement {
    div()
        .id("qt-translate-btn")
        .px(px(12.))
        .py(px(6.))
        .rounded(px(6.))
        .bg(colors::primary())
        .text_size(px(12.))
        .text_color(gpui::rgb(0x0f1117))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .hover(|s| s.opacity(0.9))
        .cursor_pointer()
        .child(SharedString::from("Translate"))
        .on_click(move |_ev, _window, cx| {
            let original = original.clone();
            let state = state.clone();
            cx.update_entity(&state, |s, cx| {
                if let Some(engine) = s.quick_translate_engine.as_mut() {
                    if let Some(job) = engine.restart_translation(original) {
                        spawn_translation(job, state.clone(), cx);
                    }
                    cx.notify();
                }
            });
        })
        .into_any_element()
}

/// Run a `TranslationJob` on the background executor and feed its outcome back
/// into the engine, then notify the popup to re-render. Mirrors the poll loop's
/// spawn in app.rs.
fn spawn_translation(
    job: crate::quick_translate::TranslationJob,
    entity: Entity<DictState>,
    cx: &mut gpui::App,
) {
    cx.spawn(async move |cx| {
        let outcome = cx
            .background_executor()
            .spawn(async move { job.run() })
            .await;
        cx.update_entity(&entity, |s, cx| {
            if let Some(engine) = s.quick_translate_engine.as_mut() {
                engine.apply_translation_result(outcome);
            }
            cx.notify();
        });
    })
    .detach();
}

fn spinner_dots() -> gpui::AnyElement {
    h_flex()
        .gap(px(2.))
        .child(dot(colors::text_secondary()))
        .child(dot(colors::text_secondary()))
        .child(dot(colors::text_secondary()))
        .into_any_element()
}

fn dot(color: gpui::Hsla) -> gpui::AnyElement {
    div()
        .w(px(4.))
        .h(px(4.))
        .rounded(px(2.))
        .bg(color)
        .into_any_element()
}

/// A small "Speak" button that reads text aloud. Uses the AI TTS API when
/// configured, else the platform TTS. `lang` is a BCP-47 hint; empty means "use
/// the default voice". Runs on a background thread so the UI never blocks.
/// The Speak button + (when a clip is loaded for this slot) the playback
/// controls: play/pause, replay, and a seek bar. Uses the shared
/// `PlaybackController` on `DictState` so clips persist across re-renders and
/// can be paused/seeked/replayed without re-synthesizing.
///
/// `playback` is the controller's current snapshot (state + total duration),
/// polled by the view on a timer and passed in so this pure view-builder needs
/// no GPUI context to read live state.
/// Which playback slot a Speak button drives. The source text and the
/// translation each get an independent `PlaybackController` on `DictState`
/// (`playback_source` / `playback_translation`) so their states never mix.
#[derive(Clone, Copy)]
enum Slot {
    Source,
    Translation,
}

impl Slot {
    fn controller<'a>(self, s: &'a DictState) -> &'a crate::playback::PlaybackController {
        match self {
            Slot::Source => &s.playback_source,
            Slot::Translation => &s.playback_translation,
        }
    }

    fn controller_mut<'a>(self, s: &'a mut DictState) -> &'a mut crate::playback::PlaybackController {
        match self {
            Slot::Source => &mut s.playback_source,
            Slot::Translation => &mut s.playback_translation,
        }
    }
}

fn speak_button(
    slot: Slot,
    text: String,
    lang: String,
    tts: mdict_rs::settings::TtsSettings,
    state: Entity<DictState>,
    playback: (crate::playback::PlaybackState, Option<std::time::Duration>),
) -> gpui::AnyElement {
    // Disambiguate the source vs. translation speak buttons (DOM id only;
    // routing is driven by the explicit `slot` param, NOT by lang emptiness —
    // a translation can legitimately have an empty target lang).
    let btn_id = match slot {
        Slot::Source => "qt-speak-src",
        Slot::Translation => "qt-speak-tr",
    };

    let (pb_state, total) = playback;
    let lang_opt = if lang.is_empty() { None } else { Some(lang.clone()) };

    // The Speak button itself. Shows "⏳ Loading…" while synthesizing.
    let label = if matches!(pb_state, crate::playback::PlaybackState::Loading) {
        "⏳ Loading…"
    } else {
        "🔊 Speak"
    };

    let speak_btn = div()
        .id(btn_id)
        .px(px(8.))
        .py(px(3.))
        .rounded(px(6.))
        .bg(colors::surface_alt())
        .border_1()
        .border_color(colors::border())
        .text_size(px(11.))
        .text_color(colors::text_secondary())
        .hover(|s| s.bg(colors::hover()).text_color(colors::text()))
        .cursor_pointer()
        .child(SharedString::from(label))
        .on_click({
            let state = state.clone();
            let text = text.clone();
            let lang = lang_opt.clone();
            let tts = tts.clone();
            move |_ev, _window, cx| {
                spawn_speak(slot, state.clone(), text.clone(), lang.clone(), tts.clone(), cx);
            }
        });

    // When a clip is loaded (Playing/Paused/Ended), append the controls row.
    // Ended keeps the seek bar full + the play button replays from start.
    match pb_state {
        crate::playback::PlaybackState::Playing { pos }
        | crate::playback::PlaybackState::Paused { pos }
        | crate::playback::PlaybackState::Ended { pos } => {
            let playing = matches!(
                pb_state,
                crate::playback::PlaybackState::Playing { .. }
            );
            h_flex()
                .gap(px(6.))
                .items_center()
                .child(speak_btn)
                .child(play_pause_button(slot, state.clone(), playing))
                .child(replay_button(slot, state.clone()))
                .child(seek_bar(slot, state.clone(), pos, total))
                .into_any_element()
        }
        _ => speak_btn.into_any_element(),
    }
}

/// Spawn the synthesis + install pipeline on the background executor.
fn spawn_speak(
    slot: Slot,
    state: Entity<DictState>,
    text: String,
    lang: Option<String>,
    tts: mdict_rs::settings::TtsSettings,
    cx: &mut gpui::App,
) {
    // Read the controller's decision via the entity (slot picks source/translation).
    let action = slot.controller(&state.read(cx)).start_load(
        text.clone(),
        lang.clone(),
        Some(tts.clone()),
    );
    if let crate::playback::LoadAction::Synthesize { text, lang, tts } = action {
        // `text` is needed twice: once for synthesis (moved into the bg task)
        // and once to tag the installed clip (so replay-by-text works). Clone
        // the latter before the move.
        let text_for_install = text.clone();
        cx.spawn(async move |cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    crate::tts::synthesize_bytes(&text, lang.as_deref(), tts.as_ref())
                })
                .await;
            match result {
                Ok(bytes) => {
                    let _ = cx.update_entity(&state, |s, _cx| {
                        slot.controller_mut(s).install_from_bytes(text_for_install, bytes);
                    });
                }
                Err(e) => {
                    let _ = cx.update_entity(&state, |s, _cx| {
                        slot.controller_mut(s).fail(e.to_string());
                    });
                }
            }
        })
        .detach();
    }
}

/// Play/Pause toggle button.
fn play_pause_button(slot: Slot, state: Entity<DictState>, playing: bool) -> gpui::AnyElement {
    let label = if playing { "⏸" } else { "▶" };
    let id = match slot {
        Slot::Source => "qt-play-pause-src",
        Slot::Translation => "qt-play-pause-tr",
    };
    div()
        .id(id)
        .px(px(8.))
        .py(px(3.))
        .rounded(px(6.))
        .bg(colors::surface_alt())
        .border_1()
        .border_color(colors::border())
        .text_size(px(12.))
        .text_color(colors::text())
        .hover(|s| s.bg(colors::hover()))
        .cursor_pointer()
        .child(SharedString::from(label))
        .on_click(move |_ev, _window, cx| {
            let _ = cx.update_entity(&state, |s, _cx| {
                slot.controller_mut(s).toggle_pause();
            });
        })
        .into_any_element()
}

/// Replay button: seek to start + play.
fn replay_button(slot: Slot, state: Entity<DictState>) -> gpui::AnyElement {
    let id = match slot {
        Slot::Source => "qt-replay-src",
        Slot::Translation => "qt-replay-tr",
    };
    div()
        .id(id)
        .px(px(8.))
        .py(px(3.))
        .rounded(px(6.))
        .bg(colors::surface_alt())
        .border_1()
        .border_color(colors::border())
        .text_size(px(12.))
        .text_color(colors::text())
        .hover(|s| s.bg(colors::hover()))
        .cursor_pointer()
        .child(SharedString::from("↺"))
        .on_click(move |_ev, _window, cx| {
            let _ = cx.update_entity(&state, |s, _cx| {
                slot.controller_mut(s).replay();
            });
        })
        .into_any_element()
}

/// Clickable seek bar showing playback progress. Click position sets the seek
/// fraction. Rendered with a fixed track width + a filled portion.
///
/// `ClickEvent::position()` is **window-relative**, not element-relative, so we
/// capture the seek bar's own bounds (via a zero-size `canvas` that records them
/// during paint) and subtract its left edge in the click handler.
fn seek_bar(slot: Slot, state: Entity<DictState>, pos: f32, total: Option<std::time::Duration>) -> gpui::AnyElement {
    use std::cell::Cell;
    use std::rc::Rc;

    // Fraction of the clip played. If we don't know total duration, show an
    // indeterminate-ish 0% track that's still clickable (seek is a no-op then).
    let frac = total
        .filter(|t| t.as_secs_f32() > 0.0)
        .map(|t| (pos / t.as_secs_f32()).clamp(0.0, 1.0))
        .unwrap_or(0.0);

    let track_w = px(120.);
    let fill_w = track_w * frac;

    // Shared cell holding the seek bar's last-painted window-relative bounds.
    // Updated every paint; read in the click handler to localize the click.
    let bounds_cell: Rc<Cell<Option<gpui::Bounds<gpui::Pixels>>>> = Rc::new(Cell::new(None));
    let bounds_for_canvas = bounds_cell.clone();
    let bounds_for_click = bounds_cell.clone();

    let id = match slot {
        Slot::Source => "qt-seek-bar-src",
        Slot::Translation => "qt-seek-bar-tr",
    };
    div()
        .id(id)
        .w(track_w)
        .h(px(6.))
        .rounded(px(3.))
        .bg(colors::surface_alt())
        .border_1()
        .border_color(colors::border())
        .relative()
        .child(
            div()
                .w(fill_w)
                .h_full()
                .rounded(px(3.))
                .bg(colors::primary())
                .absolute()
                .left_0()
                .top_0(),
        )
        // An invisible full-size canvas overlay whose only job is to record
        // this element's bounds during paint, so the click handler can map
        // window-relative coords → a fraction along the track.
        .child(
            div()
                .absolute()
                .size_full()
                .child(gpui::canvas(
                    move |_bounds, _window, _cx| {},
                    move |bounds, _t, _window, _cx| {
                        bounds_for_canvas.set(Some(bounds));
                    },
                )),
        )
        .cursor_pointer()
        .on_click(move |ev: &gpui::ClickEvent, _window, cx| {
            let Some(total) = total else { return };
            if total.as_secs_f32() <= 0.0 {
                return;
            }
            // Localize the window-relative click to the seek bar's own bounds.
            let Some(b) = bounds_for_click.get() else {
                return;
            };
            let rel = (ev.position().x - b.left()).max(px(0.));
            let fraction = (rel / b.size.width).clamp(0.0, 1.0);
            let _ = cx.update_entity(&state, |s, _cx| {
                slot.controller_mut(s).seek(fraction);
            });
        })
        .into_any_element()
}

/// View backing the Quick Translate popup window.
///
/// Holds the shared `DictState` (so it sees the engine's current `PopupStatus`),
/// focuses itself on mount (so it receives key events), and dismisses the popup
/// on Escape or a click outside the popup card.
pub struct TranslatePopupView {
    state: Entity<DictState>,
    focus: FocusHandle,
    /// Inline Options panel expanded?
    options_open: bool,
}

impl TranslatePopupView {
    pub fn new(state: Entity<DictState>, cx: &mut Context<Self>) -> Self {
        // Poll the playback controller ~10 Hz while the popup is open so the
        // seek bar tracks live position and Playing→Idle (clip finished) is
        // observed. Each tick updates the controller's state and notifies the
        // view so render() re-reads the snapshot.
        let poll_state = state.clone();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                // Update each controller's live position, then notify the view
                // so render() re-reads the snapshots and the seek bars move.
                let _ = cx.update_entity(&poll_state, |s, _cx| {
                    s.playback_source.poll_progress();
                    s.playback_translation.poll_progress();
                });
                let _ = this.update(cx, |_, cx| cx.notify());
            }
        })
        .detach();

        Self {
            state,
            focus: cx.focus_handle(),
            options_open: false,
        }
    }

    fn toggle_options(&mut self, cx: &mut Context<Self>) {
        self.options_open = !self.options_open;
        cx.notify();
    }
}

impl Focusable for TranslatePopupView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TranslatePopupView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();

        // Read popup state + full quick-translate settings + both playback
        // snapshots (source + translation are independent controllers) out of
        // DictState (cloned so the borrow ends before we pass cx).
        let (status, settings, options_open, pb_src, pb_tr) = {
            let st = self.state.read(cx);
            let engine = st.quick_translate_engine.as_ref();
            (
                engine.map(|e| e.popup_status().clone()),
                engine.map(|e| e.settings().clone()),
                self.options_open,
                st.playback_source.snapshot(),
                st.playback_translation.snapshot(),
            )
        };
        let card = match (status, settings) {
            (Some(crate::quick_translate::PopupStatus::Visible(ps)), Some(settings)) => {
                translate_popup(
                    &ps,
                    &settings,
                    &self.state,
                    options_open,
                    pb_src,
                    pb_tr,
                    cx.listener(|this, _ev, _w, cx| this.toggle_options(cx)),
                )
            }
            // No content: render an empty (invisible) root; the window will
            // be closed by the trigger logic.
            _ => div().into_any_element(),
        };

        div()
            .track_focus(&focus)
            .size_full()
            .on_key_down(cx.listener(move |this, ev: &gpui::KeyDownEvent, window, cx| {
                if ev.keystroke.key == "escape" {
                    close_popup(&this.state, window, cx);
                }
            }))
            .on_mouse_down(gpui::MouseButton::Left, {
                let state = self.state.clone();
                move |_ev, window, cx| {
                    close_popup(&state, window, cx);
                }
            })
            .child(card)
    }
}

/// Hide the popup state and close the popup window.
fn close_popup(state: &Entity<DictState>, window: &mut Window, cx: &mut gpui::App) {
    state.update(cx, |s, cx| {
        if let Some(engine) = s.quick_translate_engine.as_mut() {
            engine.hide_popup();
        }
        s.qt_popup_window = None;
        cx.notify();
    });
    window.remove_window();
}
