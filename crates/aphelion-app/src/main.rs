//! Aphelion — an interactive, physically real 3D solar system.
//!
//! This binary is the thin platform layer: it opens a window, wires winit input
//! to the camera, drives [`AppState`] once per frame and hands the resulting
//! scene to the renderer, with an egui pass on top.
//!
//! Everything interesting lives elsewhere:
//!
//! * `aphelion-core` — gravity, integrators, orbital elements;
//! * `aphelion-data` — the Solar System at J2000.0;
//! * `aphelion-gfx` — the wgpu renderer and its astronomical-scale camera.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]

mod state;
mod ui;

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use aphelion_gfx::Renderer;
use state::AppState;

/// Longest frame the simulation will act on, in seconds.
///
/// A frame that took longer than this was almost certainly a stall — a window
/// drag, a breakpoint, a laptop waking up — and replaying it as elapsed time
/// would make the system jump. Clamping keeps the simulation continuous.
const MAX_FRAME_TIME: f64 = 0.1;

/// Radians of camera rotation per pixel of mouse movement.
const ORBIT_SENSITIVITY: f64 = 0.006;

/// Zoom factor per notch of scroll wheel.
const ZOOM_PER_NOTCH: f64 = 1.15;

fn main() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("aphelion=info,warn"),
    )
    .init();

    let event_loop = EventLoop::new().context("could not create the event loop")?;
    // Redraw continuously: this is a real-time simulation, not a document.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = Application::default();
    event_loop.run_app(&mut app).context("event loop failed")?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// The window and everything that needs one.
struct Graphics {
    window: Arc<Window>,
    renderer: Renderer,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

#[derive(Default)]
struct Application {
    graphics: Option<Graphics>,
    state: Option<AppState>,
    last_frame: Option<Instant>,
    /// Smoothed frame rate, for the readout.
    fps: f64,
    /// Left mouse button held: the camera is being dragged.
    dragging: bool,
    /// Where the cursor was last seen, in physical pixels.
    cursor: Option<(f64, f64)>,
    /// Error that ended the run, surfaced from `main`.
    error: Option<anyhow::Error>,
}

impl Application {
    fn init(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let attributes = Window::default_attributes()
            .with_title("Aphelion")
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("could not open a window")?,
        );

        let size = window.inner_size();
        let renderer =
            pollster::block_on(Renderer::new(Arc::clone(&window), size.width, size.height))?;

        let egui_context = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui_context.viewport_id(),
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(
            renderer.device(),
            renderer.surface_format(),
            egui_wgpu::RendererOptions {
                // The UI draws in its own pass with no depth attachment, so the
                // scene's reverse-Z buffer is left entirely alone.
                depth_stencil_format: None,
                ..Default::default()
            },
        );

        self.graphics = Some(Graphics {
            window,
            renderer,
            egui_context,
            egui_state,
            egui_renderer,
        });
        self.state = Some(AppState::new());
        self.last_frame = Some(Instant::now());
        Ok(())
    }

    fn handle_key(&mut self, key: &Key, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match key.as_ref() {
            Key::Named(NamedKey::Space) => state.controls.paused = !state.controls.paused,
            Key::Named(NamedKey::Escape) => event_loop.exit(),
            Key::Character("[") => state.scale_time(0.5),
            Key::Character("]") => state.scale_time(2.0),
            Key::Character("o") => state.controls.show_orbits = !state.controls.show_orbits,
            Key::Character("f") => state.controls.follow = !state.controls.follow,
            Key::Character("i") => state.cycle_integrator(),
            Key::Character("r") => state.reset(),
            // Digits select a body, 1 being the Sun.
            Key::Character(digit) if digit.len() == 1 => {
                if let Some(index) = digit.chars().next().and_then(|c| c.to_digit(10)) {
                    let index = if index == 0 { 9 } else { index - 1 } as usize;
                    if index < state.sim.len() {
                        state.focus_on(Some(aphelion_core::BodyId(index)));
                    }
                }
            }
            _ => {}
        }
    }

