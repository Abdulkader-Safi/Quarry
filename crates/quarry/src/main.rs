//! M0 viewport spike: a GPUI window showing a wgpu-rendered scene.

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
                    }
                })
            },
        );
        if let Err(e) = result {
            eprintln!("failed to open the spike window: {e}");
        }
    });
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
