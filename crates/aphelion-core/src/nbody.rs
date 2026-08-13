//! The gravitational N-body problem: forces and conserved quantities.
//!
//! The force evaluation here is the direct `O(n²)` pairwise sum. For the few
//! hundred bodies a solar system needs that is the right answer: it is exact,
//! branch-free and cache-friendly. A Barnes–Hut tree only pays off in the tens
//! of thousands, and is tracked as a future addition.

use glam::DVec3;

/// Positions and velocities of every body, as parallel arrays.
///
/// Kept as a struct of arrays rather than a `Vec<Particle>` so the force loop
/// walks memory linearly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// Position of each body, in metres.
    pub positions: Vec<DVec3>,
    /// Velocity of each body, in m·s⁻¹.
    pub velocities: Vec<DVec3>,
}

impl State {
    /// An empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of bodies.
    #[inline]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Whether the state holds no bodies.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Appends a body and returns its index.
    pub fn push(&mut self, position: DVec3, velocity: DVec3) -> usize {
        self.positions.push(position);
        self.velocities.push(velocity);
        self.positions.len() - 1
    }

    /// Translates every position by `offset` and every velocity by
    /// `velocity_offset` — a change of inertial frame.
    pub fn shift_frame(&mut self, offset: DVec3, velocity_offset: DVec3) {
        for p in &mut self.positions {
            *p -= offset;
        }
        for v in &mut self.velocities {
            *v -= velocity_offset;
        }
    }
}

/// Accumulates the Newtonian acceleration of every body into `out`.
///
/// `softening` is a Plummer length ε: the pair separation is replaced by
/// `sqrt(r² + ε²)`, which bounds the force as `r → 0`. Pass `0.0` for exact
/// Newtonian gravity.
///
/// Newton's third law is used, so only `n(n-1)/2` pair terms are evaluated.
///
/// # Panics
///
/// Panics if `positions`, `masses` and `out` do not all have the same length.
pub fn accelerations(
    positions: &[DVec3],
    masses: &[f64],
    g: f64,
    softening: f64,
    out: &mut [DVec3],
) {
    assert_eq!(
        positions.len(),
        masses.len(),
        "positions/masses length mismatch"
    );
    assert_eq!(positions.len(), out.len(), "positions/out length mismatch");

    out.fill(DVec3::ZERO);
    let eps2 = softening * softening;

    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let delta = positions[j] - positions[i];
            let dist2 = delta.length_squared() + eps2;
            if dist2 <= 0.0 {
                // Two bodies exactly on top of each other with no softening.
                // There is no defined direction; the only non-destructive
                // answer is to apply nothing.
                continue;
            }
            // delta / |delta|³, without the intermediate normalise.
            let inv_dist3 = dist2.sqrt().powi(3).recip();
            let shared = delta * inv_dist3 * g;
            out[i] += shared * masses[j];
            out[j] -= shared * masses[i];
        }
    }
}

/// Adds the first post-Newtonian (1PN) acceleration produced by a single
/// dominant mass — the Schwarzschild correction for a test particle.
///
/// For a body at `r` from the central mass with velocity `v` relative to it:
///
/// ```text
/// a_1PN = GM / (c² r³) · [ (4GM/r − v²)·r + 4·(r·v)·v ]
/// ```
///
/// Applied to Mercury this reproduces the observed 43″/century advance of its
/// perihelion. It is negligible for the outer planets.
///
/// `central` is the index of the dominant mass; it receives no correction.
pub fn add_relativistic_correction(
    positions: &[DVec3],
    velocities: &[DVec3],
    masses: &[f64],
    g: f64,
    central: usize,
    out: &mut [DVec3],
) {
    let mu = g * masses[central];
    let c2 = crate::constants::C * crate::constants::C;

    for i in 0..positions.len() {
        if i == central {
            continue;
        }
        let r = positions[i] - positions[central];
        let v = velocities[i] - velocities[central];
        let dist = r.length();
        if dist <= 0.0 {
            continue;
        }
        let factor = mu / (c2 * dist.powi(3));
        out[i] += factor * ((4.0 * mu / dist - v.length_squared()) * r + 4.0 * r.dot(v) * v);
    }
}

/// Total kinetic energy of the system, in joules.
pub fn kinetic_energy(velocities: &[DVec3], masses: &[f64]) -> f64 {
    velocities
        .iter()
        .zip(masses)
        .map(|(v, m)| 0.5 * m * v.length_squared())
        .sum()
}

