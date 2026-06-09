//! Convert a parsed HTML block list into a GPUI element tree.
//!
//! Inline runs lay out via wrapping flex rows; one row per paragraph,
//! one wrapping segment per styled span. Sound links become clickable
//! pill buttons that play audio via the MDD resource lookup; images
//! are cached to a tmp directory and rendered via `gpui::img`.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use gpui::{
    div, img, px, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use gpui_component::{h_flex, v_flex};
use tracing::{debug, warn};

use crate::audio;
use crate::colors;
use crate::html::parser::{Block, BlockLayout, Inline, Link, Style};

/// Render a pre-parsed block list. Callers parse once on lookup and
/// cache the result so we don't re-parse on every frame. Iteration is
/// by reference so we avoid cloning Blocks (and their inner Vecs) on
/// every render pass; SharedString text fields make leaf clones cheap.
pub fn render_blocks(blocks: &[Block]) -> gpui::AnyElement {
    v_flex()
        .w_full()
        .gap(px(8.))
        .children(
            blocks
                .iter()
                .enumerate()
                .map(|(idx, b)| render_block(idx, b)),
        )
        .into_any_element()
}

fn render_block(idx: usize, block: &Block) -> gpui::AnyElement {
    match block {
        Block::Paragraph { runs, layout } => with_layout(paragraph(idx, runs, 14.0, None), layout),
        Block::Heading { level, runs, layout } => {
            with_layout(heading(idx, *level, runs), layout)
        }
        Block::ListItem { ordered, depth, content } => list_item(idx, *ordered, *depth, content),
        Block::Divider => divider(),
        Block::Image(src) => image_block(idx, src.as_ref()),
    }
}

fn heading(idx: usize, level: u8, inlines: &[Inline]) -> gpui::AnyElement {
    let size = match level {
        1 => 22.0,
        2 => 19.0,
        3 => 17.0,
        _ => 15.5,
    };
    paragraph(idx, inlines, size, Some(FontWeight::BOLD))
}

/// Wrap a block element with CSS margins (top / bottom / left).
/// Margins are clamped so a single misbehaving rule can't push content
/// off-screen.
fn with_layout(inner: gpui::AnyElement, layout: &BlockLayout) -> gpui::AnyElement {
    let mt = layout.margin_top_px.clamp(0.0, 80.0);
    let mb = layout.margin_bottom_px.clamp(0.0, 80.0);
    let ml = layout.margin_left_px.clamp(0.0, 80.0);
    if mt == 0.0 && mb == 0.0 && ml == 0.0 {
        return inner;
    }
    div()
        .w_full()
        .pt(px(mt))
        .pb(px(mb))
        .pl(px(ml))
        .child(inner)
        .into_any_element()
}

fn list_item(
    idx: usize,
    _ordered: bool,
    depth: u8,
    content: &[Inline],
) -> gpui::AnyElement {
    let indent = px(16.0 * depth as f32);
    h_flex()
        .w_full()
        .pl(indent)
        .gap(px(8.))
        .items_start()
        .child(
            div()
                .text_size(px(14.))
                .text_color(colors::text_secondary())
                .child("•"),
        )
        .child(paragraph(idx, content, 14.0, None))
        .into_any_element()
}

fn divider() -> gpui::AnyElement {
    div()
        .h(px(1.))
        .w_full()
        .bg(colors::border())
        .into_any_element()
}

/// Render one paragraph as a wrapping row of styled spans.
fn paragraph(
    idx: usize,
    inlines: &[Inline],
    base_size: f32,
    weight: Option<FontWeight>,
) -> gpui::AnyElement {
    // Single-run paragraphs (the common case for long body text) skip
    // the wrapping h_flex entirely; wrap in a w_full div so the text
    // knows its container width and can wrap properly!
    if inlines.len() == 1 {
        return div()
            .w_full()
            .child(render_run(idx, 0, &inlines[0], base_size, weight))
            .into_any_element();
    }

    let mut row = h_flex()
        .w_full()
        .flex_wrap()
        .gap_x(px(0.))
        .gap_y(px(2.));

    for (i, run) in inlines.iter().enumerate() {
        row = row.child(render_run(idx, i, run, base_size, weight));
    }

    row.into_any_element()
}

fn render_run(
    block_idx: usize,
    run_idx: usize,
    run: &Inline,
    base_size: f32,
    weight: Option<FontWeight>,
) -> gpui::AnyElement {
    if let Some(Link::Sound(path)) = &run.link {
        return sound_button(block_idx, run_idx, run.text.as_ref(), path.clone());
    }
    styled_span(
        block_idx,
        run_idx,
        run.text.clone(),
        &run.style,
        base_size,
        weight,
        run.link.as_ref(),
    )
}

