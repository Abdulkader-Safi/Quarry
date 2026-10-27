//! The Components page of the Quarry Figma file as GPUI building blocks.
//! Each returns an element; callers attach `.on_click` and other listeners.

use crate::assets::icon_path;
use crate::theme::{Type, modal_shadow, popover_shadow, t, text};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub fn icon(name: &str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(icon_path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// The Quarry glyph (stone cube, cobalt sledgehammer) at `size`.
pub fn logo_glyph(size: f32) -> Img {
    img("quarry/logo-glyph.svg")
        .w(px(size))
        .h(px(size * 33.0 / 32.0))
        .flex_none()
}

/// Logo/Lockup: glyph plus the lowercase wordmark.
pub fn logo_lockup() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.88))
        .child(logo_glyph(27.52))
        .child(
            div()
                .font_family(crate::theme::DISPLAY)
                .font_weight(FontWeight::BOLD)
                .text_size(px(24.08))
                .line_height(px(27.52))
                .text_color(t().text_primary)
                .child("quarry"),
        )
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonStyle {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Height 28, radius 6, Label/M. Primary is cobalt, one per bar.
pub fn button(
    id: impl Into<ElementId>,
    style: ButtonStyle,
    label: impl Into<SharedString>,
    icon_name: Option<&str>,
) -> Stateful<Div> {
    let p = t();
    let (bg, fg, border, hover) = match style {
        ButtonStyle::Primary => (Some(p.accent), p.text_on_accent, None, p.accent_hover),
        ButtonStyle::Secondary => (
            Some(p.bg_raised),
            p.text_primary,
            Some(p.border_default),
            p.bg_hover,
        ),
        ButtonStyle::Ghost => (None, p.text_secondary, None, p.bg_hover),
        ButtonStyle::Danger => (Some(p.danger_subtle), p.danger, None, p.bg_hover),
    };
    let mut b = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .h(px(28.0))
        .min_w(px(80.0))
        .px(px(12.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(move |s| s.bg(hover));
    if let Some(bg) = bg {
        b = b.bg(bg);
    }
    if let Some(border) = border {
        b = b.border_1().border_color(border);
    }
    if let Some(name) = icon_name {
        b = b.child(icon(name, 16.0, fg));
    }
    b.child(text(label, Type::LabelM, fg))
}

/// 28 square, 16px icon. Toggled marks the active tool. Every one has a tooltip.
pub fn icon_button(
    id: impl Into<ElementId>,
    icon_name: &str,
    toggled: bool,
    tip: impl Into<SharedString>,
    key: impl Into<SharedString>,
) -> Stateful<Div> {
    let p = t();
    let (tip, key) = (tip.into(), key.into());
    let mut b = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded(px(6.0))
        .cursor_pointer()
        .child(icon(
            icon_name,
            16.0,
            if toggled {
                p.text_primary
            } else {
                p.text_secondary
            },
        ))
        .tooltip(move |_, cx| tooltip_view(tip.clone(), key.clone(), cx));
    if toggled {
        b = b.bg(p.accent_strong_subtle);
    } else {
        b = b.hover(|s| s.bg(t().bg_hover));
    }
    b
}

struct Tip(SharedString, SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        tooltip(self.0.clone(), self.1.clone())
    }
}

fn tooltip_view(text: SharedString, key: SharedString, cx: &mut App) -> AnyView {
    cx.new(|_| Tip(text, key)).into()
}

/// Tooltip: bg/raised, border, Body/S text and the shortcut in Mono/S.
pub fn tooltip(label: impl Into<SharedString>, key: impl Into<SharedString>) -> Div {
    let p = t();
    let key: SharedString = key.into();
    div()
        .flex()
        .gap(px(8.0))
        .px(px(8.0))
        .py(px(4.0))
        .bg(p.bg_raised)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .shadow(popover_shadow())
        .child(text(label, Type::BodyS, p.text_primary))
        .when(!key.is_empty(), |d| {
            d.child(text(key, Type::MonoS, p.text_muted))
        })
}

/// Tab: height 32, top corners 6; active joins the panel and has a cobalt line.
pub fn tab(
    id: impl Into<ElementId>,
    icon_name: &str,
    label: &str,
    active: bool,
    pad: f32,
) -> Stateful<Div> {
    let p = t();
    let fg = if active {
        p.text_primary
    } else {
        p.text_secondary
    };
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(32.0))
        .px(px(pad))
        .rounded_t(px(6.0))
        .cursor_pointer()
        .when(active, |d| d.bg(p.bg_panel))
        .when(!active, |d| d.hover(|s| s.bg(t().bg_raised)))
        .child(icon(icon_name, 16.0, if active { p.accent } else { fg }))
        .child(text(label.to_string(), Type::LabelM, fg))
        .when(active, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(2.0))
                    .bg(p.accent),
            )
        })
}

