//! Ready-made systems for [Aphelion].
//!
//! The headline one is [`solar_system`]: the Sun, the eight planets, the Moon
//! and Pluto, placed on their J2000.0 orbits.
//!
//! [Aphelion]: https://github.com/Pulsars-science/Aphelion

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::unreadable_literal)]

pub mod solar_system;

pub use solar_system::{PLANETS, PlanetData, SUN, solar_system, solar_system_inner};
