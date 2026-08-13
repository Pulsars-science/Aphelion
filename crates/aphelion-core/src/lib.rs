//! Gravitational dynamics and orbital mechanics for [Aphelion].
//!
//! This crate is the physics heart of the simulator and carries no graphics
//! dependency: it can be used on its own, headlessly, to propagate a system and
//! export trajectories.
//!
//! # Units
//!
//! Everything is SI, in a non-rotating inertial frame:
//!
//! | quantity | unit          |
//! |----------|---------------|
//! | length   | metre (m)     |
//! | mass     | kilogram (kg) |
//! | time     | second (s)    |
//! | angle    | radian (rad)  |
//!
//! State is stored as [`f64`]. At Neptune's distance (`4.5e12 m`) that leaves
//! sub-millimetre resolution, which is far below any modelling error we care
//! about. Conversion helpers for astronomer-friendly units (AU, days, solar
//! masses, degrees) live in [`constants`].
//!
//! [Aphelion]: https://github.com/Pulsars-science/Aphelion

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::cast_precision_loss)]

pub mod body;
pub mod constants;
pub mod integrator;
pub mod kepler;
pub mod nbody;
pub mod params;
pub mod time;

pub use body::{Body, BodyId, BodyKind};
pub use integrator::{Integrator, Scratch};
pub use kepler::OrbitalElements;
pub use nbody::State;
pub use params::SimulationParams;
pub use time::Epoch;

pub use glam::DVec3;