/// The strip tabs sit in: bg/app, tabs bottom-aligned.
pub fn tab_strip() -> Div {
    div()
        .flex()
        .flex_none()
        .items_end()
        .gap(px(2.0))
        .pl(px(4.0))
        .pt(px(4.0))
        .w_full()
        .bg(t().bg_app)
}

/// Search field: 14px search icon, placeholder or the live input, shortcut hint.
pub fn search_field(content: AnyElement, shortcut: &str) -> Div {
    let p = t();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        .px(px(8.0))
        .bg(p.bg_input)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .child(icon("search", 14.0, p.text_secondary))
        .child(div().flex_1().min_w_0().overflow_hidden().child(content))
        .when(!shortcut.is_empty(), |d| {
            d.child(text(shortcut.to_string(), Type::MonoS, p.text_muted))
        })
}

/// Text field shell around an input: height 24, bg/input, radius 4.
pub fn text_field(content: impl IntoElement) -> Div {
    let p = t();
    div()
        .flex()
        .items_center()
        .h(px(24.0))
        .px(px(8.0))
        .bg(p.bg_input)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .hover(|s| s.border_color(t().border_strong))
        .child(div().flex_1().min_w_0().child(content))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FieldAxis {
    X,
    Y,
    Z,
}

/// Number field: value in Mono/M; an axis tab in the axis colour for X, Y, Z.
/// Drag sideways to change (the caller handles the drag).
pub fn number_field(
    id: impl Into<ElementId>,
    value: String,
    axis: Option<FieldAxis>,
) -> Stateful<Div> {
    let p = t();
    let mut d = div()
        .id(id)
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        .bg(p.bg_input)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .overflow_hidden()
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|s| s.border_color(t().border_strong));
    match axis {
        Some(axis) => {
            let (color, letter) = match axis {
                FieldAxis::X => (p.axis_x, "X"),
                FieldAxis::Y => (p.axis_y, "Y"),
                FieldAxis::Z => (p.axis_z, "Z"),
            };
            d = d.pr(px(8.0)).child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(18.0))
                    .h_full()
                    .bg(color)
                    .child(text(letter, Type::LabelS, p.text_on_axis)),
            );
        }
        None => d = d.px(px(8.0)),
    }
    d.child(text(value, Type::MonoM, p.text_primary))
}

/// Select (closed): bg/raised, chevron on the right. The list opens as a Menu.
pub fn select(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(24.0))
        .px(px(8.0))
        .bg(p.bg_raised)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.border_color(t().border_strong))
        .child(div().flex_1().min_w_0().overflow_hidden().child(text(
            value,
            Type::BodyS,
            p.text_primary,
        )))
        .child(icon("chevron-down", 12.0, p.text_secondary))
}

/// Menu: the floating list a Select opens, and the menu bar's menus.
pub fn menu(width: f32) -> Div {
    let p = t();
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .p(px(4.0))
        .bg(p.bg_raised)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(8.0))
        .shadow(popover_shadow())
}

/// Menu row: 26 high, label and an optional shortcut.
pub fn menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    key: &str,
    enabled: bool,
) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(26.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|s| s.bg(t().accent_subtle))
        })
        .child(div().flex_1().child(text(
            label,
            Type::BodyS,
            if enabled {
                p.text_primary
            } else {
                p.text_muted
            },
        )))
        .when(!key.is_empty(), |d| {
            d.child(text(key.to_string(), Type::MonoS, p.text_muted))
        })
}

