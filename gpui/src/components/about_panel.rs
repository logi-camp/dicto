use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px, img,
};
use gpui::prelude::FluentBuilder;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{h_flex, v_flex};

use crate::colors;

const VERSION: &str = env!("APP_VERSION");
const LICENSE: &str = env!("CARGO_PKG_LICENSE");
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const REPO_URL: &str = "https://github.com/logi-camp/dicto";

const KOFI_URL: &str = "https://ko-fi.com/mohamadkhani";

const CRYPTO_ADDRESSES: &[(&str, &str, &str)] = &[
    ("TRX", "USDT / TRX (TRC20)", "TCq1zpMfTRymMV1aREWqeAFu9dFnWcUzzM"),
    ("ETH", "USDT / ETH (ERC20)", "0xB4CA98CcdBe7c15E408ae9448DEB8B3F47df1A33"),
    ("BNB", "USDT / BNB (BEP20)", "0xB4CA98CcdBe7c15E408ae9448DEB8B3F47df1A33"),
    ("TON", "USDT / TON", "UQC9Jc961hV3KVwRX8ccfj2uZbSJD0kdMeWD3ciP-LFHIa6k"),
];

pub fn panel_content() -> gpui::AnyElement {
    // Inner scrollable: h_full + overflow_y_scrollbar, but this loses
    // flex_grow because Scrollable wraps it in a size_full() div.
    // Outer div with flex_1 + min_h(0) constrains the height so the
    // scrollbar actually activates when content overflows.
    let scroll = v_flex()
        .w_full()
        .h_full()
        .overflow_y_scrollbar()
        .items_center()
        .gap(px(0.))
        .child(hero_section())
        .child(div().py(px(8.)))
        .child(description_card())
        .child(div().py(px(6.)))
        .child(info_card())
        .child(div().py(px(8.)))
        .child(support_card())
        .child(div().py(px(16.)));

    div().flex_1().min_h(px(0.)).min_w(px(0.)).child(scroll).into_any_element()
}

fn hero_section() -> gpui::AnyElement {
    h_flex()
        .items_center()
        .gap(px(14.))
        .child(app_icon())
        .child(
            v_flex()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(colors::text())
                        .child("Dicto"),
                )
                .child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded(px(10.))
                        .bg(colors::surface())
                        .border_1()
                        .border_color(colors::border())
                        .text_size(px(11.))
                        .text_color(colors::text_secondary())
                        .child(SharedString::from(VERSION.to_string())),
                ),
        )
        .into_any_element()
}

fn app_icon() -> gpui::AnyElement {
    img("icons/app-icon.svg")
        .w(px(52.))
        .h(px(52.))
        .into_any_element()
}

fn description_card() -> gpui::AnyElement {
    div()
        .w(px(340.))
        .px(px(16.))
        .py(px(12.))
        .rounded(px(8.))
        .bg(colors::surface())
        .border_1()
        .border_color(colors::border())
        .child(
            div()
                .text_center()
                .text_size(px(12.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(
                    "A fast, offline desktop dictionary for Linux. \
                     Reads MDX/MDD dictionary files \u{2014} the same format \
                     used by GoldenDict, MDict, and most popular \
                     dictionary packs.",
                )),
        )
        .into_any_element()
}

fn support_card() -> gpui::AnyElement {
    v_flex()
        .w(px(340.))
        .rounded(px(8.))
        .bg(colors::surface())
        .border_1()
        .border_color(colors::border())
        .overflow_hidden()
        .child(
            // Header
            h_flex()
                .w_full()
                .px(px(14.))
                .py(px(10.))
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::text())
                        .child("\u{2665} Support Dicto"),
                ),
        )
        .child(
            div()
                .mx(px(14.))
                .h(px(1.))
                .bg(colors::border()),
        )
        .child(
            // Description
            div()
                .px(px(14.))
                .py(px(10.))
                .text_size(px(12.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(
                    "Dicto is free and open source. If it saves you time, \
                     consider supporting its development.",
                )),
        )
        .child(
            // Ko-fi button
            div()
                .px(px(14.))
                .pb(px(10.))
                .child(
                    div()
                        .id("about-kofi-btn")
                        .px(px(14.))
                        .py(px(7.))
                        .rounded(px(6.))
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors::bg())
                        .bg(colors::primary())
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.85))
                        .child("Buy me a coffee \u{2014} Ko-fi")
                        .on_click(move |_, _, cx| {
                            cx.open_url(KOFI_URL);
                        }),
                ),
        )
        .child(
            // Crypto section divider
            div()
                .mx(px(14.))
                .h(px(1.))
                .bg(colors::border()),
        )
        .child(
            // Crypto header
            div()
                .px(px(14.))
                .py(px(8.))
                .text_size(px(11.))
                .text_color(colors::text_secondary())
                .child("Or send cryptocurrency"),
        )
        .children(
            CRYPTO_ADDRESSES.iter().map(|(network, coins, address)| {
                crypto_row(network, coins, address)
            }),
        )
        .into_any_element()
}

