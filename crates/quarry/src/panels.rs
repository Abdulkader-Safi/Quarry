//! Side panel tabs (Tileset, UVs, Paint, Scene, Rig) and the bottom strip
//! (Timeline, History, Agent). Every control runs a registry command.

use crate::app::{BottomTab, Field, HistoryFilter, Menu, Quarry, SideTab, Tool, clock};
use crate::editor::{fmt_num, thousands};
use crate::theme::{Type, t, text};
use crate::ui::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use quarry_core::Source;
use quarry_core::id::Id;
use quarry_core::model::{Channel, Image as DocImage, ImageRole, ObjectKind, TrackTarget};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// Images drawn in panels, scaled up with nearest filtering so pixel art stays
/// crisp. Keyed by image ID; rebuilt when the pixels change.
#[derive(Default)]
pub struct ImageCache {
    entries: HashMap<(String, u32), (usize, Arc<RenderImage>)>,
}

impl ImageCache {
    pub fn get(&mut self, image: &DocImage, max: u32) -> Option<Arc<RenderImage>> {
        let key = (image.id.to_string(), max);
        let stamp = image.pixels.as_ptr() as usize;
        if let Some((s, r)) = self.entries.get(&key)
            && *s == stamp
        {
            return Some(r.clone());
        }
        let [w, h] = image.size;
        if w == 0 || h == 0 || image.pixels.len() < (w * h * 4) as usize {
            return None;
        }
        let k = (max / w.max(h)).clamp(1, 16);
        let mut out = image::RgbaImage::new(w * k, h * k);
        for (x, y, px) in out.enumerate_pixels_mut() {
            let i = (((y / k) * w + (x / k)) * 4) as usize;
            let s = &image.pixels[i..i + 4];
            *px = image::Rgba([s[2], s[1], s[0], s[3]]);
        }
        let r = Arc::new(RenderImage::new(vec![image::Frame::new(out)]));
        self.entries.insert(key, (stamp, r.clone()));
        Some(r)
    }
}

