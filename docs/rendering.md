# Rendering pipeline (HTML → GPUI → screen)

A definition body arrives from `query_all` as a `String` of HTML.
That string flows through:

```
HTML string
  └─► html::parse_styled(html, dict_name)
        ├─► tokenize  →  Vec<Event>          parser.rs::tokenize
        └─► build_blocks(events, &Stylesheet) → Vec<Block>
                            ▲
                            │ STYLESHEETS map  (per-dict)
                            │
Stylesheet loaded once at startup from:
  – CSS resources inside each dict's .mdd
  – sibling `<stem>.css` / `<stem>_*.css` files on disk

DictState.results[active].blocks  ─►  render_blocks(&blocks)
   ─►  GPUI element tree
   ─►  the right pane in DetailPanel
```

Files live under `gpui/src/html/`:

- [parser.rs](../gpui/src/html/parser.rs) — HTML tokenizer + builder
- [css.rs](../gpui/src/html/css.rs) — CSS subset parser + matcher
- [render.rs](../gpui/src/html/render.rs) — `Block` → GPUI elements
- [mod.rs](../gpui/src/html/mod.rs) — `STYLESHEETS` map, `parse_styled`

## HTML tokenizer (parser.rs)

Lenient, hand-rolled — MDX HTML is full of unbalanced tags, void
elements with explicit closers (`<img ... ></img>`), comments,
doctypes, and entity-escaped chunks.

Tokens are events: `Open(name, attrs)`, `Close(name)`,
`SelfClose(name, attrs)`, `Text(string)`.

Void elements (`br`, `img`, `hr`, `meta`, `link`, `input`) always
become `SelfClose` regardless of how the source closes them. A
trailing `</img>` from the source becomes a `Close("img")` event,
which we deliberately ignore (see "Gotchas" below).

Entity decoding (`&amp;`, `&#39;`, `&#x2022;`, `&nbsp;`, …) runs in
the builder, not the tokenizer, so attribute values stay raw.

### UTF-8 safety

Both the tokenizer and `decode_entities` iterate by `char` /
`char_indices`. An earlier version casted individual bytes via
`bytes[i] as char` — that destroyed every multi-byte sequence (IPA,
bullets, em-dashes) and produced the classic `Ã¢ÂÂ¢` mojibake.

## Builder (parser.rs::build_blocks)

`Builder` maintains four stacks plus a skip counter:

- `style_stack: Vec<Style>` — the active inline style.
- `link_stack: Vec<Link>` — sound, entry, or external anchors.
- `list_stack: Vec<bool>` — ordered/unordered `<ul>`/`<ol>` nesting.
- `element_stack: Vec<OpenElement>` — every open element with its
  tag/classes/id and whether it pushed a style frame.
- `skip_depth: u32` — non-zero while we're inside a CSS
  `display: none` subtree.

For each `Event::Open`:

1. Look up CSS-matched declarations against the element + every
   ancestor in `element_stack`.
2. If `display: none`: push a placeholder OpenElement, increment
   `skip_depth`, return early.
3. If `display: block` (CSS) or the tag is one of `<p>`, `<div>`,
   `<h1..6>`, `<ul>`, `<ol>`, `<li>`, `<table>`, `<tr>`: flush the
   current inline buffer as a Paragraph.
