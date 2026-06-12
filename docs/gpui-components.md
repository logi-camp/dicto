# gpui-component 0.5.1 API Guide (Crates.io)

Library: https://crates.io/crates/gpui-component

> **Don't reinvent the wheel.** This library already provides labels, inputs, buttons, tables, dialogs,
> tabs, and layout containers. Always check here (and the Export Map below) before reaching for `div()`.
> Raw `div()` is only appropriate when no library component fits the need.

## Initialization

Must call before using components:

```rust
fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);  // Initialize theme and resources
        
        // Now safe to use components
    });
}
```

## Theme

### Switching Theme

```rust
use gpui_component::{Theme, ThemeMode};

Theme::change(ThemeMode::Dark, None, cx);  // Dark mode
Theme::change(ThemeMode::Light, None, cx); // Light mode
```

- `ThemeMode::Dark` — dark background, light text
- `ThemeMode::Light` — light background, dark text
- Second arg `None` uses default theme; can pass custom theme struct

### Colors in Dark Mode

Default dark theme colors (approximate):
- Background: dark slate/navy
- Text: light gray/white
- Borders: medium gray
- Accents: blue, green, red

Theme colors are accessible to components automatically.

## Button Component

### Basic Button

```rust
use gpui_component::button::{Button, ButtonVariants as _};

Button::new("unique-id")
    .label("Click me")
    .on_click(|_, _, cx| {
        // Handle click
    })
```

Must provide unique string ID (e.g., `"deny-btn"`, `"allow-btn"`).

### Button Variants (Methods)

```rust
.label("Text")            // Button text
.on_click(callback)       // Click handler signature: |_, _, cx| { }
.success()                // Green button (Allow-like)
.danger()                 // Red button (Deny-like)
.warning()                // Yellow button
.w_full()                 // Full width
```

The `ButtonVariants as _` trait import adds `.success()`, `.danger()`, `.warning()` methods.

### Button Layout

```rust
Button::new("id")
    .label("Click")
    .w_full()  // Full width within parent flex container
```

Buttons inherit flexbox properties from parent container.

## Checkbox Component

### Basic Checkbox

```rust
use gpui_component::checkbox::Checkbox;

Checkbox::new("unique-id")
    .label("Option label")
    .checked(is_checked)
    .on_click({
        move |checked, _, cx| {
            // checked: &bool
            // Handle state update
        }
    })
```

Checkbox ID must be unique. `.on_click` receives `&bool` as first arg.

## Layout Components

From `gpui_component`:

```rust
use gpui_component::{h_flex, v_flex};
```

### Horizontal Flex (`h_flex`)

```rust
h_flex()
    .gap_3()            // Space between children
    .items_center()     // Vertical alignment
    .justify_between()  // Horizontal space distribution
    .px_4()             // Horizontal padding
    .child(element1)
    .child(element2)
```

Wraps `flex-direction: row`.

### Vertical Flex (`v_flex`)

```rust
v_flex()
    .size_full()        // Fill parent
    .gap_4()            // Space between children
    .items_center()     // Horizontal alignment
    .justify_center()   // Vertical alignment
    .px_5()             // Horizontal padding
    .py_4()             // Vertical padding
    .child(element1)
    .child(element2)
```

Wraps `flex-direction: column`.

### Layout Methods

Both `h_flex` and `v_flex` support all GPUI styling methods:
- `.gap_N()` — space between children
- `.items_center()` — cross-axis alignment
- `.justify_center()` — main-axis alignment
- `.justify_between()` — spread children apart
- `.px_N()`, `.py_N()` — padding
- `.w_full()`, `.h_px()` — sizing
- `.bg(color)` — background color

## Root Component

Wraps the main application view:

```rust
use gpui_component::Root;

cx.open_window(
    WindowOptions { ... },
    |window, cx| {
        let view = cx.new(|cx| MyView::new(state, cx));
        cx.new(|cx| Root::new(view, window, cx))
    },
)
```

`Root::new(view, window, cx)` is typically the outermost wrapper. It handles theme application and default styling.

## Combining Layout and Components

### Example: Decision Dialog

> **Note:** The actual Dicto GPUI app uses custom GPUI elements for
> some components. The segmented pill toggle, outlined action buttons, and grid layout are all built
> with raw `div()` + `h_flex()`/`v_flex()`. This example shows the general pattern.

