//! The window's state: the registry (the only way anything changes), the
//! agent socket, panels and tools. Rendering lives in `start`, `editor`,
//! `panels` and `dialogs`; this file holds the state and the actions.

use crate::viewport::{Drag, Viewport};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use quarry_core::model::SelectMode;
use quarry_core::{CommandRegistry, Source};
use quarry_io::autosave::{self, Autosaver, Recovery};
use quarry_io::project::QuarryFiles;
use quarry_io::settings::{Dirs, Settings};
use quarry_io::watcher::ArtWatcher;
use quarry_server::server::Server;
use quarry_server::socket::SocketAddr;
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Start,
    Editor,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SideTab {
    Tileset,
    Uvs,
    Paint,
    Scene,
    Rig,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BottomTab {
    Timeline,
    History,
    Agent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Commands,
    Export,
    Settings,
    Connect,
    Basics,
}

/// Everything the tool rails offer. What a click in the view does depends on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    // Tiles
    Place,
    FillRect,
    EraseTile,
    PickTile,
    Block,
    PlaneLock,
    Crosshair,
    Snap,
    // Scene and mesh
    Select,
    BoxSelect,
    Primitive,
    Extrude,
    Inset,
    Knife,
    Bevel,
    // Paint
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
    PaintSelect,
    VertexColor,
    // UVs
    UvMove,
    UvRotate,
    UvScale,
    Seam,
    // Rig and animate
    BoneAdd,
    Weights,
    WeightFill,
    AutoWeights,
    Ik,
    WeightsView,
    Pose,
    // Transform gizmo
    Translate,
    Spin,
    Grow,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HistoryFilter {
    All,
    Human,
    Agent,
}

/// A menu or select list that is open.
#[derive(Clone, PartialEq, Eq)]
pub enum Menu {
    Bar(&'static str),
    Select(&'static str),
}

/// A number field being dragged; applied as one command on release.
#[derive(Clone, Copy, PartialEq)]
pub struct FieldDrag {
    pub field: Field,
    pub start_x: f32,
    pub delta: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Position(usize),
    Rotation(usize),
    Scale(usize),
    ClipLength,
    Fade,
    BrushSize,
    Strength,
    WeightRadius,
    ExportScale,
    UvOffset(usize),
    UvScale(usize),
    Padding,
}

/// A timeline key being dragged; one `anim.key_move` on release.
pub struct KeyDrag {
    pub row: usize,
    pub key: usize,
    pub bone: String,
    pub channel: &'static str,
    pub from: f32,
    pub start_x: f32,
    pub dx: f32,
    pub px_per_second: f32,
}

/// Selected UV islands dragged on the UV canvas, in whole pixels; applied
/// live and undone step by step so the drag is one undo entry.
pub struct UvDrag {
    pub start: Point<Pixels>,
    pub texels_per_px: f32,
    pub applied: Option<[i32; 2]>,
}

pub struct LogEntry {
    pub time: SystemTime,
    pub command: String,
    pub summary: String,
    pub ok: bool,
    pub ms: u128,
}

pub struct Toast {
    pub title: String,
    pub body: String,
    pub meta: String,
    pub faces: Vec<(String, Vec<u32>)>,
}

pub struct Quarry {
    pub reg: CommandRegistry,
    pub dirs: Option<Dirs>,
    pub settings: Settings,
    server: Option<Server>,
    pub socket: String,
    launch_dir: PathBuf,
    watcher: Option<ArtWatcher>,
    autosaver: Autosaver,
    pub screen: Screen,
    pub overlay: Overlay,
    pub side: SideTab,
    pub bottom: Option<BottomTab>,
    pub tool: Tool,
    pub view: Viewport,
    pub recovery: Option<Recovery>,
    pub thumbs: HashMap<PathBuf, Arc<Image>>,
    pub recent_filter: Entity<InputState>,
    pub scene_filter: Entity<InputState>,
    pub palette_query: Entity<InputState>,
    pub shortcut_filter: Entity<InputState>,
    pub export_path: Entity<InputState>,
    pub agent_log: Vec<LogEntry>,
    pub agent_busy_until: Option<Instant>,
    pub agent_running: Option<String>,
    pub agent_shot: Option<(Arc<Image>, String)>,
    pub history_filter: HistoryFilter,
    pub menu: Option<Menu>,
    pub toast: Option<Toast>,
    pub notice: Option<(String, Instant)>,
    pub closed_sections: HashSet<&'static str>,
    pub closed_tree: HashSet<String>,
    pub paint_rgba: [u8; 4],
    pub brush_px: u32,
    pub pressure: bool,
    pub weight_mode: usize,
    pub weight_strength: f32,
    pub weight_radius: f32,
    pub normalise: bool,
    pub selected_bone: Option<usize>,
    pub selected_key: Option<(usize, usize)>,
    pub playing: bool,
    last_tick: Instant,
    pub onion: bool,
    pub interp: &'static str,
    pub field_drag: Option<FieldDrag>,
    tile_stroke: Option<Vec<[i32; 2]>>,
    /// Box select in the view: start and current corner, window coordinates.
    pub box_drag: Option<(Point<Pixels>, Point<Pixels>, bool)>,
    pub key_drag: Option<KeyDrag>,
    pub gizmo_drag: Option<crate::gizmo::Drag>,
    /// The magnet: gizmo drags snap to the grid, 15° and 0.1×. Off is smooth.
    pub snap: bool,
    pub uv_drag: Option<UvDrag>,
    paint_stroke: Option<Vec<[f32; 3]>>,
    pub export_selected: bool,
    pub export_scale: f32,
    pub connect_tab: usize,
    pub settings_page: usize,
    pub basics_dont_show: bool,
    pub sticky: bool,
    pub palette_index: usize,
    pub paint_image: Option<quarry_core::id::Id<quarry_core::model::Image>>,
    pub crossfade: (Option<String>, Option<String>, f32),
    pub fade_preview: Option<Instant>,
    pub uv_padding: u32,
    pub focus: FocusHandle,
    pub agent_keys: HashSet<(String, u32)>,
    pub images: crate::panels::ImageCache,
    pub tileset_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub timeline_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub rig_note: Option<String>,
}

impl Quarry {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dirs = Dirs::from_env().ok();
        let (settings, _) = dirs.as_ref().map(Settings::load).unwrap_or_default();
        crate::theme::set_light(settings.theme == quarry_io::settings::Theme::Light);

        let mut reg = CommandRegistry::new();
        reg.set_storage(Box::new(QuarryFiles {
            dirs: dirs.clone(),
            thumbnail: Some(quarry_render::thumbnail_png),
            screenshot: Some(quarry_render::screenshot_png),
        }));

        let socket = SocketAddr::from_env().ok();
        let server = socket
            .clone()
            .and_then(|addr| match Server::start(addr, reg.cancel_flag()) {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("agent socket: {e}");
                    None
                }
            });
        let recovery = dirs
            .as_ref()
            .and_then(|d| autosave::recoverable(d).into_iter().next());
        let thumbs = settings
            .recent_files
            .iter()
            .filter_map(|r| {
                let png = quarry_io::project::read_thumbnail(&r.path)?;
                Some((
                    r.path.clone(),
                    Arc::new(Image::from_bytes(ImageFormat::Png, png)),
                ))
            })
            .collect();

        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_string();
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            cx.subscribe(&state, |_, _, _: &InputEvent, cx| cx.notify())
                .detach();
            state
        };
        let palette_query = input("Search commands", window, cx);
        cx.subscribe(&palette_query, |this, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.run_palette_choice(cx);
            } else {
                this.palette_index = 0;
            }
        })
        .detach();

        let autosave_minutes = settings.autosave_minutes;

        let mut app = Self {
            reg,
            dirs,
            settings,
            server,
            socket: socket.map(|s| s.0).unwrap_or_default(),
            launch_dir: std::env::current_dir().unwrap_or_default(),
            watcher: ArtWatcher::new().ok(),
            autosaver: Autosaver::new(autosave_minutes, 0),
            screen: Screen::Start,
            overlay: Overlay::None,
            side: SideTab::Scene,
            bottom: None,
            tool: Tool::Select,
            view: Viewport::new(Default::default()),
            recovery,
            thumbs,
            recent_filter: input("Filter projects", window, cx),
            scene_filter: input("Filter scene", window, cx),
            palette_query,
            shortcut_filter: input("Filter shortcuts", window, cx),
            export_path: input("Where to write the .glb", window, cx),
            agent_log: Vec::new(),
            agent_busy_until: None,
            agent_running: None,
            agent_shot: None,
            history_filter: HistoryFilter::All,
            menu: None,
            toast: None,
            notice: None,
            closed_sections: HashSet::new(),
            closed_tree: HashSet::new(),
            paint_rgba: [239, 125, 87, 255],
            brush_px: 1,
            pressure: true,
            weight_mode: 0,
            weight_strength: 0.6,
            weight_radius: 24.0,
            normalise: true,
            selected_bone: None,
            selected_key: None,
            playing: false,
            last_tick: Instant::now(),
            onion: true,
            interp: "linear",
            field_drag: None,
            tile_stroke: None,
            box_drag: None,
            key_drag: None,
            gizmo_drag: None,
            snap: false,
            uv_drag: None,
            paint_stroke: None,
            export_selected: false,
            export_scale: 1.0,
            connect_tab: 0,
            settings_page: 3,
            basics_dont_show: false,
            sticky: false,
            palette_index: 0,
            paint_image: None,
            crossfade: (None, None, 0.25),
            fade_preview: None,
            uv_padding: 1,
            focus: cx.focus_handle(),
            agent_keys: HashSet::new(),
            images: Default::default(),
            tileset_bounds: Default::default(),
            canvas_bounds: Default::default(),
            timeline_bounds: Default::default(),
            rig_note: None,
        };
        app.sync_component_theme(cx);
        app.start_ticker(cx);
        if let Some(path) = std::env::args().nth(1).filter(|a| a.ends_with(".quarry")) {
            app.open(PathBuf::from(path), cx);
        }
        app.apply_ui_env(cx);
        app
    }

    /// `QUARRY_UI="side=rig;bottom=timeline;overlay=export"` opens panels at
    /// launch, for screenshots and demos.
    fn apply_ui_env(&mut self, cx: &mut Context<Self>) {
        let Ok(spec) = std::env::var("QUARRY_UI") else {
            return;
        };
        for part in spec.split(';') {
            match part.split_once('=').unwrap_or((part, "")) {
                ("side", v) => self.set_side(
                    match v {
                        "tileset" => SideTab::Tileset,
                        "uvs" => SideTab::Uvs,
                        "paint" => SideTab::Paint,
                        "rig" => SideTab::Rig,
                        _ => SideTab::Scene,
                    },
                    cx,
                ),
                ("bottom", v) => {
                    self.bottom = match v {
                        "timeline" => Some(BottomTab::Timeline),
                        "history" => Some(BottomTab::History),
                        "agent" => Some(BottomTab::Agent),
                        _ => None,
                    }
                }
                ("overlay", v) => {
                    self.overlay = match v {
                        "commands" => Overlay::Commands,
                        "export" => Overlay::Export,
                        "settings" => Overlay::Settings,
                        "connect" => Overlay::Connect,
                        "basics" => Overlay::Basics,
                        _ => Overlay::None,
                    }
                }
                ("mode", "face") => {
                    let _ =
                        self.reg
                            .execute("select.mode", json!({ "mode": "face" }), Source::Human);
                    let _ = self.reg.execute("select.all", json!({}), Source::Human);
                }
                ("light", _) => self.set_theme(true, cx),
                ("sample", v) => self.open_sample(v, cx),
                ("bone", v) => self.selected_bone = v.parse().ok(),
                ("run", cmd) => {
                    let _ = self.reg.execute(cmd, json!({}), Source::Human);
                }
                _ => {}
            }
        }
    }

    /// Give the text inputs the design's fonts and colours.
    pub fn sync_component_theme(&self, cx: &mut App) {
        use gpui_kit::component::theme::{Theme, ThemeMode};
        let p = crate::theme::t();
        let mode = if crate::theme::is_light() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        Theme::change(mode, None, cx);
        let theme = Theme::global_mut(cx);
        theme.font_family = crate::theme::SANS.into();
        theme.mono_font_family = crate::theme::MONO.into();
        theme.font_size = px(12.0);
        theme.colors.foreground = p.text_primary.into();
        theme.colors.muted_foreground = p.text_muted.into();
        theme.colors.caret = p.accent.into();
        theme.colors.selection = p.accent_strong_subtle.into();
        theme.colors.background = p.bg_input.into();
        theme.colors.input = p.border_default.into();
    }

    /// 60 Hz: agent requests, live refresh, autosave and animation playback.
    fn start_ticker(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let before = self.reg.state().revision;
        let mut changed = false;
        let server = self.server.take();
        if let Some(server) = &server {
            while let Some(incoming) = server.try_recv() {
                let command = incoming.command.clone();
                let summary_args = summarize_args(&incoming.args);
                let preset = incoming.args["preset"]
                    .as_str()
                    .unwrap_or("view")
                    .to_string();
                let roots = self.agent_roots();
                self.agent_running = Some(command.clone());
                let started = Instant::now();
                let result = server.handle(incoming, &mut self.reg, &roots);
                let ms = started.elapsed().as_millis();
                if command == "view.screenshot"
                    && let Ok(v) = &result
                {
                    self.capture_agent_shot(v, &preset);
                }
                if command == "anim.key_set"
                    && let Ok(v) = &result
                {
                    self.note_agent_key(v);
                }
                self.agent_log.push(LogEntry {
                    time: SystemTime::now(),
                    summary: match &result {
                        Ok(v) => summarize_result(v).unwrap_or(summary_args),
                        Err(e) => e.message.clone(),
                    },
                    ok: result.is_ok(),
                    command,
                    ms,
                });
                if self.agent_log.len() > 500 {
                    self.agent_log.remove(0);
                }
                self.agent_busy_until = Some(Instant::now() + Duration::from_millis(1500));
                if self.screen == Screen::Start && !self.reg.document().objects.is_empty() {
                    self.screen = Screen::Editor;
                }
                changed = true;
            }
            if self
                .agent_busy_until
                .is_some_and(|until| Instant::now() > until)
            {
                self.agent_busy_until = None;
                self.agent_running = None;
                changed = true;
            }
        }
        self.server = server;
        if let Some(watcher) = &mut self.watcher {
            watcher.sync(self.reg.document());
            for id in watcher.changed(self.reg.document()) {
                let old = self
                    .reg
                    .document()
                    .images
                    .get(id)
                    .map(|i| (i.size, i.name.clone()));
                match self.reg.execute(
                    "art.reload",
                    json!({ "image": id.to_string() }),
                    Source::Human,
                ) {
                    Ok(_) => {
                        let doc = self.reg.document();
                        if let (Some((old_size, name)), Some(new)) = (old, doc.images.get(id)) {
                            self.toast = Some(reload_toast(doc, id, &name, old_size, new));
                        }
                    }
                    Err(e) => self.flash(e.message),
                }
                changed = true;
            }
        }
        if let Some(dirs) = &self.dirs {
            let state = self.reg.state();
            if self.autosaver.due(Instant::now(), state.revision) {
                let (doc, path, dirs) = (state.doc.clone(), state.path.clone(), dirs.clone());
                std::thread::spawn(move || {
                    if let Err(e) = autosave::write_autosave(&dirs, &doc, path.as_deref()) {
                        tracing::warn!("{e}");
                    }
                });
            }
        }
        if self.playing && self.reg.document().view.anim.clip.is_some() {
            let fps = self.reg.document().settings.fps.max(1) as f32;
            let frame = Duration::from_secs_f32(1.0 / fps);
            if self.last_tick.elapsed() >= frame {
                self.last_tick = Instant::now();
                let _ = self.reg.execute("anim.play", json!({}), Source::Human);
                changed = true;
            }
        }
        if self
            .fade_preview
            .is_some_and(|t| t.elapsed() > Duration::from_secs(3))
        {
            self.fade_preview = None;
            changed = true;
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(6))
        {
            self.notice = None;
            changed = true;
        }
        if changed || self.reg.state().revision != before {
            self.sync_camera_from_doc();
            cx.notify();
        }
    }

    fn agent_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.launch_dir.clone()];
        if let Some(dir) = self.reg.state().path.as_ref().and_then(|p| p.parent()) {
            roots.push(dir.to_path_buf());
        }
        roots
    }

    fn capture_agent_shot(&mut self, v: &Value, preset: &str) {
        if let Some(png) = v["image"].as_str().and_then(base64_free::decode) {
            let label = format!(
                "view.screenshot · {preset} · {}×{}",
                v["width"].as_u64().unwrap_or(768),
                v["height"].as_u64().unwrap_or(768)
            );
            self.agent_shot = Some((Arc::new(Image::from_bytes(ImageFormat::Png, png)), label));
        }
    }

    fn note_agent_key(&mut self, v: &Value) {
        if let (Some(bone), Some(frame)) = (v["bone"].as_str(), v["frame"].as_u64()) {
            self.agent_keys.insert((bone.to_string(), frame as u32));
        }
    }

    /// The document camera changed (agent, view.frame, presets): follow it
    /// unless the person is dragging.
    pub fn sync_camera_from_doc(&mut self) {
        if self.view.drag.is_none() {
            self.view.camera = self.reg.document().view.camera;
        }
    }

    /// Run a command as the person at the keyboard. Errors show as a notice.
    pub fn run(&mut self, name: &str, args: Value, cx: &mut Context<Self>) -> Option<Value> {
        let result = self.reg.execute(name, args, Source::Human);
        self.sync_camera_from_doc();
        cx.notify();
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.flash(format!("{name}: {}", e.message));
                None
            }
        }
    }

    pub fn flash(&mut self, message: impl Into<String>) {
        self.notice = Some((message.into(), Instant::now()));
    }

    // ---- projects ----

    pub fn new_project(&mut self, cx: &mut Context<Self>) {
        if self
            .run("project.new", json!({ "name": "Untitled" }), cx)
            .is_some()
        {
            self.enter_editor(SideTab::Tileset, cx);
            if !self.settings.recent_files.is_empty() || self.basics_dont_show {
                return;
            }
            self.overlay = Overlay::Basics;
        }
    }

    pub fn open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self
            .run("project.open", json!({ "path": path }), cx)
            .is_some()
        {
            self.remember(&path);
            let side = if self.reg.document().skeletons.is_empty() {
                if self
                    .reg
                    .document()
                    .images
                    .iter()
                    .any(|i| i.role == quarry_core::model::ImageRole::Tileset)
                {
                    SideTab::Tileset
                } else {
                    SideTab::Scene
                }
            } else {
                SideTab::Scene
            };
            self.enter_editor(side, cx);
        }
    }

    fn remember(&mut self, path: &Path) {
        self.settings.add_recent(path);
        if let Some(dirs) = &self.dirs {
            let _ = self.settings.save(dirs);
        }
    }

    pub fn enter_editor(&mut self, side: SideTab, cx: &mut Context<Self>) {
        self.screen = Screen::Editor;
        self.set_side(side, cx);
        if !self.reg.document().objects.is_empty() {
            let _ = self.reg.execute("view.frame", json!({}), Source::Human);
        }
        self.sync_camera_from_doc();
        cx.notify();
    }

    pub fn pick_and_open(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    if path.extension().is_some_and(|e| e == "glb" || e == "gltf") {
                        this.import_gltf(path, cx);
                    } else {
                        this.open(path, cx);
                    }
                });
            }
        })
        .detach();
    }

    pub fn import_gltf(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.screen == Screen::Start {
            let _ = self.run("project.new", json!({ "name": "Imported" }), cx);
        }
        if self
            .run("import.gltf", json!({ "path": path }), cx)
            .is_some()
        {
            self.enter_editor(SideTab::Scene, cx);
        }
    }

    pub fn pick_and_import(&mut self, cx: &mut Context<Self>) {
        self.pick_and_open(cx);
    }

    pub fn save(&mut self, cx: &mut Context<Self>) {
        if self.reg.state().path.is_some() {
            if self.run("project.save", json!({}), cx).is_some()
                && let Some(path) = self.reg.state().path.clone()
            {
                self.remember(&path);
                self.flash("Saved");
            }
            return;
        }
        let dir = self.launch_dir.clone();
        let name = format!("{}.quarry", self.reg.document().name);
        let path = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = path.await {
                let _ = this.update(cx, |this, cx| {
                    if this
                        .run("project.save_as", json!({ "path": path }), cx)
                        .is_some()
                    {
                        this.remember(&path);
                    }
                });
            }
        })
        .detach();
    }

    pub fn recover(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.recovery.take()
            && self
                .run("project.recover", json!({ "autosave": r.autosave }), cx)
                .is_some()
        {
            self.enter_editor(SideTab::Scene, cx);
        }
    }

    pub fn discard_recovery(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = self.recovery.take() {
            autosave::discard(&r);
        }
        cx.notify();
    }

    /// The bundled sample robot (13 bones, two clips) or the CC0 tileset.
    pub fn open_sample(&mut self, which: &str, cx: &mut Context<Self>) {
        let Some(dirs) = self.dirs.clone() else {
            self.flash("No app data folder to unpack the sample into.");
            return;
        };
        let folder = dirs.data.join("samples");
        let _ = std::fs::create_dir_all(&folder);
        let robot = folder.join("sample-robot.quarry");
        let tiles = folder.join("tiles-16px.png");
        let _ = std::fs::write(
            &robot,
            include_bytes!("../../../assets/samples/sample-robot.quarry"),
        );
        let _ = std::fs::write(
            &tiles,
            include_bytes!("../../../assets/samples/tiles-16px.png"),
        );
        if which == "robot" {
            self.open(robot, cx);
        } else {
            let path = folder.join("Tiles sample.quarry");
            if self
                .run(
                    "project.new",
                    json!({ "name": "Tiles sample", "path": path }),
                    cx,
                )
                .is_some()
                && self
                    .run("tile.import_tileset", json!({ "path": tiles }), cx)
                    .is_some()
            {
                let _ = self.run("tile.brush", json!({ "col": 2, "row": 2 }), cx);
                self.enter_editor(SideTab::Tileset, cx);
                self.tool = Tool::Place;
            }
        }
    }

    // ---- panels and tools ----

    pub fn set_side(&mut self, side: SideTab, cx: &mut Context<Self>) {
        self.side = side;
        self.tool = match side {
            SideTab::Tileset => Tool::Place,
            SideTab::Scene => Tool::Select,
            SideTab::Paint => Tool::Pencil,
            SideTab::Uvs => Tool::Select,
            SideTab::Rig => Tool::Select,
        };
        if side == SideTab::Paint || side == SideTab::Uvs {
            let doc = self.reg.document();
            if self
                .paint_image
                .is_none_or(|id| doc.images.get(id).is_none())
            {
                self.paint_image = doc
                    .images
                    .iter()
                    .find(|i| i.role == quarry_core::model::ImageRole::Texture)
                    .map(|i| i.id);
            }
        }
        cx.notify();
    }

    pub fn toggle_bottom(&mut self, tab: BottomTab, cx: &mut Context<Self>) {
        self.bottom = if self.bottom == Some(tab) {
            None
        } else {
            Some(tab)
        };
        cx.notify();
    }

    pub fn toggle_section(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if !self.closed_sections.remove(key) {
            self.closed_sections.insert(key);
        }
        cx.notify();
    }

    pub fn open_section(&self, key: &'static str) -> bool {
        !self.closed_sections.contains(key)
    }

    pub fn set_mode(&mut self, mode: SelectMode, cx: &mut Context<Self>) {
        let name = match mode {
            SelectMode::Object => "object",
            SelectMode::Vertex => "vertex",
            SelectMode::Edge => "edge",
            SelectMode::Face => "face",
        };
        let mut args = json!({ "mode": name });
        if mode != SelectMode::Object {
            let sel = &self.reg.document().selection;
            if let Some(id) = sel.object.or_else(|| sel.objects.iter().next().copied()) {
                args["object"] = json!(id.to_string());
            }
        }
        self.run("select.mode", args, cx);
    }

    pub fn camera_preset(&mut self, preset: &str, cx: &mut Context<Self>) {
        if preset == "persp" {
            self.run("view.camera", json!({ "ortho": false }), cx);
        } else {
            self.run("view.camera", json!({ "preset": preset }), cx);
        }
    }

    pub fn display(&mut self, key: &str, cx: &mut Context<Self>) {
        let d = self.reg.document().view.display;
        let now = match key {
            "wireframe" => d.wireframe,
            "grid" => d.grid,
            "textured" => d.textured,
            "weights" => d.weights,
            "bones" => d.bones,
            _ => return,
        };
        self.run("view.display", json!({ key: !now }), cx);
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        self.run("history.undo", json!({}), cx);
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        self.run("history.redo", json!({}), cx);
    }

    /// Stop: cancel the running agent command and leave the scene as it was.
    pub fn stop_agent(&mut self, cx: &mut Context<Self>) {
        self.reg
            .cancel_flag()
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.agent_running = None;
        self.agent_busy_until = None;
        cx.notify();
    }

    pub fn agent_state(&self) -> crate::ui::AgentState {
        use crate::ui::AgentState;
        if self.agent_running.is_some() {
            AgentState::Working
        } else if self.server.as_ref().is_some_and(|s| s.clients() > 0) {
            AgentState::Connected
        } else {
            AgentState::Off
        }
    }

    // ---- viewport input ----

    pub fn view_mouse_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let orbit =
            e.button == MouseButton::Middle || (e.button == MouseButton::Left && e.modifiers.alt);
        if orbit {
            self.view.drag = Some(Drag {
                last: e.position,
                pan: e.modifiers.shift,
            });
            return;
        }
        if e.button == MouseButton::Left
            && let Some((kind, handles)) = self.gizmo()
            && let Some(h) = crate::gizmo::hit(&handles, e.position)
        {
            self.gizmo_drag = Some(crate::gizmo::Drag::new(kind, h, e.position));
            cx.notify();
            return;
        }
        let (origin, dir) = self.view.ray(e.position);
        let erase = e.button == MouseButton::Right;
        if e.button != MouseButton::Left && !erase {
            return;
        }
        let doc = self.reg.document();
        match self.tool {
            Tool::Place | Tool::Block | Tool::EraseTile | Tool::FillRect => {
                if let Some(cell) = crate::viewport::tile_cell(doc, origin, dir) {
                    self.tile_stroke = Some(vec![cell]);
                    if erase || self.tool == Tool::EraseTile {
                        self.run("tile.erase", json!({ "cells": [cell] }), cx);
                        self.tile_stroke = None;
                    }
                }
            }
            Tool::PickTile => {
                if let Some((id, face, _)) = crate::viewport::pick(doc, origin, dir) {
                    self.run(
                        "tile.eyedrop",
                        json!({ "object": id.to_string(), "face": face }),
                        cx,
                    );
                    self.tool = Tool::Place;
                }
            }
            Tool::Crosshair => {
                let hit = crate::viewport::pick(doc, origin, dir)
                    .map(|h| h.2)
                    .or_else(|| (dir.y.abs() > 1e-6).then(|| origin + dir * (-origin.y / dir.y)));
                if let Some(p) = hit {
                    self.run(
                        "view.cursor",
                        json!({ "position": p.to_array(), "snap_to": "grid" }),
                        cx,
                    );
                }
            }
            Tool::Pencil | Tool::Eraser | Tool::VertexColor => {
                if let Some((_, _, p)) = crate::viewport::pick(doc, origin, dir) {
                    self.paint_stroke = Some(vec![p.to_array()]);
                }
            }
            Tool::Weights | Tool::WeightFill => self.paint_weights(origin, dir, cx),
            Tool::BoxSelect => {
                self.box_drag = Some((e.position, e.position, e.modifiers.shift));
            }
            _ => self.select_at(origin, dir, e.modifiers.shift, cx),
        }
    }

    pub fn view_mouse_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(drag) = self.view.drag {
            let delta = e.position - drag.last;
            self.view.drag_by(delta, drag.pan || e.modifiers.shift);
            self.view.drag = Some(Drag {
                last: e.position,
                ..drag
            });
            cx.notify();
            return;
        }
        if let Some((a, _, add)) = self.box_drag {
            self.box_drag = Some((a, e.position, add));
            cx.notify();
            return;
        }
        if e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (origin, dir) = self.view.ray(e.position);
        let doc = self.reg.document();
        if let Some(stroke) = &mut self.tile_stroke
            && let Some(cell) = crate::viewport::tile_cell(doc, origin, dir)
            && stroke.last() != Some(&cell)
        {
            stroke.push(cell);
            cx.notify();
        }
        if let Some(stroke) = &mut self.paint_stroke
            && let Some((_, _, p)) = crate::viewport::pick(doc, origin, dir)
        {
            stroke.push(p.to_array());
        }
    }

    pub fn view_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.view.drag.take().is_some() {
            let args = self.view.camera_args();
            let _ = self.reg.execute("view.camera", args, Source::Human);
            cx.notify();
        }
        if let Some(cells) = self.tile_stroke.take() {
            if self.tool == Tool::FillRect && cells.len() > 1 {
                let (a, b) = (cells[0], *cells.last().unwrap());
                self.run("tile.fill_rect", json!({ "a": a, "b": b }), cx);
            } else if self.tool != Tool::EraseTile {
                self.run("tile.place", json!({ "cells": cells }), cx);
            }
        }
        if let Some((a, b, add)) = self.box_drag.take() {
            let r = self.view.bounds.get();
            let frac = |p: Point<Pixels>| {
                (
                    (f32::from(p.x - r.origin.x) / f32::from(r.size.width)).clamp(0.0, 1.0),
                    (f32::from(p.y - r.origin.y) / f32::from(r.size.height)).clamp(0.0, 1.0),
                )
            };
            let ((x0, y0), (x1, y1)) = (frac(a), frac(b));
            let rect = [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)];
            if rect[2] - rect[0] > 0.002 && rect[3] - rect[1] > 0.002 {
                let aspect = self.view.aspect();
                self.run(
                    "select.box",
                    json!({ "rect": rect, "aspect": aspect, "add": add }),
                    cx,
                );
            } else {
                let (origin, dir) = self.view.ray(a);
                self.select_at(origin, dir, add, cx);
            }
        }
        if let Some(points) = self.paint_stroke.take() {
            let rgba = if self.tool == Tool::Eraser {
                [0, 0, 0, 0]
            } else {
                self.paint_rgba
            };
            self.run(
                "paint.stroke_3d",
                json!({ "points": points, "rgba": rgba, "size": self.brush_px }),
                cx,
            );
        }
    }

    pub fn view_scroll(&mut self, e: &ScrollWheelEvent, cx: &mut Context<Self>) {
        match e.delta {
            ScrollDelta::Lines(d) => self.view.camera.zoom(0.9f32.powf(d.y)),
            ScrollDelta::Pixels(d) => {
                if e.modifiers.control || e.modifiers.platform {
                    self.view.camera.zoom(0.99f32.powf(f32::from(d.y)));
                } else {
                    self.view.drag_by(point(-d.x, -d.y), e.modifiers.shift);
                }
            }
        }
        let args = self.view.camera_args();
        let _ = self.reg.execute("view.camera", args, Source::Human);
        cx.notify();
    }

    fn select_at(
        &mut self,
        origin: glam::Vec3,
        dir: glam::Vec3,
        add: bool,
        cx: &mut Context<Self>,
    ) {
        let doc = self.reg.document();
        let mode = doc.selection.mode;
        match crate::viewport::pick(doc, origin, dir) {
            Some((id, face, _)) if mode == SelectMode::Face && doc.selection.object == Some(id) => {
                self.run("select.by_ids", json!({ "faces": [face], "add": add }), cx);
            }
            Some((id, face, _))
                if mode != SelectMode::Object && doc.selection.object == Some(id) =>
            {
                let mesh = doc.objects.get(id).and_then(|o| o.mesh.clone());
                if let Some(mesh) = mesh {
                    let corners: Vec<u32> = mesh.faces[face as usize]
                        .corners
                        .iter()
                        .map(|c| c.vertex)
                        .collect();
                    if mode == SelectMode::Vertex {
                        self.run(
                            "select.by_ids",
                            json!({ "vertices": corners, "add": add }),
                            cx,
                        );
                    } else {
                        let edges: Vec<[u32; 2]> = corners
                            .iter()
                            .zip(corners.iter().cycle().skip(1))
                            .map(|(a, b)| [*a.min(b), *a.max(b)])
                            .collect();
                        self.run("select.by_ids", json!({ "edges": edges, "add": add }), cx);
                    }
                }
            }
            Some((id, _, _)) => {
                if mode != SelectMode::Object {
                    self.run("select.mode", json!({ "mode": "object" }), cx);
                }
                self.run(
                    "select.by_ids",
                    json!({ "objects": [id.to_string()], "add": add }),
                    cx,
                );
            }
            None if !add => {
                self.run("select.none", json!({}), cx);
            }
            None => {}
        }
    }

    fn paint_weights(&mut self, origin: glam::Vec3, dir: glam::Vec3, cx: &mut Context<Self>) {
        let doc = self.reg.document();
        let Some((id, face, _)) = crate::viewport::pick(doc, origin, dir) else {
            return;
        };
        let Some(bone) = self.selected_bone_name() else {
            self.flash("Pick a bone in the skeleton list first.");
            return;
        };
        let vertices: Vec<u32> = doc
            .objects
            .get(id)
            .and_then(|o| o.mesh.as_ref())
            .map(|m| {
                m.faces[face as usize]
                    .corners
                    .iter()
                    .map(|c| c.vertex)
                    .collect()
            })
            .unwrap_or_default();
        let mode = if self.tool == Tool::WeightFill {
            "set"
        } else {
            ["add", "subtract", "smooth", "set"][self.weight_mode]
        };
        self.run(
            "rig.weight_paint",
            json!({
                "object": id.to_string(), "bone": bone, "mode": mode,
                "value": self.weight_strength, "vertices": vertices,
            }),
            cx,
        );
    }

    pub fn selected_bone_name(&self) -> Option<String> {
        let skeleton = self.reg.document().skeletons.iter().next()?;
        skeleton
            .bones
            .get(self.selected_bone?)
            .map(|b| b.name.clone())
    }

    // ---- number field drags ----

    pub fn begin_field_drag(&mut self, field: Field, e: &MouseDownEvent, cx: &mut Context<Self>) {
        self.field_drag = Some(FieldDrag {
            field,
            start_x: f32::from(e.position.x),
            delta: 0.0,
        });
        cx.notify();
    }

    pub fn root_mouse_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if let Some(d) = &mut self.field_drag {
            d.delta = f32::from(e.position.x) - d.start_x;
            cx.notify();
        } else if let Some(d) = &mut self.key_drag {
            d.dx = f32::from(e.position.x) - d.start_x;
            cx.notify();
        } else if self.gizmo_drag.is_some() {
            self.gizmo_move(e, cx);
        } else if self.uv_drag.is_some() {
            self.uv_move(e, cx);
        } else if self.view.drag.is_some() || self.box_drag.is_some() {
            self.view_mouse_move(e, cx);
        }
    }

    /// Re-apply the dragged transform: undo the last step, apply the new total,
    /// so the finished drag is one undo entry.
    fn gizmo_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(d) = &self.gizmo_drag else { return };
        let grid = self.reg.document().settings.grid_step;
        // Shift flips the magnet for this drag.
        let free = self.snap == e.modifiers.shift;
        let amount = d.amount(e.position, grid, free);
        let target = (!d.is_identity(amount)).then_some(amount);
        if target == d.applied {
            return;
        }
        let args = d.args(amount);
        if d.applied.is_some() {
            let _ = self.reg.execute("history.undo", json!({}), Source::Human);
        }
        let applied = match target {
            Some(_) => match self.reg.execute("mesh.transform", args, Source::Human) {
                Ok(_) => target,
                Err(err) => {
                    self.flash(format!("mesh.transform: {}", err.message));
                    None
                }
            },
            None => None,
        };
        if let Some(d) = &mut self.gizmo_drag {
            d.applied = applied;
        }
        cx.notify();
    }

    fn uv_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(d) = &self.uv_drag else { return };
        let k = d.texels_per_px;
        let delta = [
            (f32::from(e.position.x - d.start.x) * k).round() as i32,
            (f32::from(e.position.y - d.start.y) * k).round() as i32,
        ];
        let target = (delta != [0, 0]).then_some(delta);
        if target == d.applied {
            return;
        }
        if d.applied.is_some() {
            let _ = self.reg.execute("history.undo", json!({}), Source::Human);
        }
        let applied = match target {
            Some(t) => match self.reg.execute(
                "uv.transform",
                json!({ "translate": [t[0], t[1]] }),
                Source::Human,
            ) {
                Ok(_) => Some(t),
                Err(err) => {
                    self.flash(format!("uv.transform: {}", err.message));
                    None
                }
            },
            None => None,
        };
        if let Some(d) = &mut self.uv_drag {
            d.applied = applied;
        }
        cx.notify();
    }

    /// The gizmo for the current tool and selection, with its handles.
    pub fn gizmo(&self) -> Option<(crate::gizmo::Kind, [crate::gizmo::Handle; 3])> {
        use crate::gizmo::Kind;
        if self.screen != Screen::Editor
            || self.side != SideTab::Scene
            || self.agent_running.is_some()
            || self.animating()
        {
            return None;
        }
        let kind = match self.tool {
            Tool::Select | Tool::Translate => Kind::Move,
            Tool::Spin => Kind::Rotate,
            Tool::Grow => Kind::Scale,
            _ => return None,
        };
        let pivot = self.selection_pivot()?;
        Some((kind, crate::gizmo::handles(&self.view, pivot)?))
    }

    /// The centre of what is selected, in world space.
    pub fn selection_pivot(&self) -> Option<glam::Vec3> {
        let doc = self.reg.document();
        let sel = &doc.selection;
        let points: Vec<glam::Vec3> = if sel.mode == SelectMode::Object {
            sel.objects
                .iter()
                .filter_map(|id| doc.objects.get(*id))
                .map(|o| doc.world_matrix(o).transform_point3(glam::Vec3::ZERO))
                .collect()
        } else {
            let o = sel.object.and_then(|id| doc.objects.get(id))?;
            let mesh = o.mesh.as_ref()?;
            let world = doc.world_matrix(o);
            sel.touched_vertices(mesh)
                .into_iter()
                .filter_map(|v| mesh.positions.get(v as usize))
                .map(|p| world.transform_point3(*p))
                .collect()
        };
        (!points.is_empty())
            .then(|| points.iter().copied().sum::<glam::Vec3>() / points.len() as f32)
    }

    pub fn root_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.uv_drag.take().is_some() {
            cx.notify();
        }
        if self.gizmo_drag.take().is_some() {
            self.sync_camera_from_doc();
            cx.notify();
        }
        if let Some(d) = self.key_drag.take() {
            let fps = self.reg.document().settings.fps.max(1) as f32;
            let length = self.active_clip_length();
            let to = ((d.from + d.dx / d.px_per_second) * fps)
                .round()
                .clamp(0.0, length * fps)
                / fps;
            if (to - d.from).abs() > 1e-4 {
                self.run(
                    "anim.key_move",
                    json!({ "bone": d.bone, "channel": d.channel, "from": d.from, "to": to }),
                    cx,
                );
            }
            cx.notify();
        }
        if let Some(d) = self.field_drag.take() {
            self.apply_field(d.field, d.delta, cx);
        }
        if self.view.drag.is_some()
            || self.tile_stroke.is_some()
            || self.paint_stroke.is_some()
            || self.box_drag.is_some()
        {
            self.view_mouse_up(cx);
        }
    }

    /// Pixels dragged to the value change for each field.
    pub fn field_step(field: Field) -> f32 {
        match field {
            Field::Position(_) => 0.01,
            Field::Rotation(_) => 0.5,
            Field::Scale(_) => 0.01,
            Field::ClipLength | Field::Fade => 0.01,
            Field::BrushSize | Field::Padding => 0.1,
            Field::Strength => 0.005,
            Field::WeightRadius => 0.5,
            Field::ExportScale => 0.01,
            Field::UvOffset(_) => 0.25,
            Field::UvScale(_) => 0.01,
        }
    }

    /// The live preview of a dragged field (`base` is the stored value).
    pub fn field_value(&self, field: Field, base: f32) -> f32 {
        match self.field_drag {
            Some(d) if d.field == field => match field {
                Field::Scale(_) | Field::UvScale(_) => {
                    base * (1.0 + d.delta * Self::field_step(field))
                }
                _ => base + d.delta * Self::field_step(field),
            },
            _ => base,
        }
    }

    fn apply_field(&mut self, field: Field, delta: f32, cx: &mut Context<Self>) {
        let v = delta * Self::field_step(field);
        if v == 0.0 {
            cx.notify();
            return;
        }
        let axis = |i: usize, x: f32, rest: f32| {
            let mut a = [rest; 3];
            a[i] = x;
            a
        };
        match field {
            Field::Position(i) => {
                self.run(
                    "mesh.transform",
                    json!({ "translate": axis(i, v, 0.0) }),
                    cx,
                );
            }
            Field::Rotation(i) => {
                self.run("mesh.transform", json!({ "rotate": axis(i, v, 0.0) }), cx);
            }
            Field::Scale(i) => {
                self.run(
                    "mesh.transform",
                    json!({ "scale": axis(i, (1.0 + v).max(0.01), 1.0) }),
                    cx,
                );
            }
            Field::UvOffset(i) => {
                let mut t = [0.0, 0.0];
                t[i] = v.round();
                self.run("uv.transform", json!({ "translate": t }), cx);
            }
            Field::UvScale(i) => {
                let mut s = [1.0, 1.0];
                s[i] = (1.0 + v).max(0.01);
                self.run("uv.transform", json!({ "scale": s }), cx);
            }
            Field::ClipLength => {
                if let Some(clip) = self.active_clip_name() {
                    let len = self.active_clip_length() + v;
                    self.run(
                        "anim.clip_edit",
                        json!({ "clip": clip, "length": len.max(1.0 / 30.0) }),
                        cx,
                    );
                }
            }
            Field::Fade => self.crossfade.2 = (self.crossfade.2 + v).max(0.0),
            Field::BrushSize => {
                self.brush_px = ((self.brush_px as f32 + v).round() as u32).clamp(1, 64)
            }
            Field::Strength => self.weight_strength = (self.weight_strength + v).clamp(0.0, 1.0),
            Field::WeightRadius => self.weight_radius = (self.weight_radius + v).max(1.0),
            Field::ExportScale => self.export_scale = (self.export_scale + v).max(0.01),
            Field::Padding => {
                self.uv_padding = ((self.uv_padding as f32 + v).round().max(0.0)) as u32
            }
        }
        cx.notify();
    }

    pub fn active_clip_name(&self) -> Option<String> {
        let doc = self.reg.document();
        doc.view
            .anim
            .clip
            .and_then(|id| doc.clips.get(id))
            .map(|c| c.name.clone())
    }

    pub fn active_clip_length(&self) -> f32 {
        let doc = self.reg.document();
        doc.view
            .anim
            .clip
            .and_then(|id| doc.clips.get(id))
            .map_or(1.0, |c| c.length)
    }

    // ---- command palette ----

    /// Commands matching the query, best first: (name, description).
    pub fn palette_matches(&self, cx: &App) -> Vec<(&'static str, &'static str)> {
        let q = self.palette_query.read(cx).value().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut hits: Vec<(i32, &'static str, &'static str)> = self
            .reg
            .commands()
            .filter(|c| !c.name.starts_with("query.") && !c.name.starts_with("control."))
            .filter_map(|c| {
                let hay =
                    format!("{} {}", c.name.replace(['.', '_'], " "), c.description).to_lowercase();
                let title = command_title(c.name).to_lowercase();
                let score: i32 = words
                    .iter()
                    .map(|w| {
                        if title.starts_with(w) {
                            3
                        } else if title.contains(w) || c.name.contains(w) {
                            2
                        } else if hay.contains(w) {
                            1
                        } else {
                            -100
                        }
                    })
                    .sum();
                (score >= 0).then_some((score, c.name, c.description))
            })
            .collect();
        hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        hits.into_iter().map(|(_, n, d)| (n, d)).take(8).collect()
    }

    pub fn run_palette_choice(&mut self, cx: &mut Context<Self>) {
        let matches = self.palette_matches(cx);
        if let Some((name, _)) = matches.get(self.palette_index).copied() {
            self.overlay = Overlay::None;
            self.run_named(name, cx);
        }
    }

    /// Run a command from the palette or a menu with sensible arguments.
    pub fn run_named(&mut self, name: &str, cx: &mut Context<Self>) {
        let sel_object = self
            .reg
            .document()
            .selection
            .objects
            .iter()
            .next()
            .map(|id| id.to_string());
        let args = match name {
            "mesh.add_primitive" => json!({ "kind": "box" }),
            "mesh.extrude" => json!({ "distance": 1.0 }),
            "project.new" => {
                self.new_project(cx);
                return;
            }
            "project.save" | "project.save_as" => {
                self.save(cx);
                return;
            }
            "project.open" | "import.gltf" => {
                self.pick_and_open(cx);
                return;
            }
            "export.run" => {
                self.overlay = Overlay::Export;
                cx.notify();
                return;
            }
            "rig.bind_auto" => json!({ "object": sel_object }),
            "rig.skeleton_new" => json!({}),
            "rig.bone_add" => json!({ "parent": self.selected_bone_name() }),
            "anim.clip_new" => {
                json!({ "name": format!("clip_{}", self.reg.document().clips.len() + 1) })
            }
            "anim.autokey" => json!({ "on": !self.reg.document().view.anim.autokey }),
            "view.camera" => json!({ "preset": "iso" }),
            "view.display" => json!({ "weights": !self.reg.document().view.display.weights }),
            "select.mode" => json!({ "mode": "face" }),
            _ => json!({}),
        };
        self.run(name, args, cx);
    }

    pub fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let cmd = k.modifiers.platform || k.modifiers.control;
        match (cmd, k.modifiers.shift, k.key.as_str()) {
            (true, _, "k") => self.open_palette(window, cx),
            (true, false, "z") => self.undo(cx),
            (true, true, "z") => self.redo(cx),
            (true, _, "s") => self.save(cx),
            (true, _, "o") => self.pick_and_open(cx),
            (true, _, "e") => {
                self.overlay = Overlay::Export;
                cx.notify();
            }
            (_, _, "escape") => {
                self.overlay = Overlay::None;
                self.menu = None;
                cx.notify();
            }
            _ if self.overlay != Overlay::None || self.screen != Screen::Editor => {}
            _ if self.text_input_focused(window, cx) => {}
            (false, _, "f") => {
                self.run("view.frame", json!({}), cx);
            }
            (false, _, "1") => self.set_mode(SelectMode::Object, cx),
            (false, _, "2") => self.set_mode(SelectMode::Vertex, cx),
            (false, _, "3") => self.set_mode(SelectMode::Edge, cx),
            (false, _, "4") => self.set_mode(SelectMode::Face, cx),
            (false, false, "e") => {
                self.run("mesh.extrude", json!({ "distance": 1.0 }), cx);
            }
            (false, false, "g") => {
                self.tool = Tool::Translate;
                cx.notify();
            }
            (false, false, "r") => {
                self.tool = Tool::Spin;
                cx.notify();
            }
            (false, false, "t") => {
                self.tool = Tool::Grow;
                cx.notify();
            }
            (false, _, "space") => {
                self.playing = !self.playing;
                cx.notify();
            }
            (false, _, "delete" | "backspace") => self.delete_selection(cx),
            (false, _, key @ ("w" | "a" | "s" | "d" | "q")) => {
                let c = self.reg.document().view.cursor;
                let step = self.reg.document().settings.grid_step;
                let d = match key {
                    "w" => glam::Vec3::NEG_Z,
                    "s" => glam::Vec3::Z,
                    "a" => glam::Vec3::NEG_X,
                    "d" => glam::Vec3::X,
                    _ => glam::Vec3::NEG_Y,
                };
                self.run(
                    "view.cursor",
                    json!({ "position": (c + d * step).to_array() }),
                    cx,
                );
            }
            (false, true, "e") => {
                let c = self.reg.document().view.cursor;
                let step = self.reg.document().settings.grid_step;
                self.run(
                    "view.cursor",
                    json!({ "position": (c + glam::Vec3::Y * step).to_array() }),
                    cx,
                );
            }
            _ => {}
        }
    }

    fn text_input_focused(&self, window: &Window, cx: &App) -> bool {
        [
            &self.recent_filter,
            &self.scene_filter,
            &self.palette_query,
            &self.shortcut_filter,
            &self.export_path,
        ]
        .iter()
        .any(|s| s.read(cx).focus_handle(cx).is_focused(window))
    }

    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = Overlay::Commands;
        self.palette_index = 0;
        self.palette_query.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        cx.notify();
    }

    pub fn delete_selection(&mut self, cx: &mut Context<Self>) {
        let sel = &self.reg.document().selection;
        match sel.mode {
            SelectMode::Object => {
                let objects: Vec<String> = sel.objects.iter().map(|id| id.to_string()).collect();
                if !objects.is_empty() {
                    self.run("scene.delete", json!({ "objects": objects }), cx);
                }
            }
            SelectMode::Face => {
                self.run("mesh.delete", json!({ "what": "faces" }), cx);
            }
            _ => {
                self.run("mesh.delete", json!({ "what": "vertices" }), cx);
            }
        }
    }

    pub fn export(&mut self, cx: &mut Context<Self>) {
        let typed = self.export_path.read(cx).value().to_string();
        let path = if typed.trim().is_empty() {
            self.default_export_path()
        } else {
            PathBuf::from(expand_home(typed.trim()))
        };
        let objects: Vec<String> = self
            .reg
            .document()
            .selection
            .objects
            .iter()
            .map(|id| id.to_string())
            .collect();
        let mut args = json!({ "path": path, "scale": self.export_scale });
        if self.export_selected {
            args["selection"] = json!("selected");
            args["objects"] = json!(objects);
        }
        if let Some(report) = self.run("export.run", args, cx) {
            self.overlay = Overlay::None;
            let warnings = report["warnings"].as_array().map_or(0, |w| w.len());
            self.flash(format!(
                "Exported {}{}",
                path.display(),
                if warnings > 0 {
                    format!(" with {warnings} warnings")
                } else {
                    String::new()
                }
            ));
        }
    }

    pub fn default_export_path(&self) -> PathBuf {
        let state = self.reg.state();
        let name = format!("{}.glb", slug(&state.doc.name));
        match state.path.as_ref().and_then(|p| p.parent()) {
            Some(dir) => dir.join(name),
            None => self.launch_dir.join(name),
        }
    }

    pub fn set_theme(&mut self, light: bool, cx: &mut Context<Self>) {
        crate::theme::set_light(light);
        self.settings.theme = if light {
            quarry_io::settings::Theme::Light
        } else {
            quarry_io::settings::Theme::Dark
        };
        if let Some(dirs) = &self.dirs {
            let _ = self.settings.save(dirs);
        }
        self.sync_component_theme(cx);
        cx.notify();
    }
}

