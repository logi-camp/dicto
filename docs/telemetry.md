# Telemetry

Dicto ships privacy-first, **opt-in** anonymous analytics built on the
[Aptabase](https://aptabase.com) HTTP API. The whole system lives in the
[`telemetry`](../telemetry/src/) workspace crate; the GPUI app only calls
`telemetry::init(...)` once at startup and `telemetry::get().track(event)`
at well-defined fire points.

This doc is the source of truth for **what we collect, what we never
collect, and where each event fires**. When you add or change an event,
update the table here and the doc comment on the variant in
[`event.rs`](../telemetry/src/event.rs).

## Privacy model (read this first)

| Principle | How it's enforced |
|-----------|-------------------|
| **Opt-in by default.** A fresh install is `Undecided`; nothing is sent until the user clicks *Allow* in the consent dialog. | `Settings.telemetry_consent` is a three-state enum (`Undecided`/`OptedIn`/`OptedOut`) persisted in `settings.toml`. `telemetry::init(enabled, ...)` installs `NullTelemetry` when `enabled` is false — tracking is then a zero-cost no-op with no HTTP. |
| **No persistent user identifier.** Events carry an anonymous *installation id* (a local UUID v4), not a person. | Generated once on first run, stored in `settings.toml`. Sent as a custom property; identifies an installation, not a user. |
| **No session cross-linking across launches.** A session id is minted at `init` and lives only for that process. | Generated inside `AptabaseClient::new`; never persisted. |
| **No lookup content.** `LookupPerformed` carries a *source* (auto-preview / click / keyboard), never the word, never the dictionary. | By construction — the `Event::LookupPerformed` variant has no text field. |
| **No filesystem layout leakage.** Error messages are truncated to 180 chars and absolute path prefixes (`/home/...`, `C:\...`) are stripped before sending. | `sanitize_message` in [`event.rs`](../telemetry/src/event.rs); covered by unit tests. |
| **No dictionary identity.** `DictionaryImported` carries a *count* and a *rounded size in MB*, never a dictionary name or path. | Size is rounded to the nearest MB — a deliberate smoothing so precise size (a weak proxy for *which* dictionary) can't double as a fingerprint. |
| **Failed sends are dropped.** Analytics is best-effort; the crate is a fire-and-forget sink much like `tracing`. | `AptabaseClient` batches on a 30 s timeout / 20-event cap and warn-logs on send failure. |

### The consent flow

```
first run ─► settings.telemetry_consent == Undecided
                │
                ▼
   ┌────────────────────────────────┐
   │  consent dialog auto-opens     │   ← dedicated modal, sized + titled
   │  (visible, not a sliver)       │     so it actually renders
   └────────────────────────────────┘
        │ "Allow"        │ "Not now" / close
        ▼                 ▼
   OptedIn             (stays Undecided → treated as opt-out)
        │
        ▼
   telemetry::init(true, install_id, version)
```

The user can later flip consent from the **Telemetry tab** in the settings
dialog. Consent changes do **not** fire `SettingsChanged` (consent is not
a "settings change" in the dashboard sense).

### The app key

`APTABASE_APP_KEY` in [`lib.rs`](../telemetry/src/lib.rs) is a compile-time
constant. It's the *public* Aptabase app key (Aptabase ships it in client
apps by design — it identifies the app, it is **not** an auth secret).
While the key is still a placeholder, `init` selects `NullTelemetry` so
the app runs without sending anything. Replace it with a real key from
<https://aptabase.com> before release; the region (`us`/`eu`) is derived
from the key prefix.

## Event schema

Events are a `pub enum Event` in [`event.rs`](../telemetry/src/event.rs).
Using an enum (not strings) means a typo like `"ap_started"` is a
compile error, not a silently-missed dashboard entry.