/// Checkbox: 16 square, radius 3. On is cobalt with a white check.
pub fn checkbox(id: impl Into<ElementId>, checked: bool, label: &str) -> Stateful<Div> {
    let p = t();
    let mut boxed = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.0))
        .rounded(px(3.0));
    boxed = if checked {
        boxed
            .bg(p.accent)
            .child(icon("check", 12.0, p.text_on_accent))
    } else {
        boxed
            .bg(p.bg_input)
            .border_1()
            .border_color(p.border_strong)
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .child(boxed)
        .when(!label.is_empty(), |d| {
            d.child(text(label.to_string(), Type::BodyS, p.text_secondary))
        })
}

/// Switch: 28 by 16. Off: bg/active track; on: cobalt track, white knob right.
pub fn switch(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .w(px(28.0))
        .h(px(16.0))
        .px(px(2.0))
        .rounded_full()
        .cursor_pointer()
        .bg(if on { p.accent } else { p.bg_active })
        .when(on, |d| d.justify_end())
        .child(div().size(px(12.0)).rounded_full().bg(if on {
            p.text_on_accent
        } else {
            p.text_muted
        }))
}

/// Slider: 4px track, cobalt fill to a 14px knob, value in Mono/S.
pub fn slider(id: impl Into<ElementId>, fraction: f32, value: String) -> Stateful<Div> {
    let p = t();
    let f = fraction.clamp(0.0, 1.0);
    div()
        .id(id)
        .flex()
        .flex_1()
        .items_center()
        .gap(px(10.0))
        .h(px(24.0))
        .cursor_pointer()
        .child(
            div()
                .relative()
                .flex_1()
                .h(px(14.0))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(5.0))
                        .h(px(4.0))
                        .rounded(px(2.0))
                        .bg(p.bg_active),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(5.0))
                        .h(px(4.0))
                        .w(relative(f))
                        .rounded(px(2.0))
                        .bg(p.accent),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left(relative(f))
                        .ml(px(-7.0))
                        .size(px(14.0))
                        .rounded_full()
                        .bg(rgb(0xffffff)),
                ),
        )
        .child(text(value, Type::MonoS, p.text_secondary))
}

/// Segmented control: bg/raised well, the active segment on bg/panel (or
/// accent/strong-subtle when floating over the viewport).
pub fn segmented(
    id: &str,
    options: &[&str],
    active: usize,
    fill: bool,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let p = t();
    let on_pick = std::rc::Rc::new(on_pick);
    let mut d = div()
        .flex()
        .gap(px(2.0))
        .p(px(if fill { 3.0 } else { 4.0 }))
        .rounded(px(8.0));
    d = if fill {
        d.w_full().bg(p.bg_raised)
    } else {
        d.bg(p.bg_panel)
            .border_1()
            .border_color(p.border_default)
            .shadow(popover_shadow())
    };
    for (i, label) in options.iter().enumerate() {
        let on = i == active;
        let pick = on_pick.clone();
        let seg = div()
            .id(SharedString::from(format!("{id}-{i}")))
            .flex()
            .justify_center()
            .px(px(10.0))
            .py(px(if fill { 6.0 } else { 5.0 }))
            .rounded(px(6.0))
            .cursor_pointer()
            .when(fill, |d| d.flex_1())
            .when(on, |d| {
                d.bg(if fill {
                    p.bg_panel
                } else {
                    p.accent_strong_subtle
                })
            })
            .when(!on, |d| d.hover(|s| s.bg(t().bg_hover)))
            .on_click(move |_, w, cx| pick(i, w, cx))
            .child(text(
                label.to_string(),
                Type::LabelM,
                if on { p.text_primary } else { p.text_secondary },
            ));
        d = d.child(seg);
    }
    d
}

/// Tree row: 24 high, 16 per indent level. Selected gets accent/subtle, a 2px
/// cobalt line and a cobalt icon. Meta in Mono/S.
#[allow(clippy::too_many_arguments)]
pub fn tree_row(
    id: impl Into<ElementId>,
    depth: usize,
    chevron: Option<bool>,
    icon_name: &str,
    label: impl Into<SharedString>,
    meta: impl Into<SharedString>,
    selected: bool,
    hidden: bool,
) -> Stateful<Div> {
    let p = t();
    let label_color = if hidden { p.text_muted } else { p.text_primary };
    let pad = 8.0 + 16.0 * depth as f32;
    let mut row = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(4.0))
        .h(px(24.0))
        .w_full()
        .pl(px(pad))
        .pr(px(8.0))
        .cursor_pointer();
    if let Some(open) = chevron {
        row = row.child(icon(
            if open {
                "chevron-down"
            } else {
                "chevron-right"
            },
            12.0,
            p.text_secondary,
        ));
    }
    row = row
        .child(icon(
            icon_name,
            14.0,
            if selected { p.accent } else { p.text_secondary },
        ))
        .child(div().flex_1().min_w_0().overflow_hidden().child(text(
            label,
            Type::BodyS,
            label_color,
        )))
        .child(text(meta, Type::MonoS, p.text_muted));
    if hidden {
        row = row.child(icon("eye-off", 14.0, p.text_muted));
    }
    if selected {
        row.bg(p.accent_subtle).child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.0))
                .bg(p.accent),
        )
    } else {
        row.hover(|s| s.bg(t().bg_raised))
    }
}