fn styled_span(
    block_idx: usize,
    run_idx: usize,
    text: SharedString,
    style: &Style,
    base_size: f32,
    weight: Option<FontWeight>,
    link: Option<&Link>,
) -> gpui::AnyElement {
    let color = if let Some(c) = style.color.as_ref().and_then(|c| parse_color(c)) {
        c
    } else if matches!(link, Some(Link::Entry(_)) | Some(Link::External(_))) {
        colors::primary()
    } else {
        colors::text()
    };

    let bg = style.bg_color.as_ref().and_then(|c| parse_color(c));

    let font_weight = if style.bold || weight == Some(FontWeight::BOLD) {
        FontWeight::BOLD
    } else {
        weight.unwrap_or(FontWeight::NORMAL)
    };

    let size_px = style.font_size_px.unwrap_or(base_size);

    if let Some(Link::Entry(word)) = link {
        let target = word.clone();
        let mut el = div()
            .id(SharedString::from(format!("r-{block_idx}-{run_idx}")))
            .text_size(px(size_px))
            .font_weight(font_weight)
            .text_color(color)
            .cursor_pointer()
            .flex_shrink()
            .min_w(px(0.))
            .on_click(move |_, _, _cx| {
                debug!("entry link clicked: {target}");
            });
        if let Some(bg) = bg {
            el = el.bg(bg);
        }
        if style.italic {
            el = el.italic();
        }
        if style.underline {
            el = el.underline();
        }
        return el.child(text).into_any_element();
    }

    let mut el = div()
        .text_size(px(size_px))
        .font_weight(font_weight)
        .text_color(color)
        .flex_shrink()
        .min_w(px(0.));
    if let Some(bg) = bg {
        el = el.bg(bg);
    }
    if style.italic {
        el = el.italic();
    }
    if style.underline {
        el = el.underline();
    }
    el.child(text).into_any_element()
}

fn sound_button(
    block_idx: usize,
    run_idx: usize,
    label: &str,
    path: SharedString,
) -> gpui::AnyElement {
    let display: SharedString = if label.trim().is_empty() {
        SharedString::from("▶")
    } else {
        SharedString::from(format!("▶ {}", label.trim()))
    };

    div()
        .id(SharedString::from(format!("snd-{block_idx}-{run_idx}")))
        .px(px(8.))
        .py(px(2.))
        .mx(px(2.))
        .rounded(px(10.))
        .bg(colors::surface())
        .border_1()
        .border_color(colors::border())
        .text_size(px(13.))
        .text_color(colors::primary())
        .cursor_pointer()
        .hover(|s| s.bg(colors::border()))
        .child(display)
        .on_click(move |_, _, _cx| {
            audio::play_resource(path.as_ref());
        })
        .into_any_element()
}

fn image_block(idx: usize, src: &str) -> gpui::AnyElement {
    let bytes = match mdict_rs::query::lookup_resource(src) {
        Some(b) => b,
        None => {
            return div()
                .text_size(px(12.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(format!("[image: {src}]")))
                .into_any_element();
        }
    };

    let Some(path) = cache_image(src, &bytes) else {
        return div()
            .text_size(px(12.))
            .text_color(colors::text_secondary())
            .child(SharedString::from(format!("[image: {src}]")))
            .into_any_element();
    };

    // `img(SharedString)` routes through the embedded asset loader and
    // fails on absolute filesystem paths; passing a PathBuf hits the
    // `Resource::Path` branch instead and reads the file directly.
    img(path)
        .id(SharedString::from(format!("img-{idx}")))
        .max_w(px(360.))
        .into_any_element()
}

/// Write image bytes to a per-session tmp cache so gpui can load them
/// by path. The filename keys off a hash of the source path so repeat
/// references reuse the same file.
fn cache_image(src: &str, bytes: &[u8]) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    src.hash(&mut hasher);
    let hash = hasher.finish();

    let ext = std::path::Path::new(src)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("img");

    let mut dir = std::env::temp_dir();
    dir.push("mdict-rs-cache");
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("could not create image cache dir: {e}");
        return None;
    }

    let path = dir.join(format!("{hash:016x}.{ext}"));
    if !path.exists() {
        if let Err(e) = fs::write(&path, bytes) {
            warn!("could not write image cache: {e}");
            return None;
        }
    }
    Some(path)
}

/// Parse an HTML color value (named, `#rgb`, or `#rrggbb`) and remap
/// it for the app's dark theme: hue is preserved, lightness is pushed
/// up if the source color would be too dark to read on the background.
fn parse_color(s: &str) -> Option<gpui::Hsla> {
    let (r, g, b) = parse_rgb(s)?;
    Some(remap_for_dark_bg(r, g, b))
}

fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim();
    if let Some(rgb) = named_color(&s.to_lowercase()) {
        return Some(rgb);
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    match hex.len() {
        6 => Some((
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        )),
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some((r * 17, g * 17, b * 17))
        }
        _ => None,
    }
}

fn named_color(s: &str) -> Option<(u8, u8, u8)> {
    Some(match s {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "maroon" => (128, 0, 0),
        "navy" => (0, 0, 128),
        "olive" => (128, 128, 0),
        "purple" => (128, 0, 128),
        "teal" => (0, 128, 128),
        "lime" => (0, 255, 0),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        _ => return None,
    })
}

/// Remap a CSS color for a dark background. We lift the lightness so
/// it reads against the surface and cap saturation so primary colors
/// don't burn — pure red / magenta at 100% saturation are jarring on
/// dark themes.
fn remap_for_dark_bg(r: u8, g: u8, b: u8) -> gpui::Hsla {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let (h, mut s, mut l) = rgb_to_hsl(r, g, b);
    const MIN_L: f32 = 0.65;
    const MAX_S: f32 = 0.75;
    if l < MIN_L {
        l = MIN_L;
    }
    if s > MAX_S {
        s = MAX_S;
    }
    let (r, g, b) = hsl_to_rgb(h, s, l);
    let r = (r * 255.0).round().clamp(0.0, 255.0) as u32;
    let g = (g * 255.0).round().clamp(0.0, 255.0) as u32;
    let b = (b * 255.0).round().clamp(0.0, 255.0) as u32;
    gpui::rgb((r << 16) | (g << 8) | b).into()
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < 1e-6 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let h = h / 360.0;
    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}