fn crypto_row(network: &str, coins: &str, address: &str) -> gpui::AnyElement {
    let addr = address.to_string();
    v_flex()
        .w_full()
        .child(
            div()
                .mx(px(14.))
                .h(px(1.))
                .bg(colors::border()),
        )
        .child(
            h_flex()
                .w_full()
                .px(px(14.))
                .py(px(6.))
                .items_center()
                .justify_between()
                .child(
                    // Network + coins label
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(colors::text())
                                .child(SharedString::from(network.to_string())),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(colors::text_secondary())
                                .child(SharedString::from(coins.to_string())),
                        ),
                )
                .child(
                    // Address (truncated) + copy button
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(colors::text_secondary())
                                .child(SharedString::from(truncate_address(&addr))),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("copy-addr-{}", network)))
                                .text_size(px(10.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors::primary())
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.8))
                                .child("Copy")
                                .on_click({
                                    let addr = addr.clone();
                                    move |_, _, cx| {
                                        cx.write_to_clipboard(addr.clone().into());
                                    }
                                }),
                        ),
                ),
        )
        .into_any_element()
}

fn truncate_address(addr: &str) -> String {
    if addr.len() <= 16 {
        return addr.to_string();
    }
    format!("{}...{}", &addr[..8], &addr[addr.len() - 8..])
}

fn info_card() -> gpui::AnyElement {
    v_flex()
        .w(px(340.))
        .rounded(px(8.))
        .bg(colors::surface())
        .border_1()
        .border_color(colors::border())
        .overflow_hidden()
        .child(info_row("License", LICENSE, true))
        .child(info_row("Author", AUTHORS, true))
        .child(info_row("Engine", "GPUI (Zed)", true))
        .child(source_row())
        .into_any_element()
}

fn info_row(label: &str, value: &str, show_divider: bool) -> gpui::AnyElement {
    let row = h_flex()
        .w_full()
        .px(px(14.))
        .py(px(10.))
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(12.))
                .text_color(colors::text_secondary())
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text())
                .child(SharedString::from(value.to_string())),
        );

    v_flex()
        .w_full()
        .child(row)
        .when(show_divider, |el| {
            el.child(
                div()
                    .mx(px(14.))
                    .h(px(1.))
                    .bg(colors::border()),
            )
        })
        .into_any_element()
}

fn source_row() -> gpui::AnyElement {
    h_flex()
        .w_full()
        .px(px(14.))
        .py(px(10.))
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(12.))
                .text_color(colors::text_secondary())
                .child("Source"),
        )
        .child(
            div()
                .id("about-source-link")
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::primary())
                .cursor_pointer()
                .hover(|s| s.opacity(0.8))
                .child("github.com/logi-camp/dicto")
                .on_click(move |_, _, cx| {
                    cx.open_url(REPO_URL);
                }),
        )
        .into_any_element()
}
