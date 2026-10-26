//! Draws a document [`Snapshot`] offscreen: textured faces (nearest filtering by
//! default), a ground grid, and readback for thumbnails and screenshots.

use crate::camera::OrbitCamera;
use crate::gpu;
use crate::snapshot::{DocVertex, Snapshot};
use glam::Vec3;
use quarry_core::model::Filter;
use wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const GRID_LINES: i32 = 20;

/// FR-VIEW-05 toggles the renderer handles so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub grid: bool,
    pub textured: bool,
    pub backface_culling: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            grid: true,
            textured: true,
            backface_culling: true,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Frame {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    flags: [u32; 4],
}

pub struct DocumentRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    faces_culled: wgpu::RenderPipeline,
    faces_both: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
    white: wgpu::BindGroup,
}

impl DocumentRenderer {
    pub fn new() -> Result<Self, String> {
        let (device, queue) = gpu::device()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("document"),
            source: wgpu::ShaderSource::Wgsl(include_str!("document.wgsl").into()),
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("document"),
            bind_group_layouts: &[Some(&frame_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let pipeline = |fragment: &str, topology, cull_mode| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<DocVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let faces_culled = pipeline(
            "fs_lit",
            wgpu::PrimitiveTopology::TriangleList,
            Some(wgpu::Face::Back),
        );
        let faces_both = pipeline("fs_lit", wgpu::PrimitiveTopology::TriangleList, None);
        let lines = pipeline("fs_flat", wgpu::PrimitiveTopology::LineList, None);

        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("image"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let nearest = sampler(wgpu::FilterMode::Nearest);
        let linear = sampler(wgpu::FilterMode::Linear);

        let white = texture_group(
            &device,
            &queue,
            &texture_layout,
            &nearest,
            [1, 1],
            &[255; 4],
        );
        Ok(Self {
            device,
            queue,
            faces_culled,
            faces_both,
            lines,
            frame,
            frame_group,
            texture_layout,
            nearest,
            linear,
            white,
        })
    }

    fn texture_group(&self, size: [u32; 2], pixels: &[u8], filter: Filter) -> wgpu::BindGroup {
        let sampler = match filter {
            Filter::Nearest => &self.nearest,
            Filter::Linear => &self.linear,
        };
        texture_group(
            &self.device,
            &self.queue,
            &self.texture_layout,
            sampler,
            size,
            pixels,
        )
    }

    /// Render `snapshot` and return tightly packed sRGB RGBA8 rows.
    pub fn render(
        &self,
        snapshot: &Snapshot,
        camera: &OrbitCamera,
        display: Display,
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        // ponytail: textures and vertex buffers are rebuilt every call, fine for
        // thumbnails and screenshots; the live viewport caches them by revision.
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let attachment = |format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("document target"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let target = attachment(
            FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = attachment(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());

        // Fixed world light: top brightest, the two sides clearly different.
        let light = Vec3::new(0.35, 0.9, 0.6);
        let frame = Frame {
            view_proj: camera
                .view_proj(width as f32 / height as f32)
                .to_cols_array_2d(),
            light: light.normalize().extend(0.0).into(),
            flags: [display.textured as u32, 0, 0, 0],
        };
        self.queue
            .write_buffer(&self.frame, 0, bytemuck::bytes_of(&frame));

        let groups: Vec<_> = snapshot
            .images
            .iter()
            .map(|i| (i.id, self.texture_group(i.size, &i.pixels, i.filter)))
            .collect();
        let buffers: Vec<_> = snapshot
            .batches
            .iter()
            .filter(|b| !b.vertices.is_empty())
            .map(|b| {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("batch"),
                        contents: bytemuck::cast_slice(&b.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let group = b
                    .image
                    .and_then(|id| groups.iter().find(|(g, _)| *g == id))
                    .map_or(&self.white, |(_, g)| g);
                (buffer, b.vertices.len() as u32, group)
            })
            .collect();
        let grid = display.grid.then(|| {
            let lines = grid_lines(snapshot.grid_step);
            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("grid"),
                    contents: bytemuck::cast_slice(&lines),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            (buffer, lines.len() as u32)
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("document"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.022,
                            b: 0.027,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            pass.set_pipeline(if display.backface_culling {
                &self.faces_culled
            } else {
                &self.faces_both
            });
            for (buffer, count, group) in &buffers {
                pass.set_bind_group(1, *group, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
            if let Some((buffer, count)) = &grid {
                pass.set_pipeline(&self.lines);
                pass.set_bind_group(1, &self.white, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        gpu::read_texture(&self.device, &self.queue, &target, width, height)
    }

    /// Render and encode as PNG.
    pub fn render_png(
        &self,
        snapshot: &Snapshot,
        camera: &OrbitCamera,
        display: Display,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        let pixels = self.render(snapshot, camera, display, width, height);
        let image =
            image::RgbaImage::from_raw(width, height, pixels).ok_or("readback size mismatch")?;
        let mut png = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(png.into_inner())
    }
}

fn texture_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    size: [u32; 2],
    pixels: &[u8],
) -> wgpu::BindGroup {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("image"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        pixels,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// A ground grid on XZ, with the X axis red and the Z axis blue.
fn grid_lines(step: f32) -> Vec<DocVertex> {
    let extent = step * GRID_LINES as f32;
    let mut out = Vec::new();
    let mut line = |a: Vec3, b: Vec3, color: [f32; 4]| {
        for p in [a, b] {
            out.push(DocVertex {
                position: p.into(),
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color,
            });
        }
    };
    for i in -GRID_LINES..=GRID_LINES {
        let t = i as f32 * step;
        let (x_axis, z_axis) = if i == 0 {
            ([0.55, 0.12, 0.12, 1.0], [0.12, 0.2, 0.55, 1.0])
        } else {
            ([0.09, 0.09, 0.1, 1.0], [0.09, 0.09, 0.1, 1.0])
        };
        line(
            Vec3::new(-extent, 0.0, t),
            Vec3::new(extent, 0.0, t),
            x_axis,
        );
        line(
            Vec3::new(t, 0.0, -extent),
            Vec3::new(t, 0.0, extent),
            z_axis,
        );
    }
    out
}
