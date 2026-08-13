//! The wgpu renderer.
//!
//! Two pipelines, one uniform buffer, one sphere mesh. Bodies are drawn
//! instanced — one draw call for the whole system — and tracks are drawn as a
//! single line list.
//!
//! Frames are explicit so that a caller can slot its own passes in:
//!
//! ```no_run
//! # use aphelion_gfx::{Renderer, Scene, Camera};
//! # fn demo(renderer: &mut Renderer, scene: &Scene, camera: &Camera) -> anyhow::Result<()> {
//! let mut frame = renderer.begin_frame()?;
//! renderer.draw_scene(&mut frame, scene, camera);
//! // ... a UI pass could go here, loading rather than clearing ...
//! renderer.finish_frame(frame);
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::camera::{Camera, scale_to_render, to_render_space};
use crate::mesh::{self, Vertex};
use crate::scene::Scene;

/// Depth buffer format. 32-bit float, which is what makes reverse-Z pay off.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Latitude bands on the body mesh.
const SPHERE_RINGS: u32 = 48;
/// Longitude divisions on the body mesh.
const SPHERE_SEGMENTS: u32 = 96;

/// Per-frame uniforms shared by both pipelines.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    /// xyz: light position in camera-relative render space. w: ambient.
    light: [f32; 4],
}

/// Per-body instance data.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BodyRaw {
    model: [[f32; 4]; 4],
    /// rgb: base colour. a: emissive flag.
    color: [f32; 4],
}

impl BodyRaw {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<BodyRaw>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
        ],
    };
}

/// One endpoint of a line segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct TrackVertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl TrackVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<TrackVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
    };
}

/// A frame in flight: the surface texture being drawn into, plus its encoder.
///
/// Hand it back to [`Renderer::finish_frame`] to submit and present.
pub struct Frame {
    /// The acquired swap-chain texture.
    pub surface_texture: wgpu::SurfaceTexture,
    /// View of that texture, as a colour attachment.
    pub view: wgpu::TextureView,
    /// Command encoder for this frame. Extra passes may be recorded into it.
    pub encoder: wgpu::CommandEncoder,
}

/// Draws [`Scene`]s with wgpu.
pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    depth_view: wgpu::TextureView,

    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,

    body_pipeline: wgpu::RenderPipeline,
    track_pipeline: wgpu::RenderPipeline,

    sphere_vertices: wgpu::Buffer,
    sphere_indices: wgpu::Buffer,
    sphere_index_count: u32,

    instances: wgpu::Buffer,
    instance_capacity: usize,
    instance_staging: Vec<BodyRaw>,

    track_buffer: wgpu::Buffer,
    track_capacity: usize,
    track_staging: Vec<TrackVertex>,

    /// Colour the frame is cleared to. Near-black, faintly blue.
    pub clear_color: wgpu::Color,
}

