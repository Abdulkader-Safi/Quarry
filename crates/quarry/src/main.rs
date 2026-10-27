//! Quarry: the editor window (start screen, editor, dialogs) built from the
//! Figma design, plus the headless server and the M0 renderer checks.

mod app;
mod assets;
mod dialogs;
mod editor;
mod gizmo;
mod keys;
mod panels;
mod start;
mod theme;
mod ui;
mod viewport;

#[cfg(target_os = "macos")]
mod pen;
#[cfg(target_os = "macos")]
mod present;
#[cfg(not(target_os = "macos"))]
mod readback;

use gpui_kit::*;
use std::time::{Duration, Instant};

const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1000;

#[cfg(target_os = "macos")]
type Presenter = present::Presenter;
#[cfg(not(target_os = "macos"))]
type Presenter = readback::Presenter;

struct Spike {
    renderer: quarry_render::Renderer,
    presenter: Presenter,
    started: Instant,
    report_at: Instant,
    frames: u32,
    quit_after: Option<Duration>,
    /// Click-to-frame latencies in ms; `--latency-check` posts its own clicks (macOS).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    latencies: Vec<f64>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    latency_check: bool,
}

impl Spike {
    /// Click (event timestamp) to the end of the frame that follows it. The
    /// display shows that frame at the next refresh, up to one interval later.
    #[cfg(target_os = "macos")]
    fn measure_latency(&mut self, elapsed: Duration, cx: &mut Context<Self>) {
        if let Some(clicked) = pen::take_click() {
            let ms = (pen::uptime() - clicked) * 1000.0;
            println!("click_to_frame_ms={ms:.1}");
            self.latencies.push(ms);
        }
        if !self.latency_check || elapsed < Duration::from_secs(2) {
            return;
        }
        if self.latencies.len() >= 30 {
            let mut sorted = self.latencies.clone();
            sorted.sort_by(f64::total_cmp);
            println!(
                "latency samples={} median_ms={:.1} max_ms={:.1}",
                sorted.len(),
                sorted[sorted.len() / 2],
                sorted[sorted.len() - 1]
            );
            cx.quit();
        } else if self.frames.is_multiple_of(10) {
            pen::post_click(
                objc2::MainThreadMarker::new().expect("render runs on the main thread"),
            );
        }
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let elapsed = self.started.elapsed();
        #[cfg(target_os = "macos")]
        {
            self.renderer.render(elapsed.as_secs_f32());
            self.presenter.present();
        }
        #[cfg(not(target_os = "macos"))]
        self.presenter.render(&self.renderer, elapsed.as_secs_f32());

        #[cfg(target_os = "macos")]
        self.measure_latency(elapsed, cx);

        self.frames += 1;
        let since_report = self.report_at.elapsed();
        if since_report >= Duration::from_secs(1) {
            println!("fps={:.1}", self.frames as f64 / since_report.as_secs_f64());
            self.frames = 0;
            self.report_at = Instant::now();
        }
        if self.quit_after.is_some_and(|limit| elapsed >= limit) {
            cx.quit();
        }

        window.request_animation_frame();
        #[cfg(target_os = "macos")]
        let readout = pen::reading();
        #[cfg(not(target_os = "macos"))]
        let readout = "pressure: not wired on this OS (M0 pen spike is macOS only)".to_string();
        div()
            .size_full()
            .relative()
            .child(self.presenter.element())
            .child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .left(px(12.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .bg(rgb(0x000000))
                    .text_color(rgb(0xffffff))
                    .child(readout),
            )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let number = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<f32>().ok())
    };
    let has = |name: &str| args.iter().any(|a| a == name);

