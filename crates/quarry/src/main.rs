//! M0 viewport spike: a GPUI window showing a wgpu-rendered scene.

use gpui_kit::*;

struct Spike;

impl Render for Spike {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x10_10_14))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--bench" || a == "--check") {
        let frames = args
            .iter()
            .position(|a| a == "--frames")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        if args.iter().any(|a| a == "--check") {
            match quarry_render::Renderer::new(1600, 1000) {
                Ok(r) => {
                    r.render(0.5);
                    let pixels = r.read_target();
                    let distinct: std::collections::HashSet<[u8; 4]> = pixels
                        .chunks_exact(4)
                        .map(|p| [p[0], p[1], p[2], p[3]])
                        .collect();
                    println!(
                        "readback bytes={} distinct_colors={}",
                        pixels.len(),
                        distinct.len()
                    );
                }
                Err(e) => eprintln!("check failed: {e}"),
            }
            return;
        }
        match quarry_render::bench(1600, 1000, frames) {
            Ok(()) => return,
            Err(e) => {
                eprintln!("bench failed: {e}");
                std::process::exit(1);
            }
        }
    }

    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Spike))
            .expect("failed to open the spike window");
    });
}