/// Section header: bg/raised, radius 6, cobalt chevron, layers icon, H3, more.
pub fn section_header(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    open: bool,
) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(32.0))
        .w_full()
        .pl(px(8.0))
        .pr(px(4.0))
        .bg(p.bg_raised)
        .rounded(px(6.0))
        .cursor_pointer()
        .child(icon(
            if open {
                "chevron-down"
            } else {
                "chevron-right"
            },
            12.0,
            p.accent,
        ))
        .child(icon("layers", 14.0, p.text_secondary))
        .child(div().flex_1().child(text(title, Type::H3, p.text_primary)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.0))
                .child(icon("more", 16.0, p.text_secondary)),
        )
}

/// Property row: label column 76 wide, then the controls, gap 4.
pub fn property_row(label: &str, controls: impl IntoIterator<Item = AnyElement>) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h(px(24.0))
        .w_full()
        .child(div().w(px(76.0)).flex_none().child(text(
            label.to_string(),
            Type::BodyS,
            t().text_secondary,
        )))
        .children(controls)
}

/// Caption header with an optional count on the right ("SKELETON  13 bones").
pub fn caption_row(label: &str, meta: impl Into<SharedString>) -> Div {
    let p = t();
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .w_full()
        .child(
            div()
                .flex_1()
                .child(text(label.to_string(), Type::Caption, p.text_muted)),
        )
        .child(text(meta, Type::MonoS, p.text_muted))
}

/// Badge: height 20, radius 4, a 6px dot and Label/S.
pub fn badge(label: impl Into<SharedString>, dot: Rgba) -> Div {
    let p = t();
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(20.0))
        .px(px(8.0))
        .bg(p.bg_raised)
        .rounded(px(4.0))
        .child(div().size(px(6.0)).rounded_full().bg(dot))
        .child(text(label, Type::LabelS, p.text_secondary))
}