| Event | Wire name | Properties | Fires at | Notes |
|-------|-----------|------------|----------|-------|
| `AppStarted` | `app_started` | — | [`main.rs`](../gpui/src/main.rs) at startup; also reused by the dev-only "Send test event" button | One per process launch. |
| `WindowOpened` | `window_opened` | — | [`main.rs`](../gpui/src/main.rs) when the dictionary window opens | At startup or "Show" from the tray. |
| `WindowClosed` | `window_closed` | — | [`app.rs`](../gpui/src/app.rs) title-bar close | User hid/closed the window. |
| `LookupPerformed` | `lookup_performed` | `source` | [`app.rs`](../gpui/src/app.rs) (submit), [`word_list.rs`](../gpui/src/components/word_list.rs) (click) | **No word text.** `source` ∈ `auto_preview`/`click`/`keyboard`. |
| `PronunciationPlayed` | `pronunciation_played` | — | [`html/render.rs`](../gpui/src/html/render.rs) | Playback actually started (fires from the render path). |
| `PronunciationPlaybackFailed` | `pronunciation_playback_failed` | `reason` | [`audio.rs`](../gpui/src/audio.rs) (7 sites) | `reason` ∈ `no_device`/`sink_failed`/`ffmpeg_failed`/`resource_not_found`. |
| `DictionaryImported` | `dictionary_imported` | `count`, `duration_ms`, `size_mb` | [`import_panel.rs`](../gpui/src/components/import_panel.rs) end of batch | **One aggregate event per batch.** `count` = enabled dictionaries after import; `duration_ms` = whole-batch wall clock (copy + index); `size_mb` = total bytes copied, rounded to nearest MB. Never name/path. |
| `SettingsChanged` | `settings_changed` | `change` | [`settings_panel.rs`](../gpui/src/components/settings_panel.rs) Save button | `change` ∈ `dictionary_list` today. Consent toggle does **not** fire this. |
| `SettingsOpened` | `settings_opened` | `source` | [`app.rs`](../gpui/src/app.rs) | `source` ∈ `gear_button`/`tray_menu`. |
| `SettingsTabSelected` | `settings_tab_selected` | `tab` | [`settings_window.rs`](../gpui/src/components/settings_window.rs) | `tab` ∈ `dictionaries`/`import`/`download`/`telemetry`/`about`. |
| `ErrorOccurred` | `error_occurred` | `kind`, `message` | [`import_panel.rs`](../gpui/src/components/import_panel.rs) (3 sites, `kind=import`), [`indexing.rs`](../gpui/src/indexing.rs) (`kind=indexing`) | `kind` ∈ `indexing`/`import`; `message` truncated to 180 chars + path prefixes stripped. |

### Property-value enums

Each property is its own enum, so a rename surfaces at the call site
rather than silently changing a string on the dashboard:

| Enum | Values (wire strings) |
|------|----------------------|
| `LookupSource` | `auto_preview`, `click`, `keyboard` |
| `PlaybackFailureReason` | `no_device`, `sink_failed`, `ffmpeg_failed`, `resource_not_found` |
| `SettingsSource` | `gear_button`, `tray_menu` |
| `SettingsTab` | `dictionaries`, `import`, `download`, `telemetry`, `about` |
| `SettingsChange` | `dictionary_list` *(only variant today; add a variant when theme/font/shortcut saves land)* |
| `ErrorKind` | `indexing`, `import` *(narrow on purpose — only the actionable "my dictionary didn't load" failures)* |

## Architecture

```
                   call site (UI handler or bg thread)
                            │
                            ▼
              telemetry::get().track(Event::...)
                            │
                            ▼
        ┌───────────── Arc<dyn Telemetry> ─────────────┐
        │                                              │
        ▼  (consent == OptedIn && real key)            ▼  (otherwise)
  AptabaseClient                                NullTelemetry
  ├─ own tokio::runtime::Runtime                └─ track() = no-op
  ├─ batch: 30s timeout OR 20 events
  ├─ session id (per-process, not persisted)
  ├─ system props: app version, OS, install id
  └─ POST https://{us|eu}.aptabase.com/api/v0/events
            │
            ▼ (on failure: warn-log, drop)
```

### Why a trait, not the Aptabase SDK directly

- **Backend-swappable.** `NullTelemetry` is the pre-`init` and opt-out
  fallback; a future self-hosted or local-logging backend slots in
  without touching call sites.
- **Sync + infallible API.** Calling sites never `.await` and never
  handle errors: `telemetry::get().track(...)` works from a GPUI click
  handler or a background `tokio` task with identical, panic-free
  ergonomics. The `AptabaseClient` owns its own runtime and does the
  async HTTP internally.
- **Compile-time event names.** The enum means the set of events is
  auditable by reading one file; no string keys can drift out of sync
  with the dashboard.

## Adding a new event

1. Add a variant to `Event` in [`event.rs`](../telemetry/src/event.rs),
   with a doc comment stating **what fires it** and **what (if anything)
   it carries**. If it has a property, add (or extend) a value enum with
   an `as_str()` — never a free-form string.
2. Add the wire mapping (`event_name` + `props`) in
   [`aptabase.rs`](../telemetry/src/aptabase.rs) `Event::to_payload`.
3. Add the fire site in the GPUI crate. Mirror the existing pattern:
   `dicto_telemetry::get().track(dicto_telemetry::Event::Foo { .. });`
4. **Update the table in this doc** and the doc comment together.
5. If the event carries user-derived text, route it through
   `sanitize_message` (or add a test proving it can't leak).
6. `cargo test -p dicto-telemetry` — the 12 existing tests cover
   sanitization, session-id freshness, and payload shape.

## What we deliberately do NOT collect

- **The word the user looked up** — not the lookup text, not the
  dictionary that answered it.
- **Dictionary names or file paths** — `DictionaryImported` is count +
  rounded size only.
- **Absolute filesystem paths** — stripped from `ErrorOccurred.message`.
- **Any value the user typed into a settings field** — settings events
  carry only the *category* of change (`dictionary_list`), never values.
- **Persistent cross-session identity** — session ids are per-process.
- **Anything before opt-in** — `NullTelemetry` until consent is `OptedIn`.