4. Build a merged style: clone the parent (text properties only —
   margins reset to zero, they don't inherit), clear `bg_color` for
   block-level elements so backgrounds attach to the wrapper instead
   of every child run, apply hard-coded tag rules (`<b>` → bold,
   `<i>` → italic, `<u>` → underline, `<sup>` / `<sub>` →
   reduced-size baseline shifts, `<font color>` → color), then apply
   CSS decls.
5. Push the merged style + the OpenElement.
6. Run tag-specific side effects: `<a>` pushes onto `link_stack`,
   `<ul>` / `<ol>` push onto `list_stack`, `<hN>` records the level
   in `pending_heading` for the matching close to flush as Heading.

Some dictionary-specific wrappers are intentionally treated as inline
despite being `<div>` elements. `mwaled` uses `div.scnt`,
`div.sense`, and `div.sblock_labels` as float-heavy structural
wrappers; keeping them block-level caused sense numbers and labels to
split across multiple lines.

`Event::Close` walks `element_stack` *backwards* to find a matching
tag, auto-closes anything in between (forgiving parser), then pops
the matched element. If no matching open exists the stray close is
silently discarded.

### Style struct

```rust
pub struct Style {
    pub bold, italic, underline: bool,
    pub superscript, subscript: bool,
    pub color, bg_color: Option<SharedString>,
    pub font_size_px: Option<f32>,
    pub padding_top_px, padding_right_px, padding_bottom_px, padding_left_px: f32,
    pub margin_right_px, border_radius_px: f32,
    pub margin_top_px, margin_bottom_px, margin_left_px: f32,
}
```

**Text properties inherit** (clone from parent before mutating).
**Margins do not** (reset to zero each open) — matches CSS box
semantics. Inline box-model fields are kept because some dictionaries
style anchors as chips / buttons rather than plain text spans.

### Inline buffer & cross-run whitespace

Text events become `Inline { text, style, link }` runs accumulated
in `inline_buf`. `push_text` does two things:

1. `collapse_whitespace(&text)` — multiple consecutive whitespace
   chars collapse to one space (HTML inline rules).
2. **Cross-run trim** — if the previous inline already ends with
   whitespace (or the buffer is empty), strip leading whitespace
   from the new run. Without this, adjacent `<span>` siblings like
   `<span> ,</span><span> adjective</span>` rendered as
   ` , adjective` instead of `, adjective`.

### Block enum

```rust
pub enum Block {
    Paragraph { runs: Vec<Inline>, layout: BlockLayout },
    Heading   { level: u8, runs: Vec<Inline>, layout: BlockLayout },
    ListItem  { ordered: bool, depth: u8, content: Vec<Inline> },
    Divider,
    Image(SharedString),
}

pub struct BlockLayout {
    pub margin_top_px, margin_bottom_px, margin_left_px: f32,
    pub bg_color: Option<SharedString>,
}
```

`BlockLayout` is captured from the current style frame at flush
time, so margins and block-level backgrounds set via CSS on the block
element propagate to the emitted `Block`.

## CSS subset (css.rs)

Selectors we honor:

- Tag: `p`
- Class: `.SE_EntryAssets`
- Id: `#root`
- Compound: `span.foo`, `a.bar.baz`
- Descendant chains: `.Sense .EXAMPLE .BASE`
- Lists: `a, b, c { ... }`

Selectors we skip (whole rule is ignored if any selector uses one):

- Pseudo-classes / pseudo-elements: `:link`, `::before`, etc.
- Adjacent / child / sibling combinators: `a + b`, `a > b`, `a ~ b`
- Attribute selectors: `a[href]`
- `@media`, `@import`, other at-rules

Declarations we honor:

| Property | Maps to |
|----------|---------|
| `display: none` | Drops the element and its subtree (`skip_depth++`). |
| `display: block / list-item / table*` | Block flush around the element. |
| `display: inline / inline-block` | No flush. |
| `color: <named or hex>` | `Style.color` (then dark-theme remap). |
| `background-color`, `background` | `Style.bg_color`. |
| `font-weight: bold / bolder / 600+` | `Style.bold = true`. |
| `font-style: italic / oblique` | `Style.italic = true`. |
| `text-decoration: ...underline...` | `Style.underline = true`. |
| `font-size: 12px / 1.2em / 14pt / 110%` | `Style.font_size_px`. |
| `margin-top / -bottom / -left` | block layout fields. |
| `margin-right` | inline chip spacing. |
| `margin` (1, 2, or 4 values) | shorthand → block layout fields. |
| `padding-*`, `padding` | inline chip padding. |
| `border-radius` | inline chip rounding. |

Length units accepted: `px` (default), `em`, `rem`, `pt` (× 1.333),
`%` (treated as % of 14 px base font), plain numbers (= px).

Specificity is **not** modeled — later rules override earlier ones
(source order). This matches what works for real MDX dictionary
stylesheets in practice; if you find a dict relying on real CSS
specificity, raise an issue.

## Stylesheet loading (per dictionary)

[gpui/src/main.rs::load_stylesheets](../gpui/src/main.rs)

Each enabled dictionary gets its **own** stylesheet keyed by file
stem. Loading is a union of two sources:

1. **Inside the `.mdd`**: every `path LIKE '%.css'` resource is
   pulled from the MDD index. LDOCE5 ships `ldoceaz.css` this way
   (~1500 rules covering `.Entry`, `.HWD`, `.POS`, `.EXAMPLE`, etc.).
2. **Sibling files on disk**: any `<stem>.css` or `<stem>_*.css`
   next to the `.mdx`. The user can hand-edit these to override the
   embedded CSS.

Why per-dictionary? Class names like `.hw`, `.hwd`, `.pos`,
`.headword`, `.entry`, `.example` are reused with different
semantics across dictionaries. A global stylesheet would mean an
LDOCE5 rule like `.hw { display: none }` wiping out a Cambridge
headword using the same class name. Per-dict scoping prevents that.

If no CSS is found for a dictionary, `parse_styled` falls back to an
empty stylesheet — the dictionary renders with hard-coded HTML tag
defaults only (which is still legible, just unstyled).

## Renderer (render.rs)

`render_blocks(&[Block])` is invoked once per re-render of the
detail panel. Cost matters here.

### Element strategy

| Block | Element |
|-------|---------|
| `Paragraph` | If only one run, wrap in a `w_full` div containing a single styled `div` (text wraps inside it; the outer div ensures a bounded width). Multi-run paragraphs use an `h_flex().flex_wrap()` with one child per inline run; each child uses `flex_shrink()` and `min_w(px(0))` to allow wrapping inside individual runs. |
| `Heading` | Same as Paragraph but with a larger `font-size` and bold. |
| `ListItem` | `h_flex` with a `•` and a child Paragraph indented by depth. |
| `Divider` | A 1 px line. |
| `Image` | Bytes from `lookup_resource` → cached to `/tmp/mdict-rs-cache/<hash>.<ext>` → `gpui::img(PathBuf)`. |

Each emitted block is wrapped by `with_layout(...)` in a
padding-bearing div if the `BlockLayout` has any non-zero margins.
Margins are clamped to 0..80 px so a misbehaving CSS rule can't push
content off-screen.

### Inline runs

`render_run` makes one element per `Inline`. Three specializations:

- **Sound link with image child** — render the dictionary-provided
  image resource (for example speaker icons from an MDD) as
  a clickable audio button.
- **Sound link without inline content** — synthesize a fallback audio
  control on close. `mwaled` uses empty icon-font anchors, so we keep
  the anchor's color / font-size / spacing and render a modern SVG
  play icon from the UI assets.
- **Entry link** — stateful div with `cursor_pointer` + `on_click`
  (currently just logs; wiring it back to the search bar is a
  follow-up).

All other runs are plain styled `div`s with no `.id(...)` — they
don't need state. Dropping per-run ids saved an allocation per run
per frame on the previous performance pass.

### Grouped styled links

Some dictionaries style a whole `<a>` as a chip
while nesting multiple child spans inside it:

```html
<a href="#entry_1"><span class="kw">very<sup>1</sup></span> <i>adverb</i></a>
```

Rendering each child independently loses the shared background,
padding, and border radius. The renderer now groups consecutive runs
belonging to the same entry / external link when the first run carries
box-model styling, then applies the chip container styles once around
the whole group.

### Color remap for dark theme

Foreground text and backgrounds now use different remaps:

- `parse_text_color` → `remap_text_color_for_dark_theme` lifts dark
  colors (`MIN_L = 0.65`) and caps saturation (`MAX_S = 0.75`) so
  dictionary-defined text remains readable.
- `parse_bg_color` → `remap_bg_color_for_dark_theme` keeps neutral
  panels dark and caps colored backgrounds to a darker range so chip
  backgrounds do not glow or lose contrast with white text.

This keeps a dictionary's semantic palette (blue = entry link, green
= register label, orange = sense heading) recognizable while avoiding
washed-out chips and pale header bars on Dicto's dark UI.

