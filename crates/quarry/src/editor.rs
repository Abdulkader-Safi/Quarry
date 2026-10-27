//! Editor screens 02 to 08: top bar, tool rail, viewport with its floating
//! bars, side and bottom panels, status bar.

use crate::app::{BottomTab, Menu, Overlay, Quarry, SideTab, Tool};
use crate::start::tilde;
use crate::theme::{Type, t, text};
use crate::ui::*;
use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::*;
use quarry_core::model::{Axis as PlaneAxis, SelectMode};
use serde_json::json;

/// (tool, icon, tooltip, key) for each rail, as in the design.
fn rail(side: SideTab, animating: bool) -> Vec<(Tool, &'static str, &'static str, &'static str)> {
    if animating {
        return vec![
            (Tool::Select, "pointer", "Select bone", "Q"),
            (Tool::Pose, "rotate", "Pose", "R"),
            (Tool::UvMove, "move", "Move", "G"),
            (Tool::Ik, "target", "IK target", "T"),
        ];
    }
    match side {
        SideTab::Tileset => vec![
            (Tool::Place, "brush", "Place tiles", "B"),
            (Tool::FillRect, "select-box", "Rectangle fill", "R"),
            (Tool::EraseTile, "eraser", "Erase", "X"),
            (Tool::PickTile, "eyedropper", "Pick tile", "I"),
            (Tool::Block, "cube", "Block brush", "K"),
            (Tool::PlaneLock, "lock", "Plane lock", "L"),
            (Tool::Crosshair, "crosshair", "Place crosshair", "C"),
            (Tool::Snap, "magnet", "Snap", "N"),
        ],
        SideTab::Scene => vec![
            (Tool::Select, "pointer", "Select", "Q"),
            (Tool::BoxSelect, "select-box", "Box select", "B"),
            (Tool::Primitive, "shapes", "Add primitive", "A"),
            (Tool::Extrude, "sledgehammer", "Extrude", "E"),
            (Tool::Inset, "plus", "Make face", "I"),
            (Tool::Knife, "knife", "Split", "K"),
            (Tool::Bevel, "wand", "Merge", "M"),
            (Tool::Snap, "magnet", "Snap", "N"),
        ],
        SideTab::Paint => vec![
            (Tool::Pencil, "brush", "Pencil", "B"),
            (Tool::Eraser, "eraser", "Eraser", "E"),
            (Tool::Fill, "fill", "Fill", "G"),
            (Tool::Eyedropper, "eyedropper", "Eyedropper", "I"),
            (Tool::PaintSelect, "select-box", "Rectangle", "R"),
            (Tool::VertexColor, "sliders", "Vertex colour", "V"),
        ],
        SideTab::Uvs => vec![
            (Tool::Select, "pointer", "Select", "Q"),
            (Tool::UvMove, "move", "Move islands", "G"),
            (Tool::UvRotate, "rotate", "Rotate islands", "R"),
            (Tool::UvScale, "scale", "Scale islands", "S"),
            (Tool::Seam, "knife", "Mark seam", "K"),
        ],
        SideTab::Rig => vec![
            (Tool::Select, "pointer", "Select", "Q"),
            (Tool::BoneAdd, "bone", "Add bone", "B"),
            (Tool::Weights, "brush", "Weight brush", "W"),
            (Tool::WeightFill, "fill", "Weight fill", "G"),
            (Tool::AutoWeights, "wand", "Auto weights", "⌘W"),
            (Tool::Ik, "target", "IK target", "T"),
            (Tool::WeightsView, "eye", "Weights view", "⇧W"),
        ],
    }
}

impl Quarry {
    pub fn animating(&self) -> bool {
        self.bottom == Some(BottomTab::Timeline)
    }