    if has("--headless") {
        std::process::exit(match headless(&args) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{e}");
                1
            }
        });
    }

    if has("--bench") || has("--check") || has("--present-check") {
        let frames = number("--frames").unwrap_or(300.0) as u32;
        if has("--check") {
            match quarry_render::Renderer::new(WIDTH, HEIGHT) {
                Ok(r) => {
                    r.render(0.5);
                    let pixels = r.read_target();
                    println!(
                        "readback bytes={} distinct_colors={}",
                        pixels.len(),
                        distinct_colors(&pixels)
                    );
                }
                Err(e) => eprintln!("check failed: {e}"),
            }
            return;
        }
        if has("--present-check") {
            #[cfg(target_os = "macos")]
            present_check();
            #[cfg(not(target_os = "macos"))]
            println!("present-check is macOS only; this platform uses the readback path");
            return;
        }
        match quarry_render::bench(WIDTH, HEIGHT, frames) {
            Ok(()) => return,
            Err(e) => {
                eprintln!("bench failed: {e}");
                std::process::exit(1);
            }
        }
    }

    let quit_after = number("--secs").map(Duration::from_secs_f32);
    let latency_check = has("--latency-check");
    if !has("--spike") {
        run_editor(quit_after);
        return;
    }

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        #[cfg(target_os = "macos")]
        pen::install();
        let result = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(80.0), px(80.0)),
                    size(px(WIDTH as f32), px(HEIGHT as f32)),
                ))),
                ..Default::default()
            },
            cx,
            move |_, cx| {
                cx.new(move |_| {
                    let renderer =
                        quarry_render::Renderer::new(WIDTH, HEIGHT).expect("renderer start");
                    let presenter = Presenter::new(&renderer).expect("presenter start");
                    Spike {
                        renderer,
                        presenter,
                        started: Instant::now(),
                        report_at: Instant::now(),
                        frames: 0,
                        quit_after,
                        latencies: Vec::new(),
                        latency_check,
                    }
                })
            },
        );
        if let Err(e) = result {
            eprintln!("failed to open the spike window: {e}");
        }
    });
}

/// The editor window. `--secs N` closes it after N seconds (smoke tests).
fn run_editor(quit_after: Option<Duration>) {
    if let Ok(dirs) = quarry_io::settings::Dirs::from_env() {
        std::mem::forget(quarry_io::logging::init(&dirs));
    }
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::load_fonts(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1600.0), px(900.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Quarry".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(14.0), px(16.0))),
                }),
                window_min_size: Some(size(px(1100.0), px(640.0))),
                ..Default::default()
            };
            let opened = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::Quarry::new(window, cx))
            });
            if let Err(e) = opened {
                eprintln!("failed to open the Quarry window: {e}");
                cx.quit();
            }
            if let Some(limit) = quit_after {
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(limit).await;
                    cx.update(|cx| cx.quit());
                })
                .detach();
            }
        });
}

impl Render for app::Quarry {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        let body = match self.screen {
            app::Screen::Start => self.render_start(cx),
            app::Screen::Editor => self.render_editor(window, cx),
        };
        let p = theme::t();
        div()
            .id("root")
            .relative()
            .size_full()
            .bg(p.bg_app)
            .font_family(theme::SANS)
            .text_color(p.text_primary)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e, window, cx| this.key_down(e, window, cx)))
            .on_mouse_move(cx.listener(|this, e, _, cx| this.root_mouse_move(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.root_mouse_up(cx)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, _, _, cx| this.root_mouse_up(cx)),
            )
            .child(body)
            .children(self.render_toast(cx))
            .children(self.render_overlay(cx))
    }
}

/// `quarry --headless [--socket <path>] [--dir <folder>] [--commands <file.jsonl>]`
fn headless(args: &[String]) -> Result<(), String> {
    let options = quarry_server::headless::Options::from_args(args)?;
    let _log = quarry_io::logging::init(&options.dirs)?;
    quarry_server::headless::run(options)
}

fn distinct_colors(pixels: &[u8]) -> usize {
    let set: std::collections::HashSet<[u8; 4]> = pixels
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect();
    set.len()
}

/// Render one frame, convert it to NV12, read the buffer back and compare the
/// converted colours with the scene target.
#[cfg(target_os = "macos")]
fn present_check() {
    let renderer = quarry_render::Renderer::new(WIDTH, HEIGHT).expect("renderer start");
    let mut presenter = Presenter::new(&renderer).expect("presenter start");

    renderer.render(0.5);
    presenter.present();
    renderer.wait();

    let bgra = renderer.read_target();
    let rgb = presenter.read_rgb();
    let mut worst = 0i32;
    let mut total = 0i64;
    for (index, pixel) in rgb.iter().enumerate() {
        let source = &bgra[index * 4..index * 4 + 3];
        let (b, g, r) = (source[0], source[1], source[2]);
        for (converted, expected) in pixel.iter().zip([r, g, b]) {
            let error = (*converted as i32 - expected as i32).abs();
            worst = worst.max(error);
            total += error as i64;
        }
    }
    println!(
        "present pixels={} distinct_colors={} mean_channel_error={:.2} worst_channel_error={worst}",
        rgb.len(),
        rgb.iter().collect::<std::collections::HashSet<_>>().len(),
        total as f64 / (rgb.len() as i64 * 3) as f64
    );
}