impl Renderer {
    /// Creates a renderer targeting `target`, which must outlive it — an
    /// `Arc<Window>` is the usual choice.
    ///
    /// # Errors
    ///
    /// Fails if no adapter supports the surface, or if the device cannot be
    /// created.
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .context("could not create a surface for the window")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .context("no graphics adapter supports this surface")?;
        log::info!("adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("aphelion device"),
                // The renderer deliberately sticks to the baseline feature set
                // so it runs on integrated GPUs and, eventually, on the web.
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .context("could not create the graphics device")?;

        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .context("the surface is not supported by this adapter")?;
        // Prefer an sRGB target: colours are authored and lit in linear space,
        // and the hardware then does the encoding on write.
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(srgb) = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
        {
            config.format = srgb;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        let format = config.format;
        surface.configure(&device, &config);

        let depth_view = create_depth_view(&device, config.width, config.height);

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aphelion globals"),
            size: size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("aphelion globals layout"),
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
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aphelion globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("aphelion pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        let body_pipeline = create_body_pipeline(&device, &pipeline_layout, format);
        let track_pipeline = create_track_pipeline(&device, &pipeline_layout, format);

        let sphere = mesh::uv_sphere(SPHERE_RINGS, SPHERE_SEGMENTS);
        let sphere_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aphelion sphere vertices"),
            contents: bytemuck::cast_slice(&sphere.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let sphere_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("aphelion sphere indices"),
            contents: bytemuck::cast_slice(&sphere.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let sphere_index_count = u32::try_from(sphere.indices.len()).unwrap_or(u32::MAX);

        let instance_capacity = 64;
        let instances =
            create_vertex_buffer::<BodyRaw>(&device, "aphelion instances", instance_capacity);
        let track_capacity = 8192;
        let track_buffer =
            create_vertex_buffer::<TrackVertex>(&device, "aphelion tracks", track_capacity);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth_view,
            globals_buffer,
            globals_bind_group,
            body_pipeline,
            track_pipeline,
            sphere_vertices,
            sphere_indices,
            sphere_index_count,
            instances,
            instance_capacity,
            instance_staging: Vec::new(),
            track_buffer,
            track_capacity,
            track_staging: Vec::new(),
            clear_color: wgpu::Color {
                r: 0.004,
                g: 0.005,
                b: 0.011,
                a: 1.0,
            },
        })
    }

    /// The graphics device, for callers that need to build their own resources
    /// (an egui pass, for instance).
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The command queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Format of the surface the renderer draws into.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Current surface size, in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// Width divided by height, guarded against a zero-height window.
    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    /// Reconfigures the surface after the window changes size.
    ///
    /// Zero-sized requests are ignored, which is what a minimised window
    /// reports on Windows.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, width, height);
    }

    /// Re-applies the current configuration. Call after a lost surface.
    pub fn reconfigure(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Acquires the next frame.
    ///
    /// # Errors
    ///
    /// Fails if the swap chain could not deliver a texture. Callers should
    /// treat that as recoverable: [`Renderer::reconfigure`] and try again next
    /// frame.
    pub fn begin_frame(&mut self) -> Result<Frame> {
        // wgpu reports recoverable swap-chain states as ordinary values, not
        // errors: a lost or outdated surface just needs reconfiguring, and an
        // occluded or timed-out one needs the frame skipped.
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.reconfigure();
                texture
            }
            other => anyhow::bail!("could not acquire the next frame: {other:?}"),
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aphelion frame"),
            });
        Ok(Frame {
            surface_texture,
            view,
            encoder,
        })
    }

    /// Submits the frame and presents it.
    pub fn finish_frame(&mut self, frame: Frame) {
        self.queue.submit(Some(frame.encoder.finish()));
        self.queue.present(frame.surface_texture);
    }

    /// Draws a scene, clearing the colour and depth attachments first.
    pub fn draw_scene(&mut self, frame: &mut Frame, scene: &Scene, camera: &Camera) {
        self.upload_globals(scene, camera);
        self.upload_instances(scene, camera);
        self.upload_tracks(scene, camera);

        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aphelion scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        // Reverse-Z: "furthest away" is zero, so that is what an
                        // empty depth buffer must hold.
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

        pass.set_bind_group(0, &self.globals_bind_group, &[]);

        if !self.instance_staging.is_empty() {
            pass.set_pipeline(&self.body_pipeline);
            pass.set_vertex_buffer(0, self.sphere_vertices.slice(..));
            pass.set_vertex_buffer(1, self.instances.slice(..));
            pass.set_index_buffer(self.sphere_indices.slice(..), wgpu::IndexFormat::Uint32);
            let instances = u32::try_from(self.instance_staging.len()).unwrap_or(u32::MAX);
            pass.draw_indexed(0..self.sphere_index_count, 0, 0..instances);
        }

        if !self.track_staging.is_empty() {
            pass.set_pipeline(&self.track_pipeline);
            pass.set_vertex_buffer(0, self.track_buffer.slice(..));
            let vertices = u32::try_from(self.track_staging.len()).unwrap_or(u32::MAX);
            pass.draw(0..vertices, 0..1);
        }
    }

    fn upload_globals(&mut self, scene: &Scene, camera: &Camera) {
        let globals = Globals {
            view_projection: camera
                .view_projection(self.aspect_ratio())
                .to_cols_array_2d(),
            light: {
                let light = to_render_space(scene.light, camera.position);
                [light.x, light.y, light.z, scene.ambient]
            },
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));
    }

    fn upload_instances(&mut self, scene: &Scene, camera: &Camera) {
        self.instance_staging.clear();
        self.instance_staging
            .extend(scene.bodies.iter().map(|body| {
                let translation = to_render_space(body.position, camera.position);
                let radius = scale_to_render(body.radius).max(f32::MIN_POSITIVE);
                // Tilt first, then spin about the tilted axis — the order a
                // real body's orientation is built up in.
                #[allow(clippy::cast_possible_truncation)]
                let rotation = Quat::from_rotation_x(body.axial_tilt as f32)
                    * Quat::from_rotation_z((body.spin % std::f64::consts::TAU) as f32);
                BodyRaw {
                    model: Mat4::from_scale_rotation_translation(
                        Vec3::splat(radius),
                        rotation,
                        translation,
                    )
                    .to_cols_array_2d(),
                    color: [
                        body.color[0],
                        body.color[1],
                        body.color[2],
                        if body.emissive { 1.0 } else { 0.0 },
                    ],
                }
            }));

        if self.instance_staging.len() > self.instance_capacity {
            self.instance_capacity = self.instance_staging.len().next_power_of_two();
            self.instances = create_vertex_buffer::<BodyRaw>(
                &self.device,
                "aphelion instances",
                self.instance_capacity,
            );
        }
        if !self.instance_staging.is_empty() {
            self.queue.write_buffer(
                &self.instances,
                0,
                bytemuck::cast_slice(&self.instance_staging),
            );
        }
    }

    fn upload_tracks(&mut self, scene: &Scene, camera: &Camera) {
        self.track_staging.clear();
        for track in &scene.tracks {
            if track.points.len() < 2 {
                continue;
            }
            // A line list rather than a strip: it costs one extra vertex per
            // segment but lets every track share a single draw call.
            let segments = track.points.len() - usize::from(!track.closed);
            for index in 0..segments {
                let start = track.points[index];
                let end = track.points[(index + 1) % track.points.len()];
                self.track_staging.push(TrackVertex {
                    position: to_render_space(start, camera.position).to_array(),
                    color: track.color,
                });
                self.track_staging.push(TrackVertex {
                    position: to_render_space(end, camera.position).to_array(),
                    color: track.color,
                });
            }
        }

        if self.track_staging.len() > self.track_capacity {
            self.track_capacity = self.track_staging.len().next_power_of_two();
            self.track_buffer = create_vertex_buffer::<TrackVertex>(
                &self.device,
                "aphelion tracks",
                self.track_capacity,
            );
        }
        if !self.track_staging.is_empty() {
            self.queue.write_buffer(
                &self.track_buffer,
                0,
                bytemuck::cast_slice(&self.track_staging),
            );
        }
    }
}

fn create_vertex_buffer<T>(device: &wgpu::Device, label: &str, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (capacity * size_of::<T>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("aphelion depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Depth state shared by both pipelines.
///
/// `Greater` rather than `Less`, because the projection is reverse-Z.
fn depth_state(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(wgpu::CompareFunction::Greater),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn create_body_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("aphelion body shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/body.wgsl").into()),
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aphelion bodies"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(Vertex::LAYOUT), Some(BodyRaw::LAYOUT)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(depth_state(true)),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_track_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("aphelion track shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/track.wgsl").into()),
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aphelion tracks"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(TrackVertex::LAYOUT)],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        // Tracks test against the bodies — an orbit passing behind a planet is
        // correctly hidden — but do not write depth, so overlapping tracks
        // blend instead of fighting.
        depth_stencil: Some(depth_state(false)),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
