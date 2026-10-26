//! macOS presentation: render into an IOSurface-backed NV12 CVPixelBuffer and hand it
//! to GPUI's surface element. GPUI's renderer only accepts 420 bi-planar buffers.

use objc2::runtime::ProtocolObject;
use objc2_core_foundation::{CFDictionary, CFRetained, CFType};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetIOSurface,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use objc2_io_surface::IOSurfaceRef;
use objc2_metal::{
    MTLDevice, MTLPixelFormat, MTLStorageMode, MTLTextureDescriptor, MTLTextureUsage,
};
use std::ptr::NonNull;

pub struct Presenter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pixel_buffer: CFRetained<CVPixelBuffer>,
    y_view: wgpu::TextureView,
    chroma_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    pipeline_y: wgpu::RenderPipeline,
    pipeline_chroma: wgpu::RenderPipeline,
}

impl Presenter {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let mtl_device = unsafe { device.as_hal::<wgpu::hal::api::Metal>() }
            .ok_or("device is not Metal")?
            .raw_device()
            .clone();

        let pixel_buffer = create_nv12(width, height)?;
        let surface = CVPixelBufferGetIOSurface(Some(&pixel_buffer))
            .ok_or("pixel buffer has no IOSurface")?;

        let y_texture = wrap_plane(
            device,
            &mtl_device,
            &surface,
            0,
            MTLPixelFormat::R8Unorm,
            width,
            height,
            wgpu::TextureFormat::R8Unorm,
        )?;
        let chroma_texture = wrap_plane(
            device,
            &mtl_device,
            &surface,
            1,
            MTLPixelFormat::RG8Unorm,
            width / 2,
            height / 2,
            wgpu::TextureFormat::Rg8Unorm,
        )?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present"),
            source: wgpu::ShaderSource::Wgsl(include_str!("present.wgsl").into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("present sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("present"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline_y = pipeline(
            device,
            &shader,
            &pipeline_layout,
            "fs_y",
            wgpu::TextureFormat::R8Unorm,
        );
        let pipeline_chroma = pipeline(
            device,
            &shader,
            &pipeline_layout,
            "fs_chroma",
            wgpu::TextureFormat::Rg8Unorm,
        );

        Ok(Self {
            device: device.clone(),
            queue: queue.clone(),
            pixel_buffer,
            y_view: y_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            chroma_view: chroma_texture.create_view(&wgpu::TextureViewDescriptor::default()),
            bind_group,
            pipeline_y,
            pipeline_chroma,
        })
    }

    /// Convert the scene target into the NV12 buffer, ready for `paint_surface`.
    pub fn present(&self) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present"),
            });
        for (view, pipeline) in [
            (&self.y_view, &self.pipeline_y),
            (&self.chroma_view, &self.pipeline_chroma),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    pub fn buffer(&self) -> CFRetained<CVPixelBuffer> {
        self.pixel_buffer.clone()
    }

    /// Read the NV12 buffer back as RGB. Used by `--present-check` to prove the
    /// conversion shaders actually wrote the IOSurface.
    pub fn read_rgb(&self) -> Vec<[u8; 3]> {
        use objc2_core_video::{
            CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
            CVPixelBufferGetHeightOfPlane, CVPixelBufferGetWidthOfPlane,
            CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
        };
        unsafe { CVPixelBufferLockBaseAddress(&self.pixel_buffer, CVPixelBufferLockFlags(0)) };
        let (width, height) = (
            CVPixelBufferGetWidthOfPlane(&self.pixel_buffer, 0),
            CVPixelBufferGetHeightOfPlane(&self.pixel_buffer, 0),
        );
        let luma = (
            CVPixelBufferGetBaseAddressOfPlane(&self.pixel_buffer, 0) as *const u8,
            CVPixelBufferGetBytesPerRowOfPlane(&self.pixel_buffer, 0),
        );
        let chroma = (
            CVPixelBufferGetBaseAddressOfPlane(&self.pixel_buffer, 1) as *const u8,
            CVPixelBufferGetBytesPerRowOfPlane(&self.pixel_buffer, 1),
        );

        let mut out = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let luma = unsafe { *luma.0.add(y * luma.1 + x) } as f32 / 255.0;
                let (cb, cr) = unsafe {
                    (
                        *chroma.0.add((y / 2) * chroma.1 + (x / 2) * 2),
                        *chroma.0.add((y / 2) * chroma.1 + (x / 2) * 2 + 1),
                    )
                };
                let cb = cb as f32 / 255.0 - 128.0 / 255.0;
                let cr = cr as f32 / 255.0 - 128.0 / 255.0;
                let luma = (luma * 255.0 - 16.0) / 219.0;
                let b = luma + 1.8556 * cb;
                let r = luma + 1.5748 * cr;
                let g = (luma - 0.2126 * r - 0.0722 * b) / 0.7152;
                let to_byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                out.push([to_byte(r), to_byte(g), to_byte(b)]);
            }
        }
        unsafe { CVPixelBufferUnlockBaseAddress(&self.pixel_buffer, CVPixelBufferLockFlags(0)) };
        out
    }
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    entry_point: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("present"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_nv12(width: u32, height: u32) -> Result<CFRetained<CVPixelBuffer>, String> {
    let attributes = CFDictionary::from_slices(
        &[unsafe { kCVPixelBufferIOSurfacePropertiesKey }],
        &[&*CFDictionary::<CFType, CFType>::empty()],
    );
    let mut pixel_buffer: *mut CVPixelBuffer = std::ptr::null_mut();
    let status = unsafe {
        CVPixelBufferCreate(
            None,
            width as usize,
            height as usize,
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            Some(attributes.as_opaque()),
            NonNull::from(&mut pixel_buffer),
        )
    };
    if status != 0 {
        return Err(format!("CVPixelBufferCreate failed: {status}"));
    }
    let pixel_buffer = NonNull::new(pixel_buffer).ok_or("CVPixelBufferCreate returned null")?;
    Ok(unsafe { CFRetained::from_raw(pixel_buffer) })
}

#[allow(clippy::too_many_arguments)]
fn wrap_plane(
    device: &wgpu::Device,
    mtl_device: &ProtocolObject<dyn MTLDevice>,
    surface: &IOSurfaceRef,
    plane: usize,
    pixel_format: MTLPixelFormat,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> Result<wgpu::Texture, String> {
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            pixel_format,
            width as usize,
            height as usize,
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::RenderTarget);
    descriptor.setStorageMode(MTLStorageMode::Shared);
    let texture = mtl_device
        .newTextureWithDescriptor_iosurface_plane(&descriptor, surface, plane)
        .ok_or("Metal refused the IOSurface texture")?;

    let hal_texture = unsafe {
        wgpu::hal::metal::Device::texture_from_raw(
            texture,
            format,
            objc2_metal::MTLTextureType::Type2D,
            1,
            1,
            wgpu::hal::CopyExtent {
                width,
                height,
                depth: 1,
            },
            None,
        )
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("nv12 plane"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    };
    Ok(unsafe {
        device.create_texture_from_hal::<wgpu::hal::api::Metal>(
            hal_texture,
            &desc,
            wgpu::TextureUses::UNINITIALIZED,
        )
    })
}
