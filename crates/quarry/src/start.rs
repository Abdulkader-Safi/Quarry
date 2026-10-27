//! 01 Start screen: rail with the ways in, samples and help; recent projects.

use crate::app::{Overlay, Quarry, ago};
use crate::theme::{Type, t, text};
use crate::ui::*;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;

impl Quarry {
    pub fn render_start(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        div()
            .flex()
            .size_full()
            .bg(p.bg_app)
            .child(self.start_rail(cx))
            .child(self.start_main(cx))
    }

    fn start_rail(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let row =
            |id: &'static str, icon_name: &'static str, label: &'static str, meta: &'static str| {
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .h(px(24.0))
                    .w_full()
                    .pr(px(8.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(t().bg_raised))
                    .child(icon(icon_name, 14.0, p.text_secondary))
                    .child(
                        div()
                            .flex_1()
                            .child(text(label, Type::BodyS, p.text_primary)),
                    )
                    .child(text(meta, Type::MonoS, p.text_muted))
            };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(24.0))
            .w(px(300.0))
            .h_full()
            .p(px(24.0))
            .bg(p.bg_panel)
            .child(div().h(px(12.0)))
            .child(logo_lockup())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        button("new", ButtonStyle::Primary, "New project", Some("plus"))
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| this.new_project(cx))),
                    )
                    .child(
                        button("open", ButtonStyle::Secondary, "Open…", Some("folder"))
                            .w_full()
                            .on_click(cx.listener(|this, _, _, cx| this.pick_and_open(cx))),
                    )
                    .child(
                        button(
                            "import",
                            ButtonStyle::Secondary,
                            "Import .glb or .gltf",
                            Some("file"),
                        )
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| this.pick_and_import(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(text("Samples", Type::Caption, p.text_muted))
                    .child(
                        row("sample-robot", "bone", "Sample robot", "rigged")
                            .on_click(cx.listener(|this, _, _, cx| this.open_sample("robot", cx))),
                    )
                    .child(
                        row("sample-tiles", "tiles", "Sample tileset 16 px", "CC0")
                            .on_click(cx.listener(|this, _, _, cx| this.open_sample("tiles", cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(text("Help", Type::Caption, p.text_muted))
                    .child(
                        row("help-basics", "mouse", "Basics: camera and placement", "").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.overlay = Overlay::Basics;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        row("help-connect", "plug", "Connect Claude", "").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.overlay = Overlay::Connect;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        row("help-keys", "keyboard", "Keyboard shortcuts", "").on_click(
                            cx.listener(|this, _, _, cx| {
                                this.settings_page = 3;
                                this.overlay = Overlay::Settings;
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(div().flex_1())
            .child(text(
                format!(
                    "v{}  ·  MIT  ·  {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS.replace("macos", "macOS")
                ),
                Type::MonoS,
                p.text_muted,
            ))
    }

    fn start_main(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let filter = self.recent_filter.read(cx).value().to_lowercase();
        let recents: Vec<_> = self
            .settings
            .recent_with_status()
            .into_iter()
            .filter(|(r, exists)| {
                *exists && r.path.to_string_lossy().to_lowercase().contains(&filter)
            })
            .map(|(r, _)| r.clone())
            .take(10)
            .collect();

        let mut main = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(24.0))
            .p(px(48.0))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(text(greeting(), Type::DisplayL, p.text_primary))
                    .child(text(
                        "Pick up where you left off, or start a new asset for Cairn.",
                        Type::BodyM,
                        p.text_secondary,
                    )),
            );

        if let Some(r) = &self.recovery {
            let name = r
                .sidecar
                .project
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| r.sidecar.name.clone());
            let when = crate::app::clock(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(r.sidecar.saved_at),
            );
            main = main.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .w_full()
                    .px(px(16.0))
                    .py(px(10.0))
                    .bg(p.warning_subtle)
                    .rounded(px(6.0))
                    .child(icon("warning", 16.0, p.warning))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.0))
                            .child(text("Quarry closed unexpectedly", Type::H3, p.text_primary))
                            .child(text(
                                format!(
                                    "An autosave of {name} from {} is ready. Your own file was not touched.",
                                    &when[..5]
                                ),
                                Type::BodyS,
                                p.text_secondary,
                            )),
                    )
                    .child(
                        button("discard", ButtonStyle::Ghost, "Discard", None)
                            .on_click(cx.listener(|this, _, _, cx| this.discard_recovery(cx))),
                    )
                    .child(
                        button("recover", ButtonStyle::Primary, "Recover", Some("refresh"))
                            .on_click(cx.listener(|this, _, _, cx| this.recover(cx))),
                    ),
            );
        }

        main = main.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .w_full()
                .child(text("Recent projects", Type::Caption, p.text_muted))
                .child(div().flex_1())
                .child(
                    search_field(
                        Input::new(&self.recent_filter)
                            .appearance(false)
                            .into_any_element(),
                        "⌘K",
                    )
                    .w(px(240.0)),
                ),
        );

        if recents.is_empty() {
            main = main.child(note_box(
                Tone::Neutral,
                "No recent projects yet. Start a new one, open a .quarry file, or try the sample robot.",
            ));
        } else {
            let mut grid = div().flex().flex_wrap().gap(px(20.0)).w_full();
            for (i, r) in recents.iter().enumerate() {
                let path = r.path.clone();
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().replace(['_', '-'], " "))
                    .unwrap_or_default();
                let shown = tilde(&path);
                let thumb = self.thumbs.get(&path).cloned();
                let open = path.clone();
                grid = grid.child(
                    div()
                        .id(("recent", i))
                        .flex()
                        .flex_col()
                        .w(px(372.0))
                        .h(px(249.0))
                        .bg(p.bg_panel)
                        .border_1()
                        .border_color(p.border_subtle)
                        .rounded(px(12.0))
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(|s| s.border_color(t().border_default))
                        .on_click(cx.listener(move |this, _, _, cx| this.open(open.clone(), cx)))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .bg(rgb(0x121316))
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .children(thumb.map(|img_data| {
                                    img(img_data).size_full().object_fit(ObjectFit::Cover)
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .pt(px(12.0))
                                .pb(px(14.0))
                                .px(px(12.0))
                                .child(text(capitalise_first(&name), Type::H3, p.text_primary))
                                .child(text(shown, Type::MonoS, p.text_muted))
                                .child(text(ago(r.opened_at), Type::BodyS, p.text_muted)),
                        ),
                );
            }
            main = main.child(grid);
        }
        main
    }
}

fn greeting() -> String {
    let hour: i64 = crate::app::chrono_like_now()[..2].parse().unwrap_or(12);
    let part = match hour {
        5..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    };
    match std::env::var("USER").ok().filter(|u| !u.is_empty()) {
        Some(user) => format!("{part}, {}", capitalise_first(&user)),
        None => part.to_string(),
    }
}

fn capitalise_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Paths under the home folder shown with `~`.
pub fn tilde(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if s.starts_with(&home) => format!("~{}", &s[home.len()..]),
        _ => s,
    }
}