/// Chip: a filter with a count.
pub fn chip(id: impl Into<ElementId>, label: &str, count: usize, toggled: bool) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .h(px(20.0))
        .px(px(8.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .bg(if toggled {
            p.accent_subtle
        } else {
            p.bg_raised
        })
        .child(text(
            label.to_string(),
            Type::LabelS,
            if toggled {
                p.text_primary
            } else {
                p.text_secondary
            },
        ))
        .child(text(count.to_string(), Type::MonoS, p.text_muted))
}

#[derive(Clone, Copy, PartialEq)]
pub enum AgentState {
    Connected,
    Working,
    Off,
}

/// Agent pill: the MCP server state in the top bar.
pub fn agent_pill(state: AgentState) -> Div {
    let p = t();
    let (label, color, border) = match state {
        AgentState::Connected => ("Agent connected", p.success, p.border_subtle),
        AgentState::Working => ("Agent working", p.agent, p.agent),
        AgentState::Off => ("Agent off", p.text_muted, p.border_subtle),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .py(px(5.0))
        .bg(p.bg_panel)
        .border_1()
        .border_color(border)
        .rounded_full()
        .child(icon("plug", 14.0, color))
        .child(text(label, Type::LabelS, p.text_secondary))
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Neutral,
    Agent,
    Warning,
}

/// Note box: says what a panel does, or which agent command runs behind it.
pub fn note_box(tone: Tone, body: impl Into<SharedString>) -> Div {
    let p = t();
    let (bg, icon_name, icon_color) = match tone {
        Tone::Neutral => (p.bg_raised, "info", p.text_secondary),
        Tone::Agent => (p.info_subtle, "plug", p.agent),
        Tone::Warning => (p.warning_subtle, "info", p.warning),
    };
    div()
        .flex()
        .flex_none()
        .items_start()
        .gap(px(8.0))
        .w_full()
        .px(px(12.0))
        .py(px(10.0))
        .bg(bg)
        .rounded(px(6.0))
        .child(icon(icon_name, 16.0, icon_color))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .font_family(crate::theme::SANS)
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(p.text_secondary)
                .child(body.into()),
        )
}

/// Keyframe diamond. Agent marks keys the agent set (violet).
pub fn keyframe(state: KeyState) -> Div {
    let p = t();
    let (fill, ring) = match state {
        KeyState::Default => (p.text_secondary, None),
        KeyState::Selected => (p.accent, Some(p.text_primary)),
        KeyState::Agent => (p.agent, None),
    };
    let mut d = div().relative().size(px(12.0)).flex_none();
    if let Some(ring) = ring {
        d = d.child(
            div()
                .absolute()
                .inset_0()
                .child(icon("diamond", 12.0, ring)),
        );
        d = d.child(
            div()
                .absolute()
                .top(px(2.0))
                .left(px(2.0))
                .child(icon("diamond", 8.0, fill)),
        );
    } else {
        d = d.child(
            div()
                .absolute()
                .top(px(1.0))
                .left(px(1.0))
                .child(icon("diamond", 10.0, fill)),
        );
    }
    d
}

#[derive(Clone, Copy, PartialEq)]
pub enum KeyState {
    Default,
    Selected,
    Agent,
}

/// History row: one undo step; agent rows carry a violet plug.
pub fn history_row(
    id: impl Into<ElementId>,
    time: String,
    command: String,
    agent: bool,
    current: bool,
) -> Stateful<Div> {
    let p = t();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .h(px(24.0))
        .w_full()
        .px(px(8.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .when(current, |d| d.bg(p.accent_subtle))
        .when(!current, |d| d.hover(|s| s.bg(t().bg_raised)))
        .child(text(time, Type::MonoS, p.text_muted))
        .child(icon(
            if agent { "plug" } else { "pointer" },
            14.0,
            if agent { p.agent } else { p.text_secondary },
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(text(command, Type::MonoS, p.text_primary)),
        )
        .child(text(
            if agent { "agent" } else { "you" },
            Type::LabelS,
            if agent { p.agent } else { p.text_secondary },
        ))
}

/// Dialog shell: header with title, mono subtitle and close; body; footer.
pub fn dialog(
    width: f32,
    title: &str,
    subtitle: impl Into<SharedString>,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    body: impl IntoElement,
    footer: impl IntoElement,
) -> Div {
    let p = t();
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .bg(p.bg_panel)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(16.0))
        .shadow(modal_shadow())
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.0))
                .pt(px(20.0))
                .pb(px(16.0))
                .pl(px(24.0))
                .pr(px(16.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(4.0))
                        .child(text(title.to_string(), Type::H1, p.text_primary))
                        .child(text(subtitle, Type::MonoS, p.text_muted)),
                )
                .child(
                    icon_button("dialog-close", "close", false, "Close", "esc").on_click(on_close),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .px(px(24.0))
                .pb(px(20.0))
                .child(body),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(24.0))
                .py(px(12.0))
                .border_t_1()
                .border_color(p.border_subtle)
                .child(footer),
        )
}

/// A dimmed scrim that centres its child (dialogs and the command palette).
pub fn scrim(child: impl IntoElement) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(hsla(0.0, 0.0, 0.0, 0.6))
        .child(child)
}

/// Floating bar over the viewport: bg/panel, border, radius 8, popover shadow.
pub fn floating_bar() -> Div {
    let p = t();
    div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(12.0))
        .py(px(8.0))
        .bg(p.bg_panel)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(8.0))
        .shadow(popover_shadow())
}

pub fn divider_v(height: f32) -> Div {
    div()
        .w(px(1.0))
        .h(px(height))
        .flex_none()
        .bg(t().border_default)
}

/// Kbd hint: a key in a small box.
pub fn kbd(key: &str) -> Div {
    let p = t();
    div()
        .px(px(6.0))
        .py(px(1.0))
        .bg(p.bg_panel)
        .border_1()
        .border_color(p.border_default)
        .rounded(px(4.0))
        .child(text(key.to_string(), Type::MonoS, p.text_secondary))
}