CSS named colors (`red`, `navy`, `teal`, …) and `#rgb` / `#rrggbb`
hex forms are recognized; everything else falls back to default
text color.

### Performance evolution

Several incremental tightenings landed:

1. **Cache parsed blocks.** Each lookup parses HTML once on the
   background executor; the cached `Vec<Block>` re-renders for free.
2. **`Inline.text: SharedString`.** Arc-clone instead of full heap
   copy per re-render.
3. **Render by reference.** No `.iter().cloned()` over the whole
   block list each frame.
4. **One element per styled run.** Earlier the renderer split each
   text run into one element per word so words could wrap inside
   `flex_wrap`. That produced hundreds of nested elements per
   paragraph and dominated scroll cost. Now each styled run is a
   single div; wrapping happens inside the div via gpui's text
   layout, and breaks at run boundaries between divs.
5. **Drop ids on non-interactive containers.** Paragraph and list
   wrappers no longer call `.id(format!("p-{idx}"))` — one fewer
   `SharedString::from(format!())` per block per frame.

If the detail panel is still slow on a particularly large entry,
the next lever is true viewport virtualization
(`gpui_component::v_virtual_list`). That needs precomputed per-block
heights and isn't worth doing pre-emptively.

## Audio (gpui/src/audio.rs)

`play_resource(path)` looks the bytes up via
`mdict_rs::query::lookup_resource(path)` then either:

