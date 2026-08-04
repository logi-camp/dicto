//! Audio playback controller for TTS clips.
//!
//! Owns a persistent `rodio` output stream and the current clip's `Sink`, so the
//! UI can pause/resume, seek, and replay without re-synthesizing. Playback state
//! (`Idle`/`Loading`/`Playing`/`Paused`/`Error`) is observable by the popup so the
//! Speak button shows a spinner while fetching, and the controls (play/pause,
//! replay, seek bar) reflect the live position.
//!
//! The controller is held by `DictState`; the popup polls `progress()` on a
//! timer to drive the seek bar.

use std::io::Cursor;
use std::sync::Mutex;
use std::time::Duration;

use mdict_rs::settings::TtsSettings;
use rodio::Source;

/// Observable playback state for one clip.
#[derive(Clone, Debug)]
pub enum PlaybackState {
    /// Nothing loaded yet.
    Idle,
    /// Synthesizing / decoding audio (HTTP fetch for AI TTS, or a platform
    /// subprocess). The Speak button renders a spinner in this state.
    Loading,
    /// Audio is playing. `pos` is the playback position in seconds.
    Playing { pos: f32 },
    /// Paused; `pos` is where playback will resume.
    Paused { pos: f32 },
    /// Clip finished naturally. The audio buffer is still loaded — the user
    /// can replay (seek to 0 + play) or seek within it. We keep the controls
    /// visible (seek bar full, ▶ + ↺) instead of dropping back to Idle.
    Ended { pos: f32 },
    /// Fetch or decode failed.
    Error(String),
}

impl Default for PlaybackState {
    fn default() -> Self {
        PlaybackState::Idle
    }
}

impl PlaybackState {
    pub fn is_loading(&self) -> bool {
        matches!(self, PlaybackState::Loading)
    }
}

/// Decoded clip held alongside the live `Sink` so we can replay/seek without
/// re-fetching. `total` is the clip's full duration (for the seek bar's scale).
///
/// The raw `bytes` are kept so the clip can be replayed after it finishes: a
/// rodio `Sink` consumes its source on play, and once the queue drains the
/// source is dropped — `try_seek(0)` on an exhausted sink is a no-op. So
/// replay/seek-after-end re-decodes from `bytes` and appends a fresh source.
struct Clip {
    sink: rodio::Sink,
    total: Option<Duration>,
    bytes: Vec<u8>,
}

/// The audio backend. Lazily created on first `load` so apps that never use
/// TTS don't open an audio device.
struct Backend {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
}

/// Playback controller. The `Sink`/`Backend` are behind a `Mutex` because the
/// controller is stored in `DictState` (which GPUI accesses from `&self`) and
/// the audio thread + background tasks mutate them.
pub struct PlaybackController {
    /// The rodio output stream + handle. `None` until the first clip loads.
    backend: Mutex<Option<Backend>>,
    /// The currently loaded clip's sink + duration. `None` when idle/error.
    clip: Mutex<Option<Clip>>,
    /// Observable state, read by the popup every render.
    state: Mutex<PlaybackState>,
    /// The text the current clip was synthesized from, so we know whether a
    /// new `load` request is a re-play of the same clip (skip synthesis) or a
    /// new one.
    current_text: Mutex<String>,
}

impl Default for PlaybackController {
    fn default() -> Self {
        Self {
            backend: Mutex::new(None),
            clip: Mutex::new(None),
            state: Mutex::new(PlaybackState::Idle),
            current_text: Mutex::new(String::new()),
        }
    }
}

/// What `PlaybackController::start_load` decided to do — lets the caller spawn
/// the synthesis on the right executor without the controller needing a GPUI
/// context handle.
pub enum LoadAction {
    /// Same clip already loaded; we replayed synchronously. Nothing to spawn.
    Replay,
    /// New clip; caller should spawn a background task that synthesizes the
    /// bytes and calls `controller.install_from_bytes(bytes)`.
    Synthesize {
        text: String,
        lang: Option<String>,
        tts: Option<TtsSettings>,
    },
}

impl PlaybackController {
    /// Begin loading `text`. Sets Loading state (or replays if the same text is
    /// already loaded). Returns the action the caller should perform — either
    /// nothing (`Replay`) or spawn synthesis (`Synthesize`), whose task calls
    /// `install_from_bytes` with the result.
    pub fn start_load(
        &self,
        text: String,
        lang: Option<String>,
        tts: Option<TtsSettings>,
    ) -> LoadAction {
        // Fast path: same text already loaded → just replay.
        if *self.current_text.lock().unwrap() == text {
            self.replay();
            return LoadAction::Replay;
        }

        // Slow path: mark Loading, clear any prior clip, hand synthesis to caller.
        self.set_state(PlaybackState::Loading);
        self.clear_clip();
        LoadAction::Synthesize { text, lang, tts }
    }