    /// Advances and draws one frame. Returns `true` if the user asked to quit.
    fn redraw(&mut self) -> bool {
        let (Some(graphics), Some(state)) = (self.graphics.as_mut(), self.state.as_mut()) else {
            return false;
        };

        let now = Instant::now();
        let elapsed = self
            .last_frame
            .replace(now)
            .map_or(0.0, |previous| now.duration_since(previous).as_secs_f64());
        let frame_time = elapsed.min(MAX_FRAME_TIME);
        if elapsed > 0.0 {
            // Exponential smoothing: a raw per-frame reciprocal is unreadable.
            self.fps = self.fps.mul_add(0.9, 0.1 / elapsed);
        }

        state.update(frame_time);

        // --- UI ---------------------------------------------------------
        let raw_input = graphics.egui_state.take_egui_input(&graphics.window);
        let fps = self.fps;
        let mut quit = false;
        let output = graphics.egui_context.run_ui(raw_input, |root| {
            quit = ui::draw(root, state, fps);
        });
        graphics
            .egui_state
            .handle_platform_output(&graphics.window, output.platform_output);
        let primitives = graphics
            .egui_context
            .tessellate(output.shapes, output.pixels_per_point);

        // --- Draw -------------------------------------------------------
        // egui's texture upkeep has to happen whether or not this frame ever
        // reaches the screen. The font atlas is allocated once and then patched
        // incrementally, so dropping a single frame's deltas leaves the *next*
        // frame patching a texture that was never created — which panics inside
        // egui-wgpu. Skipped frames are routine (a window that opens behind
        // another one reports Occluded), so this must not depend on acquiring
        // the surface. Uploading before begin_frame makes that structural.
        //
        // One texture can receive several partial updates in a frame — the font
        // atlas growing mid-layout, for instance — hence the inner loop.
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                graphics.egui_renderer.update_texture(
                    graphics.renderer.device(),
                    graphics.renderer.queue(),
                    *id,
                    delta,
                );
            }
        }

        let mut frame = match graphics.renderer.begin_frame() {
            Ok(frame) => frame,
            Err(error) => {
                // A lost, outdated or occluded swap chain is routine — during a
                // resize, a monitor change, a window opening behind another.
                // Skip the frame and carry on, but still retire the textures
                // egui has finished with.
                log::debug!("skipping frame: {error:#}");
                graphics.renderer.reconfigure();
                for id in &output.textures_delta.free {
                    graphics.egui_renderer.free_texture(id);
                }
                return false;
            }
        };

        graphics
            .renderer
            .draw_scene(&mut frame, &state.scene, &state.camera.camera());

        let (width, height) = graphics.renderer.size();
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: output.pixels_per_point,
        };
        graphics.egui_renderer.update_buffers(
            graphics.renderer.device(),
            graphics.renderer.queue(),
            &mut frame.encoder,
            &primitives,
            &screen,
        );
        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("aphelion ui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            // Load, not clear: the UI composites over the scene.
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            graphics
                .egui_renderer
                .render(&mut pass, &primitives, &screen);
        }
        for id in &output.textures_delta.free {
            graphics.egui_renderer.free_texture(id);
        }

        graphics.renderer.finish_frame(frame);
        quit
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_some() {
            return;
        }
        if let Err(error) = self.init(event_loop) {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(graphics) = self.graphics.as_mut() else {
            return;
        };

        // Give egui first refusal: if the pointer is over a panel, the camera
        // must not also react to the drag.
        let response = graphics
            .egui_state
            .on_window_event(&graphics.window, &event);
        let ui_wants_pointer = response.consumed;
        if response.repaint {
            graphics.window.request_redraw();
        }

        if matches!(event, WindowEvent::RedrawRequested) {
            if self.redraw() {
                event_loop.exit();
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                graphics.renderer.resize(size.width, size.height);
            }

            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !ui_wants_pointer =>
            {
                let key = event.logical_key.clone();
                self.handle_key(&key, event_loop);
            }

            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.dragging = state == ElementState::Pressed && !ui_wants_pointer;
            }

            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x, position.y);
                if let (Some(previous), true) = (self.cursor, self.dragging)
                    && let Some(app_state) = self.state.as_mut()
                {
                    app_state.camera.orbit(
                        -(current.0 - previous.0) * ORBIT_SENSITIVITY,
                        (current.1 - previous.1) * ORBIT_SENSITIVITY,
                    );
                }
                self.cursor = Some(current);
            }

            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
                self.dragging = false;
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if ui_wants_pointer {
                    return;
                }
                let notches = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    // Trackpads report pixels; 50 px is about one wheel notch.
                    MouseScrollDelta::PixelDelta(position) => position.y / 50.0,
                };
                if let Some(app_state) = self.state.as_mut() {
                    app_state.camera.zoom(ZOOM_PER_NOTCH.powf(-notches));
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(graphics) = self.graphics.as_ref() {
            graphics.window.request_redraw();
        }
    }
}