```rust
v_flex()
    .size_full()
    .bg(color_bg())
    // Header
    .child(
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .bg(color_amber_dim())
            .border_b_1()
            .border_color(color_amber())
            .child(
                div().text_color(color_amber()).child("⚠  WARNING")
            )
            .child(
                div().text_color(color_amber()).child("10s")
            )
    )
    // Content
    .child(
        v_flex()
            .flex_1()
            .px_5()
            .py_4()
            .gap_4()
            .child(
                Checkbox::new("remember")
                    .label("Remember this decision")
                    .checked(false)
                    .on_click({
                        move |checked, _, cx| {
                            // Update state
                        }
                    })
            )
    )
    // Footer buttons
    .child(
        h_flex()
            .w_full()
            .gap_3()
            .px_5()
            .py_4()
            .child(
                Button::new("deny")
                    .label("Deny")
                    .danger()
                    .w_full()
                    .on_click(|_, _, cx| { /* handle */ })
            )
            .child(
                Button::new("allow")
                    .label("Allow")
                    .success()
                    .w_full()
                    .on_click(|_, _, cx| { /* handle */ })
            )
    )
```

## Key Imports

```rust
use gpui_component::{
    Root,
    Theme,
    ThemeMode,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    h_flex, v_flex,
};
```

## Common Patterns

### Conditional Component Visibility

```rust
.when(is_visible, |el| {
    el.child(Checkbox::new("id").label("Text"))
})
```

Requires `gpui::prelude::FluentBuilder as _` import from gpui (not gpui-component).

### Callback Closures

```rust
Button::new("id")
    .label("Click")
    .on_click({
        let state_weak = self.state.downgrade();
        move |_, _, cx| {
            if let Some(state) = state_weak.upgrade() {
                cx.update_entity(&state, |s, cx| {
                    s.field = new_value;
                    cx.notify();
                }).ok();
            }
        }
    })
```

Use block `{ ... }` to move bindings into closure. Capture weak refs to entities.

### Multiple Buttons in Row

```rust
h_flex()
    .w_full()
    .gap_3()  // Space between buttons
    .child(Button::new("btn1").label("Left").w_full())
    .child(Button::new("btn2").label("Right").w_full())
```

`.w_full()` on buttons makes them equally sized in flex row.

## Table Component

The `Table` component displays tabular data using a delegate pattern. It supports striped rows, column resizing, sorting, and row selection.

### Key Types

```rust
use gpui_component::table::{Table, TableDelegate, TableState, TableEvent, Column};
```

### TableDelegate Trait

Implement `TableDelegate` to provide data and rendering for a table:

```rust
struct MyDelegate {
    columns: Vec<Column>,
    items: Vec<MyItem>,
}

impl TableDelegate for MyDelegate {
    fn columns_count(&self, _cx: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _cx: &App) -> usize {
        self.items.len()
    }

    fn column(&self, col_ix: usize, _cx: &App) -> &Column {
        &self.columns[col_ix]
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let item = &self.items[row_ix];
        match col_ix {
            0 => div().child(item.name.clone()),
            1 => div().child(item.value.to_string()),
            _ => div().child("—"),
        }
    }
}
```

**Key trait methods (all have default implementations except `render_td`):**
- `fn columns_count(&self, cx: &App) -> usize` — number of columns
- `fn rows_count(&self, cx: &App) -> usize` — number of rows
- `fn column(&self, col_ix: usize, cx: &App) -> &Column` — column definition (returns `&Column`, not `Column`)
- `fn render_td(&mut self, row_ix, col_ix, window, cx) -> impl IntoElement` — render a cell
- `fn render_tr(&mut self, row_ix, window, cx) -> Option<Div>` — customize row wrapper
- `fn render_th(&mut self, col_ix, window, cx) -> Option<Div>` — customize header cell
- `fn render_empty(&self, window, cx) -> Option<impl IntoElement>` — empty state
- `fn perform_sort(&mut self, col_ix, sort, window, cx)` — handle column sort
- `fn has_more(&self, cx: &App) -> bool` — for infinite scroll
- `fn load_more(&mut self, window, cx)` — load more data

### Column Definition

```rust
Column::new("id", "ID")
    .width(px(140.))
    .resizable(true)
    .sortable()

Column::new("actions", "Actions")
    .width(px(150.))
    .resizable(false)
```

