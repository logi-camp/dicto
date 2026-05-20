mod app;
mod audio;
mod colors;
mod components;
mod html;
mod indexing;
mod state;

use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, WindowBounds, WindowDecorations, WindowOptions, px, size,
};
use gpui_component::{Root, Theme, ThemeMode};
use gpui_platform::application;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use tray_icon::{
    Icon, TrayIconBuilder,
    menu::{IsMenuItem, Menu, MenuEvent, MenuId, MenuItem},
};

use crate::app::DictApp;
use crate::state::DictState;

fn main() {
    #[cfg(target_os = "linux")]
    gtk::init().expect("failed to init GTK");

    // symphonia (rodio's underlying demuxer) prints a WARN for every
    // byte it can't make sense of when handed a non-mp3 stream — for
    // Speex clips that's hundreds of lines per click. Silence its
    // crates here; the audio module logs a single line on failure.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,symphonia_bundle_mp3=error,symphonia_core=error,symphonia_format_ogg=error",
        )
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load any indexes that already exist so the UI is usable immediately
    // for cached dictionaries. New/unindexed dicts are built in the background
    // (see `indexing::spawn`) after the window opens.
    mdict_rs::registry::reload();
    indexing::load_stylesheets();

    let app = application();
    app.with_assets(gpui_component_assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

            setup_tray(cx);
            open_dictionary_window(cx);

            cx.activate(true);
        });
}

fn setup_tray(cx: &mut App) {
    let show_item = MenuItem::with_id(MenuId::new("show"), "Show Dictionary", true, None);
    let quit_item = MenuItem::with_id(MenuId::new("quit"), "Quit", true, None);

    let menu = Menu::new();
    menu.append_items(&[&show_item as &dyn IsMenuItem, &quit_item as &dyn IsMenuItem])
        .unwrap();

    let icon = tray_pixel_icon();
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Dicto")
        .with_icon(icon)
        .build()
        .unwrap();

    cx.spawn(async move |cx| {
        loop {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                match event.id.as_ref() {
                    "show" => {
                        cx.update(|cx| {
                            open_dictionary_window(cx);
                        });
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }

            #[cfg(target_os = "linux")]
            {
                while gtk::events_pending() {
                    gtk::main_iteration_do(false);
                }
            }

            cx.background_executor()
                .timer(Duration::from_millis(50))
                .await;
        }
    })
    .detach();
}

fn open_dictionary_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(920.), px(680.)), cx);

    let state_for_indexing: std::cell::RefCell<Option<gpui::Entity<DictState>>> =
        std::cell::RefCell::new(None);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_decorations: Some(WindowDecorations::Client),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Dicto".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            window_min_size: Some(size(px(600.), px(400.))),
            is_resizable: true,
            app_id: Some("dicto".into()),
            ..Default::default()
        },
        |window, cx| {
            let state = cx.new(|_cx| DictState::new());
            *state_for_indexing.borrow_mut() = Some(state.clone());
            let view = cx.new(|cx| DictApp::new(state, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .expect("failed to open window");

    if let Some(state) = state_for_indexing.into_inner() {
        indexing::spawn(state, cx);
    }
}

fn tray_pixel_icon() -> Icon {
    let sz: u32 = 64;
    let mut rgba = vec![0u8; (sz * sz * 4) as usize];

    let cx_f = sz as f32 / 2.0;
    let cy_f = sz as f32 / 2.0;
    let r = sz as f32 * 0.38;

    for y in 0..sz {
        for x in 0..sz {
            let dx = x as f32 - cx_f;
            let dy = y as f32 - cy_f;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * sz + x) * 4) as usize;

            if dist <= r {
                let edge = r * 0.9;
                if dist > edge {
                    let alpha = 1.0 - (dist - edge) / (r - edge);
                    rgba[idx] = 122;
                    rgba[idx + 1] = 162;
                    rgba[idx + 2] = 247;
                    rgba[idx + 3] = (alpha * 255.0) as u8;
                } else {
                    rgba[idx] = 122;
                    rgba[idx + 1] = 162;
                    rgba[idx + 2] = 247;
                    rgba[idx + 3] = 255;
                }
            } else {
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, sz, sz).unwrap()
}