/// "rig.bind_auto" to "Bind auto".
pub fn command_title(name: &str) -> String {
    let verb = name.split('.').nth(1).unwrap_or(name).replace('_', " ");
    let area = name.split('.').next().unwrap_or("");
    let mut s = match area {
        "mesh" | "tile" | "rig" | "paint" | "uv" | "anim" | "scene" | "view" | "select" => {
            format!("{} · {}", capitalise(&verb), area)
        }
        _ => capitalise(&verb),
    };
    if name == "rig.bind_auto" {
        s = "Auto weights".into();
    }
    s
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

pub fn command_icon(name: &str) -> &'static str {
    match name.split('.').next().unwrap_or("") {
        "mesh" => "cube",
        "tile" => "tiles",
        "paint" => "brush",
        "uv" => "grid",
        "rig" => "bone",
        "anim" => "keyframe",
        "view" => "eye",
        "select" => "pointer",
        "scene" => "layers",
        "project" => "folder",
        "history" => "history",
        "export" | "import" => "export",
        "art" => "image",
        _ => "command",
    }
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_string(),
    }
}

/// "box 1×1×2 at crosshair"-style text from a request's arguments.
fn summarize_args(args: &Value) -> String {
    let Some(obj) = args.as_object() else {
        return String::new();
    };
    obj.iter()
        .take(3)
        .map(|(k, v)| match v {
            Value::String(s) => format!("{k} {s}"),
            Value::Array(a) if a.len() <= 3 => format!(
                "{k} {}",
                a.iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join("×")
            ),
            Value::Array(a) => format!("{k} ×{}", a.len()),
            other => format!("{k} {other}"),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn summarize_result(v: &Value) -> Option<String> {
    let obj = v.as_object()?;
    let mut parts = Vec::new();
    for key in ["name", "object", "placed", "count", "objects", "path"] {
        if let Some(x) = obj.get(key) {
            parts.push(match x {
                Value::String(s) => s.clone(),
                Value::Array(a) => format!("{} {key}", a.len()),
                other => format!("{other} {key}"),
            });
        }
    }
    (!parts.is_empty()).then(|| format!("ok · {}", parts.join(" · ")))
}

fn reload_toast(
    doc: &quarry_core::Document,
    id: quarry_core::id::Id<quarry_core::model::Image>,
    name: &str,
    old: [u32; 2],
    new: &quarry_core::model::Image,
) -> Toast {
    let source = new.source_path.clone().unwrap_or_else(|| name.to_string());
    let file = Path::new(&source)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or(source.clone());
    let now = chrono_like_now();
    let mut faces = Vec::new();
    let mut past = 0;
    if old != new.size {
        for o in doc.objects.iter() {
            let Some(mesh) = &o.mesh else { continue };
            let hits: Vec<u32> = mesh
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    f.image == Some(id)
                        && f.corners
                            .iter()
                            .any(|c| c.uv.x > new.size[0] as f32 || c.uv.y > new.size[1] as f32)
                })
                .map(|(i, _)| i as u32)
                .collect();
            past += hits.len();
            if !hits.is_empty() {
                faces.push((o.id.to_string(), hits));
            }
        }
    }
    let body = if old == new.size {
        "Textures and tiles show the new pixels.".to_string()
    } else {
        format!(
            "The image changed from {}×{} to {}×{}. UVs stayed in pixel positions, so {past} faces now point past the old edge.",
            old[0], old[1], new.size[0], new.size[1]
        )
    };
    Toast {
        title: format!("{file} reloaded"),
        body,
        meta: format!("{now}  ·  {source}"),
        faces,
    }
}

/// Local wall-clock time as HH:MM:SS without a date crate.
pub fn chrono_like_now() -> String {
    clock(SystemTime::now())
}

pub fn clock(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64
        + local_offset_secs();
    let s = secs.rem_euclid(86_400);
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// The local UTC offset, read once from `date +%z` (falls back to UTC).
fn local_offset_secs() -> i64 {
    static OFFSET: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        let out = std::process::Command::new("date")
            .arg("+%z")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        let s = out.trim();
        if s.len() != 5 {
            return 0;
        }
        let sign = if s.starts_with('-') { -1 } else { 1 };
        let h: i64 = s[1..3].parse().unwrap_or(0);
        let m: i64 = s[3..5].parse().unwrap_or(0);
        sign * (h * 3600 + m * 60)
    })
}