1. Plays directly via `rodio` if the codec is supported (mp3, wav,
   ogg-vorbis, flac).
2. Skips straight to `ffmpeg` for known-incompatible codecs (Speex,
   detected by `.spx` extension or `OggS`/`Speex` magic bytes).

When the rodio path "succeeds" but produces no sound (`symphonia`'s
mp3 demuxer happily accepts Speex bytes and returns a decoder that
emits zero samples), we wouldn't see an error. `needs_transcode()`
makes the decision up front and bypasses rodio entirely for Speex.

### ffmpeg fallback details

- Input bytes are written to `/tmp/mdict-rs-cache/<hash>.in`.
- `ffmpeg -loglevel error -y -i <in> -f wav -acodec pcm_s16le <out>`.
- Output is a real file (`/tmp/mdict-rs-cache/<hash>.wav`) — not a
  pipe — so the WAV header has correct chunk sizes. Pipe-mode
  output uses `0xFFFFFFFF` as the size sentinel, which rodio's
  symphonia WAV reader rejects silently.
- The cached `.wav` is reused on subsequent clicks for the same
  resource.

### tracing filter

`gpui/src/main.rs` defaults `EnvFilter` to
`info,symphonia_bundle_mp3=error,symphonia_core=error,symphonia_format_ogg=error`
because symphonia's mp3 demuxer prints a WARN per malformed byte
when handed Speex bytes — hundreds of lines per click. The `audio`
module logs one line on failure; everything else stays silent.

## Image resources

[gpui/src/html/render.rs::image_block](../gpui/src/html/render.rs)

1. `lookup_resource(src)` — bytes from any enabled MDD.
2. `cache_image(src, &bytes)` — hash the source path with
   `DefaultHasher`, write to `/tmp/mdict-rs-cache/<hash>.<ext>`.