    /// Called by the background synthesis task with the synthesized bytes.
    /// Decodes + appends to a fresh sink, computes total duration, sets state
    /// to Playing (or Error).
    pub fn install_from_bytes(&self, text: String, bytes: Vec<u8>) {
        *self.current_text.lock().unwrap() = text;
        match self.install_clip(bytes) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = %e, "tts: playback install failed");
                self.set_state(PlaybackState::Error(e.to_string()));
            }
        }
    }

    /// Synthesis failed before producing bytes.
    pub fn fail(&self, error: String) {
        self.set_state(PlaybackState::Error(error));
    }

    /// Decode `bytes` and append to a fresh sink. Computes total duration for
    /// the seek bar. Sets state to Playing on success.
    fn install_clip(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
        // Lazily create the audio backend on first use.
        let mut backend_guard = self.backend.lock().unwrap();
        if backend_guard.is_none() {
            let (stream, handle) = rodio::OutputStream::try_default()
                .map_err(|e| anyhow::anyhow!("no audio device: {e}"))?;
            *backend_guard = Some(Backend { _stream: stream, handle });
        }
        let handle = backend_guard
            .as_ref()
            .expect("backend just initialized")
            .handle
            .clone();
        drop(backend_guard);

        let sink = rodio::Sink::try_new(&handle)
            .map_err(|e| anyhow::anyhow!("sink: {e}"))?;

        // Compute total duration. The decoder's `total_duration()` is unreliable
        // for MP3 (symphonia often returns None for streams without explicit
        // timecodes), so as a fallback we fully decode once and compute from
        // sample count. For short TTS clips (a few seconds) this is cheap.
        let total = match compute_total_duration(&bytes) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!(error = %e, "tts: duration unknown, seek bar will be position-only");
                None
            }
        };
        tracing::info!(
            bytes = bytes.len(),
            total_secs = ?total.map(|d| d.as_secs_f32()),
            "tts: clip decoded, installing sink"
        );

        // Keep the raw bytes so replay/seek-after-end can re-decode (a rodio
        // Sink consumes + drops its source once the queue drains, so seeking an
        // exhausted sink is a no-op — we must append a fresh decoder).
        let stored_bytes = bytes.clone();
        let decoder = rodio::Decoder::new(Cursor::new(bytes))
            .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
        sink.append(decoder);

        *self.clip.lock().unwrap() = Some(Clip {
            sink,
            total,
            bytes: stored_bytes,
        });
        self.set_state(PlaybackState::Playing { pos: 0.0 });
        Ok(())
    }

    /// Stop the sink and append a freshly-decoded source from the clip's saved
    /// bytes, then play. Used by replay and play-from-ended, where the prior
    /// source has been consumed and can't be seeked.
    fn restart_clip(clip: &Clip) -> anyhow::Result<()> {
        clip.sink.stop();
        let decoder = rodio::Decoder::new(Cursor::new(clip.bytes.clone()))
            .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
        clip.sink.append(decoder);
        clip.sink.play();
        Ok(())
    }

    /// Toggle between Playing and Paused. From Ended → restart the clip from
    /// the start (the prior source was consumed). From Paused → resume.
    /// No-op if no clip or Loading/Error.
    ///
    /// Lock order: clip before state (consistent with poll_progress/seek) to
    /// avoid AB-BA deadlock with the poll timer.
    pub fn toggle_pause(&self) {
        let clip_guard = self.clip.lock().unwrap();
        let Some(clip) = clip_guard.as_ref() else {
            return;
        };
        let mut state = self.state.lock().unwrap();
        match &*state {
            PlaybackState::Playing { .. } => {
                clip.sink.pause();
                let pos = clip.sink.get_pos().as_secs_f32();
                *state = PlaybackState::Paused { pos };
            }
            PlaybackState::Paused { .. } => {
                // If the source was consumed (paused at end), restart it.
                if clip.sink.empty() {
                    let _ = Self::restart_clip(clip);
                } else {
                    clip.sink.play();
                }
                *state = PlaybackState::Playing { pos: 0.0 };
            }
            PlaybackState::Ended { .. } => {
                // User pressed play on a finished clip → restart from bytes.
                let _ = Self::restart_clip(clip);
                *state = PlaybackState::Playing { pos: 0.0 };
            }
            _ => {}
        }
    }

    /// Seek back to the start and play. Replays the loaded clip without
    /// re-synthesizing — re-decodes from the saved bytes if the source was
    /// already consumed.
    pub fn replay(&self) {
        let clip_guard = self.clip.lock().unwrap();
        if let Some(clip) = clip_guard.as_ref() {
            if clip.sink.empty() {
                let _ = Self::restart_clip(clip);
            } else {
                let _ = clip.sink.try_seek(Duration::ZERO);
                clip.sink.play();
            }
            drop(clip_guard);
            self.set_state(PlaybackState::Playing { pos: 0.0 });
        }
    }

    /// Seek to a fraction (0.0..=1.0) of the clip's total duration. If the
    /// source was consumed, restart from bytes then seek to the target.
    pub fn seek(&self, fraction: f32) {
        let clip_guard = self.clip.lock().unwrap();
        if let Some(clip) = clip_guard.as_ref() {
            if let Some(total) = clip.total {
                let target = total.mul_f32(fraction.clamp(0.0, 1.0));
                let seek_result = if clip.sink.empty() {
                    let _ = Self::restart_clip(clip);
                    clip.sink.try_seek(target)
                } else {
                    clip.sink.try_seek(target)
                };
                if let Err(e) = &seek_result {
                    tracing::warn!(error = ?e, fraction, target_secs = target.as_secs_f32(), "tts: try_seek failed");
                }
                // After a successful seek, ensure the sink is playing (a seek
                // on a paused sink leaves it paused, which is fine — but if the
                // clip was Ended we restarted and should be playing).
                let pos = clip.sink.get_pos().as_secs_f32();
                drop(clip_guard);
                let playing = matches!(*self.state.lock().unwrap(), PlaybackState::Playing { .. });
                self.set_state(if playing {
                    PlaybackState::Playing { pos }
                } else {
                    PlaybackState::Paused { pos }
                });
            }
        }
    }

    /// Stop and clear the current clip (e.g. when the popup closes).
    pub fn stop(&self) {
        self.clear_clip();
        self.set_state(PlaybackState::Idle);
    }

    /// Drop the current clip's sink (stops playback).
    fn clear_clip(&self) {
        if let Some(clip) = self.clip.lock().unwrap().take() {
            clip.sink.stop();
        }
        *self.current_text.lock().unwrap() = String::new();
    }

    /// Poll the live playback position. Call this on a timer (~10 Hz) while the
    /// popup is open to drive the seek bar. Transitions Playing→Ended when the
    /// clip finishes naturally (the clip buffer is preserved so Replay works).
    ///
    /// Note: `Sink::empty()` returns `true` for a brief moment right after
    /// `append()` and after `try_seek(0)` while the source buffer refills —
    /// so we only treat `empty` as "ended" if we've actually advanced past a
    /// small startup threshold. This prevents the play button from briefly
    /// flipping to "ended" right after Replay/Play.
    pub fn poll_progress(&self) {
        let (pos, empty, total) = {
            let clip_guard = self.clip.lock().unwrap();
            let Some(clip) = clip_guard.as_ref() else {
                return;
            };
            (
                clip.sink.get_pos().as_secs_f32(),
                clip.sink.empty(),
                clip.total,
            )
        };

        let mut state = self.state.lock().unwrap();
        // Only demote Playing → Ended once playback has actually advanced past
        // ~150 ms. Below that, `empty` is the start-up / re-seek gap and the
        // sink is just priming.
        let advanced = total
            .map(|t| pos > 0.15)
            .unwrap_or(pos > 0.15);
        if empty && advanced && matches!(*state, PlaybackState::Playing { .. }) {
            let end = total.map(|d| d.as_secs_f32()).unwrap_or(pos);
            *state = PlaybackState::Ended { pos: end };
            return;
        }
        match &*state {
            PlaybackState::Playing { .. } => {
                *state = PlaybackState::Playing { pos };
            }
            PlaybackState::Paused { .. } => {
                *state = PlaybackState::Paused { pos };
            }
            PlaybackState::Ended { .. } => {
                *state = PlaybackState::Ended { pos };
            }
            _ => {}
        }
    }

    /// Snapshot the current observable state + total duration (for the seek
    /// bar's scale). Read by the popup each render.
    pub fn snapshot(&self) -> (PlaybackState, Option<Duration>) {
        let state = self.state.lock().unwrap().clone();
        let total = self
            .clip
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|c| c.total);
        (state, total)
    }

    fn set_state(&self, s: PlaybackState) {
        *self.state.lock().unwrap() = s;
    }
}

