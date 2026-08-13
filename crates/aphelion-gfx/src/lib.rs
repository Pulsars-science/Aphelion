//! Real-time 3D rendering for [Aphelion], on top of wgpu.
//!
//! The problem this crate solves is *scale*. A solar system spans twelve orders
//! of magnitude — from a 1700 km moon to a 30 AU orbit — and a conventional
//! renderer falls apart well before that: `f32` positions quantise into visible
//! steps, and the depth buffer collapses into z-fighting. Two techniques deal
//! with it, and both are explained where they are implemented, in [`camera`]:
//!
//! * **camera-relative rendering** in astronomical units, with the `f64` → `f32`
//!   conversion happening last;
//! * a **reverse-Z projection with an infinite far plane**, which spreads depth
//!   precision evenly across every scale.
//!
//! Nothing here depends on a windowing library: [`Renderer::new`] takes any
//! wgpu surface target, so the same code can drive a desktop window today and a
//! canvas later.
//!
//! [Aphelion]: https://github.com/Pulsars-science/Aphelion

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::missing_errors_doc)]
// f64 -> f32 truncation is the entire point of the render path: see `camera`.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]

pub mod camera;
pub mod mesh;
pub mod renderer;
pub mod scene;

pub use camera::{Camera, OrbitCamera, RENDER_UNIT};
pub use mesh::{MeshData, Vertex, uv_sphere};
pub use renderer::{DEPTH_FORMAT, Frame, Renderer};
pub use scene::{BodyInstance, Scene, Track};