impl Quarry {
    pub fn side_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let mut strip = tab_strip();
        for (i, (which, icon_name, label)) in [
            (SideTab::Tileset, "tiles", "Tileset"),
            (SideTab::Uvs, "grid", "UVs"),
            (SideTab::Paint, "brush", "Paint"),
            (SideTab::Scene, "layers", "Scene"),
            (SideTab::Rig, "bone", "Rig"),
        ]
        .into_iter()
        .enumerate()
        {
            strip = strip.child(
                tab(("side", i), icon_name, label, self.side == which, 8.0)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_side(which, cx))),
            );
        }
        let content = match self.side {
            SideTab::Tileset => self.tileset_panel(cx),
            SideTab::Uvs => self.uv_panel(cx),
            SideTab::Paint => self.paint_panel(cx),
            SideTab::Scene => self.scene_panel(cx),
            SideTab::Rig if self.animating() => self.clips_panel(cx),
            SideTab::Rig => self.rig_panel(cx),
        };
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(380.0))
            .h_full()
            .bg(p.bg_panel)
            .child(strip)
            .child(
                div()
                    .id("side-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(12.0))
                            .p(px(12.0))
                            .children(content),
                    ),
            )
    }

    /// A Select that opens its option list as a Menu.
    fn dropdown(
        &mut self,
        key: &'static str,
        value: String,
        options: Vec<String>,
        on_pick: impl Fn(&mut Self, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu == Some(Menu::Select(key));
        let on_pick = std::rc::Rc::new(on_pick);
        let mut d = div()
            .relative()
            .flex_1()
            .min_w_0()
            .child(
                select(key, value).on_click(cx.listener(move |this, _, _, cx| {
                    this.menu = if this.menu == Some(Menu::Select(key)) {
                        None
                    } else {
                        Some(Menu::Select(key))
                    };
                    cx.notify();
                })),
            );
        if open {
            let mut m = menu(220.0);
            for (i, label) in options.into_iter().enumerate() {
                let pick = on_pick.clone();
                m = m.child(menu_row((key, i), label, "", true).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.menu = None;
                        pick(this, i, cx);
                        cx.notify();
                    },
                )));
            }
            d = d.child(deferred(anchored().child(div().mt(px(26.0)).child(m))).with_priority(1));
        }
        d.into_any_element()
    }

    fn header(
        &mut self,
        key: &'static str,
        title: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.open_section(key);
        section_header(key, title, open)
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_section(key, cx)))
            .into_any_element()
    }

    fn number(
        &mut self,
        id: &'static str,
        field: Field,
        base: f32,
        axis: Option<FieldAxis>,
        unit: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let v = self.field_value(field, base);
        let shown = match field {
            Field::Rotation(_) => format!("{}", v.round() as i64),
            Field::BrushSize | Field::Padding | Field::UvOffset(_) => {
                format!("{}{unit}", v.round() as i64)
            }
            _ => format!(
                "{}{unit}",
                if unit.is_empty() {
                    format!("{v:.2}")
                } else {
                    fmt_num(v)
                }
            ),
        };
        number_field(id, shown, axis)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e, _, cx| this.begin_field_drag(field, e, cx)),
            )
            .into_any_element()
    }

    // ---- Tileset ----

    fn tileset_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = t();
        // Cheap: the arenas inside are shared, so this copies a few pointers.
        let doc = self.reg.document().clone();
        let tilesets: Vec<(Id<DocImage>, String)> = doc
            .images
            .iter()
            .filter(|i| i.role == ImageRole::Tileset)
            .map(|i| (i.id, i.name.clone()))
            .collect();
        let brush = doc.view.tile.brush;
        let current = brush
            .map(|b| b.tileset)
            .or_else(|| tilesets.first().map(|t| t.0));
        let mut out = Vec::new();
        let names: Vec<String> = tilesets.iter().map(|t| t.1.clone()).collect();
        let ids: Vec<Id<DocImage>> = tilesets.iter().map(|t| t.0).collect();
        let value = current
            .and_then(|id| doc.images.get(id))
            .map_or("No tileset yet".to_string(), |i| i.name.clone());
        let dd = self.dropdown(
            "tileset",
            value,
            names,
            move |this, i, cx| {
                this.run(
                    "tile.brush",
                    json!({ "tileset": ids[i].to_string(), "col": 0, "row": 0 }),
                    cx,
                );
            },
            cx,
        );
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(dd)
                .child(
                    icon_button("ts-import", "plus", false, "Import a tileset", "")
                        .on_click(cx.listener(|this, _, _, cx| this.import_art("tileset", cx))),
                )
                .child(
                    icon_button("ts-more", "more", false, "Reload from disk", "").on_click(
                        cx.listener(|this, _, _, cx| {
                            if let Some(b) = this.reg.document().view.tile.brush {
                                this.run(
                                    "art.reload",
                                    json!({ "image": b.tileset.to_string() }),
                                    cx,
                                );
                            }
                        }),
                    ),
                )
                .into_any_element(),
        );
        let Some(image) = current.and_then(|id| doc.images.get(id)).cloned() else {
            out.push(note_box(Tone::Neutral, "Import a PNG or PSD tileset with +. Tiles are 16 px by default and 1 tile is 1 metre.").into_any_element());
            out.push(
                button(
                    "ts-sample",
                    ButtonStyle::Secondary,
                    "Use the sample tileset",
                    Some("tiles"),
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| this.open_sample("tiles", cx)))
                .into_any_element(),
            );
            return out;
        };
        let tile = image.tile_px.max(1);
        let (cols, rows) = (image.size[0] / tile, image.size[1] / tile);
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(div().flex_1().child(text(
                    format!(
                        "{} px  ·  pad {}  ·  {} × {} tiles",
                        tile, image.padding, cols, rows
                    ),
                    Type::MonoS,
                    p.text_muted,
                )))
                .child(badge(
                    if image.source_path.is_some() && image.watch {
                        "Live"
                    } else {
                        "Embedded"
                    },
                    if image.source_path.is_some() {
                        p.success
                    } else {
                        p.text_muted
                    },
                ))
                .into_any_element(),
        );
        // The tileset grid: click a tile, shift-click to grow the brush.
        let picture = self.images.get(&image, 356);
        let width = 356.0;
        let scale = width / image.size[0].max(1) as f32;
        let height = image.size[1] as f32 * scale;
        let mut grid = div()
            .id("tileset-grid")
            .relative()
            .w_full()
            .h(px(height))
            .border_1()
            .border_color(p.border_default)
            .rounded(px(8.0))
            .overflow_hidden()
            .cursor_pointer()
            .children(picture.map(|i| img(i).absolute().inset_0().size_full()));
        if let Some(b) = brush.filter(|b| b.tileset == image.id) {
            let cell = tile as f32 * scale;
            grid = grid.child(
                div()
                    .absolute()
                    .left(px(b.col as f32 * cell))
                    .top(px(b.row as f32 * cell))
                    .w(px(b.width as f32 * cell))
                    .h(px(b.height as f32 * cell))
                    .border_2()
                    .border_color(p.accent)
                    .shadow(crate::theme::focus_ring()),
            );
        }
        let bounds = self.tileset_bounds.clone();
        let image_id = image.id;
        grid = grid
            .child(crate::viewport::measure(bounds.clone()))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                let b = bounds.get();
                let x = f32::from(e.position.x - b.origin.x) / f32::from(b.size.width);
                let y = f32::from(e.position.y - b.origin.y) / f32::from(b.size.height);
                let (col, row) = ((x * cols as f32) as u32, (y * rows as f32) as u32);
                let doc = this.reg.document();
                let args = match doc.view.tile.brush.filter(|b| e.modifiers.shift && b.tileset == image_id) {
                    Some(b) => {
                        let (c0, r0) = (b.col.min(col), b.row.min(row));
                        json!({ "tileset": image_id.to_string(), "col": c0, "row": r0,
                                "width": b.col.max(col) - c0 + 1, "height": b.row.max(row) - r0 + 1 })
                    }
                    None => json!({ "tileset": image_id.to_string(), "col": col, "row": row, "width": 1, "height": 1 }),
                };
                this.run("tile.brush", args, cx);
                this.tool = Tool::Place;
            }));
        out.push(grid.into_any_element());
        let rotate = brush.map_or(0, |b| b.rotate);
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(
                    icon_button("br-rot", "rotate", false, "Rotate brush 90°", "R").on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.run(
                                "tile.brush",
                                json!({ "rotate": (rotate as u32 + 1) % 4 }),
                                cx,
                            );
                        }),
                    ),
                )
                .child(
                    icon_button(
                        "br-flip",
                        "refresh",
                        brush.is_some_and(|b| b.flip),
                        "Flip top to bottom",
                        "V",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let flip = this.reg.document().view.tile.brush.is_some_and(|b| b.flip);
                        this.run("tile.brush", json!({ "flip": !flip }), cx);
                    })),
                )
                .child(
                    icon_button(
                        "br-mirror",
                        "scale",
                        brush.is_some_and(|b| b.mirror),
                        "Mirror left to right",
                        "H",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mirror = this
                            .reg
                            .document()
                            .view
                            .tile
                            .brush
                            .is_some_and(|b| b.mirror);
                        this.run("tile.brush", json!({ "mirror": !mirror }), cx);
                    })),
                )
                .child(div().flex_1())
                .child(
                    checkbox("sticky", self.sticky, "Sticky placement").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.sticky = !this.sticky;
                            cx.notify();
                        },
                    )),
                )
                .into_any_element(),
        );
        out.push(self.header("plane", "Placement plane", cx));
        if self.open_section("plane") {
            let axis = doc.view.tile.axis;
            let active = match axis {
                quarry_core::model::Axis::X => 0,
                quarry_core::model::Axis::Y => 1,
                quarry_core::model::Axis::Z => 2,
            };
            let entity = cx.entity();
            out.push(
                property_row(
                    "Lock to",
                    [segmented(
                        "plane-axis",
                        &["X", "Y", "Z"],
                        active,
                        true,
                        move |i, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.run("tile.plane", json!({ "axis": (["x", "y", "z"][i]) }), cx);
                            })
                        },
                    )
                    .into_any_element()],
                )
                .into_any_element(),
            );
            out.push(
                property_row(
                    "Height",
                    [
                        button("h-down", ButtonStyle::Secondary, "−1", None)
                            .on_click(cx.listener(|this, _, _, cx| this.nudge_cursor(-1.0, cx)))
                            .into_any_element(),
                        number_field("h-val", fmt_num(doc.view.cursor.y), None).into_any_element(),
                        button("h-up", ButtonStyle::Secondary, "+1", None)
                            .on_click(cx.listener(|this, _, _, cx| this.nudge_cursor(1.0, cx)))
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            );
        }
        out.push(self.header("linked", "Linked file", cx));
        if self.open_section("linked") {
            let body = match &image.source_path {
                Some(src) => format!(
                    "{}  ·  {}. Save in Krita or Affinity and the tiles update here.",
                    src,
                    if image.watch { "watching for changes" } else { "not watched" }
                ),
                None => "This tileset lives only inside the project. Link it to a PNG or PSD to edit it in another app.".into(),
            };
            out.push(note_box(Tone::Neutral, body).into_any_element());
            let src = image.source_path.clone();
            let id = image.id;
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button(
                            "open-ext",
                            ButtonStyle::Secondary,
                            "Open in editor",
                            Some("link"),
                        )
                        .flex_1()
                        .on_click(cx.listener(move |this, _, _, _| match &src {
                            Some(path) => this.open_external(path),
                            None => this.flash("No linked file yet."),
                        })),
                    )
                    .child(
                        button(
                            "reimport",
                            ButtonStyle::Secondary,
                            "Re-import",
                            Some("refresh"),
                        )
                        .flex_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.run("art.reload", json!({ "image": id.to_string() }), cx);
                        })),
                    )
                    .into_any_element(),
            );
        }
        out
    }

    fn nudge_cursor(&mut self, dy: f32, cx: &mut Context<Self>) {
        let c = self.reg.document().view.cursor;
        self.run(
            "view.cursor",
            json!({ "position": [c.x, c.y + dy, c.z] }),
            cx,
        );
    }

    /// Open a linked image in the external editor. The path comes from the
    /// project file, so it is checked first: it must be an existing image file,
    /// and it goes to the opener as one absolute argument, never through a shell.
    pub fn open_external(&mut self, path: &str) {
        let Some(file) = safe_image_path(path) else {
            self.flash(format!("{path} is not an image file Quarry can open."));
            return;
        };
        let app = self.settings.external_editor.clone();
        #[cfg(target_os = "macos")]
        let mut cmd = {
            let mut c = std::process::Command::new("open");
            if let Some(app) = &app {
                c.args(["-a", app]);
            }
            c
        };
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let _ = &app;
            std::process::Command::new("explorer.exe")
        };
        #[cfg(all(unix, not(target_os = "macos")))]
        let mut cmd = {
            let _ = &app;
            std::process::Command::new("xdg-open")
        };
        if let Err(e) = cmd.arg(&file).spawn() {
            self.flash(format!("Could not open {path}: {e}"));
        }
    }

    pub fn import_art(&mut self, role: &'static str, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if role == "tileset" {
                        if this
                            .run("tile.import_tileset", json!({ "path": path }), cx)
                            .is_some()
                        {
                            this.run("tile.brush", json!({ "col": 0, "row": 0 }), cx);
                        }
                    } else {
                        this.run("art.import", json!({ "path": path, "as": "texture" }), cx);
                    }
                });
            }
        })
        .detach();
    }

    // ---- Scene ----

    fn scene_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = t();
        let mut out = Vec::new();
        out.push(
            search_field(self.search_input(), "⌘K")
                .w_full()
                .into_any_element(),
        );
        let filter = self.scene_filter.read(cx).value().to_lowercase();
        let doc = self.reg.document();
        let selected = doc.selection.objects.clone();
        let mut tree = div().flex().flex_col().w_full();
        // Objects as a tree by parent, in their stored order.
        let mut roots: Vec<_> = doc.objects.iter().filter(|o| o.parent.is_none()).collect();
        roots.sort_by_key(|o| o.order);
        let mut rows: Vec<(usize, Id<quarry_core::model::Object>)> = Vec::new();
        fn walk(
            doc: &quarry_core::Document,
            id: Id<quarry_core::model::Object>,
            depth: usize,
            closed: &std::collections::HashSet<String>,
            rows: &mut Vec<(usize, Id<quarry_core::model::Object>)>,
        ) {
            rows.push((depth, id));
            if closed.contains(&id.to_string()) {
                return;
            }
            let mut kids: Vec<_> = doc
                .objects
                .iter()
                .filter(|o| o.parent == Some(id))
                .collect();
            kids.sort_by_key(|o| o.order);
            for k in kids {
                walk(doc, k.id, depth + 1, closed, rows);
            }
        }
        for r in roots {
            walk(doc, r.id, 0, &self.closed_tree, &mut rows);
        }
        for (i, (depth, id)) in rows.into_iter().enumerate() {
            let Some(o) = doc.objects.get(id) else {
                continue;
            };
            if !filter.is_empty() && !o.name.to_lowercase().contains(&filter) {
                continue;
            }
            let has_kids = doc.objects.iter().any(|c| c.parent == Some(id));
            let chevron = has_kids.then(|| !self.closed_tree.contains(&id.to_string()));
            let (icon_name, meta) = match o.kind {
                ObjectKind::Mesh => (
                    "cube",
                    doc.drawn_mesh(o).map_or(String::new(), |m| {
                        if depth == 0 && has_kids {
                            format!("{} tris", thousands(m.triangle_count()))
                        } else {
                            thousands(m.triangle_count())
                        }
                    }),
                ),
                ObjectKind::Instance => (
                    "link",
                    doc.drawn_mesh(o)
                        .map_or(String::new(), |m| thousands(m.triangle_count())),
                ),
                ObjectKind::Marker => ("target", String::new()),
                ObjectKind::Layer => ("layers", String::new()),
            };
            let label = if o.kind == ObjectKind::Instance {
                format!("{}  · instance", o.name)
            } else {
                o.name.clone()
            };
            let meta = if o.collision && !o.visible {
                "hidden".to_string()
            } else {
                meta
            };
            let key = id.to_string();
            tree = tree.child(
                tree_row(
                    ("obj", i),
                    depth,
                    chevron,
                    if o.collision { "shapes" } else { icon_name },
                    label,
                    meta,
                    selected.contains(&id),
                    !o.visible,
                )
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    if e.click_count() > 1 {
                        if !this.closed_tree.remove(&key) {
                            this.closed_tree.insert(key.clone());
                        }
                        cx.notify();
                        return;
                    }
                    if this.reg.document().selection.mode != quarry_core::model::SelectMode::Object
                    {
                        this.run("select.mode", json!({ "mode": "object" }), cx);
                    }
                    let add = e.modifiers().shift;
                    this.run(
                        "select.by_ids",
                        json!({ "objects": [key.clone()], "add": add }),
                        cx,
                    );
                })),
            );
        }
        for (si, s) in doc.skeletons.iter().enumerate() {
            tree = tree.child(
                tree_row(
                    ("skel", si),
                    0,
                    Some(false),
                    "bone",
                    s.name.clone(),
                    format!("{} bones", s.bones.len()),
                    false,
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_side(SideTab::Rig, cx))),
            );
        }
        if doc.objects.is_empty() && doc.skeletons.is_empty() {
            tree = tree.child(note_box(Tone::Neutral, "Nothing here yet. Add a primitive from the Mesh menu, place tiles, or import a .glb."));
        }
        out.push(tree.into_any_element());

        let one = (selected.len() == 1)
            .then(|| selected.iter().next().and_then(|id| doc.objects.get(*id)))
            .flatten()
            .cloned();
        if self.agent_state() != AgentState::Off {
            out.extend(self.history_list(cx, true));
        }
        let Some(object) = one else {
            if !selected.is_empty() {
                out.push(
                    note_box(
                        Tone::Neutral,
                        format!(
                            "{} objects selected. Drag a number field to move them together.",
                            selected.len()
                        ),
                    )
                    .into_any_element(),
                );
                out.extend(self.transform_rows(None, cx));
            }
            return out;
        };
        out.push(self.header("transform", "Transform", cx));
        if self.open_section("transform") {
            out.extend(self.transform_rows(Some(&object), cx));
        }
        out.push(self.header("object", "Object", cx));
        if self.open_section("object") {
            let name = object.id.to_string();
            let smooth = object
                .mesh
                .as_ref()
                .is_some_and(|m| m.faces.iter().any(|f| f.smooth_group > 0));
            let (n1, n2) = (name.clone(), name.clone());
            out.push(
                div()
                    .flex()
                    .gap(px(16.0))
                    .child(
                        checkbox("collision", object.collision, "Collision").on_click(cx.listener(
                            move |this, _, _, cx| {
                                let on = this
                                    .reg
                                    .document()
                                    .objects
                                    .iter()
                                    .any(|o| o.id.to_string() == n1 && o.collision);
                                this.run(
                                    "scene.set_collision",
                                    json!({ "objects": [n1.clone()], "collision": !on }),
                                    cx,
                                );
                            },
                        )),
                    )
                    .child(checkbox("smooth", smooth, "Smooth shading"))
                    .child(
                        checkbox("locked", object.locked, "Locked").on_click(cx.listener(
                            move |this, _, _, cx| {
                                let on = this
                                    .reg
                                    .document()
                                    .objects
                                    .iter()
                                    .any(|o| o.id.to_string() == n2 && o.locked);
                                this.run(
                                    "scene.lock",
                                    json!({ "objects": [n2.clone()], "locked": !on }),
                                    cx,
                                );
                            },
                        )),
                    )
                    .into_any_element(),
            );
            let tris = object.mesh.as_ref().map_or(0, |m| m.triangle_count());
            let verts = object.mesh.as_ref().map_or(0, |m| m.positions.len());
            out.push(
                text(
                    format!(
                        "{}  ·  {} tris  ·  {} verts",
                        object.id,
                        thousands(tris),
                        thousands(verts)
                    ),
                    Type::MonoS,
                    p.text_muted,
                )
                .into_any_element(),
            );
        }
        out.push(self.header("props", "Custom properties", cx));
        if self.open_section("props") {
            if object.properties.is_empty() {
                out.push(
                    text(
                        "None yet. Agents and the glTF extras use these: team, hp, loot.",
                        Type::BodyS,
                        p.text_muted,
                    )
                    .into_any_element(),
                );
            }
            for (k, v) in &object.properties {
                let shown = match v {
                    quarry_core::model::PropValue::Bool(b) => b.to_string(),
                    quarry_core::model::PropValue::Number(n) => fmt_num(*n as f32),
                    quarry_core::model::PropValue::Text(s) => s.clone(),
                };
                out.push(
                    property_row(
                        k,
                        [text_field(text(shown, Type::BodyS, p.text_primary))
                            .flex_1()
                            .into_any_element()],
                    )
                    .into_any_element(),
                );
            }
        }
        out
    }

    fn transform_rows(
        &mut self,
        object: Option<&quarry_core::model::Object>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let tr = object.map(|o| o.transform).unwrap_or_default();
        let (rx, ry, rz) = tr.rotation.to_euler(glam::EulerRot::XYZ);
        let rot = [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()];
        let axes = [FieldAxis::X, FieldAxis::Y, FieldAxis::Z];
        let mut out = Vec::new();
        for (label, kind) in [("Position", 0), ("Rotation", 1), ("Scale", 2)] {
            let mut fields = Vec::new();
            for i in 0..3 {
                let (field, base) = match kind {
                    0 => (Field::Position(i), tr.translation[i]),
                    1 => (Field::Rotation(i), rot[i]),
                    _ => (Field::Scale(i), tr.scale[i]),
                };
                let id: &'static str =
                    ["px", "py", "pz", "rx", "ry", "rz", "sx", "sy", "sz"][kind * 3 + i];
                fields.push(self.number(id, field, base, Some(axes[i]), "", cx));
            }
            out.push(property_row(label, fields).into_any_element());
        }
        out
    }

    // ---- Paint ----

    fn paint_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = t();
        let mut out = Vec::new();
        let mut tools = div()
            .flex()
            .w_full()
            .gap(px(2.0))
            .p(px(3.0))
            .bg(p.bg_raised)
            .rounded(px(8.0));
        for (i, (tool, icon_name, tip)) in [
            (Tool::Pencil, "brush", "Pencil"),
            (Tool::Eraser, "eraser", "Eraser"),
            (Tool::Fill, "fill", "Fill"),
            (Tool::Eyedropper, "eyedropper", "Eyedropper"),
            (Tool::PaintSelect, "select-box", "Rectangle"),
        ]
        .into_iter()
        .enumerate()
        {
            tools = tools.child(
                icon_button(("paint-tool", i), icon_name, self.tool == tool, tip, "")
                    .flex_1()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tool = tool;
                        cx.notify();
                    })),
            );
        }
        out.push(tools.into_any_element());
        let doc = self.reg.document().clone();
        let texture = self.paint_image.and_then(|id| doc.images.get(id)).cloned();
        match &texture {
            Some(image) => {
                let picture = self.images.get(image, 352);
                let bounds = self.canvas_bounds.clone();
                let id = image.id;
                let [w, h] = image.size;
                let aspect = h as f32 / w.max(1) as f32;
                // Faces using this texture, outlined by their UV bounds.
                let mut canvas = div()
                    .id("paint-canvas")
                    .relative()
                    .w_full()
                    .h(px(354.0 * aspect + 2.0))
                    .bg(p.bg_input)
                    .border_1()
                    .border_color(p.border_default)
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .cursor(CursorStyle::Crosshair)
                    .child(crate::viewport::measure(bounds.clone()))
                    .children(picture.map(|i| img(i).absolute().inset_0().size_full()));
                for (r, _) in self.uv_rects(id, true) {
                    canvas = canvas.child(uv_outline(r, w, h, p.accent, false));
                }
                canvas = canvas.on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    let b = bounds.get();
                    let x = (f32::from(e.position.x - b.origin.x) / f32::from(b.size.width) * w as f32) as u32;
                    let y = (f32::from(e.position.y - b.origin.y) / f32::from(b.size.height) * h as f32) as u32;
                    this.paint_pixel(id, x.min(w - 1), y.min(h - 1), cx);
                }));
                out.push(canvas.into_any_element());
            }
            None => out.push(note_box(Tone::Neutral, "Select faces in Face mode (4), then make a texture for them. Painting works on the model and here.").into_any_element()),
        }
        let palette = doc
            .settings
            .active_palette
            .as_ref()
            .and_then(|n| doc.palettes.iter().find(|p| &p.name == n))
            .or_else(|| doc.palettes.first())
            .map(|p| (p.name.clone(), p.colors.clone()))
            .unwrap_or_else(|| {
                (
                    "sweetie-16".into(),
                    SWEETIE16
                        .iter()
                        .map(|c| [(c >> 16) as u8, (c >> 8) as u8, *c as u8, 255])
                        .collect(),
                )
            });
        let lock = doc.settings.palette_lock;
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(div().flex_1().child(text(
                    format!("{}  ·  {} colours", palette.0, palette.1.len()),
                    Type::MonoS,
                    p.text_muted,
                )))
                .child(
                    switch("lock", lock).on_click(cx.listener(move |this, _, _, cx| {
                        this.run("paint.palette_lock", json!({ "lock": !lock }), cx);
                    })),
                )
                .child(text("Lock", Type::LabelS, p.text_secondary))
                .into_any_element(),
        );
        let mut swatches = div().flex().flex_wrap().gap(px(4.0)).w_full();
        for (i, c) in palette.1.iter().enumerate() {
            let c = *c;
            let on = c == self.paint_rgba;
            swatches = swatches.child(
                div()
                    .id(("swatch", i))
                    .w(px(39.0))
                    .h(px(22.0))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .bg(rgb(((c[0] as u32) << 16)
                        | ((c[1] as u32) << 8)
                        | c[2] as u32))
                    .when(on, |d| d.border_2().border_color(p.text_primary))
                    .when(!on, |d| d.border_1().border_color(p.border_default))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.paint_rgba = c;
                        cx.notify();
                    })),
            );
        }
        out.push(swatches.into_any_element());
        let size = self.field_value(Field::BrushSize, self.brush_px as f32);
        out.push(
            property_row(
                "Size",
                [
                    slider("size", size / 16.0, format!("{} px", size.round() as i64))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, e, _, cx| {
                                this.begin_field_drag(Field::BrushSize, e, cx)
                            }),
                        )
                        .into_any_element(),
                ],
            )
            .into_any_element(),
        );
        out.push(
            div()
                .flex()
                .gap(px(16.0))
                .child(
                    checkbox("pressure", self.pressure, "Pen pressure").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.pressure = !this.pressure;
                            cx.notify();
                        },
                    )),
                )
                .child(
                    checkbox("vcolor", self.tool == Tool::VertexColor, "Vertex colour").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.tool = if this.tool == Tool::VertexColor {
                                Tool::Pencil
                            } else {
                                Tool::VertexColor
                            };
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element(),
        );
        out.push(self.header("textures", "Textures", cx));
        if self.open_section("textures") {
            let mut list = div().flex().flex_col().w_full();
            for (i, image) in doc
                .images
                .iter()
                .filter(|i| i.role == ImageRole::Texture)
                .enumerate()
            {
                let id = image.id;
                list = list.child(
                    tree_row(
                        ("tex", i),
                        0,
                        None,
                        "image",
                        image.name.clone(),
                        format!("{}×{}", image.size[0], image.size[1]),
                        self.paint_image == Some(id),
                        false,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.paint_image = Some(id);
                        cx.notify();
                    })),
                );
            }
            out.push(list.into_any_element());
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button(
                            "tex-new",
                            ButtonStyle::Ghost,
                            "New blank texture",
                            Some("plus"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(v) = this.run(
                                "paint.texture_new",
                                json!({ "width": 32, "height": 32 }),
                                cx,
                            ) {
                                let _ = this.reg.execute("uv.unwrap", json!({}), Source::Human);
                                this.paint_image = v["image"].as_str().and_then(|s| s.parse().ok());
                                if this.paint_image.is_none() {
                                    this.paint_image = this
                                        .reg
                                        .document()
                                        .images
                                        .iter()
                                        .filter(|i| i.role == ImageRole::Texture)
                                        .last()
                                        .map(|i| i.id);
                                }
                            }
                        })),
                    )
                    .child(
                        button(
                            "tex-import",
                            ButtonStyle::Ghost,
                            "Import PNG or PSD",
                            Some("image"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.import_art("texture", cx))),
                    )
                    .into_any_element(),
            );
        }
        out
    }

    fn paint_pixel(&mut self, image: Id<DocImage>, x: u32, y: u32, cx: &mut Context<Self>) {
        let name = image.to_string();
        match self.tool {
            Tool::Fill => {
                self.run(
                    "paint.fill",
                    json!({ "image": name, "x": x, "y": y, "rgba": self.paint_rgba }),
                    cx,
                );
            }
            Tool::Eyedropper => {
                if let Some(img) = self.reg.document().images.get(image) {
                    let i = ((y * img.size[0] + x) * 4) as usize;
                    if let Some(px) = img.pixels.get(i..i + 4) {
                        self.paint_rgba = [px[0], px[1], px[2], px[3]];
                        self.tool = Tool::Pencil;
                    }
                }
                cx.notify();
            }
            _ => {
                let rgba = if self.tool == Tool::Eraser {
                    [0, 0, 0, 0]
                } else {
                    self.paint_rgba
                };
                let n = self.brush_px.max(1);
                self.run(
                    "paint.rect",
                    json!({ "image": name, "x": x, "y": y, "width": n, "height": n, "rgba": rgba }),
                    cx,
                );
            }
        }
    }

    /// Pixel rectangles [x0, y0, x1, y1] of the faces that use `image`.
    fn uv_rects(&self, image: Id<DocImage>, selected_only: bool) -> Vec<([f32; 4], bool)> {
        let doc = self.reg.document();
        let sel = &doc.selection;
        let mut out = Vec::new();
        for o in doc.objects.iter() {
            let Some(mesh) = &o.mesh else { continue };
            for (fi, f) in mesh.faces.iter().enumerate() {
                if f.image != Some(image) {
                    continue;
                }
                let picked = sel.object == Some(o.id) && sel.faces.contains(&(fi as u32));
                if selected_only && !picked {
                    continue;
                }
                let (mut a, mut b) = ([f32::MAX; 2], [f32::MIN; 2]);
                for c in &f.corners {
                    a = [a[0].min(c.uv.x), a[1].min(c.uv.y)];
                    b = [b[0].max(c.uv.x), b[1].max(c.uv.y)];
                }
                out.push(([a[0], a[1], b[0], b[1]], picked));
                if out.len() > 400 {
                    return out;
                }
            }
        }
        out
    }

    // ---- UVs ----

    fn uv_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = t();
        let mut out = Vec::new();
        let doc = self.reg.document();
        let textures: Vec<(Id<DocImage>, String)> =
            doc.images.iter().map(|i| (i.id, i.name.clone())).collect();
        let current = self.paint_image.and_then(|id| doc.images.get(id)).cloned();
        let ids: Vec<Id<DocImage>> = textures.iter().map(|t| t.0).collect();
        let dd = self.dropdown(
            "uv-image",
            current
                .as_ref()
                .map_or("No texture".into(), |i| i.name.clone()),
            textures.iter().map(|t| t.1.clone()).collect(),
            move |this, i, _| this.paint_image = Some(ids[i]),
            cx,
        );
        let zoom = current
            .as_ref()
            .map_or(1, |i| (352 / i.size[0].max(i.size[1]).max(1)).max(1));
        out.push(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(dd)
                .child(text(
                    current.as_ref().map_or(String::new(), |i| {
                        format!("{}×{} · {zoom}×", i.size[0], i.size[1])
                    }),
                    Type::MonoS,
                    p.text_muted,
                ))
                .into_any_element(),
        );
        if let Some(image) = &current {
            let picture = self.images.get(image, 352);
            let [w, h] = image.size;
            let aspect = h as f32 / w.max(1) as f32;
            let mut canvas = div()
                .relative()
                .w_full()
                .h(px(354.0 * aspect + 2.0))
                .bg(p.bg_input)
                .border_1()
                .border_color(p.border_default)
                .rounded(px(8.0))
                .overflow_hidden()
                .children(picture.map(|i| img(i).absolute().inset_0().size_full().opacity(0.55)));
            for (r, picked) in self.uv_rects(image.id, false) {
                canvas = canvas.child(uv_outline(
                    r,
                    w,
                    h,
                    if picked { p.accent } else { p.text_secondary },
                    picked,
                ));
            }
            out.push(canvas.into_any_element());
        } else {
            out.push(note_box(Tone::Neutral, "Select faces in Face mode, then make or pick a texture in Paint. Their UVs show here in pixels.").into_any_element());
        }
        out.push(self.header("uv-transform", "Transform", cx));
        if self.open_section("uv-transform") {
            let ox = self.number("uv-ox", Field::UvOffset(0), 0.0, Some(FieldAxis::X), "", cx);
            let oy = self.number("uv-oy", Field::UvOffset(1), 0.0, Some(FieldAxis::Y), "", cx);
            out.push(property_row("Offset px", [ox, oy]).into_any_element());
            let rot = self.dropdown(
                "uv-rot",
                "0°".into(),
                vec!["90°".into(), "180°".into(), "270°".into()],
                |this, i, cx| {
                    this.run(
                        "uv.transform",
                        json!({ "rotate": (i as f32 + 1.0) * 90.0 }),
                        cx,
                    );
                },
                cx,
            );
            out.push(property_row("Rotate", [rot]).into_any_element());
            let sx = self.number("uv-sx", Field::UvScale(0), 1.0, Some(FieldAxis::X), "", cx);
            let sy = self.number("uv-sy", Field::UvScale(1), 1.0, Some(FieldAxis::Y), "", cx);
            out.push(property_row("Scale", [sx, sy]).into_any_element());
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button("flip-h", ButtonStyle::Secondary, "Flip H", None)
                            .flex_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run("uv.transform", json!({ "flip": "horizontal" }), cx);
                            })),
                    )
                    .child(
                        button("flip-v", ButtonStyle::Secondary, "Flip V", None)
                            .flex_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run("uv.transform", json!({ "flip": "vertical" }), cx);
                            })),
                    )
                    .child(
                        button("snap-px", ButtonStyle::Secondary, "Snap to px", None)
                            .flex_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run("uv.snap_pixels", json!({}), cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        out.push(self.header("unwrap", "Unwrap", cx));
        if self.open_section("unwrap") {
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button(
                            "unwrap",
                            ButtonStyle::Primary,
                            "Unwrap at seams",
                            Some("wand"),
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run("uv.unwrap", json!({}), cx);
                        })),
                    )
                    .child(
                        button(
                            "project",
                            ButtonStyle::Secondary,
                            "Project from view",
                            Some("camera"),
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.run("uv.project_view", json!({}), cx);
                        })),
                    )
                    .into_any_element(),
            );
            let pad = self.number(
                "uv-pad",
                Field::Padding,
                self.uv_padding as f32,
                None,
                " px",
                cx,
            );
            out.push(property_row("Padding", [pad]).into_any_element());
            out.push(
                button("pack", ButtonStyle::Secondary, "Pack islands", Some("grid"))
                    .w_full()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.run("uv.unwrap", json!({}), cx);
                    }))
                    .into_any_element(),
            );
        }
        out
    }

    // ---- Rig ----

    fn rig_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let _p = t();
        let mut out = Vec::new();
        let doc = self.reg.document();
        let skeleton = doc.skeletons.iter().next().cloned();
        let count = skeleton.as_ref().map_or(0, |s| s.bones.len());
        out.push(
            caption_row("Skeleton", format!("{count} bones"))
                .child(
                    icon_button(
                        "bone-add",
                        "plus",
                        false,
                        if skeleton.is_some() {
                            "Add bone"
                        } else {
                            "New skeleton"
                        },
                        "B",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.reg.document().skeletons.is_empty() {
                            this.run("rig.skeleton_new", json!({}), cx);
                        } else {
                            this.run(
                                "rig.bone_add",
                                json!({ "parent": this.selected_bone_name() }),
                                cx,
                            );
                        }
                    })),
                )
                .into_any_element(),
        );
        if let Some(s) = &skeleton {
            let mut tree = div().flex().flex_col().w_full();
            fn depth(s: &quarry_core::model::Skeleton, mut i: usize) -> usize {
                let mut d = 0;
                while let Some(parent) = s.bones[i].parent {
                    i = parent as usize;
                    d += 1;
                    if d > 64 {
                        break;
                    }
                }
                d
            }
            // Depth-first order so children sit under their parent.
            let mut order = Vec::new();
            fn visit(
                s: &quarry_core::model::Skeleton,
                parent: Option<u16>,
                order: &mut Vec<usize>,
            ) {
                for (i, b) in s.bones.iter().enumerate() {
                    if b.parent == parent {
                        order.push(i);
                        visit(s, Some(i as u16), order);
                    }
                }
            }
            visit(s, None, &mut order);
            for (row, i) in order.into_iter().enumerate() {
                let b = &s.bones[i];
                let has_kids = s.bones.iter().any(|c| c.parent == Some(i as u16));
                let ik = s.ik_chains.iter().any(|c| {
                    c.tip_bone as usize == i
                        || s.bones[c.tip_bone as usize].parent == Some(i as u16)
                });
                tree = tree.child(
                    tree_row(
                        ("bone", row),
                        depth(s, i),
                        has_kids.then_some(true),
                        "bone",
                        b.name.clone(),
                        if ik { "IK" } else { "" },
                        self.selected_bone == Some(i),
                        false,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_bone = Some(i);
                        cx.notify();
                    })),
                );
            }
            out.push(tree.into_any_element());
        } else {
            out.push(note_box(Tone::Neutral, "No skeleton yet. Press + to make one, then add bones from the crosshair. Bones point along their own +Y.").into_any_element());
        }
        out.push(self.header("weights", "Weight brush", cx));
        if self.open_section("weights") {
            let entity = cx.entity();
            out.push(
                segmented(
                    "wmode",
                    &["Add", "Subtract", "Smooth", "Set"],
                    self.weight_mode,
                    true,
                    move |i, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.weight_mode = i;
                            this.tool = Tool::Weights;
                            cx.notify();
                        })
                    },
                )
                .into_any_element(),
            );
            let strength = self.field_value(Field::Strength, self.weight_strength);
            out.push(
                property_row(
                    "Strength",
                    [slider("strength", strength, format!("{strength:.2}"))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, e, _, cx| {
                                this.begin_field_drag(Field::Strength, e, cx)
                            }),
                        )
                        .into_any_element()],
                )
                .into_any_element(),
            );
            let radius = self.number(
                "wradius",
                Field::WeightRadius,
                self.weight_radius,
                None,
                " px",
                cx,
            );
            out.push(property_row("Radius", [radius]).into_any_element());
            out.push(
                div()
                    .flex()
                    .gap(px(16.0))
                    .child(
                        checkbox("w-pressure", self.pressure, "Pressure").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.pressure = !this.pressure;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        checkbox("w-normalise", self.normalise, "Normalise").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.normalise = !this.normalise;
                                cx.notify();
                            },
                        )),
                    )
                    .into_any_element(),
            );
        }
        out.push(self.header("binding", "Binding", cx));
        if self.open_section("binding") {
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button("auto-w", ButtonStyle::Primary, "Auto weights", Some("wand"))
                            .flex_1()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let object = this
                                    .reg
                                    .document()
                                    .selection
                                    .objects
                                    .iter()
                                    .next()
                                    .map(|i| i.to_string());
                                if let Some(v) =
                                    this.run("rig.bind_auto", json!({ "object": object }), cx)
                                {
                                    this.rig_note = v["note"]
                                        .as_str()
                                        .or(v["warning"].as_str())
                                        .map(str::to_string);
                                }
                            })),
                    )
                    .child(
                        button(
                            "rigid",
                            ButtonStyle::Secondary,
                            "Rigid to bone",
                            Some("link"),
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            let object = this
                                .reg
                                .document()
                                .selection
                                .objects
                                .iter()
                                .next()
                                .map(|i| i.to_string());
                            match (object, this.selected_bone_name()) {
                                (Some(object), Some(bone)) => {
                                    this.run(
                                        "rig.bind_rigid",
                                        json!({ "object": object, "bone": bone }),
                                        cx,
                                    );
                                }
                                _ => this.flash("Select an object and a bone first."),
                            }
                        })),
                    )
                    .into_any_element(),
            );
            out.push(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        button(
                            "mirror-w",
                            ButtonStyle::Secondary,
                            "Mirror .L → .R",
                            Some("refresh"),
                        )
                        .flex_1()
                        .on_click(cx.listener(|this, _, _, cx| {
                            let object = this
                                .reg
                                .document()
                                .selection
                                .objects
                                .iter()
                                .next()
                                .map(|i| i.to_string());
                            this.run("rig.mirror_weights", json!({ "object": object }), cx);
                        })),
                    )
                    .child(
                        button(
                            "add-ik",
                            ButtonStyle::Secondary,
                            "Add IK target",
                            Some("target"),
                        )
                        .flex_1()
                        .on_click(
                            cx.listener(|this, _, _, cx| match this.selected_bone_name() {
                                Some(bone) => {
                                    this.run("rig.ik_add", json!({ "tip_bone": bone }), cx);
                                }
                                None => this
                                    .flash("Pick the chain's tip bone (a shin or forearm) first."),
                            }),
                        ),
                    )
                    .into_any_element(),
            );
            if let Some(note) = &self.rig_note {
                out.push(note_box(Tone::Warning, note.clone()).into_any_element());
            }
        }
        out
    }

    fn clips_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = t();
        let mut out = Vec::new();
        let doc = self.reg.document().clone();
        let active = doc.view.anim.clip;
        let clips: Vec<_> = doc
            .clips
            .iter()
            .map(|c| (c.id, c.name.clone(), c.length, c.looping))
            .collect();
        out.push(caption_row("Clips", clips.len().to_string()).into_any_element());
        let mut list = div().flex().flex_col().w_full();
        for (i, (id, name, length, _)) in clips.iter().enumerate() {
            let n = name.clone();
            list = list.child(
                tree_row(
                    ("clip", i),
                    0,
                    None,
                    "keyframe",
                    name.clone(),
                    format!("{length:.2} s"),
                    active == Some(*id),
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run("anim.playhead", json!({ "clip": n.clone(), "time": 0 }), cx);
                })),
            );
        }
        out.push(list.into_any_element());
        out.push(
            button("clip-new", ButtonStyle::Ghost, "New clip", Some("plus"))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.reg.document().skeletons.is_empty() {
                        this.flash("Make a skeleton in the Rig tab first.");
                    } else {
                        this.run_named("anim.clip_new", cx);
                    }
                }))
                .into_any_element(),
        );
        let Some((_, name, length, looping)) = clips.iter().find(|c| Some(c.0) == active).cloned()
        else {
            return out;
        };
        let fps = doc.settings.fps;
        out.push(self.header("clip", format!("Clip · {name}"), cx));
        if self.open_section("clip") {
            let len = self.field_value(Field::ClipLength, length);
            out.push(
                property_row(
                    "Length",
                    [
                        number_field(
                            "clip-frames",
                            format!("{} f", (len * fps as f32).round() as i64),
                            None,
                        )
                        .into_any_element(),
                        number_field("clip-secs", format!("{len:.2} s"), None)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, e, _, cx| {
                                    this.begin_field_drag(Field::ClipLength, e, cx)
                                }),
                            )
                            .into_any_element(),
                    ],
                )
                .into_any_element(),
            );
            let rate = self.dropdown(
                "fps",
                format!("{fps} fps (project)"),
                ["12", "24", "30", "60"]
                    .iter()
                    .map(|f| format!("{f} fps"))
                    .collect(),
                |this, i, cx| {
                    this.run(
                        "project.set_settings",
                        json!({ "fps": ([12, 24, 30, 60][i]) }),
                        cx,
                    );
                },
                cx,
            );
            out.push(property_row("Frame rate", [rate]).into_any_element());
            let n = name.clone();
            out.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(76.0))
                            .child(text("Loop", Type::BodyS, p.text_secondary)),
                    )
                    .child(
                        switch("loop", looping).on_click(cx.listener(move |this, _, _, cx| {
                            this.run(
                                "anim.clip_edit",
                                json!({ "clip": n.clone(), "looping": !looping }),
                                cx,
                            );
                        })),
                    )
                    .into_any_element(),
            );
        }
        out.push(self.header("fade", "Crossfade preview", cx));
        if self.open_section("fade") {
            let names: Vec<String> = clips.iter().map(|c| c.1.clone()).collect();
            let from = self.crossfade.0.clone().unwrap_or(name.clone());
            let to = self
                .crossfade
                .1
                .clone()
                .or_else(|| names.iter().find(|n| **n != from).cloned())
                .unwrap_or_default();
            let (n1, n2) = (names.clone(), names.clone());
            let a = self.dropdown(
                "fade-from",
                from.clone(),
                names.clone(),
                move |this, i, _| this.crossfade.0 = Some(n1[i].clone()),
                cx,
            );
            let b = self.dropdown(
                "fade-to",
                to.clone(),
                names.clone(),
                move |this, i, _| this.crossfade.1 = Some(n2[i].clone()),
                cx,
            );
            out.push(property_row("From → to", [a, b]).into_any_element());
            let fade = self.number("fade", Field::Fade, self.crossfade.2, None, " s", cx);
            out.push(property_row("Fade", [fade]).into_any_element());
            out.push(
                button("preview-fade", ButtonStyle::Secondary, "Preview fade", Some("play"))
                    .w_full()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let fade = this.crossfade.2;
                        if let Some(v) = this.run("anim.crossfade_preview", json!({ "from": from.clone(), "to": to.clone(), "fade": fade, "time": fade / 2.0 }), cx) {
                            this.flash(format!("Halfway through the fade the blend weight is {}", v["weight"]));
                        }
                    }))
                    .into_any_element(),
            );
        }
        out.push(self.header("ik", "IK chains", cx));
        if self.open_section("ik")
            && let Some(s) = doc.skeletons.iter().next()
        {
            {
                for (i, c) in s.ik_chains.iter().enumerate() {
                    let tip = &s.bones[c.tip_bone as usize].name;
                    let root = s.bones[c.tip_bone as usize]
                        .parent
                        .map_or("", |p| s.bones[p as usize].name.as_str());
                    out.push(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(div().flex_1().child(text(
                                format!("{root} → {tip}"),
                                Type::MonoS,
                                p.text_primary,
                            )))
                            .child(switch(("ik-on", i), true))
                            .into_any_element(),
                    );
                }
                if s.ik_chains.is_empty() {
                    out.push(
                        text(
                            "No IK chains. Add one from the Rig tab's Binding section.",
                            Type::BodyS,
                            p.text_muted,
                        )
                        .into_any_element(),
                    );
                }
            }
        }
        out
    }

    // ---- Bottom panel ----

    pub fn bottom_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let mut strip = tab_strip();
        for (i, (which, icon_name, label)) in [
            (BottomTab::Timeline, "keyframe", "Timeline"),
            (BottomTab::History, "history", "History"),
            (BottomTab::Agent, "plug", "Agent"),
        ]
        .into_iter()
        .enumerate()
        {
            strip = strip.child(
                tab(
                    ("bottom", i),
                    icon_name,
                    label,
                    self.bottom == Some(which),
                    10.0,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_bottom(which, cx))),
            );
        }
        let mut panel = div()
            .flex()
            .flex_col()
            .flex_none()
            .w_full()
            .bg(p.bg_panel)
            .child(strip);
        match self.bottom {
            Some(BottomTab::Timeline) => panel = panel.child(self.timeline(cx).h(px(300.0))),
            Some(BottomTab::History) => {
                panel = panel.child(
                    div()
                        .id("history-scroll")
                        .h(px(240.0))
                        .overflow_y_scroll()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .p(px(12.0))
                                .children(self.history_list(cx, false)),
                        ),
                )
            }
            Some(BottomTab::Agent) => panel = panel.child(self.agent_body().h(px(240.0))),
            None => {}
        }
        panel
    }

    fn history_list(&mut self, cx: &mut Context<Self>, compact: bool) -> Vec<AnyElement> {
        let p = t();
        let entries = self.reg.history().entries();
        let (all, agent) = (
            entries.len(),
            entries.iter().filter(|e| e.source == Source::Agent).count(),
        );
        let mut out = vec![caption_row("History", format!("{all} steps")).into_any_element()];
        let mut chips = div().flex().gap(px(4.0));
        for (i, (f, label, n)) in [
            (HistoryFilter::All, "All", all),
            (HistoryFilter::Human, "You", all - agent),
            (HistoryFilter::Agent, "Agent", agent),
        ]
        .into_iter()
        .enumerate()
        {
            chips = chips.child(
                chip(("hchip", i), label, n, self.history_filter == f).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.history_filter = f;
                        cx.notify();
                    },
                )),
            );
        }
        out.push(chips.into_any_element());
        let mut list = div().flex().flex_col().w_full();
        let total = entries.len();
        let shown: Vec<(usize, String, String, bool)> = entries
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, e)| match self.history_filter {
                HistoryFilter::All => true,
                HistoryFilter::Human => e.source == Source::Human,
                HistoryFilter::Agent => e.source == Source::Agent,
            })
            .take(if compact { 6 } else { 200 })
            .map(|(i, e)| (i, clock(e.time), e.label.clone(), e.source == Source::Agent))
            .collect();
        for (row, (i, time, label, is_agent)) in shown.into_iter().enumerate() {
            let steps = total - i - 1;
            list = list.child(
                history_row(("hist", row), time, label, is_agent, i + 1 == total).on_click(
                    cx.listener(move |this, _, _, cx| {
                        for _ in 0..steps {
                            let _ = this.reg.execute("history.undo", json!({}), Source::Human);
                        }
                        this.sync_camera_from_doc();
                        cx.notify();
                    }),
                ),
            );
        }
        if total == 0 {
            list = list.child(text("Nothing to undo yet.", Type::BodyS, p.text_muted));
        }
        out.push(list.into_any_element());
        out.push(note_box(Tone::Agent, "Every agent edit is one undo step. ⌘Z undoes the agent's last command like your own.").into_any_element());
        out
    }

    fn agent_body(&self) -> Div {
        let p = t();
        let mut log = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(2.0))
            .h_full()
            .overflow_hidden();
        let n = self.agent_log.len();
        for (i, e) in self.agent_log.iter().enumerate().skip(n.saturating_sub(9)) {
            let last = i + 1 == n && self.agent_running.is_some();
            log = log.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(10.0))
                    .h(px(24.0))
                    .px(px(8.0))
                    .rounded(px(4.0))
                    .when(last, |d| d.bg(p.accent_subtle))
                    .child(text(clock(e.time), Type::MonoS, p.text_muted))
                    .child(div().size(px(7.0)).rounded_full().bg(if last {
                        p.agent
                    } else if e.ok {
                        p.success
                    } else {
                        p.danger
                    }))
                    .child(div().w(px(170.0)).flex_none().child(text(
                        e.command.clone(),
                        Type::MonoS,
                        p.text_primary,
                    )))
                    .child(div().flex_1().min_w_0().overflow_hidden().child(text(
                        e.summary.clone(),
                        Type::BodyS,
                        if e.ok { p.text_secondary } else { p.danger },
                    )))
                    .child(text(format!("{} ms", e.ms), Type::MonoS, p.text_muted)),
            );
        }
        if n == 0 {
            log = log.child(note_box(
                Tone::Agent,
                if self.socket.is_empty() {
                    "The agent socket is off. Start Quarry from a normal session to let Claude connect.".to_string()
                } else {
                    format!("No agent commands yet. Connect Claude with quarry-mcp; it talks to this window over {}.", self.socket)
                },
            ));
        }
        let mut shot = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(6.0))
            .w(px(240.0))
            .h_full();
        match &self.agent_shot {
            Some((image, label)) => {
                shot = shot
                    .child(
                        div()
                            .w(px(240.0))
                            .h(px(160.0))
                            .border_1()
                            .border_color(p.border_default)
                            .rounded(px(8.0))
                            .overflow_hidden()
                            .child(img(image.clone()).size_full().object_fit(ObjectFit::Cover)),
                    )
                    .child(text(label.clone(), Type::MonoS, p.text_muted))
                    .child(text("What the agent saw", Type::BodyS, p.text_secondary));
            }
            None => {
                shot = shot
                    .child(
                        div()
                            .w(px(240.0))
                            .h(px(160.0))
                            .border_1()
                            .border_color(p.border_default)
                            .rounded(px(8.0))
                            .bg(p.bg_input),
                    )
                    .child(text(
                        "Screenshots the agent takes show here",
                        Type::BodyS,
                        p.text_muted,
                    ));
            }
        }
        div()
            .flex()
            .gap(px(16.0))
            .p(px(12.0))
            .w_full()
            .child(log)
            .child(shot)
    }

    fn timeline(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let doc = self.reg.document();
        let fps = doc.settings.fps.max(1) as f32;
        let clip = doc.view.anim.clip.and_then(|id| doc.clips.get(id)).cloned();
        let names: Vec<String> = doc.clips.iter().map(|c| c.name.clone()).collect();
        let time = doc.view.anim.time;
        let autokey = doc.view.anim.autokey;
        let skeleton = doc.skeletons.iter().next().cloned();
        let n = names.clone();
        let clip_select = self.dropdown(
            "clip-select",
            clip.as_ref().map_or("No clip".into(), |c| c.name.clone()),
            names,
            move |this, i, cx| {
                this.run(
                    "anim.playhead",
                    json!({ "clip": n[i].clone(), "time": 0 }),
                    cx,
                );
            },
            cx,
        );
        let length = clip.as_ref().map_or(1.0, |c| c.length);
        let frames = (length * fps).round().max(1.0) as u32;
        let frame = (time * fps).round() as u32;
        let interp = self.interp;
        let interp_select = self.dropdown(
            "interp",
            capital(interp),
            vec!["Step".into(), "Linear".into(), "Ease in out".into()],
            |this, i, cx| {
                this.interp = ["step", "linear", "ease_in_out"][i];
                cx.notify();
            },
            cx,
        );
        let header = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .border_b_1()
            .border_color(p.border_subtle)
            .child(div().w(px(150.0)).child(clip_select))
            .child(
                icon_button("tl-undo", "undo", false, "Undo", "⌘Z")
                    .on_click(cx.listener(|this, _, _, cx| this.undo(cx))),
            )
            .child(
                icon_button("tl-play", "play", self.playing, "Play", "Space").on_click(
                    cx.listener(|this, _, _, cx| {
                        this.playing = true;
                        cx.notify();
                    }),
                ),
            )
            .child(
                icon_button("tl-pause", "pause", false, "Pause", "Space").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.playing = false;
                        cx.notify();
                    },
                )),
            )
            .child(
                icon_button("tl-stop", "stop", false, "Stop", "").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.playing = false;
                        this.run("anim.stop", json!({}), cx);
                    },
                )),
            )
            .child(
                icon_button(
                    "tl-loop",
                    "refresh",
                    clip.as_ref().is_some_and(|c| c.looping),
                    "Loop",
                    "",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Some(name) = this.active_clip_name() {
                        let looping = this
                            .reg
                            .document()
                            .clips
                            .by_name(&name)
                            .is_some_and(|c| c.looping);
                        this.run(
                            "anim.clip_edit",
                            json!({ "clip": name, "looping": !looping }),
                            cx,
                        );
                    }
                })),
            )
            .child(text(
                format!(
                    "{frame} / {frames}   ·   {} fps   ·   {length:.2} s   ·   {}",
                    fps as u32,
                    if clip.as_ref().is_some_and(|c| c.looping) {
                        "loop"
                    } else {
                        "once"
                    }
                ),
                Type::MonoS,
                p.text_secondary,
            ))
            .child(div().flex_1())
            .child(text("Interpolation", Type::BodyS, p.text_secondary))
            .child(div().w(px(110.0)).child(interp_select))
            .child(
                checkbox("autokey", autokey, "Auto-key").on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.run("anim.autokey", json!({ "on": !autokey }), cx);
                    },
                )),
            )
            .child(
                checkbox("onion", self.onion, "Onion skin").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.onion = !this.onion;
                        cx.notify();
                    },
                )),
            )
            .child(
                button(
                    "tl-fade",
                    ButtonStyle::Secondary,
                    "Crossfade",
                    Some("refresh"),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_side(SideTab::Rig, cx);
                    this.closed_sections.remove("fade");
                })),
            );

        let Some(clip) = clip else {
            return div().flex().flex_col().w_full().child(header).child(
                div().p(px(12.0)).child(note_box(Tone::Neutral, "No clip yet. Make a skeleton in Rig, then New clip. Keys go on bones at the playhead; IK targets bake to rotations on export.")),
            );
        };
        // Track geometry: names in the first 180 px, frames across the rest.
        let left = 186.0;
        let width = 960.0;
        let x_of = |t: f32| left + 6.0 + (t / clip.length.max(1e-3)) * width;
        let bounds = self.timeline_bounds.clone();
        let mut tracks = div()
            .id("tracks")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(crate::viewport::measure(bounds.clone()))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(180.0))
                    .h_full()
                    .bg(p.bg_panel),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w_full()
                    .h(px(22.0))
                    .bg(p.bg_raised),
            );
        let clip_len = clip.length;
        tracks = tracks.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                let b = bounds.get();
                let x = f32::from(e.position.x - b.origin.x);
                let y = f32::from(e.position.y - b.origin.y);
                if y < 22.0 && x > left {
                    let time = ((x - left - 6.0) / width).clamp(0.0, 1.0) * clip_len;
                    let fps = this.reg.document().settings.fps.max(1) as f32;
                    this.run(
                        "anim.playhead",
                        json!({ "time": (time * fps).round() / fps }),
                        cx,
                    );
                }
            }),
        );
        let step = (frames / 6).max(1);
        let mut f = 0;
        while f <= frames {
            tracks = tracks.child(
                div()
                    .absolute()
                    .left(px(x_of(f as f32 / fps) - 3.0))
                    .top(px(4.0))
                    .child(text(f.to_string(), Type::MonoS, p.text_muted)),
            );
            f += step;
        }
        for (row, track) in clip.tracks.iter().enumerate().take(10) {
            let y = 22.0 + row as f32 * 26.0;
            let bone = skeleton
                .as_ref()
                .and_then(|s| {
                    let i = match track.target {
                        TrackTarget::Bone(b) => b,
                        TrackTarget::Ik(c) => s.ik_chains.get(c as usize)?.tip_bone,
                    };
                    s.bones.get(i as usize)
                })
                .map_or("?".to_string(), |b| b.name.clone());
            let (icon_name, channel) = match track.channel {
                Channel::Translation => ("bone", ".loc"),
                Channel::Rotation => ("bone", ".rot"),
                Channel::Scale => ("bone", ".scale"),
                Channel::IkTarget => ("target", ".loc"),
                Channel::IkPole => ("target", ".pole"),
            };
            let sel_row = self.selected_key.is_some_and(|(r, _)| r == row);
            if sel_row {
                tracks = tracks
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(y))
                            .w_full()
                            .h(px(26.0))
                            .bg(p.accent_subtle),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(y))
                            .w(px(2.0))
                            .h(px(26.0))
                            .bg(p.accent),
                    );
            }
            tracks = tracks
                .child(div().absolute().left(px(12.0)).top(px(y + 6.0)).child(icon(
                    icon_name,
                    14.0,
                    if sel_row { p.accent } else { p.text_secondary },
                )))
                .child(
                    div()
                        .absolute()
                        .left(px(32.0))
                        .top(px(y + 5.0))
                        .flex()
                        .gap(px(2.0))
                        .child(text(bone.clone(), Type::BodyS, p.text_primary))
                        .child(text(channel, Type::MonoS, p.text_muted)),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(y + 25.0))
                        .w_full()
                        .h(px(1.0))
                        .bg(p.border_subtle),
                );
            if track.keys.len() > 1 && track.channel != Channel::IkTarget {
                tracks = tracks.child(
                    div()
                        .absolute()
                        .left(px(x_of(track.keys[0].time)))
                        .top(px(y + 12.0))
                        .w(px(
                            x_of(track.keys[track.keys.len() - 1].time) - x_of(track.keys[0].time)
                        ))
                        .h(px(2.0))
                        .bg(p.border_default),
                );
            }
            for (ki, key) in track.keys.iter().enumerate() {
                let state = if self.selected_key == Some((row, ki)) {
                    KeyState::Selected
                } else if self
                    .agent_keys
                    .contains(&(bone.clone(), (key.time * fps).round() as u32))
                {
                    KeyState::Agent
                } else {
                    KeyState::Default
                };
                tracks = tracks.child(
                    div()
                        .id(("key", row * 1000 + ki))
                        .absolute()
                        .left(px(x_of(self.key_drag_time(
                            row,
                            ki,
                            key.time,
                            clip.length,
                            fps,
                        )) - 6.0))
                        .top(px(y + 7.0))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener({
                                let (bone, channel, from) =
                                    (bone.clone(), channel_arg(track.channel), key.time);
                                move |this, e: &MouseDownEvent, _, cx| {
                                    this.selected_key = Some((row, ki));
                                    this.key_drag = Some(crate::app::KeyDrag {
                                        row,
                                        key: ki,
                                        bone: bone.clone(),
                                        channel,
                                        from,
                                        start_x: f32::from(e.position.x),
                                        dx: 0.0,
                                        px_per_second: width / clip_len.max(1e-3),
                                    });
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            }),
                        )
                        .child(keyframe(state)),
                );
            }
        }
        let ph = x_of(time);
        tracks = tracks
            .child(
                div()
                    .absolute()
                    .left(px(ph - 1.0))
                    .top_0()
                    .w(px(2.0))
                    .h_full()
                    .bg(p.accent),
            )
            .child(
                div()
                    .absolute()
                    .left(px(ph - 10.0))
                    .top(px(2.0))
                    .px(px(4.0))
                    .py(px(1.0))
                    .rounded(px(4.0))
                    .bg(p.accent)
                    .child(text(frame.to_string(), Type::MonoS, p.text_on_accent)),
            );
        let _ = cx;
        div().flex().flex_col().w_full().child(header).child(tracks)
    }
}