- First arg: unique key (used for sort identification)
- Second arg: display name shown in header
- `.width(px(N))` — fixed column width
- `.resizable(bool)` — allow user to resize
- `.sortable()` — enable sort indicator
- `.fixed_left()` / `.fixed_right()` — freeze column

### Creating and Rendering Table

```rust
// In your view's new():
let table_state = cx.new(|cx| {
    TableState::new(my_delegate, window, cx)
        .row_selectable(true)
});

// In render():
Table::new(&self.table_state)
    .stripe(true)
    .bordered(true)
```

### Table Events

Subscribe to table events for row interactions:

```rust
cx.subscribe_in(&table_state, window, |view, _table, event, window, cx| {
    match event {
        TableEvent::DoubleClickedRow(row_ix) => {
            // Open detail dialog
        }
        TableEvent::SelectRow(row_ix) => {
            // Handle row selection
        }
        _ => {}
    }
});
```

**Important:** `subscribe_in` returns `Subscription`, NOT `()`. Store subscriptions in a Vec field to keep them alive:

```rust
struct MyView {
    table: Entity<TableState<MyDelegate>>,
    _subscriptions: Vec<Subscription>,
}
```

### TableState Methods

```rust
// Access delegate
table.delegate()          // &D
table.delegate_mut()      // &mut D

// Selection
table.set_selected_row(row_ix, cx);
table.selected_row()      // Option<usize>

// Refresh after data change
table.refresh(cx);
```

### Updating Delegate Data

```rust
self.table.update(cx, |table, _| {
    table.delegate_mut().items = new_items;
});
```

## Dialog Component

Modal dialogs for user interaction. Requires `Root` wrapper and `WindowExt` trait.

### Key Types

```rust
use gpui_component::dialog::{Dialog, DialogButtonProps};
use gpui_component::{Root, WindowExt as _};
```

### Opening a Dialog

```rust
// In a method with &mut Window and &mut Context<Self>:
window.open_dialog(cx, move |dialog, _, _| {
    dialog
        .title("Add Item")
        .w(px(480.))
        .button_props(
            DialogButtonProps::default()
                .ok_text("Save")
                .cancel_text("Cancel"),
        )
        .confirm()           // ← REQUIRED to show OK/Cancel buttons
        .child(
            v_flex()
                .gap(px(12.))
                .child(div().child("Content here"))
        )
        .on_ok(|_, _, _| true)   // Return true to close
});
```

**Dialog closure is `Fn` (not `FnOnce`):** All captured values must be re-usable. For `String` values used in both `.is_empty()` check and else branch, use `.clone()` in the else:

```rust
let name = item.name.clone();
window.open_dialog(cx, move |dialog, _, _| {
    dialog.child(
        div().child(if name.is_empty() { "—".into() } else { name.clone() })
    )
});
```

### Dialog Builder Methods

```rust
dialog
    .title(impl IntoElement)     // Dialog title (any element)
    .w(px(500.))                 // Fixed width
    .max_w(px(600.))             // Max width
    .close_button(bool)          // Show X button
    .overlay(bool)               // Show backdrop overlay
    .overlay_closable(bool)      // Click overlay to close
    .keyboard(bool)              // Enable keyboard shortcuts
    .button_props(               // Customise button labels/variants
        DialogButtonProps::default()
            .ok_text("Save")
            .cancel_text("Cancel"),
    )
    .confirm()                   // Adds OK + Cancel footer buttons
    .alert()                     // Adds OK-only footer button
    .child(impl IntoElement)     // Add content
    .on_ok(|_, window, cx| bool)      // OK handler — return true to close
    .on_cancel(|_, window, cx| bool)  // Cancel handler — return true to close
    .on_close(|_, window, cx|)        // Called after ok/cancel closes
```

### ⚠ `.confirm()` / `.alert()` Is Required for Buttons to Appear (unless using `.footer()`)

`on_ok` alone does **not** render any buttons. You must call `.confirm()` (OK + Cancel), `.alert()` (OK only), **or** provide a custom `.footer()` closure to get buttons. Without one of these the dialog opens but has no way to be confirmed.

