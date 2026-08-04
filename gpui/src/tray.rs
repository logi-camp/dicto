//! System tray via the StatusNotifierItem (SNI) protocol.
//!
//! Uses our forked `ksni` (github.com/mohamadkhani/ksni) which adds the
//! `ProvideXdgActivationToken` SNI method. Unlike the old `tray-icon` +
//! `libayatana-appindicator` + nested-GTK-pump stack, ksni speaks the SNI
//! D-Bus protocol directly (no GTK dependency), which is what actually works
//! on GNOME/Wayland and KDE.
//!
//! Tray actions are sent over an `mpsc` channel; the GPUI main loop polls it
//! (see `main.rs`). The "Quick Translate" item just sets the same
//! `TRAY_TRANSLATE_TRIGGERED` flag the `dicto --translate` IPC path uses, so
//! both share one trigger path handled in `app.rs`.

use std::sync::{mpsc, Arc, Mutex};

use ksni::{
    menu::{MenuItem, StandardItem},
    Category, Icon, ToolTip, Tray, TrayMethods,
};

/// The last compositor-provided activation token, shared between the SNI
/// service thread (which receives it) and the menu-item click handlers.
///
/// Stashed even though Dicto's current GPUI lacks `Window::activate_with_token`
/// — once the GPUI fork bump lands, the token can be forwarded to the window
/// for authoritative focus on GNOME/Wayland.
pub type SharedToken = Arc<Mutex<Option<String>>>;

/// Actions the tray requests the GPUI main loop to perform.
#[derive(Debug)]
pub enum TrayAction {
    Show,
    QuickTranslate,
    Quit,
}

struct DictoTray {
    action_tx: mpsc::Sender<TrayAction>,
    #[allow(dead_code)]
    token: SharedToken,
}

impl DictoTray {
    fn new(action_tx: mpsc::Sender<TrayAction>, token: SharedToken) -> Self {
        Self { action_tx, token }
    }
}

impl Tray for DictoTray {
    fn id(&self) -> String {
        "dicto".into()
    }
    fn title(&self) -> String {
        "Dicto".into()
    }
    fn category(&self) -> Category {
        Category::ApplicationStatus
    }
    fn icon_name(&self) -> String {
        // Deliberately empty: GNOME's AppIndicator extension prefers
        // `IconName` over `IconPixmap` whenever the name resolves in the icon
        // theme, which would shadow our pixel icon. Returning "" forces it to
        // render our `icon_pixmap`.
        String::new()
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![dicto_icon()]
    }
    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Dicto".into(),
            description: "Dictionary & quick translate".into(),
            ..Default::default()
        }
    }

    fn on_activation_token(&mut self, token: String) {
        tracing::debug!(
            chars = token.len(),
            "tray: ProvideXdgActivationToken received"
        );
        if let Ok(mut g) = self.token.lock() {
            *g = Some(token);
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let tx_show = self.action_tx.clone();
        let tx_translate = self.action_tx.clone();
        let tx_quit = self.action_tx.clone();

        vec![
            MenuItem::Standard(StandardItem {
                label: "Show Dictionary".into(),
                enabled: true,
                activate: Box::new(move |_this| {
                    let _ = tx_show.send(TrayAction::Show);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "Quick Translate".into(),
                enabled: true,
                activate: Box::new(move |_this| {
                    let _ = tx_translate.send(TrayAction::QuickTranslate);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".into(),
                enabled: true,
                activate: Box::new(move |_this| {
                    let _ = tx_quit.send(TrayAction::Quit);
                }),
                ..Default::default()
            }),
        ]
    }
}

/// Spawn the ksni tray on a dedicated thread with its own current-thread
/// tokio runtime. Returns the channel the GPUI main loop polls for actions
/// and the shared activation-token slot.
pub fn spawn_tray() -> (mpsc::Receiver<TrayAction>, SharedToken) {
    let (action_tx, action_rx) = mpsc::channel::<TrayAction>();
    let token: SharedToken = Arc::new(Mutex::new(None));

    let tray = DictoTray::new(action_tx, token.clone());

    std::thread::Builder::new()
        .name("ksni-tray".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::warn!("tray: failed to build tokio runtime: {e}");
                    return;
                }
            };

            runtime.block_on(async move {
                match tray.spawn().await {
                    Ok(_handle) => {
                        tracing::info!("tray: ksni service spawned");
                        // Hold the runtime alive for the lifetime of the
                        // process. The handle owns the background D-Bus task;
                        // dropping it would tear down the tray.
                        std::future::pending::<()>().await;
                    }
                    Err(e) => {
                        tracing::warn!("tray: ksni spawn failed: {e}");
                    }
                }
            });
        })
        .expect("spawn ksni thread");

    (action_rx, token)
}

/// Render the Dicto mark — a filled blue circle with soft anti-aliased edges —
/// as ARGB32 (network byte order), the format SNI `IconPixmap` expects.
fn dicto_icon() -> Icon {
    const SZ: u32 = 64;
    let cx_f = SZ as f32 / 2.0;
    let cy_f = SZ as f32 / 2.0;
    let r = SZ as f32 * 0.38;

    // Build RGBA first (the natural pixel layout).
    let mut rgba = vec![0u8; (SZ * SZ * 4) as usize];
    for y in 0..SZ {
        for x in 0..SZ {
            let dx = x as f32 - cx_f;
            let dy = y as f32 - cy_f;
            let dist = (dx * dx + dy * dy).sqrt();
            let idx = ((y * SZ + x) * 4) as usize;

            if dist <= r {
                let edge = r * 0.9;
                let alpha = if dist > edge {
                    1.0 - (dist - edge) / (r - edge)
                } else {
                    1.0
                };
                rgba[idx] = 122; // R
                rgba[idx + 1] = 162; // G
                rgba[idx + 2] = 247; // B
                rgba[idx + 3] = (alpha * 255.0) as u8; // A
            }
            // else: leave at 0 (transparent)
        }
    }

    // Convert RGBA → ARGB32 by rotating each pixel right by one byte
    // (R,G,B,A → A,R,G,B).
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.rotate_right(1);
    }

    Icon {
        width: SZ as i32,
        height: SZ as i32,
        data: rgba,
    }
}