    pub fn render_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = t();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(p.bg_app)
            .child(self.top_bar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(2.0))
                    .child(self.tool_rail(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.0))
                            .child(self.viewport(window, cx))
                            .child(self.bottom_panel(cx)),
                    )
                    .child(self.side_panel(cx)),
            )
            .child(self.status_bar())
    }

    fn top_bar(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let state = self.reg.state();
        let file = state
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.quarry", state.doc.name));
        let dirty = self.reg.is_dirty();
        let mut menus = div().flex().items_center().gap(px(2.0));
        for name in ["File", "Edit", "View", "Mesh", "Rig", "Animate", "Help"] {
            let open = self.menu == Some(Menu::Bar(name));
            menus = menus.child(
                div()
                    .relative()
                    .child(
                        div()
                            .id(name)
                            .px(px(8.0))
                            .py(px(4.0))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .when(open, |d| d.bg(p.bg_hover))
                            .hover(|s| s.bg(t().bg_hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu = if this.menu == Some(Menu::Bar(name)) {
                                    None
                                } else {
                                    Some(Menu::Bar(name))
                                };
                                cx.notify();
                            }))
                            .child(text(name, Type::LabelM, p.text_secondary)),
                    )
                    .when(open, |d| d.child(self.bar_menu(name, cx))),
            );
        }
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.0))
            .h(px(44.0))
            .pl(px(14.0))
            .pr(px(10.0))
            .bg(p.bg_app)
            .border_b_1()
            .border_color(p.border_subtle)
            .child(div().w(px(60.0)))
            .child(
                div()
                    .id("home")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.screen = crate::app::Screen::Start;
                        cx.notify();
                    }))
                    .child(logo_glyph(20.0)),
            )
            .child(menus)
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(div().size(px(6.0)).rounded_full().bg(if dirty {
                        p.warning
                    } else {
                        p.success
                    }))
                    .child(text(file, Type::MonoS, p.text_secondary)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("search")
                    .w(px(220.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| this.open_palette(window, cx)))
                    .child(search_field(
                        text("Search commands", Type::BodyS, p.text_muted).into_any_element(),
                        "⌘K",
                    )),
            )
            .child(
                div()
                    .id("agent-pill")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.overlay = Overlay::Connect;
                        cx.notify();
                    }))
                    .child(agent_pill(self.agent_state())),
            )
            .child(
                icon_button("settings", "settings", false, "Settings", "⌘,").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.overlay = Overlay::Settings;
                        cx.notify();
                    },
                )),
            )
            .child(
                button("export", ButtonStyle::Primary, "Export", Some("export")).on_click(
                    cx.listener(|this, _, window, cx| {
                        let path = tilde(&this.default_export_path());
                        this.export_path
                            .update(cx, |s, cx| s.set_value(path, window, cx));
                        this.overlay = Overlay::Export;
                        cx.notify();
                    }),
                ),
            )
    }

    fn bar_menu(&self, name: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let items: Vec<(&str, &str, &str)> = match name {
            "File" => vec![
                ("New project", "⌘N", "project.new"),
                ("Open…", "⌘O", "project.open"),
                ("Save", "⌘S", "project.save"),
                ("Import .glb or .gltf…", "", "import.gltf"),
                ("Export…", "⌘E", "export.run"),
            ],
            "Edit" => vec![
                ("Undo", "⌘Z", "history.undo"),
                ("Redo", "⇧⌘Z", "history.redo"),
                ("Select all", "⌘A", "select.all"),
                ("Select none", "", "select.none"),
                ("Invert selection", "", "select.invert"),
            ],
            "View" => vec![
                ("Frame all", "F", "view.frame"),
                ("Iso view", "", "view.camera"),
                ("Toggle wireframe", "", "@wireframe"),
                ("Toggle grid", "", "@grid"),
                ("Light theme", "", "@light"),
                ("Dark theme", "", "@dark"),
            ],
            "Mesh" => vec![
                ("Add box", "", "@box"),
                ("Add cylinder", "", "@cylinder"),
                ("Add cone", "", "@cone"),
                ("Add sphere", "", "@sphere"),
                ("Add wedge", "", "@wedge"),
                ("Add plane", "", "@plane"),
                ("Extrude", "E", "mesh.extrude"),
                ("Merge", "M", "mesh.merge"),
                ("Split", "", "mesh.split"),
                ("Make face", "", "mesh.make_face"),
                ("Flip normals", "", "mesh.flip_normals"),
            ],
            "Rig" => vec![
                ("New skeleton", "", "rig.skeleton_new"),
                ("Add bone", "B", "rig.bone_add"),
                ("Auto weights", "⌘W", "rig.bind_auto"),
                ("Mirror bones", "", "rig.mirror_bones"),
                ("Reset pose", "", "rig.pose_reset"),
            ],
            "Animate" => vec![
                ("New clip", "", "anim.clip_new"),
                ("Play or pause", "Space", "@play"),
                ("Stop", "", "anim.stop"),
                ("Auto-key on or off", "K", "anim.autokey"),
            ],
            _ => vec![
                ("The basics", "", "@basics"),
                ("Connect Claude", "", "@connect"),
                ("Keyboard shortcuts", "", "@keys"),
            ],
        };
        let mut m = menu(230.0);
        for (i, (label, key, action)) in items.into_iter().enumerate() {
            let action = action.to_string();
            m = m.child(
                menu_row((name, i), label.to_string(), key, true).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.menu = None;
                        this.menu_action(&action, cx);
                    },
                )),
            );
        }
        deferred(anchored().child(div().mt(px(28.0)).child(m))).with_priority(1)
    }

    fn menu_action(&mut self, action: &str, cx: &mut Context<Self>) {
        match action {
            "@wireframe" => self.display("wireframe", cx),
            "@grid" => self.display("grid", cx),
            "@light" => self.set_theme(true, cx),
            "@dark" => self.set_theme(false, cx),
            "@play" => self.playing = !self.playing,
            "@basics" => self.overlay = Overlay::Basics,
            "@connect" => self.overlay = Overlay::Connect,
            "@keys" => {
                self.settings_page = 3;
                self.overlay = Overlay::Settings;
            }
            shape if shape.starts_with('@') => {
                self.run("mesh.add_primitive", json!({ "kind": &shape[1..] }), cx);
                self.set_side(SideTab::Scene, cx);
            }
            name => self.run_named(name, cx),
        }
        cx.notify();
    }

    fn tool_rail(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let tools = rail(self.side, self.animating());
        let mut d = div()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .p(px(6.0))
            .h_full()
            .bg(p.bg_panel);
        let split = match self.side {
            SideTab::Tileset if !self.animating() => Some(5),
            _ => None,
        };
        for (i, (tool, icon_name, tip, key)) in tools.into_iter().enumerate() {
            if Some(i) == split {
                d = d.child(div().w(px(20.0)).h(px(1.0)).bg(p.border_subtle));
            }
            let on = self.tool == tool || (tool == Tool::Snap && self.snap);
            d = d.child(
                icon_button(("tool", i), icon_name, on, tip, key)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_tool(tool, cx))),
            );
        }
        d
    }

    /// Tools that act right away (add a primitive, auto weights) run now; the
    /// rest become the active tool for clicks in the view.
    fn pick_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        match tool {
            Tool::Primitive => {
                self.run("mesh.add_primitive", json!({ "kind": "box" }), cx);
            }
            Tool::Extrude => {
                self.run("mesh.extrude", json!({ "distance": 1.0 }), cx);
            }
            Tool::Inset => {
                self.run("mesh.make_face", json!({}), cx);
            }
            Tool::Knife => {
                self.run("mesh.split", json!({}), cx);
            }
            Tool::Bevel => {
                self.run("mesh.merge", json!({}), cx);
            }
            Tool::AutoWeights => {
                let object = self
                    .reg
                    .document()
                    .selection
                    .objects
                    .iter()
                    .next()
                    .map(|i| i.to_string());
                self.run("rig.bind_auto", json!({ "object": object }), cx);
            }
            Tool::WeightsView => self.display("weights", cx),
            Tool::BoneAdd => {
                self.run(
                    "rig.bone_add",
                    json!({ "parent": self.selected_bone_name() }),
                    cx,
                );
            }
            Tool::Seam => {
                self.run("uv.mark_seam", json!({}), cx);
            }
            Tool::PlaneLock => {
                let next = match self.reg.document().view.tile.axis {
                    PlaneAxis::Y => "z",
                    PlaneAxis::Z => "x",
                    PlaneAxis::X => "y",
                };
                self.run("tile.plane", json!({ "axis": next }), cx);
            }
            Tool::Snap => {
                self.snap = !self.snap;
                let step = fmt_num(self.reg.document().settings.grid_step);
                self.flash(if self.snap {
                    format!("Snap on: {step} m, 15°, 0.1×. Hold Shift to drag freely.")
                } else {
                    "Snap off: drags are smooth. Hold Shift to snap.".to_string()
                });
            }
            other => self.tool = other,
        }
        cx.notify();
    }

    fn viewport(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = t();
        let revision = self.reg.state().revision;
        let image = self.view.frame(self.reg.document(), revision, window);
        let agent = self.agent_running.is_some();
        let bounds = self.view.bounds.clone();
        let mut v = div()
            .id("viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .bg(rgb(0x121316))
            .child(crate::viewport::measure(bounds))
            .children(image.map(|i| img(i).absolute().inset_0().size_full()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.view_mouse_down(e, cx)),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, e, _, cx| this.view_mouse_down(e, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e, _, cx| this.view_mouse_down(e, cx)),
            )
            .on_mouse_move(cx.listener(|this, e, _, cx| this.view_mouse_move(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.view_mouse_up(cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _, _, cx| this.view_mouse_up(cx)),
            )
            .on_scroll_wheel(cx.listener(|this, e, _, cx| this.view_scroll(e, cx)));
        if agent {
            v = v.border_2().border_color(p.agent);
        }
        let top_left = match self.side {
            SideTab::Scene | SideTab::Uvs | SideTab::Rig if !self.animating() => {
                Some(self.mode_segmented(cx))
            }
            _ => None,
        };
        if self.reg.document().view.display.bones {
            let mut bones = Vec::new();
            for skeleton in self.reg.document().skeletons.iter() {
                let posed = skeleton.bones.iter().zip(skeleton.pose_world());
                for (i, (bone, world)) in posed.enumerate() {
                    let length = bone.head.distance(bone.tail);
                    let head = self.view.project(world.transform_point3(glam::Vec3::ZERO));
                    let tail = self
                        .view
                        .project(world.transform_point3(glam::Vec3::Y * length));
                    if let (Some(h), Some(t)) = (head, tail) {
                        let picked = self.side == SideTab::Rig && self.selected_bone == Some(i);
                        bones.push((h, t, picked));
                    }
                }
            }
            if !bones.is_empty() {
                v = v.child(crate::gizmo::paint_bones(bones));
            }
        }
        if let Some((kind, handles)) = self.gizmo() {
            let active = self.gizmo_drag.as_ref().map(|d| d.axis);
            v = v.child(crate::gizmo::paint(handles, kind, active));
            if let Some(d) = &self.gizmo_drag
                && let Some(amount) = d.applied
            {
                let o = self.view.bounds.get().origin;
                let at = handles[0].from;
                v = v.child(
                    div()
                        .absolute()
                        .left(at.x - o.x + px(12.0))
                        .top(at.y - o.y + px(12.0))
                        .child(tooltip(d.label(amount), "")),
                );
            }
        }
        if let Some((a, b, _)) = self.box_drag {
            let o = self.view.bounds.get().origin;
            let (x0, y0) = (a.x.min(b.x) - o.x, a.y.min(b.y) - o.y);
            let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
            v = v.child(
                div()
                    .absolute()
                    .left(x0)
                    .top(y0)
                    .w(w)
                    .h(h)
                    .border_1()
                    .border_color(p.accent)
                    .bg(Rgba {
                        a: 0.12,
                        ..p.accent
                    }),
            );
        }
        // While the agent runs, its banner takes the top edge (design 08).
        if !agent {
            let toolbar = self.viewport_toolbar(cx);
            v = v.child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .left(px(12.0))
                    .flex()
                    .gap(px(8.0))
                    .children(top_left)
                    .child(toolbar)
                    .map(shield),
            );
            v = v.child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .right(px(12.0))
                    .child(self.presets(cx))
                    .map(shield),
            );
        }
        v = v.child(
            div()
                .absolute()
                .bottom(px(12.0))
                .left(px(12.0))
                .child(self.hint_bar())
                .map(shield),
        );
        v = v.child(
            div()
                .absolute()
                .bottom(px(12.0))
                .right(px(12.0))
                .child(axis_gizmo(&self.view.camera))
                .map(shield),
        );
        if let Some(cmd) = self.agent_running.clone() {
            v = v.child(
                div()
                    .absolute()
                    .top(px(10.0))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(self.agent_banner(cmd, cx).map(shield)),
            );
        }
        if self.reg.document().objects.is_empty() {
            v = v.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(12.0))
                            .child(text("An empty scene", Type::H3, p.text_primary))
                            .child(text(
                                "Place tiles from the Tileset tab, add a shape, import a glTF model, or ask the agent.",
                                Type::BodyS,
                                p.text_secondary,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .map(shield)
                                    .child(button("empty-box", ButtonStyle::Secondary, "Add a box", Some("cube")).on_click(cx.listener(|this, _, _, cx| {
                                        this.run("mesh.add_primitive", json!({ "kind": "box" }), cx);
                                        this.set_side(SideTab::Scene, cx);
                                    })))
                                    .child(button("empty-import", ButtonStyle::Secondary, "Import .glb", Some("file")).on_click(cx.listener(|this, _, _, cx| this.pick_and_import(cx)))),
                            ),
                    ),
            );
        }
        v
    }

    fn mode_segmented(&mut self, cx: &mut Context<Self>) -> Div {
        let mode = self.reg.document().selection.mode;
        let active = match mode {
            SelectMode::Object => 0,
            SelectMode::Vertex => 1,
            SelectMode::Edge => 2,
            SelectMode::Face => 3,
        };
        let entity = cx.entity();
        segmented(
            "mode",
            &["Object", "Vertex", "Edge", "Face"],
            active,
            false,
            move |i, _, cx| {
                entity.update(cx, |this, cx| {
                    let m = [
                        SelectMode::Object,
                        SelectMode::Vertex,
                        SelectMode::Edge,
                        SelectMode::Face,
                    ][i];
                    this.set_mode(m, cx);
                })
            },
        )
    }

    fn viewport_toolbar(&mut self, cx: &mut Context<Self>) -> Div {
        let p = t();
        let d = self.reg.document().view.display;
        let tool = self.tool;
        let item =
            |id: &'static str,
             icon_name: &'static str,
             on: bool,
             tip: &'static str,
             key: &'static str| { icon_button(id, icon_name, on, tip, key) };
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(4.0))
            .bg(p.bg_panel)
            .border_1()
            .border_color(p.border_default)
            .rounded(px(8.0))
            .shadow(crate::theme::popover_shadow())
            .child(
                item("vt-select", "pointer", tool == Tool::Select, "Select", "Q").on_click(
                    cx.listener(|this, _, _, cx| {
                        this.tool = Tool::Select;
                        cx.notify();
                    }),
                ),
            )
            .child(
                item(
                    "vt-move",
                    "move",
                    tool == Tool::Translate,
                    "Move · drag a handle",
                    "G",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Tool::Translate;
                    cx.notify();
                })),
            )
            .child(
                item(
                    "vt-rotate",
                    "rotate",
                    tool == Tool::Spin,
                    "Rotate · drag a handle",
                    "R",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Tool::Spin;
                    cx.notify();
                })),
            )
            .child(
                item(
                    "vt-scale",
                    "scale",
                    tool == Tool::Grow,
                    "Scale · drag a handle",
                    "T",
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.tool = Tool::Grow;
                    cx.notify();
                })),
            )
            .child(divider_v(20.0))
            .child(
                item("vt-snap", "magnet", self.snap, "Snap to grid", "N")
                    .on_click(cx.listener(|this, _, _, cx| this.pick_tool(Tool::Snap, cx))),
            )
            .child(
                item("vt-grid", "sidebar-left", d.grid, "Grid", "⇧G")
                    .on_click(cx.listener(|this, _, _, cx| this.display("grid", cx))),
            )
            .child(
                item("vt-wire", "eye", d.wireframe, "Wireframe", "⇧Z")
                    .on_click(cx.listener(|this, _, _, cx| this.display("wireframe", cx))),
            )
            .child(
                item("vt-shot", "camera", false, "Frame all", "F").on_click(cx.listener(
                    |this, _, _, cx| {
                        this.run("view.frame", json!({}), cx);
                    },
                )),
            )
    }

    fn presets(&mut self, cx: &mut Context<Self>) -> Div {
        let cam = self.view.camera;
        let options = ["Persp", "Iso", "Front", "Top", "Side"];
        let active = if !cam.ortho {
            0
        } else if cam.pitch > 1.5 {
            3
        } else if cam.pitch.abs() < 0.01 && cam.yaw.abs() < 0.01 {
            2
        } else if cam.pitch.abs() < 0.01 {
            4
        } else {
            1
        };
        let entity = cx.entity();
        segmented("preset", &options, active, false, move |i, _, cx| {
            entity.update(cx, |this, cx| {
                this.camera_preset(["persp", "iso", "front", "top", "side"][i], cx)
            })
        })
    }

    fn hint_bar(&self) -> Div {
        let p = t();
        let doc = self.reg.document();
        let mut bar = floating_bar();
        match self.side {
            SideTab::Tileset if !self.animating() => {
                let tile = doc.view.tile;
                let axis = match tile.axis {
                    PlaneAxis::X => "X",
                    PlaneAxis::Y => "Y",
                    PlaneAxis::Z => "Z",
                };
                let brush = tile.brush.map_or("none".to_string(), |b| {
                    format!(
                        "{} × {}  ·  rot {}°  ·  {}",
                        b.width,
                        b.height,
                        b.rotate as u32 * 90,
                        if b.flip || b.mirror {
                            "flipped"
                        } else {
                            "no flip"
                        }
                    )
                });
                bar = bar
                    .child(text("Plane", Type::Caption, p.text_muted))
                    .child(text(
                        format!(
                            "{axis}  ·  height {}  ·  tilt {}°",
                            fmt_num(doc.view.cursor.y),
                            tile.tilt
                        ),
                        Type::MonoS,
                        p.text_primary,
                    ))
                    .child(divider_v(14.0))
                    .child(text("Brush", Type::Caption, p.text_muted))
                    .child(text(brush, Type::MonoS, p.text_primary))
                    .child(divider_v(14.0))
                    .child(text(
                        if self.sticky { "Sticky" } else { "Free" },
                        Type::LabelS,
                        p.text_secondary,
                    ));
            }
            _ => {
                let mirror = doc
                    .selection
                    .object
                    .and_then(|id| doc.objects.get(id))
                    .and_then(|o| o.mesh.as_ref())
                    .and_then(|m| m.mirror)
                    .map_or("off".to_string(), |a| format!("{a:?}  live"));
                bar = bar
                    .child(text("Mirror", Type::Caption, p.text_muted))
                    .child(text(mirror, Type::MonoS, p.text_primary))
                    .child(divider_v(14.0));
                for (label, key) in [
                    ("Extrude", "E"),
                    ("Frame", "F"),
                    ("Modes", "1–4"),
                    ("Orbit", "⌥ drag"),
                ] {
                    bar = bar
                        .child(text(label, Type::LabelS, p.text_secondary))
                        .child(text(key, Type::MonoS, p.text_muted));
                }
            }
        }
        bar
    }

    fn agent_banner(&mut self, command: String, cx: &mut Context<Self>) -> Div {
        let p = t();
        let target = self
            .reg
            .document()
            .selection
            .objects
            .iter()
            .next()
            .and_then(|id| self.reg.document().objects.get(*id))
            .map(|o| format!("on {}", o.name))
            .unwrap_or_default();
        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .pl(px(12.0))
            .pr(px(6.0))
            .py(px(6.0))
            .bg(p.bg_panel)
            .border_1()
            .border_color(p.agent)
            .rounded(px(8.0))
            .shadow(crate::theme::popover_shadow())
            .child(icon("plug", 16.0, p.agent))
            .child(text("Agent is running", Type::BodyS, p.text_secondary))
            .child(text(command, Type::MonoS, p.text_primary))
            .child(text(target, Type::BodyS, p.text_secondary))
            .child(divider_v(14.0))
            .child(text(
                format!("step {}", self.agent_log.len()),
                Type::MonoS,
                p.text_muted,
            ))
            .child(
                button("stop", ButtonStyle::Danger, "Stop", Some("stop"))
                    .on_click(cx.listener(|this, _, _, cx| this.stop_agent(cx))),
            )
    }

    fn status_bar(&self) -> Div {
        let p = t();
        let doc = self.reg.document();
        let state = self.reg.state();
        let dirty = self.reg.is_dirty();
        let file = state
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.quarry", doc.name));
        let (tris, verts) = doc
            .objects
            .iter()
            .filter_map(|o| doc.drawn_mesh(o))
            .fold((0, 0), |a, m| {
                (a.0 + m.triangle_count(), a.1 + m.positions.len())
            });
        let sel = &doc.selection;
        let selection = match sel.mode {
            SelectMode::Object if sel.objects.len() == 1 => {
                let name = sel
                    .objects
                    .iter()
                    .next()
                    .and_then(|id| doc.objects.get(*id))
                    .map(|o| o.name.clone())
                    .unwrap_or_default();
                format!("sel 1 obj · {name}")
            }
            SelectMode::Object => format!("sel {} obj", sel.objects.len()),
            SelectMode::Vertex => format!("sel {} verts", sel.vertices.len()),
            SelectMode::Edge => format!("sel {} edges", sel.edges.len()),
            SelectMode::Face => format!("sel {} faces", sel.faces.len()),
        };
        let middle = match &self.agent_running {
            Some(cmd) => format!("{file}   ·   agent: {cmd}"),
            None => format!(
                "{file}   ·   {}   ·   {}",
                tool_name(self.tool),
                if self.snap {
                    format!("snap {} m", fmt_num(doc.settings.grid_step))
                } else {
                    "snap off".to_string()
                }
            ),
        };
        let notice = self.notice.as_ref().map(|(m, _)| m.clone());
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(16.0))
            .h(px(24.0))
            .px(px(12.0))
            .bg(p.bg_app)
            .border_t_1()
            .border_color(p.border_subtle)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().size(px(6.0)).rounded_full().bg(if dirty {
                        p.warning
                    } else {
                        p.success
                    }))
                    .child(text(
                        if dirty { "Unsaved changes" } else { "Saved" },
                        Type::MonoS,
                        p.text_secondary,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(match notice {
                        Some(n) => text(n, Type::MonoS, p.text_primary),
                        None => text(middle, Type::MonoS, p.text_muted),
                    }),
            )
            .child(text(selection, Type::MonoS, p.text_secondary))
            .child(text(
                format!("{} tris · {} verts", thousands(tris), thousands(verts)),
                Type::MonoS,
                p.text_secondary,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon(
                        "plug",
                        12.0,
                        match self.agent_state() {
                            AgentState::Off => p.text_muted,
                            AgentState::Working => p.agent,
                            AgentState::Connected => p.success,
                        },
                    ))
                    .child(text(
                        if self.socket.is_empty() {
                            "mcp off"
                        } else {
                            "mcp ready"
                        },
                        Type::MonoS,
                        p.text_secondary,
                    )),
            )
            .child(text(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                Type::MonoS,
                p.text_muted,
            ))
    }

    pub fn search_input(&self) -> AnyElement {
        Input::new(&self.scene_filter)
            .appearance(false)
            .into_any_element()
    }
}

