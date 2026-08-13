//! The knobs a user is allowed to turn.
//!
//! Aphelion is meant to be played with: "what if gravity were twice as strong",
//! "what if Jupiter were ten times heavier". Every such dial lives here, in one
//! struct, so the UI, the save format and the physics all agree on what the
//! tunable axes are.

use crate::constants;

/// Tunable parameters of a running simulation.
///
/// All the scale factors are multiplicative and default to `1.0`, so
/// [`SimulationParams::default()`] reproduces the real universe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationParams {
    /// Multiplier applied to the gravitational constant `G`.
    ///
    /// Below 1 orbits unwind outwards, above 1 they tighten and speed up. Note
    /// that a body already in a circular orbit is at exactly the wrong speed
    /// the instant you change this — that is the interesting part.
    pub gravity_scale: f64,

    /// Multiplier applied to every body's mass.
    ///
    /// Because the dynamics only ever see the product `G·M`, scaling all masses
    /// by `k` is dynamically identical to scaling gravity by `k`. It is exposed
    /// separately because it *also* changes derived quantities the UI reports
    /// (density, surface gravity, escape velocity).
    pub mass_scale: f64,

    /// Multiplier applied to every body's radius **for display only**.
    ///
    /// At true scale the planets are invisible dots: Earth is 1/10 000 of an AU
    /// across. Values around 500–2000 give the familiar textbook picture.
    /// This never influences the trajectories.
    pub radius_scale: f64,

    /// Plummer softening length, in metres.
    ///
    /// Replaces `1/r²` with `1/(r² + ε²)`, which caps the force during close
    /// encounters. Zero keeps the exact Newtonian law — correct, but a
    /// near-collision can then eject a body and ruin the run. A few thousand
    /// kilometres is a sane guard for a planetary system.
    pub softening: f64,

    /// Enables the first post-Newtonian correction from the dominant mass.
    ///
    /// This is what makes Mercury's perihelion advance by the famous
    /// 43″ per century that Newtonian gravity alone cannot explain.
    pub relativistic_correction: bool,
}

impl Default for SimulationParams {
    fn default() -> Self {
        Self {
            gravity_scale: 1.0,
            mass_scale: 1.0,
            radius_scale: 1.0,
            softening: 0.0,
            relativistic_correction: false,
        }
    }
}

impl SimulationParams {
    /// The gravitational constant actually used by the integrator, in
    /// m³·kg⁻¹·s⁻².
    #[inline]
    pub fn gravitational_constant(&self) -> f64 {
        constants::G * self.gravity_scale
    }

    /// Whether these parameters describe the real universe.
    // Exact comparison is what we want: the question is whether a dial has been
    // touched at all, not whether it is nearly untouched.
    #[allow(clippy::float_cmp)]
    pub fn is_physical(&self) -> bool {
        self.gravity_scale == 1.0 && self.mass_scale == 1.0 && self.softening == 0.0
    }

    /// Resets the physical dials to reality, keeping the display-only ones.
    pub fn reset_physics(&mut self) {
        let radius_scale = self.radius_scale;
        *self = Self {
            radius_scale,
            ..Self::default()
        };
    }
}