```rust
// ❌ BROKEN — no buttons shown, dialog cannot be confirmed
dialog.on_ok(|_, _, _| true)

// ✅ CORRECT — footer with OK + Cancel rendered
dialog.confirm().on_ok(|_, _, _| true)

// ✅ ALSO CORRECT — custom footer (do NOT also call .confirm())
dialog
    .footer(|ok, cancel, w, cx| vec![modal_footer(cancel(w, cx), ok(w, cx))])
    .on_ok(|_, _, _| true)
```

### Using `.content()` for Dynamic Content

The `.content()` method takes a closure that receives `DialogContent, &mut Window, &mut App` and returns `DialogContent`. This is useful when you need to read entity state at render time:

```rust
window.open_dialog(cx, move |dialog, _window, _cx| {
    let state = state.clone();
    dialog
        .title("Settings")
        .w_full()
        .close_button(true)
        .content(move |content, _window, cx| {
            let active_tab = state.read(cx).settings_active_tab;
            content.child(
                v_flex().w_full()
                    .child(if active_tab == 0 {
                        /* tab 0 content */
                        div().into_any_element()
                    } else {
                        /* tab 1 content */
                        div().into_any_element()
                    })
            )
        })
        .footer(
            h_flex().justify_end().gap(px(8.))
                .child(/* Cancel button */)
                .child(/* Save button */)
        )
});
```

**Key points:**
- `.content()` closure is `Fn` (called each render), so clone entities before the closure.
- `DialogContent` has `.child()` just like `div()`.
- `.footer()` takes an `impl IntoElement` (not a closure like the older pattern).
- `window.close_dialog(cx)` closes the dialog from any click handler.

### Nested Dialogs

Dialogs can be opened from within another dialog (e.g., a detail dialog from a settings dialog). Each `open_dialog` call pushes onto `Root::active_dialogs`. `close_dialog` closes the topmost one. All dialogs render via `Root::render_dialog_layer()`.

### Custom Full-Bleed Dialog Header

`Dialog` implements the `Styled` trait, so calling `.p(px(0.))` zeroes all internal padding. Combined with `.close_button(false)` and a custom `.title()`, this gives a full-width header bar:

```rust
use crate::components::{field_label, modal_footer, modal_header};

window.open_dialog(cx, move |dialog, _, _cx| {
    dialog
        .p(px(0.))                   // zero internal padding → full-bleed title
        .close_button(false)         // hide default X (we add our own in modal_header)
        .title(modal_header("⊕", "ADD EGRESS"))
        .w(px(480.))
        .button_props(
            DialogButtonProps::default()
                .ok_text("Save")
                .cancel_text("Cancel"),
        )
        .footer(|ok, cancel, w, cx| vec![modal_footer(cancel(w, cx), ok(w, cx))])
        .child(
            v_flex()
                .px(px(16.))
                .py(px(16.))
                .gap(px(16.))
                .child(v_flex().gap(px(4.))
                    .child(field_label("NAME"))
                    .child(Input::new(&name_c))
                )
        )
        .on_ok(move |_, _, cx| {
            // save ...
            true
        })
});
```