/// "2 hours ago" style, for the recent list.
pub fn ago(opened_at: u64) -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let d = now.saturating_sub(opened_at);
    match d {
        0..60 => "Opened just now".into(),
        60..3600 => format!("Opened {} min ago", d / 60),
        3600..7200 => "Opened an hour ago".into(),
        7200..86_400 => format!("Opened {} hours ago", d / 3600),
        86_400..172_800 => "Opened yesterday".into(),
        172_800..604_800 => format!("Opened {} days ago", d / 86_400),
        604_800..1_209_600 => "Opened last week".into(),
        _ => format!("Opened {} weeks ago", d / 604_800),
    }
}

/// Base64 decoding for screenshot replies, without another dependency.
mod base64_free {
    pub fn decode(s: &str) -> Option<Vec<u8>> {
        let val = |c: u8| -> Option<u32> {
            Some(match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return None,
            } as u32)
        };
        let bytes: Vec<u8> = s
            .bytes()
            .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
            .collect();
        let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
        for chunk in bytes.chunks(4) {
            let mut n = 0u32;
            for (i, c) in chunk.iter().enumerate() {
                n |= val(*c)? << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if chunk.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if chunk.len() > 3 {
                out.push(n as u8);
            }
        }
        Some(out)
    }

    #[test]
    fn round_trip() {
        assert_eq!(decode("aGVsbG8=").unwrap(), b"hello");
    }
}