3. `gpui::img(path: PathBuf)` — note `PathBuf` is critical;
   `gpui::img(SharedString)` routes the string through gpui's
   **embedded asset loader** (`Resource::Embedded`) which fails for
   absolute paths with `Embedded resource not found`. Only the
   `PathBuf` / `&Path` conversions hit `Resource::Path` for
   filesystem reads.

If the resource isn't found, we render `[image: <src>]` as
placeholder text instead of breaking the layout.

## GPUI gotchas hit during this session

### 1. TitleBar absorbs clicks on Linux

`gpui_component::TitleBar` marks itself as
`WindowControlArea::Drag` on Linux, which lets the compositor
intercept mouse events for window-move. Placing the settings cog
button inside `TitleBar`'s children meant clicks never reached our
handler.

**Fix**: put interactive buttons in a row *below* the titlebar (we
added a `right_slot: Option<AnyElement>` to `SearchBar`).

### 2. `Root::render_dialog_layer` is not automatic

`window.open_dialog(...)` adds a dialog to `Root.active_dialogs` but
does not paint it. The host app's `Render` must explicitly include
`Root::render_dialog_layer(window, cx)` as a sibling of the main
view, or the dialog mutates state silently with no visible overlay.

```rust
impl Render for DictApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let main = v_flex().size_full().bg(...). /* ... */ ;
        div().size_full().child(main).children(dialog_layer).into_any_element()
    }
}
```

### 3. `Scrollable::size_full()` needs a sized parent

`overflow_y_scrollbar()` wraps the element in a `div().size_full()`
relative-positioned container. `size_full` only fills a parent with
a **bounded height**. Wrap the scrollable in `flex_1().min_h(0)` (or
a fixed `h(...)`) so it has somewhere to anchor.

If you just give the inner v_flex `max_h(...)` the wrapper still
collapses — `max_h` only caps an otherwise-unbounded element.

### 4. TabBar consumes wheel events

`TabBar` is implemented as `.overflow_x_scroll()` so vertical wheel
events over it get eaten when they should bubble to the detail
panel's scroll area. Place the TabBar **outside** the scroll region
(e.g. above it) so wheel events on the tab row don't compete with
body scroll.

### 5. Stray `</img>` corrupts the element stack

MDX HTML often writes `<img src="..." ></img>` even though `<img>`
is a void element. Our tokenizer correctly emits `SelfClose("img")`
for the opener, but the source's `</img>` still produces a stray
`Close("img")` event. `handle_close` short-circuits for void element
names; otherwise it would pop the surrounding `<a>` or `<font>`,
unbalancing the link and style stacks for everything that follows
(which is how *every* downstream text node ended up wrapped in a
sound button).

### 6. Mismatched tags

Real MDX HTML routinely has un-popped tags. `handle_close` walks
`element_stack` backwards for a matching tag and auto-closes
anything in between. If no match exists the close is dropped. This
keeps the parser from going off the rails on imperfect input.

### 7. `gpui::img(SharedString)` vs `gpui::img(PathBuf)`

Covered above; worth restating because the failure mode is silent
`asset error: Embedded resource not found` followed by no image.
Always pass paths as `PathBuf` (or `&Path`); URLs as `SharedUri`.

### 8. `ElementId::Integer` exists

`From<usize>` for `ElementId` saves a `SharedString::from(format!(...))`
per frame when generating unique element ids. Use it for hot loops.

### 9. Text wrapping in GPUI needs a bounded container width

GPUI's internal text layout engine will only compute line breaks if the element containing the text has a known/bounded width. For a single text div, you must ensure:
1. All parent elements have a bounded width
2. The text div itself has `w_full()` or a fixed/constrained width

For flex-wrap containers with multiple inline items, also add:
- `.flex_shrink()` to items so they can shrink below their intrinsic width
- `.min_w(px(0))` to ensure items don't refuse to shrink beyond their content width

Without these, text runs will overflow horizontally, and lines will only break between flex items, not inside them.
