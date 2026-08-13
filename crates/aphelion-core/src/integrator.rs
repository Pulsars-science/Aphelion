//! Time integration schemes.
//!
//! Orbits are integrated for millions of steps, so what matters is not the
//! error of one step but whether the error accumulates. *Symplectic*
//! integrators conserve a slightly perturbed energy exactly, so their energy
//! error oscillates around zero forever instead of drifting. That is why a
//! second-order symplectic scheme beats a fourth-order Runge–Kutta over long
//! runs, and why [`Integrator::VelocityVerlet`] is the default.

use glam::DVec3;

use crate::nbody::State;

/// Reusable buffers, so stepping does not allocate.
///
/// Grown automatically by [`Scratch::resize`]; you normally never touch this
/// directly — [`Simulation`](crate::Simulation) owns one.
#[derive(Debug, Clone, Default)]
pub struct Scratch {
    acc: Vec<DVec3>,
    k_pos: [Vec<DVec3>; 4],
    k_vel: [Vec<DVec3>; 4],
    tmp: State,
}

impl Scratch {
    /// An empty scratch space.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes sure every buffer can hold `n` bodies.
    pub fn resize(&mut self, n: usize) {
        self.acc.resize(n, DVec3::ZERO);
        for buffer in &mut self.k_pos {
            buffer.resize(n, DVec3::ZERO);
        }
        for buffer in &mut self.k_vel {
            buffer.resize(n, DVec3::ZERO);
        }
        self.tmp.positions.resize(n, DVec3::ZERO);
        self.tmp.velocities.resize(n, DVec3::ZERO);
    }
}

/// The available time-stepping schemes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Integrator {
    /// Semi-implicit (symplectic) Euler. First order.
    ///
    /// Only useful as a baseline: it is symplectic, so it does not blow up, but
    /// it precesses orbits visibly within a few revolutions.
    SemiImplicitEuler,

    /// Velocity Verlet, in kick–drift–kick form. Second order, symplectic,
    /// time-reversible. Two force evaluations per step.
    ///
    /// The workhorse of celestial mechanics and the default here.
    #[default]
    VelocityVerlet,

    /// Yoshida's fourth-order symplectic composition of three Verlet steps.
    ///
    /// Six force evaluations per step, but the error falls as `dt⁴`, so it is
    /// usually cheaper than Verlet at equal accuracy. Use it for long runs or
    /// tight orbits.
    Yoshida4,

    /// Classical fourth-order Runge–Kutta. Not symplectic.
    ///
    /// Accurate over short spans and the only scheme here that handles strongly
    /// velocity-dependent forces cleanly, but its energy error grows without
    /// bound, so it is a poor choice for century-long integrations.
    Rk4,
}

impl Integrator {
    /// Every variant, in increasing order of accuracy.
    pub const ALL: &'static [Integrator] = &[
        Integrator::SemiImplicitEuler,
        Integrator::VelocityVerlet,
        Integrator::Yoshida4,
        Integrator::Rk4,
    ];

    /// Short human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Integrator::SemiImplicitEuler => "Semi-implicit Euler",
            Integrator::VelocityVerlet => "Velocity Verlet",
            Integrator::Yoshida4 => "Yoshida 4",
            Integrator::Rk4 => "Runge-Kutta 4",
        }
    }

    /// Order of the local truncation error.
    pub fn order(self) -> u32 {
        match self {
            Integrator::SemiImplicitEuler => 1,
            Integrator::VelocityVerlet => 2,
            Integrator::Yoshida4 | Integrator::Rk4 => 4,
        }
    }

    /// Whether the scheme preserves phase-space volume, and therefore keeps the
    /// energy error bounded rather than drifting.
    pub fn is_symplectic(self) -> bool {
        !matches!(self, Integrator::Rk4)
    }

    /// Number of force evaluations per step.
    pub fn force_evaluations(self) -> u32 {
        match self {
            Integrator::SemiImplicitEuler => 1,
            Integrator::VelocityVerlet => 2,
            Integrator::Yoshida4 => 6,
            Integrator::Rk4 => 4,
        }
    }

    /// Advances `state` by `dt` seconds.
    ///
    /// `accel` is called as `accel(positions, velocities, out)` and must write
    /// the acceleration of every body into `out`. Velocities are passed so that
    /// velocity-dependent terms (drag, the relativistic correction) can be
    /// included.
    ///
    /// # Panics
    ///
    /// Panics if `scratch` was not sized for `state.len()`.
    pub fn step<F>(self, state: &mut State, dt: f64, scratch: &mut Scratch, accel: F)
    where
        F: Fn(&[DVec3], &[DVec3], &mut [DVec3]),
    {
        assert!(
            scratch.acc.len() >= state.len(),
            "scratch is not sized for this system; call Scratch::resize first"
        );
        match self {
            Integrator::SemiImplicitEuler => euler_step(state, dt, scratch, &accel),
            Integrator::VelocityVerlet => verlet_step(state, dt, scratch, &accel),
            Integrator::Yoshida4 => yoshida4_step(state, dt, scratch, &accel),
            Integrator::Rk4 => rk4_step(state, dt, scratch, &accel),
        }
    }
}

fn euler_step<F>(state: &mut State, dt: f64, scratch: &mut Scratch, accel: &F)
where
    F: Fn(&[DVec3], &[DVec3], &mut [DVec3]),
{
    let n = state.len();
    accel(&state.positions, &state.velocities, &mut scratch.acc[..n]);
    for i in 0..n {
        // Velocity first, then position with the *new* velocity: that ordering
        // is what makes plain Euler symplectic.
        state.velocities[i] += scratch.acc[i] * dt;
        state.positions[i] += state.velocities[i] * dt;
    }
}

