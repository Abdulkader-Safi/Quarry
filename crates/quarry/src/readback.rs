//! Windows and Linux presentation: GPUI has no zero-copy surface element there,
//! so the frame is copied back to RAM and handed to GPUI as an image.

use gpui_kit::{AnyElement, ImageSource, IntoElement as _, RenderImage, Styled as _, img};
use std::sync::Arc;

pub struct Presenter {
    image: Arc<RenderImage>,
}

impl Presenter {
    pub fn new(renderer: &quarry_render::Renderer) -> Result<Self, String> {
        renderer.render(0.0);
        Ok(Self {
            image: snapshot(renderer),
        })
    }

    /// Render one frame and copy it back to the CPU.
    pub fn render(&mut self, renderer: &quarry_render::Renderer, time: f32) {
        renderer.render(time);
        self.image = snapshot(renderer);
    }

    pub fn element(&self) -> AnyElement {
        // shortcut: one RenderImage (and one GPUI texture) per frame, which only
        // a spike can afford. Replace when the real viewport element lands.
        img(ImageSource::Render(self.image.clone()))
            .size_full()
            .into_any_element()
    }
}

fn snapshot(renderer: &quarry_render::Renderer) -> Arc<RenderImage> {
    let pixels = renderer.read_target();
    let buffer = image::RgbaImage::from_raw(renderer.width(), renderer.height(), pixels)
        .expect("readback matches the target size");
    Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}
