# GPUI 0.2.2 API Guide (Crates.io Version)

## Entry Point

```rust
use gpui::{App, Application};

fn main() {
    Application::new().run(|cx: &mut App| {
        // Initialize app here
        cx.open_window(WindowOptions { ... }, |window, cx| {
            // Create root view
        }).expect("failed to open window");
    });
}
```

Key difference from git version: **crates.io uses `Application::new()` directly**, not `gpui_platform::application()`.

## Entities (Reactive State)

### Creating an Entity

```rust
let state = cx.new(|_| MyState { /* init */ });
```

Entities are reference-counted wrappers (`Entity<T>`). They enable reactive updates and re-renders.

### Reading Entity Data

```rust
let state = self.state.read(cx);
let value = state.some_field;
```

Reading in `render()` creates automatic dependency tracking—accessing a field makes the view "subscribe" to that field.

### Updating Entity Data

```rust
cx.update_entity(&state, |s, cx| {
    s.some_field = new_value;
    cx.notify();  // Trigger re-render
}).ok();
```

The closure captures mutable access to the entity. Call `cx.notify()` to trigger dependent views to re-render.

### Observing Changes

```rust
cx.observe(&state, |_, _, cx| cx.notify()).detach();
```

Runs a callback whenever the entity changes. Use `.detach()` to drop the subscription handle (keeps it alive for the view's lifetime).

### Weak References for Async

```rust
let state_weak = self.state.downgrade();

cx.spawn(async move |cx| {
    // Do async work
    if let Some(state) = state_weak.upgrade() {
        cx.update_entity(&state, |s, cx| {
            // Update state
            cx.notify();
        }).ok();
    }
}).detach();
```

`.downgrade()` creates a `WeakEntity<T>`. Use `.upgrade()` inside the async context to safely upgrade back to `Entity<T>`. Returns `Option<Entity<T>>` (not `Result`).

## Rendering (Render Trait)

### Basic Render Implementation

```rust
impl Render for MyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .bg(color_bg())
            .child("Hello")
    }
}
```

Return type must implement `IntoElement`. Typically use the builder methods below.

### Helper Element Types

- `div()` — container, flexbox
- `h_flex()` — horizontal flexbox (from gpui_component)
- `v_flex()` — vertical flexbox (from gpui_component)

All return builders with chainable methods.

## Styling

### Layout Methods

```rust
.w_full()        // width: 100%
.h_px()          // height: 1px
.size_full()     // width: 100%, height: 100%
.gap_1()         // gap: 0.25rem (1 unit)
.gap_2()         // gap: 0.5rem
.gap_3()         // gap: 0.75rem
.gap_4()         // gap: 1rem
.px_4()          // padding-left/right: 1rem
.py_3()          // padding-top/bottom: 0.75rem
.px_5()          // padding-left/right: 1.25rem
.py_4()          // padding-top/bottom: 1rem
```

Gap/padding units are fractions of `rem` (1 unit = 0.25rem).

### Flexbox Methods

```rust
.flex_1()              // flex: 1
.items_center()        // align-items: center
.justify_center()      // justify-content: center
.justify_between()     // justify-content: space-between
```

### Border Methods

```rust
.border_b_1()          // border-bottom: 1px
.border_t_1()          // border-top: 1px
.border_color(color)   // border color
.rounded(px(4.))       // border-radius: 4px
```

### Color Methods

```rust
.bg(color)             // background-color
.text_color(color)     // color (text)
```

### Typography Methods

```rust
.text_xl()             // font-size: large
.text_lg()             // font-size: large
.text_sm()             // font-size: small
.text_xs()             // font-size: x-small
.font_weight(FontWeight::BOLD)  // font-weight (not .font_bold())
```

`FontWeight` enum: `NORMAL`, `BOLD`, `SEMIBOLD`, etc. (import from `gpui`).

### Conditional Rendering

```rust
.when_some(opt_value, |el, value| {
    el.child(
        div().child("value exists")
    )
})

.when(condition, |el| {
    el.child("shown when true")
})
```

These are from the `prelude::FluentBuilder` trait.

## Colors

Colors are `gpui::Hsla` (Hue, Saturation, Lightness, Alpha):

```rust
use gpui::Hsla;

fn color_amber() -> Hsla {
    gpui::hsla(38. / 360., 0.92, 0.50, 1.)  // Pure amber
}

fn color_bg() -> Hsla {
    gpui::hsla(222. / 360., 0.47, 0.07, 1.)  // Deep slate
}
```

Or use `px()` for pixel values:

```rust
.rounded(px(4.))   // 4 pixels
```

## Size and Position

```rust
use gpui::{size, px};

WindowBounds::Windowed(gpui::Bounds::centered(
    None,
    size(px(480.), px(580.)),  // width, height
    cx,
))
```

## Window Configuration

```rust
use gpui::{WindowOptions, TitlebarOptions};

cx.open_window(
    WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(480.), px(580.)),
            cx,
        ))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(SharedString::from("App Title")),
            appears_transparent: false,
            ..Default::default()
        }),
        ..Default::default()
    },
    |window, cx| {
        // Create root view
    },
)
.expect("failed to open window");
```

## Background Async Tasks

### Basic Task

```rust
cx.spawn(async move |cx| {
    // Do async work (can include .await)
    
    // Update UI
    if let Some(state) = state_weak.upgrade() {
        cx.update_entity(&state, |s, cx| {
            s.field = new_value;
            cx.notify();
        }).ok();
    }
})
.detach();  // Drop handle, keep task alive
```

### Background Executor (Non-blocking)

```rust
cx.background_executor()
    .spawn(async move {
        // Runs on background thread
        let result = blocking_io_operation().await;
        result
    })
    .await
```

### Timer

```rust
cx.background_executor()
    .timer(Duration::from_secs(1))
    .await
```

Blocks in the spawned task, not the UI thread.

## Converting Between Types

```rust
.into_any_element()  // Convert specific element to AnyElement
```

Required when returning different element types from branches (e.g., in match statements).

## SharedString for Dynamic Text

```rust
use gpui::SharedString;

div().child(SharedString::from("text"))
```

Use `SharedString` for dynamic values in elements (text, attributes). It's ref-counted and cheap to clone.

## Key Imports for GPUI 0.2.2

```rust
use gpui::{
    App, AppContext as _, Application, Context, Entity, FontWeight, 
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, 
    SharedString, StatefulInteractiveElement, Styled, Window, 
    WindowOptions, div, prelude::FluentBuilder as _, px, size,
};
```

- `AppContext as _` imports context trait (provides methods like `notify()`, `update_entity()`)
- `InteractiveElement` adds `.on_click()`, `.on_key_down()` and other event handlers
- `StatefulInteractiveElement` adds `.id()` (required before attaching event handlers)
- `ParentElement` trait adds `.child()` method
- `Styled` trait adds styling methods
- `FluentBuilder as _` imports conditional rendering methods (`.when_some()`, `.when()`)
- `KeyDownEvent` provides keyboard input via `.keystroke.key` and `.keystroke.modifiers`

## No-Go Methods (Not in Crates.io 0.2.2)

- `.font_bold()` — use `.font_weight(FontWeight::BOLD)` instead
- `.tracking_widest()` — no letter-spacing in 0.2.2
- `.tracking_wide()` — no letter-spacing in 0.2.2
- `gpui_platform::application()` — use `Application::new()` instead
- `.when_some()` without `FluentBuilder` import — must import from prelude

## cx.spawn Signatures

The `cx.spawn()` signature differs depending on context:

**On `Context<T>` (inside `Render` or view methods):**
```rust
// Takes 2 arguments: WeakEntity<Self> and &mut AsyncApp
cx.spawn(async move |_this: WeakEntity<Self>, cx: &mut AsyncApp| {
    // async work
})
.detach();
```

**Inside `on_click` callbacks (from `InteractiveElement`):**
```rust
// Takes 1 argument: &mut AsyncApp
.on_click(move |_, _, cx| {
    cx.spawn(async move |cx: &mut AsyncApp| {
        // async work
    })
    .detach();
})
```

**Important:** The project uses the git version of gpui where `AsyncApp::update_entity()` returns `R` directly (not `Result<R>`). Do NOT chain `.ok()` on `update_entity` calls.

## Keyboard Events

### Global key handler on root element

```rust
use gpui::KeyDownEvent;

div()
    .size_full()
    .on_key_down(cx.listener(move |_this, event: &KeyDownEvent, window, cx| {
        let modifiers = &event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();

        if modifiers.control && key == "l" {
            // Ctrl+L action
        } else if key == "escape" {
            // Escape action
        }
    }))
    .child(content)
```

Key points:
- Attach `.on_key_down()` to the root `div()` to capture all keyboard events
- Use `cx.listener()` to create the callback from within `Render::render()`
- `event.keystroke.modifiers` has `.control`, `.alt`, `.shift`, `.platform` booleans
- `event.keystroke.key` is a `SharedString`; use `.as_str()` for comparison
- The listener receives `window: &mut Window` and `cx: &mut Context<Self>`

### Keystroke modifier fields

```rust
struct Modifiers {
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,  // Command on macOS, Super on Linux
}
```
