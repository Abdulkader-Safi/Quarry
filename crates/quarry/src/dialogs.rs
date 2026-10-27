//! 09 Command search, 10 Export, 11 Settings, 12 Connect Claude, 13 The
//! basics, and the 14 live refresh toast.

use crate::app::{Field, Overlay, Quarry, command_icon, command_title};
use crate::theme::{Type, t, text};
use crate::ui::*;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use quarry_core::model::ImageRole;
use serde_json::json;

impl Quarry {
    pub fn render_overlay(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let close = cx.listener(|this, _: &ClickEvent, _, cx| {
            this.overlay = Overlay::None;
            cx.notify();
        });
        let body = match self.overlay {
            Overlay::None => return None,
            Overlay::Commands => self.palette(cx).into_any_element(),
            Overlay::Export => self.export_dialog(close, cx).into_any_element(),
            Overlay::Settings => self.settings_dialog(close, cx).into_any_element(),
            Overlay::Connect => self.connect_dialog(close, cx).into_any_element(),
            Overlay::Basics => self.basics_dialog(close, cx).into_any_element(),
        };
        // Clicks on the scrim close; clicks inside the dialog stop there.
        Some(
            scrim(
                div()
                    .id("dialog")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(body),
            )
            .id("scrim")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.overlay = Overlay::None;
                    cx.notify();
                }),
            )
            .into_any_element(),
        )
    }

    fn palette(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let matches = self.palette_matches(cx);
        let area = matches
            .first()
            .map(|(n, _)| n.split('.').next().unwrap_or("").to_string())
            .unwrap_or_default();
        let mut results = div().flex().flex_col().p(px(6.0));
        for (i, (name, _desc)) in matches.iter().enumerate() {
            let on = i == self.palette_index;
            let name = *name;
            results = results.child(
                div()
                    .id(("cmd", i))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(34.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.accent_subtle))
                    .hover(|s| s.bg(t().accent_subtle))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.overlay = Overlay::None;
                        this.run_named(name, cx);
                    }))
                    .child(icon(
                        command_icon(name),
                        16.0,
                        if on { p.accent } else { p.text_secondary },
                    ))
                    .child(div().w(px(220.0)).flex_none().child(text(
                        command_title(name),
                        Type::BodyM,
                        p.text_primary,
                    )))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(text(name, Type::MonoS, p.text_muted)),
                    )
                    .child(text(
                        if crate::keys::ACTIONS.iter().any(|a| a.0 == name) {
                            crate::keys::display(crate::keys::bound(&self.settings.shortcuts, name))
                        } else {
                            String::new()
                        },
                        Type::MonoS,
                        p.text_secondary,
                    )),
            );
        }
        if matches.is_empty() {
            results = results.child(div().p(px(10.0)).child(text(
                "No command matches. Try a verb: extrude, unwrap, key.",
                Type::BodyS,
                p.text_muted,
            )));
        }
        div()
            .flex()
            .flex_col()
            .w(px(640.0))
            .bg(p.bg_panel)
            .border_1()
            .border_color(p.border_default)
            .rounded(px(12.0))
            .shadow(crate::theme::modal_shadow())
            .overflow_hidden()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                let n = this.palette_matches(cx).len().max(1);
                match e.keystroke.key.as_str() {
                    "down" => this.palette_index = (this.palette_index + 1) % n,
                    "up" => this.palette_index = (this.palette_index + n - 1) % n,
                    _ => return,
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(16.0))
                    .py(px(14.0))
                    .border_b_1()
                    .border_color(p.border_subtle)
                    .child(icon("search", 16.0, p.text_secondary))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(16.0))
                            .child(Input::new(&self.palette_query).appearance(false)),
                    )
                    .when(!area.is_empty(), |d| {
                        d.child(badge(capital(&area), p.success))
                    }),
            )
            .child(results)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .bg(p.bg_raised)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(kbd("↑↓"))
                            .child(text("move", Type::BodyS, p.text_muted)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(kbd("↵"))
                            .child(text("run", Type::BodyS, p.text_muted)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(kbd("esc"))
                            .child(text("close", Type::BodyS, p.text_muted)),
                    )
                    .child(div().flex_1())
                    .child(text(
                        "Every action here is also an MCP tool",
                        Type::BodyS,
                        p.text_muted,
                    )),
            )
    }

    fn export_dialog(
        &mut self,
        close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = t();
        let doc = self.reg.document();
        let file = self.default_export_path();
        let file_name = file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let meshes: Vec<_> = doc
            .objects
            .iter()
            .filter_map(|o| doc.drawn_mesh(o))
            .collect();
        let tris: usize = meshes.iter().map(|m| m.triangle_count()).sum();
        let textures = doc
            .images
            .iter()
            .filter(|i| i.role == ImageRole::Texture)
            .count()
            + doc
                .images
                .iter()
                .filter(|i| i.role == ImageRole::Tileset)
                .count();
        let bones: usize = doc.skeletons.iter().map(|s| s.bones.len()).sum();
        let clips: Vec<String> = doc.clips.iter().map(|c| c.name.clone()).collect();
        let markers = doc
            .objects
            .iter()
            .filter(|o| o.marker_type.is_some())
            .count();
        let collision = doc.objects.iter().filter(|o| o.collision).count();
        let props: usize = doc.objects.iter().map(|o| o.properties.len()).sum();
        let sel = doc.selection.objects.len();
        let fps = doc.settings.fps;
        let filter = match doc.settings.filter {
            quarry_core::model::Filter::Nearest => "nearest",
            quarry_core::model::Filter::Linear => "linear",
        };
        let entity = cx.entity();
        let scale = self.field_value(Field::ExportScale, self.export_scale);
        let summary_row = |label: &str, value: String, ok: bool| {
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(icon(
                    if ok { "check" } else { "close" },
                    14.0,
                    if ok { p.success } else { p.text_muted },
                ))
                .child(div().w(px(80.0)).child(text(
                    label.to_string(),
                    Type::BodyS,
                    p.text_secondary,
                )))
                .child(
                    div()
                        .flex_1()
                        .child(text(value, Type::MonoS, p.text_primary)),
                )
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(text("What", Type::Caption, p.text_muted))
            .child(segmented(
                "what",
                &["Whole scene", &format!("Selected objects ({sel})")],
                self.export_selected as usize,
                true,
                move |i, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.export_selected = i == 1;
                        cx.notify();
                    })
                },
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(110.0))
                            .child(text("File", Type::BodyS, p.text_secondary)),
                    )
                    .child(text_field(Input::new(&self.export_path).appearance(false)).flex_1())
                    .child(
                        button("browse", ButtonStyle::Secondary, "Browse", None)
                            .on_click(cx.listener(|this, _, _, cx| this.browse_export(cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(110.0))
                            .child(text("Preset", Type::BodyS, p.text_secondary)),
                    )
                    .child(select("preset", "Cairn default").flex_1()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(110.0))
                            .child(text("Scale", Type::BodyS, p.text_secondary)),
                    )
                    .child(
                        number_field("export-scale", format!("{scale:.2}"), None).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, e, _, cx| {
                                this.begin_field_drag(Field::ExportScale, e, cx)
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().w(px(110.0)).child(text(
                        "Frame rate",
                        Type::BodyS,
                        p.text_secondary,
                    )))
                    .child(select("export-fps", format!("{fps} fps (project)")).flex_1()),
            )
            .child(text("In the file", Type::Caption, p.text_muted))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(12.0))
                    .bg(p.bg_input)
                    .border_1()
                    .border_color(p.border_subtle)
                    .rounded(px(6.0))
                    .child(summary_row(
                        "Meshes",
                        format!(
                            "{}  ·  {} tris  ·  shared vertices",
                            meshes.len(),
                            crate::editor::thousands(tris)
                        ),
                        !meshes.is_empty(),
                    ))
                    .child(summary_row(
                        "Textures",
                        format!("{textures} PNG  ·  {filter} sampler"),
                        textures > 0,
                    ))
                    .child(summary_row(
                        "Skeleton",
                        format!("{bones} bones  ·  max 4 influences"),
                        bones > 0,
                    ))
                    .child(summary_row(
                        "Clips",
                        if clips.is_empty() {
                            "none".into()
                        } else {
                            format!("{}  ·  sampled to LINEAR at {fps} fps", clips.join(" "))
                        },
                        !clips.is_empty(),
                    ))
                    .child(summary_row(
                        "Extras",
                        format!(
                            "{markers} markers  ·  {} collision  ·  {props} custom props",
                            collision
                        ),
                        markers + collision + props > 0,
                    )),
            );
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().size(px(6.0)).rounded_full().bg(if meshes.is_empty() {
                        p.warning
                    } else {
                        p.success
                    }))
                    .child(text(
                        if meshes.is_empty() {
                            "Nothing to export yet"
                        } else {
                            "Passes the Cairn importer checks"
                        },
                        Type::BodyS,
                        p.text_muted,
                    )),
            )
            .child(
                button("cancel", ButtonStyle::Secondary, "Cancel", None).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overlay = Overlay::None;
                        cx.notify();
                    },
                )),
            )
            .child(
                button("do-export", ButtonStyle::Primary, "Export", Some("export"))
                    .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
            );
        dialog(
            600.0,
            "Export",
            format!("{file_name}  ·  glTF 2.0 binary"),
            close,
            body,
            footer,
        )
    }

    fn browse_export(&mut self, cx: &mut Context<Self>) {
        let path = self.default_export_path();
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let name = path.file_name().map(|f| f.to_string_lossy().to_string());
        let pick = cx.prompt_for_new_path(&dir, name.as_deref());
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = pick.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.export_path.update(cx, |s, cx| {
                        s.set_value(path.display().to_string(), window, cx)
                    });
                });
            }
        })
        .detach();
    }

    fn settings_dialog(
        &mut self,
        close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = t();
        let mut pages = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .w(px(180.0))
            .flex_none();
        for (i, (icon_name, label)) in [
            ("settings", "General"),
            ("sun", "Appearance"),
            ("link", "External apps"),
            ("keyboard", "Shortcuts"),
            ("plug", "Agent"),
        ]
        .into_iter()
        .enumerate()
        {
            let on = self.settings_page == i;
            pages = pages.child(
                div()
                    .id(("page", i))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.accent_subtle))
                    .when(!on, |d| d.hover(|s| s.bg(t().bg_raised)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_page = i;
                        cx.notify();
                    }))
                    .child(icon(
                        icon_name,
                        16.0,
                        if on { p.accent } else { p.text_secondary },
                    ))
                    .child(text(
                        label,
                        Type::LabelM,
                        if on { p.text_primary } else { p.text_secondary },
                    )),
            );
        }
        let pane: AnyElement = match self.settings_page {
            1 => {
                let entity = cx.entity();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(property_row("Theme", [segmented("theme", &["Dark", "Light"], crate::theme::is_light() as usize, true, move |i, _, cx| {
                        entity.update(cx, |this, cx| this.set_theme(i == 1, cx))
                    }).into_any_element()]))
                    .child(note_box(Tone::Neutral, "Stone and cobalt. Every panel follows the Theme variables, so both modes match the design file."))
                    .into_any_element()
            }
            2 => div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(property_row("Image editor", [text_field(text(self.settings.external_editor.clone().unwrap_or("System default".into()), Type::BodyS, p.text_primary)).flex_1().into_any_element()]))
                .child(note_box(Tone::Neutral, "Open in editor uses this app for linked tilesets and textures. Saving there refreshes Quarry live."))
                .into_any_element(),
            4 => div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(property_row("Socket", [text(if self.socket.is_empty() { "off".to_string() } else { self.socket.clone() }, Type::MonoS, p.text_primary).into_any_element()]))
                .child(note_box(Tone::Agent, "The agent can write only inside the open project's folder. It cannot delete files, change settings or quit Quarry."))
                .into_any_element(),
            0 => div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(property_row("Autosave", [text(format!("every {} min", self.settings.autosave_minutes), Type::MonoS, p.text_primary).into_any_element()]))
                .child(property_row("UI scale", [text(format!("{:.1}×", self.settings.ui_scale), Type::MonoS, p.text_primary).into_any_element()]))
                .into_any_element(),
            _ => self.shortcuts_pane(cx).into_any_element(),
        };
        let body = div()
            .flex()
            .gap(px(20.0))
            .w_full()
            .child(pages)
            .child(div().flex_1().min_w_0().child(pane));
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                button(
                    "reset",
                    ButtonStyle::Ghost,
                    "Reset to default",
                    Some("refresh"),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.shortcuts.clear();
                    if let Some(dirs) = &this.dirs {
                        let _ = this.settings.save(dirs);
                    }
                    cx.notify();
                })),
            )
            .child(div().flex_1())
            .child(
                button("done", ButtonStyle::Primary, "Done", None).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overlay = Overlay::None;
                        cx.notify();
                    },
                )),
            );
        let path = self
            .dirs
            .as_ref()
            .map_or(String::new(), |d| crate::start::tilde(&d.settings_file()));
        dialog(760.0, "Settings", path, close, body, footer)
    }

    fn shortcuts_pane(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let filter = self.shortcut_filter.read(cx).value().to_lowercase();
        let mut list = div()
            .flex()
            .flex_col()
            .py(px(4.0))
            .bg(p.bg_input)
            .border_1()
            .border_color(p.border_subtle)
            .rounded(px(6.0));
        let matches = |label: &str| filter.is_empty() || label.to_lowercase().contains(&filter);
        let row = |label: &'static str, area: &'static str, chip: Stateful<Div>| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.0))
                .h(px(32.0))
                .px(px(12.0))
                .child(
                    div()
                        .flex_1()
                        .child(text(label, Type::BodyS, p.text_primary)),
                )
                .child(text(area, Type::BodyS, p.text_muted))
                .child(chip)
        };
        let chip = |id: SharedString, key: String, on: bool| {
            div()
                .id(id)
                .w(px(110.0))
                .flex()
                .justify_center()
                .px(px(8.0))
                .py(px(3.0))
                .bg(if on { p.accent_subtle } else { p.bg_raised })
                .border_1()
                .border_color(if on { p.accent } else { p.border_default })
                .rounded(px(4.0))
                .child(text(
                    key,
                    Type::MonoS,
                    if on { p.text_primary } else { p.text_secondary },
                ))
        };
        for &(action, label, area, _) in crate::keys::ACTIONS.iter().filter(|a| matches(a.1)) {
            let on = self.rebinding == Some(action);
            let key = if on {
                "Press a key".to_string()
            } else {
                crate::keys::display(crate::keys::bound(&self.settings.shortcuts, action))
            };
            let chip = chip(format!("key-{action}").into(), key, on)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    // Keep the filter box from typing the key.
                    window.blur(cx);
                    this.rebinding = Some(action);
                    this.rebind_note = None;
                    cx.notify();
                }));
            list = list.child(row(label, area, chip));
        }
        for &(label, area, key) in crate::keys::MOUSE.iter().filter(|m| matches(m.0)) {
            let chip = chip(format!("mouse-{label}").into(), key.into(), false);
            list = list.child(row(label, area, chip));
        }
        let note = match (&self.rebinding, &self.rebind_note) {
            (Some(_), _) => Some("Press the new key. Esc cancels.".to_string()),
            (None, Some(n)) => Some(n.clone()),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        search_field(
                            Input::new(&self.shortcut_filter)
                                .appearance(false)
                                .into_any_element(),
                            "⌘K",
                        )
                        .flex_1(),
                    )
                    .child(select("preset-keys", "Quarry default").w(px(170.0))),
            )
            .child(
                list.id("shortcut-list")
                    .max_h(px(420.0))
                    .overflow_y_scroll(),
            )
            .when_some(note, |d, n| d.child(text(n, Type::BodyS, p.text_muted)))
    }

    fn connect_dialog(
        &mut self,
        close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = t();
        let mcp = mcp_path();
        let command = if self.connect_tab == 0 {
            format!("claude mcp add quarry -- \\\n  {mcp}")
        } else {
            format!("\"mcpServers\": {{\n  \"quarry\": {{ \"command\": \"{mcp}\" }}\n}}")
        };
        let copy = command.replace("\\\n  ", "");
        let entity = cx.entity();
        let connected = self.agent_state() != AgentState::Off;
        let can = |ok: bool, line: &str| {
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(icon(
                    if ok { "check" } else { "close" },
                    14.0,
                    if ok { p.success } else { p.danger },
                ))
                .child(
                    div()
                        .flex_1()
                        .child(text(line.to_string(), Type::BodyS, p.text_secondary)),
                )
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(segmented(
                "client",
                &["Claude Code", "Claude desktop app"],
                self.connect_tab,
                true,
                move |i, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.connect_tab = i;
                        cx.notify();
                    })
                },
            ))
            .child(text(
                if self.connect_tab == 0 {
                    "Run this once in a terminal"
                } else {
                    "Add this to claude_desktop_config.json"
                },
                Type::BodyS,
                p.text_secondary,
            ))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.0))
                    .p(px(12.0))
                    .bg(p.bg_input)
                    .border_1()
                    .border_color(p.border_default)
                    .rounded(px(6.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_family(crate::theme::MONO)
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .text_color(p.text_primary)
                            .children(command.lines().map(|l| div().child(l.to_string()))),
                    )
                    .child(
                        button("copy", ButtonStyle::Secondary, "Copy", None).on_click(cx.listener(
                            move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                                this.flash("Copied");
                            },
                        )),
                    ),
            )
            .child(text("What the agent can do", Type::Caption, p.text_muted))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(can(
                        true,
                        "Run every editor action as a named tool, rigging and animation included",
                    ))
                    .child(can(
                        true,
                        "Write files inside this project's folder, read images you point it at",
                    ))
                    .child(can(
                        false,
                        "Delete files, change settings or shortcuts, or quit Quarry",
                    )),
            );
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().size(px(6.0)).rounded_full().bg(if connected {
                        p.success
                    } else {
                        p.warning
                    }))
                    .child(text(
                        if connected {
                            "Agent connected"
                        } else {
                            "Waiting for a connection"
                        },
                        Type::BodyS,
                        p.text_muted,
                    )),
            )
            .child(
                button("connect-done", ButtonStyle::Primary, "Done", None).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overlay = Overlay::None;
                        cx.notify();
                    },
                )),
            );
        dialog(
            600.0,
            "Connect Claude",
            "Quarry MCP server  ·  stdio  ·  this machine only",
            close,
            body,
            footer,
        )
    }

    fn basics_dialog(
        &mut self,
        close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = t();
        let card = |title: &str, picture: &str, mouse: &str, keys: &str| {
            div()
                .flex()
                .flex_col()
                .w(px(266.0))
                .bg(p.bg_raised)
                .rounded(px(12.0))
                .overflow_hidden()
                .child(
                    div()
                        .h(px(120.0))
                        .w_full()
                        .bg(rgb(0x121316))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(picture, 40.0, p.text_muted)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .p(px(12.0))
                        .child(text(title.to_string(), Type::H3, p.text_primary))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(icon("mouse", 14.0, p.text_secondary))
                                .child(text(mouse.to_string(), Type::MonoS, p.text_secondary)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(icon("keyboard", 14.0, p.text_secondary))
                                .child(text(keys.to_string(), Type::MonoS, p.text_secondary)),
                        ),
                )
        };
        let body = div()
            .flex()
            .flex_wrap()
            .gap(px(16.0))
            .w(px(830.0))
            .child(card(
                "Orbit",
                "rotate",
                "Middle-drag or ⌥ drag",
                "Two-finger drag",
            ))
            .child(card(
                "Pan",
                "move",
                "Shift + middle-drag",
                "Shift + two fingers",
            ))
            .child(card("Zoom", "search", "Scroll", "⌘ + two fingers"))
            .child(card(
                "Move crosshair",
                "crosshair",
                "W A S D",
                "Q ⇧E for height",
            ))
            .child(card("Place tile", "brush", "Click", "Drag to paint"))
            .child(card("Erase", "eraser", "Right-click", "Two-finger click"));
        let dont = self.basics_dont_show;
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .child(div().size(px(6.0)).rounded_full().bg(p.accent))
                    .child(div().size(px(6.0)).rounded_full().bg(p.bg_active))
                    .child(div().size(px(6.0)).rounded_full().bg(p.bg_active)),
            )
            .child(div().flex_1())
            .child(
                checkbox("dont-show", dont, "Don't show again").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.basics_dont_show = !this.basics_dont_show;
                        cx.notify();
                    },
                )),
            )
            .child(
                button("got-it", ButtonStyle::Primary, "Got it", None).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overlay = Overlay::None;
                        cx.notify();
                    },
                )),
            );
        dialog(
            880.0,
            "The basics",
            "Six moves get you from an empty grid to a level. Open this again from Help.",
            close,
            body,
            footer,
        )
    }

    /// 14: the linked file reloaded; says what changed and offers a fix.
    pub fn render_toast(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = t();
        let toast = self.toast.as_ref()?;
        let has_faces = !toast.faces.is_empty();
        Some(
            div()
                .absolute()
                .bottom(px(40.0))
                .right(px(396.0))
                .w(px(360.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(14.0))
                .bg(p.bg_panel)
                .border_1()
                .border_color(p.border_default)
                .rounded(px(8.0))
                .shadow(crate::theme::popover_shadow())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(icon("refresh", 16.0, p.warning))
                        .child(div().flex_1().child(text(
                            toast.title.clone(),
                            Type::H3,
                            p.text_primary,
                        )))
                        .child(
                            icon_button("toast-close", "close", false, "Dismiss", "").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toast = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                )
                .child(
                    div()
                        .font_family(crate::theme::SANS)
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(p.text_secondary)
                        .child(toast.body.clone()),
                )
                .child(text(toast.meta.clone(), Type::MonoS, p.text_muted))
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .when(has_faces, |d| {
                            d.child(
                                button(
                                    "toast-select",
                                    ButtonStyle::Secondary,
                                    "Select those faces",
                                    None,
                                )
                                .flex_1()
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        if let Some((object, faces)) = this
                                            .toast
                                            .as_ref()
                                            .and_then(|t| t.faces.first().cloned())
                                        {
                                            this.run(
                                                "select.mode",
                                                json!({ "mode": "face", "object": object }),
                                                cx,
                                            );
                                            this.run(
                                                "select.by_ids",
                                                json!({ "faces": faces }),
                                                cx,
                                            );
                                        }
                                        this.toast = None;
                                    },
                                )),
                            )
                        })
                        .child(
                            button("toast-dismiss", ButtonStyle::Ghost, "Dismiss", None).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toast = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                )
                .into_any_element(),
        )
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Where quarry-mcp sits: next to this executable when installed or built.
fn mcp_path() -> String {
    let exe = std::env::current_exe().ok();
    let name = if cfg!(windows) {
        "quarry-mcp.exe"
    } else {
        "quarry-mcp"
    };
    exe.as_ref()
        .and_then(|e| e.parent())
        .map(|d| d.join(name))
        .map_or(name.to_string(), |p| p.display().to_string())
}