**Key rules:**
- `.p(px(0.))` must be called before `.title()` — it tells Dialog to render the title wrapper with zero padding, making it flush with the dialog edges.
- Do **not** call `.confirm()` when using `.footer()` — they conflict; `.footer()` provides the buttons directly.
- `window.close_dialog(cx)` can be called inside any `on_click` handler where `window: &mut Window` is available (e.g. inside `modal_header`'s ✕ button).

### ⚠ `render_dialog_layer` Must Be Called in Your `Render` Impl

`window.open_dialog(...)` pushes the dialog into `Root::active_dialogs`, but `Root::render` does **not** render it automatically. You must call `Root::render_dialog_layer` yourself inside your view's `render()`:

```rust
impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(/* your content */)
            // ← Without this line, dialogs are registered but never appear
            .children(Root::render_dialog_layer(window, &mut **cx))
    }
}
```

Note: `cx` is `&mut Context<Self>` but `render_dialog_layer` expects `&mut App`. Use `&mut **cx` to explicitly double-deref (auto-coercion does not work in free-function call position even though `Context<T>: DerefMut<Target = App>`).

### Root Requirement

Dialog requires `Root` as the outermost view wrapper:

```rust
cx.open_window(WindowOptions { ... }, |window, cx| {
    let view = cx.new(|cx| MyView::new(state, window, cx));
    cx.new(|cx| Root::new(view, window, cx))
});
```

### Input Fields Inside Dialogs

The `Input` component **is** available in 0.5.1. Create `InputState` entities **before** the dialog closure (since the closure is `Fn`, not `FnOnce`):

```rust
fn open_form_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let name_input = cx.new(|cx| {
        let mut s = InputState::new(window, cx);
        s.set_value("default value", window, cx);
        s
    });
    let name_input_c = name_input.clone();

    window.open_dialog(cx, move |dialog, _, _cx| {
        dialog
            .confirm()
            .child(Input::new(&name_input_c))
            .on_ok(move |_, _, cx| {
                let value = name_input_c.read(cx).value().to_string();
                // use value...
                true
            })
    });
}
```

For shared mutable state across the `Fn` closure (e.g. a protocol selector), use `Arc<Mutex<T>>`:

```rust
let selected: Arc<Mutex<MyEnum>> = Arc::new(Mutex::new(MyEnum::Default));
let selected_c = selected.clone();

window.open_dialog(cx, move |dialog, _, _cx| {
    let for_btn = selected.clone();
    let for_ok  = selected_c.clone();
    dialog
        .confirm()
        .child(div().on_click(move |_, _, _| {
            *for_btn.lock().unwrap() = MyEnum::Other;
        }))
        .on_ok(move |_, _, _| {
            let val = for_ok.lock().unwrap().clone();
            // use val...
            true
        })
});
```

## Design System Components (`components/modal.rs`)

Reusable UI primitives matching the Dicto design language. Import from `crate::components::*`.

### `modal_header(icon, title)`

Full-width dialog title bar with icon + UPPERCASE title on the left, ✕ close button on the right.

```rust
pub fn modal_header(icon: &str, title: &str) -> impl IntoElement
```

- Background: `surface_container_high`
- Bottom border: `colors::border()`
- ✕ button calls `window.close_dialog(cx)` — requires `gpui_component::WindowExt as _`

Usage: pass as `.title(modal_header("⊕", "ADD PROXY"))` on a Dialog that has `.p(px(0.))`.

### `modal_footer(cancel, ok)`

Styled footer bar with top border and `surface_container_high` background. Accepts already-rendered cancel and ok `AnyElement`s.

```rust
pub fn modal_footer(cancel: AnyElement, ok: AnyElement) -> AnyElement
```

Usage inside `.footer()`:
```rust
dialog.footer(|ok, cancel, w, cx| vec![modal_footer(cancel(w, cx), ok(w, cx))])
```

Note: `RenderButtonFn` is a private type alias in gpui-component. Never name it — let the compiler infer closure parameter types.

### `field_label(text)`

UPPERCASE label for form fields (10px bold muted text):

```rust
pub fn field_label(text: &str) -> AnyElement
```

### `table_badge(label, color)`

Inline bordered badge for Status, Type, and Protocol columns:

```rust
pub fn table_badge(label: &str, color: Hsla) -> AnyElement
```

### `action_btn(id, label, color)`

Small outline button for table row actions. Returns `Stateful<Div>` so callers can chain `.on_click(...)`:

```rust
pub fn action_btn(id: impl Into<SharedString>, label: &str, color: Hsla) -> Stateful<Div>
```

Border color defaults to `color`. For toggle buttons where text and border differ, override with `.border_color(other)`:

```rust
action_btn("tog-btn", "Disable", colors::muted())
    .border_color(colors::border())    // overrides the default
    .on_click(move |_, _, cx| { ... })
```

### `proto_btn(label, text_color, border_color, on_click)`

Protocol selector button for proxy form dialogs (SOCKS5 / HTTP / SS):

```rust
pub fn proto_btn(
    label: &str,
    text_color: Hsla,
    border_color: Hsla,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement
```

## TabBar Component

Tab navigation for switching between views:

```rust
use gpui_component::tab::{Tab, TabBar};

TabBar::new("my-tabs")
    .selected_index(0)
    .on_click(cx.listener(|this, index: &usize, _, cx| {
        // Handle tab change
    }))
    .child(Tab::new("tab-0").label("Rules"))
    .child(Tab::new("tab-1").label("Egress"))
    .child(Tab::new("tab-2").label("Proxies"))
```

## Theme Customization (Advanced)

Custom themes are possible but require deeper configuration. For most cases, use built-in `ThemeMode::Dark` / `ThemeMode::Light` and override colors manually with GPUI's `.bg(color)` and `.text_color(color)` methods.

## Component Library Limitations (0.5.1)

- No `DataTable` (git-only) — use `Table` with `TableDelegate` instead
- No `DialogHeader` / `DialogTitle` / `DialogFooter` (git-only) — use `Dialog::title()` and `.child()` instead
- Theme customization is limited to two modes (Dark/Light)

## Crates.io vs Git Version

The crates.io version (0.5.1) differs from the git repo version:

| Feature | crates.io 0.5.1 | git repo |
|---------|-----------------|----------|
| Table | ✅ `Table<D>` | ✅ `DataTable<D>` |
| TableDelegate | ✅ | ✅ |
| Dialog | ✅ `Dialog` + `DialogButtonProps` | ✅ rich (`DialogHeader`, `DialogTitle`, etc.) |
| TabBar | ✅ | ✅ |
| Input | ✅ `Input` + `InputState` | ✅ |
| Select | ✅ | ✅ |
| Sizable | ✅ | ✅ |

**Official installation uses git repos:**
```toml
gpui = { git = "https://github.com/zed-industries/zed" }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
```

**Current project uses crates.io:**
```toml
gpui = "0.2"
gpui-component = "0.5"
```

---

## Label Component

`Label` is a text element with optional secondary text. It is **not re-exported from the crate root** — import from the submodule:

```rust
use gpui_component::label::Label;
```

### Basic Usage

```rust
Label::new("hello")
    .text_size(px(12.))
    .text_color(colors::text())
    .into_any_element()
```

### With Secondary Text

```rust
Label::new("socks-client")
    .secondary("ui-123abc")   // rendered below in muted/smaller style
```

### Key Methods

| Method | Description |
|---|---|
| `.text_size(px(N))` | Override font size |
| `.text_color(hsla)` | Override text color |
| `.secondary(text)` | Dim sub-label below the main text |
| `.masked(bool)` | Replace content with bullet dots |

### When to Use Label vs div

Prefer `Label` whenever you are rendering a single line of read-only text in a table cell, form row, or summary line. Use `div()` only when you need to nest child elements or apply layout (flex, sizing, border, bg) directly on the text container — `Label` is a leaf, not a container.

```rust
// ✅ Correct — single read-only text
Label::new(dest_text(&rule.destination)).text_size(px(12.))

// ✅ Correct — container with children
h_flex()
    .gap(px(6.))
    .child(Label::new("name"))
    .child(some_badge)

// ❌ Avoid — raw div for plain text
div().text_size(px(12.)).child("some text")
```

---

## Export Map — What Is and Is Not Re-exported

Many gpui-component types are in submodules and **not** re-exported at the crate root. Always verify before using a bare `gpui_component::Foo` import.

| Type | Import path |
|---|---|
| `h_flex` / `v_flex` | `gpui_component::h_flex` (crate root ✅) |
| `Root` | `gpui_component::Root` (crate root ✅) |
| `Theme` / `ThemeMode` | `gpui_component::{Theme, ThemeMode}` (crate root ✅) |
| `Table` | `gpui_component::table::Table` |
| `TableState` | `gpui_component::table::TableState` |
| `TableDelegate` | `gpui_component::table::TableDelegate` |
| `Column` | `gpui_component::table::Column` |
| `TableEvent` | `gpui_component::table::TableEvent` |
| `Dialog` | `gpui_component::dialog::Dialog` |
| `DialogButtonProps` | `gpui_component::dialog::DialogButtonProps` |
| `Input` | `gpui_component::input::Input` |
| `InputState` | `gpui_component::input::InputState` |
| `Label` | `gpui_component::label::Label` ⚠ submodule only |
| `Tab` / `TabBar` | `gpui_component::tab::{Tab, TabBar}` |
| `Button` | `gpui_component::button::Button` |
| `ButtonVariants` | `gpui_component::button::ButtonVariants` |
| `Checkbox` | `gpui_component::checkbox::Checkbox` |

**Rule:** If `use gpui_component::Foo` causes an unresolved import error, drop to the submodule: `use gpui_component::foo_module::Foo`.

---

## Layout: h_flex / v_flex vs div

`h_flex()` and `v_flex()` from `gpui_component` are pre-configured flex containers. They are the preferred layout primitives in Dicto's GPUI code.

```rust
// ✅ Prefer
h_flex().gap(px(8.)).items_center().child(a).child(b)
v_flex().gap(px(12.)).child(row1).child(row2)

// ❌ Avoid — more verbose, same result
div().flex().flex_row().gap(px(8.)).items_center().child(a).child(b)
```

`div()` is reserved for:
- A non-flex block (e.g. a colored dot, a divider, a wrapper that only needs bg/border/sizing)
- A container that uses `flex_col` via explicit `.flex_col()` when `v_flex` is not imported or not appropriate

---

## Project Design-System Layer (`components/ds.rs`)

Dicto wraps the lowest-level GPUI/gpui-component primitives in `gpui/src/components/`. Always check the components directory before writing inline UI atoms.

### Available Primitives

| Function | Signature | Purpose |
|---|---|---|
| `badge` | `badge(text, color: Hsla) -> AnyElement` | Read-only colored pill chip (bg tint + alpha border) |
| `chip` | `chip(id, label, selected, on_click) -> AnyElement` | Interactive toggle chip (primary fill when active) |
| `dest_text` | `dest_text(dest: &DestinationMatcher) -> String` | Human-readable destination string for any matcher variant |
| `dest_kind_label` | `dest_kind_label(dest: &DestinationMatcher) -> &str` | Short type tag (`"IP"`, `"CIDR"`, `"DOMAIN"`, …) |
| `cidr_picker` | `cidr_picker(ip: &str, active_octets: u8, state_weak) -> AnyElement` | Interactive CIDR octet picker (4 fixed-width chips + `/prefix` badge) |
| `label_row` | `label_row(label, content: AnyElement) -> AnyElement` | 72px muted label + right-side content — standard form row layout |

### badge vs chip vs Label

| Need | Use |
|---|---|
| Read-only colored tag (Action, Duration, Protocol) | `ds::badge` |
| Interactive toggle (scope selection) | `ds::chip` |
| Plain read-only text (table cell, summary) | `gpui_component::label::Label` |
| Custom interactive element | `div().id(...).on_click(...)` |

### label_row layout contract

Every "label + interactive content" row in the settings panels must use `label_row`. This keeps the 72px left column aligned across all rows:

```rust
ds::label_row("Process",     chips.into_any_element())
ds::label_row("Destination", cidr_or_domain_chips.into_any_element())
ds::label_row("Duration",    scope_toggle(...))
ds::label_row("Route via",   egress_selector(...))
```

### cidr_picker interaction model

- 4 octet chips, each 38px wide, center-aligned text
- Active octet: primary color background + border, real value displayed
- Masked octet: dim border, strikethrough `0`
- Clicking active octet N → masks it and all after (min 1 active)
- Clicking masked octet N → activates it and all before
- `/prefix` badge (32px fixed) updates live: active_octets × 8
- State is stored in `AppState.dest_scope = DestScope::IpCidr(active_octets)`

### dest_text output examples

| DestinationMatcher | Output |
|---|---|
| `Any` | `"any destination"` |
| `IpExact("1.1.1.1")` | `"1.1.1.1"` |
| `Cidr("10.0.0.0/8")` | `"10.0.0.0/8"` |
| `DomainExact("google.com")` | `"google.com"` |
| `DomainWildcard("google.com")` | `"*.google.com"` |

---

## Domain Apex Extraction

When computing a wildcard label from a destination domain, the apex depends on the number of dots:

```rust
fn domain_apex(domain: &str) -> String {
    let dot_count = domain.chars().filter(|&c| c == '.').count();
    if dot_count >= 2 {
        // "accounts.google.com" → "google.com" (strip leftmost label)
        domain.splitn(2, '.').nth(1).unwrap_or(domain).to_string()
    } else {
        // "google.com" → "google.com" (already the apex)
        domain.to_string()
    }
}
```

**Rule:** Never use `.splitn(2, '.').nth(1)` alone — it produces `"com"` from `"google.com"`. Always guard with a dot-count check.

Wildcard chips: `*.{apex}`. The `DomainWildcard` variant in the rule stores just the apex (without the `*.` prefix), and the policy-engine's `wildcard_matches` prepends `*.` when matching.