fn verlet_step<F>(state: &mut State, dt: f64, scratch: &mut Scratch, accel: &F)
where
    F: Fn(&[DVec3], &[DVec3], &mut [DVec3]),
{
    let n = state.len();
    let half = 0.5 * dt;

    // Kick.
    accel(&state.positions, &state.velocities, &mut scratch.acc[..n]);
    for i in 0..n {
        state.velocities[i] += scratch.acc[i] * half;
    }
    // Drift.
    for i in 0..n {
        state.positions[i] += state.velocities[i] * dt;
    }
    // Kick, with the force at the new position.
    accel(&state.positions, &state.velocities, &mut scratch.acc[..n]);
    for i in 0..n {
        state.velocities[i] += scratch.acc[i] * half;
    }
}

/// Yoshida's coefficients: `w₁ + w₀ + w₁ = 1` with the third-order error terms
/// cancelling between the forward and backward sub-steps.
const YOSHIDA_W1: f64 = 1.351_207_191_959_657_6; // 1 / (2 − 2^(1/3))
const YOSHIDA_W0: f64 = -1.702_414_383_919_315; // −2^(1/3) / (2 − 2^(1/3))

fn yoshida4_step<F>(state: &mut State, dt: f64, scratch: &mut Scratch, accel: &F)
where
    F: Fn(&[DVec3], &[DVec3], &mut [DVec3]),
{
    verlet_step(state, YOSHIDA_W1 * dt, scratch, accel);
    verlet_step(state, YOSHIDA_W0 * dt, scratch, accel);
    verlet_step(state, YOSHIDA_W1 * dt, scratch, accel);
}

fn rk4_step<F>(state: &mut State, dt: f64, scratch: &mut Scratch, accel: &F)
where
    F: Fn(&[DVec3], &[DVec3], &mut [DVec3]),
{
    let n = state.len();

    // Stage 1, at the current state.
    scratch.k_pos[0][..n].copy_from_slice(&state.velocities[..n]);
    accel(
        &state.positions,
        &state.velocities,
        &mut scratch.k_vel[0][..n],
    );

    // Stages 2 and 3, at the midpoint; stage 4, at the endpoint.
    for (stage, weight) in [(1usize, 0.5), (2, 0.5), (3, 1.0)] {
        let prev = stage - 1;
        let h = dt * weight;
        for i in 0..n {
            scratch.tmp.positions[i] = state.positions[i] + scratch.k_pos[prev][i] * h;
            scratch.tmp.velocities[i] = state.velocities[i] + scratch.k_vel[prev][i] * h;
        }
        scratch.k_pos[stage][..n].copy_from_slice(&scratch.tmp.velocities[..n]);
        accel(
            &scratch.tmp.positions,
            &scratch.tmp.velocities,
            &mut scratch.k_vel[stage][..n],
        );
    }

    let sixth = dt / 6.0;
    for i in 0..n {
        state.positions[i] += sixth
            * (scratch.k_pos[0][i]
                + 2.0 * scratch.k_pos[1][i]
                + 2.0 * scratch.k_pos[2][i]
                + scratch.k_pos[3][i]);
        state.velocities[i] += sixth
            * (scratch.k_vel[0][i]
                + 2.0 * scratch.k_vel[1][i]
                + 2.0 * scratch.k_vel[2][i]
                + scratch.k_vel[3][i]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit-frequency harmonic oscillator: `a = −x`. Exact solution is a circle
    /// in phase space, so both the amplitude and the energy must be preserved.
    fn oscillator(positions: &[DVec3], _velocities: &[DVec3], out: &mut [DVec3]) {
        for (o, p) in out.iter_mut().zip(positions) {
            *o = -*p;
        }
    }

    fn energy(state: &State) -> f64 {
        0.5 * (state.positions[0].length_squared() + state.velocities[0].length_squared())
    }

    #[test]
    fn every_scheme_tracks_the_analytic_orbit() {
        for &integrator in Integrator::ALL {
            let mut state = State {
                positions: vec![DVec3::X],
                velocities: vec![DVec3::Y],
            };
            let mut scratch = Scratch::new();
            scratch.resize(1);

            let steps = 10_000_i32;
            let dt = std::f64::consts::TAU / f64::from(steps); // exactly one period
            for _ in 0..steps {
                integrator.step(&mut state, dt, &mut scratch, oscillator);
            }

            let error = (state.positions[0] - DVec3::X).length();
            let tolerance = match integrator.order() {
                1 => 1e-2,
                2 => 1e-5,
                _ => 1e-10,
            };
            assert!(
                error < tolerance,
                "{}: returned to {:?} after one period (error {error:e})",
                integrator.name(),
                state.positions[0],
            );
        }
    }

    #[test]
    fn symplectic_schemes_keep_energy_bounded() {
        for &integrator in Integrator::ALL.iter().filter(|i| i.is_symplectic()) {
            let mut state = State {
                positions: vec![DVec3::X],
                velocities: vec![DVec3::Y],
            };
            let mut scratch = Scratch::new();
            scratch.resize(1);

            let initial = energy(&state);
            let dt = 0.01;
            let mut worst: f64 = 0.0;
            // ~2000 periods: a non-symplectic scheme would have visibly drifted
            // by now, a symplectic one just oscillates around the true energy.
            for _ in 0..200_000 {
                integrator.step(&mut state, dt, &mut scratch, oscillator);
                worst = worst.max(((energy(&state) - initial) / initial).abs());
            }
            assert!(
                worst < 2e-2,
                "{} drifted by {worst:e} over 2000 periods",
                integrator.name()
            );
        }
    }
}