/// Compute a clip's total duration from its encoded bytes.
///
/// Tries the decoder's `total_duration()` first (cheap, accurate when the
/// container/embedded metadata provides it). Falls back to fully decoding the
/// clip once and dividing sample count by sample rate — reliable for MP3 where
/// `total_duration()` returns `None`. For short TTS clips the full decode is
/// negligible.
fn compute_total_duration(bytes: &[u8]) -> anyhow::Result<std::time::Duration> {
    // Fast path: trust the decoder's duration if it has one.
    let decoder = rodio::Decoder::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
    if let Some(d) = decoder.total_duration() {
        return Ok(d);
    }

    // Fallback: decode fully, counting samples. sample_rate + channels let us
    // convert the sample count into seconds.
    let mut decoder = rodio::Decoder::new(Cursor::new(bytes.to_vec()))
        .map_err(|e| anyhow::anyhow!("decode: {e}"))?;
    let rate = decoder.sample_rate();
    let channels = decoder.channels() as u64;
    if rate == 0 || channels == 0 {
        anyhow::bail!("decode: zero sample_rate/channels");
    }
    let mut samples: u64 = 0;
    while decoder.next().is_some() {
        samples += 1;
    }
    let frames = samples / channels;
    let secs = frames as f64 / rate as f64;
    Ok(std::time::Duration::from_secs_f64(secs))
}