/// Floating bars over the view keep their clicks: without this a click on a
/// toolbar button also reaches the view underneath and changes the selection.
fn shield(d: Div) -> Div {
    d.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
}

/// The small X/Y/Z gizmo in the corner, turning with the camera.
fn axis_gizmo(cam: &quarry_core::camera::OrbitCamera) -> Div {
    let p = t();
    let view = cam.view();
    let mut g = div()
        .relative()
        .size(px(64.0))
        .rounded_full()
        .bg(hsla(0.0, 0.0, 0.0, 0.25));
    let mut axes = [
        (glam::Vec3::X, p.axis_x, "X"),
        (glam::Vec3::Y, p.axis_y, "Y"),
        (glam::Vec3::Z, p.axis_z, "Z"),
    ]
    .map(|(a, c, l)| (view.transform_vector3(a), c, l));
    axes.sort_by(|a, b| a.0.z.total_cmp(&b.0.z));
    for (v, color, letter) in axes {
        let (x, y) = (32.0 + v.x * 24.0, 32.0 - v.y * 24.0);
        g = g.child(
            div()
                .absolute()
                .left(px(x - 7.0))
                .top(px(y - 7.0))
                .size(px(14.0))
                .rounded_full()
                .bg(color)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .font_family(crate::theme::SANS)
                        .font_weight(FontWeight::MEDIUM)
                        .text_size(px(9.0))
                        .text_color(p.text_on_axis)
                        .child(letter),
                ),
        );
    }
    g
}

pub fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 1e-4 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Place => "Tile brush",
        Tool::FillRect => "Rectangle fill",
        Tool::EraseTile => "Erase tiles",
        Tool::PickTile => "Pick tile",
        Tool::Crosshair => "Crosshair",
        Tool::Pencil => "Pencil",
        Tool::Eraser => "Eraser",
        Tool::Weights => "Weight brush",
        Tool::WeightFill => "Weight fill",
        _ => "Select",
    }
}