fn uv_outline(r: [f32; 4], w: u32, h: u32, color: Rgba, filled: bool) -> Div {
    let sx = 352.0 / w.max(1) as f32;
    let sy = sx;
    let _ = h;
    let mut d = div()
        .absolute()
        .left(px(1.0 + r[0] * sx))
        .top(px(1.0 + r[1] * sy))
        .w(px(((r[2] - r[0]) * sx).max(1.0)))
        .h(px(((r[3] - r[1]) * sy).max(1.0)))
        .border_1()
        .border_color(color);
    if filled {
        d = d.border_2().bg(Rgba { a: 0.25, ..color });
    }
    d
}

fn capital(s: &str) -> String {
    let s = s.replace('_', " ");
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Sweetie 16 (CC0), the palette in the design, used until a project has one.
const SWEETIE16: [u32; 16] = [
    0x1a1c2c, 0x5d275d, 0xb13e53, 0xef7d57, 0xffcd75, 0xa7f070, 0x38b764, 0x257179, 0x29366f,
    0x3b5dc9, 0x41a6f6, 0x73eff7, 0xf4f4f4, 0x94b0c2, 0x566c86, 0x333c57,
];

/// The canonical absolute path of an existing PNG or PSD file, or None.
/// Canonical paths start with `/` (or a drive), so they never read as a flag.
fn safe_image_path(path: &str) -> Option<std::path::PathBuf> {
    let file = std::fs::canonicalize(path).ok()?;
    let ext = file.extension()?.to_str()?.to_ascii_lowercase();
    (file.is_file() && matches!(ext.as_str(), "png" | "psd")).then_some(file)
}

/// `anim.key_move` channel names.
fn channel_arg(c: Channel) -> &'static str {
    match c {
        Channel::Translation => "translation",
        Channel::Rotation => "rotation",
        Channel::Scale => "scale",
        Channel::IkTarget => "ik_target",
        Channel::IkPole => "ik_pole",
    }
}

impl Quarry {
    /// Where a key shows while it is dragged: snapped to whole frames.
    fn key_drag_time(&self, row: usize, key: usize, time: f32, length: f32, fps: f32) -> f32 {
        match &self.key_drag {
            Some(d) if d.row == row && d.key == key => {
                ((time + d.dx / d.px_per_second) * fps)
                    .round()
                    .clamp(0.0, length * fps)
                    / fps
            }
            _ => time,
        }
    }
}

#[cfg(test)]
mod open_tests {
    #[::core::prelude::v1::test]
    fn only_existing_image_files_are_opened() {
        let dir = std::env::temp_dir().join("quarry-open-test");
        let _ = std::fs::create_dir_all(&dir);
        let png = dir.join("t.png");
        std::fs::write(&png, b"x").unwrap();
        assert!(super::safe_image_path(png.to_str().unwrap()).is_some());
        assert!(super::safe_image_path("calc.exe & echo").is_none());
        assert!(super::safe_image_path(dir.to_str().unwrap()).is_none());
        let txt = dir.join("t.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(super::safe_image_path(txt.to_str().unwrap()).is_none());
    }
}