/// Total gravitational potential energy of the system, in joules (negative for
/// a bound system).
pub fn potential_energy(positions: &[DVec3], masses: &[f64], g: f64, softening: f64) -> f64 {
    let eps2 = softening * softening;
    let mut energy = 0.0;
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            let dist = (positions[j] - positions[i]).length_squared() + eps2;
            if dist > 0.0 {
                energy -= g * masses[i] * masses[j] / dist.sqrt();
            }
        }
    }
    energy
}

/// Total mechanical energy, in joules.
///
/// A symplectic integrator keeps this bounded and oscillating; a steady drift
/// means the step size is too large. See
/// [`Simulation::energy_drift`](crate::Simulation::energy_drift).
pub fn total_energy(state: &State, masses: &[f64], g: f64, softening: f64) -> f64 {
    kinetic_energy(&state.velocities, masses)
        + potential_energy(&state.positions, masses, g, softening)
}

/// Total angular momentum about the origin, in kg·m²·s⁻¹.
///
/// Unlike energy, this is conserved exactly by every integrator here, because
/// the pairwise forces are central. It is a useful independent check.
pub fn angular_momentum(state: &State, masses: &[f64]) -> DVec3 {
    state
        .positions
        .iter()
        .zip(&state.velocities)
        .zip(masses)
        .map(|((r, v), m)| *m * r.cross(*v))
        .fold(DVec3::ZERO, |acc, l| acc + l)
}

/// Total mass, in kilograms.
pub fn total_mass(masses: &[f64]) -> f64 {
    masses.iter().sum()
}

/// Centre of mass of the system, in metres.
pub fn barycentre(positions: &[DVec3], masses: &[f64]) -> DVec3 {
    weighted_mean(positions, masses)
}

/// Velocity of the centre of mass, in m·s⁻¹.
pub fn barycentre_velocity(velocities: &[DVec3], masses: &[f64]) -> DVec3 {
    weighted_mean(velocities, masses)
}

fn weighted_mean(values: &[DVec3], masses: &[f64]) -> DVec3 {
    let total = total_mass(masses);
    if total <= 0.0 {
        return DVec3::ZERO;
    }
    values
        .iter()
        .zip(masses)
        .map(|(value, m)| *value * *m)
        .fold(DVec3::ZERO, |acc, x| acc + x)
        / total
}

/// Radius of the Hill sphere of `satellite_mass` orbiting `primary_mass` at
/// semi-major axis `a` with eccentricity `e`, in metres.
///
/// Inside this radius the smaller body dominates gravitationally; it is the
/// practical limit for a stable moon.
pub fn hill_radius(a: f64, e: f64, satellite_mass: f64, primary_mass: f64) -> f64 {
    if primary_mass <= 0.0 {
        return 0.0;
    }
    a * (1.0 - e) * (satellite_mass / (3.0 * primary_mass)).cbrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_body_forces_are_equal_and_opposite() {
        let positions = [DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0)];
        let masses = [2.0, 3.0];
        let mut acc = [DVec3::ZERO; 2];
        accelerations(&positions, &masses, 1.0, 0.0, &mut acc);

        // a1 = G·m2/r², a2 = −G·m1/r²
        assert!((acc[0].x - 3.0).abs() < 1e-12);
        assert!((acc[1].x + 2.0).abs() < 1e-12);
        // Momentum change cancels.
        let net = acc[0] * masses[0] + acc[1] * masses[1];
        assert!(net.length() < 1e-12);
    }

    #[test]
    fn softening_bounds_the_force() {
        let positions = [DVec3::ZERO, DVec3::new(1e-9, 0.0, 0.0)];
        let masses = [1.0, 1.0];
        let mut acc = [DVec3::ZERO; 2];
        accelerations(&positions, &masses, 1.0, 1.0, &mut acc);
        assert!(acc[0].length() < 1.0, "softened force should stay finite");
    }

    #[test]
    fn barycentre_of_equal_masses_is_the_midpoint() {
        let positions = [DVec3::new(-2.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 0.0)];
        let masses = [1.0, 1.0];
        assert!((barycentre(&positions, &masses) - DVec3::new(1.0, 0.0, 0.0)).length() < 1e-12);
    }
}
