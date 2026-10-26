//! wgpu renderer for the Quarry viewport.
//!
//! M0 scope: one offscreen colour target, a depth buffer and a benchmark scene of
//! 100k triangles across 5 skinned meshes with 50 bones. Presentation is the app's job.

pub mod camera;
pub mod snapshot;

use glam::Mat4;
use std::time::Instant;
use wgpu::util::DeviceExt;

const MAX_BONES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Camera {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    joints: [u32; 4],
    weights: [f32; 4],
}

impl Vertex {
    const ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Uint32x4,
        4 => Float32x4,
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

/// Offscreen renderer. `new` builds the device and the benchmark scene.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    camera: wgpu::Buffer,
    joints: wgpu::Buffer,
    meshes: Vec<Mesh>,
    depth: wgpu::TextureView,
    target: wgpu::Texture,
    target_view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends =
            wgpu::Backends::METAL | wgpu::Backends::VULKAN | wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(instance_desc);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("quarry"),
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU device: {e}"))?;

        let format = wgpu::TextureFormat::Bgra8Unorm;

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });

        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: std::mem::size_of::<Camera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let joints = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joints"),
            size: (MAX_BONES * 64) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene bind group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: joints.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::layout())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let meshes = (0..5)
            .map(|m| {
                let (vertices, indices) = build_sphere(m, 100);
                Mesh {
                    vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vertices"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                    indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("indices"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                    index_count: indices.len() as u32,
                }
            })
            .collect();

        let renderer = Self {
            device,
            queue,
            pipeline,
            bind_group,
            camera,
            joints,
            meshes,
            depth,
            target,
            target_view,
            width,
            height,
        };
        renderer.write_camera();
        Ok(renderer)
    }

    fn write_camera(&self) {
        let eye = glam::Vec3::new(0.0, 1.6, 4.2);
        let view = glam::camera::rh::view::look_at_mat4(eye, glam::Vec3::ZERO, glam::Vec3::Y);
        let proj = glam::camera::rh::proj::directx::perspective(
            50f32.to_radians(),
            self.width as f32 / self.height as f32,
            0.1,
            100.0,
        );
        let camera = Camera {
            view_proj: (proj * view).to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.camera, 0, bytemuck::bytes_of(&camera));
    }

    /// Render one frame at `time` seconds.
    pub fn render(&self, time: f32) -> wgpu::SubmissionIndex {
        let mut palette = [[0f32; 4]; 4 * MAX_BONES];
        for bone in 0..50 {
            let angle = (time * 1.5 + bone as f32 * 0.35).sin() * 0.06;
            let m = Mat4::from_rotation_y(angle);
            palette[bone * 4..bone * 4 + 4].copy_from_slice(&m.to_cols_array_2d());
        }
        self.queue
            .write_buffer(&self.joints, 0, bytemuck::cast_slice(&palette));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for mesh in &self.meshes {
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        self.queue.submit([encoder.finish()])
    }

    /// Copy the colour target back to the CPU. Forces the GPU work to complete.
    pub fn read_target(&self) -> Vec<u8> {
        let unpadded = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * self.height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self.queue.submit([encoder.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(5)),
        });
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));

        let mapped = buffer.slice(..).get_mapped_range().expect("map readback");
        let mut out = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        out
    }

    pub fn wait(&self) {
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
    }

    pub fn wait_for(&self, submission: wgpu::SubmissionIndex) {
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: None,
        });
    }

    pub fn target(&self) -> &wgpu::Texture {
        &self.target
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn target_view(&self) -> &wgpu::TextureView {
        &self.target_view
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}

/// Render 300 frames and report timings. Used by `quarry --bench`.
pub fn bench(width: u32, height: u32, frames: u32) -> Result<(), String> {
    let renderer = Renderer::new(width, height)?;
    for i in 0..10 {
        renderer.render(i as f32 / 60.0);
    }
    renderer.wait();

    let start = Instant::now();
    let mut last = None;
    for i in 0..frames {
        last = Some(renderer.render(i as f32 / 60.0));
    }
    let submit = start.elapsed();
    renderer.wait_for(last.expect("at least one frame"));
    let total = start.elapsed();

    println!(
        "frames={frames} size={width}x{height} submit_avg={:.2}ms frame_avg={:.2}ms fps={:.1}",
        submit.as_secs_f64() * 1000.0 / frames as f64,
        total.as_secs_f64() * 1000.0 / frames as f64,
        frames as f64 / total.as_secs_f64(),
    );
    Ok(())
}

/// A UV sphere with `rings` x `rings` quads: 2 tris per quad, 20k tris at 100.
/// Ten bones per sphere, two influences per vertex.
fn build_sphere(index: usize, rings: u32) -> (Vec<Vertex>, Vec<u32>) {
    let segments = rings;
    let radius = 0.55f32;
    let center = glam::Vec3::new(
        (index as f32 - 2.0) * 1.15,
        ((index as f32 * 1.7).sin()) * 0.35,
        ((index as f32 * 2.3).cos()) * 0.45,
    );
    let base_bone = index as u32 * 10;
    let color = [
        [0.85, 0.35, 0.25],
        [0.30, 0.70, 0.45],
        [0.30, 0.50, 0.90],
        [0.90, 0.75, 0.30],
        [0.65, 0.40, 0.80],
    ][index % 5];

    let mut vertices = Vec::with_capacity(((rings + 1) * (segments + 1)) as usize);
    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let theta = v * std::f32::consts::PI;
        for x in 0..=segments {
            let u = x as f32 / segments as f32;
            let phi = u * std::f32::consts::TAU;
            let normal = glam::Vec3::new(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            );
            let position = center + normal * radius;

            // Two nearest of this mesh's ten bones, by height.
            let slot = ((normal.y * 0.5 + 0.5) * 8.0).floor() as u32;
            let b0 = base_bone + slot.min(8);
            let b1 = base_bone + (slot + 1).min(9);
            vertices.push(Vertex {
                position: position.to_array(),
                normal: normal.to_array(),
                color,
                joints: [b0, b1, base_bone, base_bone],
                weights: [1.0, 0.0, 0.0, 0.0],
            });
        }
    }

    let mut indices = Vec::with_capacity((rings * segments * 6) as usize);
    let row = segments + 1;
    for y in 0..rings {
        for x in 0..segments {
            let a = y * row + x;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    (vertices, indices)
}
