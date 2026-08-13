//! Keplerian orbital elements and their conversion to and from state vectors.
//!
//! Ephemerides describe orbits as six angles and lengths rather than as a
//! position and a velocity, because for a two-body problem five of the six are
//! constant. This module is the bridge between that description and the
//! Cartesian state the integrator works on:
//!
//! * [`OrbitalElements::to_state`] — set up a body on a given orbit;
//! * [`OrbitalElements::from_state`] — read the current orbit back out of a
//!   simulation, which is how the UI reports a live semi-major axis or
//!   eccentricity, and how orbit tracks are drawn.
//!
//! The reference plane is the J2000 ecliptic, with `+x` towards the vernal
//! equinox and `+z` towards the ecliptic north pole.

use std::f64::consts::{PI, TAU};

use glam::{DMat3, DVec3};

/// The six classical (Keplerian) elements, plus the epoch anomaly.
///
/// Angles are in radians, lengths in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalElements {
    /// Semi-major axis `a`, in metres. Negative for hyperbolic orbits.
    pub semi_major_axis: f64,
    /// Eccentricity `e`. 0 is a circle, `<1` an ellipse, 1 a parabola,
    /// `>1` a hyperbola.
    pub eccentricity: f64,
    /// Inclination `i` to the reference plane, in radians.
    pub inclination: f64,
    /// Longitude of the ascending node `Ω`, in radians.
    pub longitude_of_ascending_node: f64,
    /// Argument of periapsis `ω`, in radians.
    pub argument_of_periapsis: f64,
    /// Mean anomaly `M` at the epoch, in radians.
    pub mean_anomaly: f64,
}

impl Default for OrbitalElements {
    fn default() -> Self {
        Self::circular(1.0)
    }
}

impl OrbitalElements {
    /// A circular, uninclined orbit of the given radius, in metres.
    pub fn circular(radius: f64) -> Self {
        Self {
            semi_major_axis: radius,
            eccentricity: 0.0,
            inclination: 0.0,
            longitude_of_ascending_node: 0.0,
            argument_of_periapsis: 0.0,
            mean_anomaly: 0.0,
        }
    }

    /// Builds elements from the angles as ephemerides usually publish them:
    /// semi-major axis in metres, and **degrees** for the four angles, given as
    /// mean longitude `L` and longitude of periapsis `ϖ` rather than mean
    /// anomaly and argument of periapsis.
    ///
    /// The conversion is `ω = ϖ − Ω` and `M = L − ϖ`.
    pub fn from_longitudes_deg(
        semi_major_axis: f64,
        eccentricity: f64,
        inclination_deg: f64,
        longitude_of_ascending_node_deg: f64,
        longitude_of_periapsis_deg: f64,
        mean_longitude_deg: f64,
    ) -> Self {
        let node = longitude_of_ascending_node_deg.to_radians();
        let periapsis = longitude_of_periapsis_deg.to_radians();
        let mean_longitude = mean_longitude_deg.to_radians();
        Self {
            semi_major_axis,
            eccentricity,
            inclination: inclination_deg.to_radians(),
            longitude_of_ascending_node: wrap_tau(node),
            argument_of_periapsis: wrap_tau(periapsis - node),
            mean_anomaly: wrap_tau(mean_longitude - periapsis),
        }
    }

    /// Whether the orbit is bound (elliptic or circular).
    pub fn is_bound(&self) -> bool {
        self.eccentricity < 1.0 && self.semi_major_axis > 0.0
    }

    /// Periapsis distance `a(1 − e)`, in metres.
    pub fn periapsis(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity)
    }

    /// Apoapsis distance `a(1 + e)`, in metres. Meaningless for unbound orbits.
    pub fn apoapsis(&self) -> f64 {
        self.semi_major_axis * (1.0 + self.eccentricity)
    }

    /// Semi-minor axis `b = a√(1 − e²)`, in metres.
    pub fn semi_minor_axis(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity * self.eccentricity).abs().sqrt()
    }

    /// Orbital period, in seconds, for a primary of gravitational parameter
    /// `mu = G·M`. Returns [`f64::INFINITY`] for unbound orbits.
    pub fn period(&self, mu: f64) -> f64 {
        if !self.is_bound() || mu <= 0.0 {
            return f64::INFINITY;
        }
        TAU * (self.semi_major_axis.powi(3) / mu).sqrt()
    }

    /// Mean motion `n = √(µ/a³)`, in rad·s⁻¹.
    pub fn mean_motion(&self, mu: f64) -> f64 {
        (mu / self.semi_major_axis.abs().powi(3)).sqrt()
    }

    /// Returns these elements advanced by `dt` seconds.
    ///
    /// Only the mean anomaly changes — which is exactly the point of the
    /// Keplerian description, and why drawing an orbit track costs nothing.
    #[must_use]
    pub fn propagate(&self, mu: f64, dt: f64) -> Self {
        let mut next = *self;
        next.mean_anomaly = wrap_tau(self.mean_anomaly + self.mean_motion(mu) * dt);
        next
    }

    /// Rotation taking perifocal coordinates to the reference frame:
    /// `Rz(Ω)·Rx(i)·Rz(ω)`.
    pub fn perifocal_to_reference(&self) -> DMat3 {
        DMat3::from_rotation_z(self.longitude_of_ascending_node)
            * DMat3::from_rotation_x(self.inclination)
            * DMat3::from_rotation_z(self.argument_of_periapsis)
    }

    /// Converts to a position and velocity relative to the primary.
    ///
    /// `mu` is the gravitational parameter of the two-body system,
    /// `G(M_primary + m_body)`.
    pub fn to_state(&self, mu: f64) -> (DVec3, DVec3) {
        let e = self.eccentricity;
        let a = self.semi_major_axis;
        let eccentric = solve_kepler(self.mean_anomaly, e);

        let (sin_e, cos_e) = eccentric.sin_cos();
        // Radius and position in the orbital plane, periapsis along +x.
        let radius = a * (1.0 - e * cos_e);
        let position = DVec3::new(a * (cos_e - e), a * (1.0 - e * e).sqrt() * sin_e, 0.0);

        // Differentiating the above with respect to time, using
        // dE/dt = n·a/r, gives the perifocal velocity.
        let speed_factor = (mu * a).sqrt() / radius;
        let velocity = DVec3::new(
            -speed_factor * sin_e,
            speed_factor * (1.0 - e * e).sqrt() * cos_e,
            0.0,
        );

        let rotation = self.perifocal_to_reference();
        (rotation * position, rotation * velocity)
    }

    /// Recovers the elements from a position and velocity relative to the
    /// primary.
    ///
    /// Degenerate orbits are handled by convention: for a circular orbit the
    /// argument of periapsis is taken as 0, for an equatorial one the
    /// ascending node is taken as 0.
    pub fn from_state(position: DVec3, velocity: DVec3, mu: f64) -> Self {
        const EPS: f64 = 1e-12;

        let r = position.length();
        let v2 = velocity.length_squared();

        // Specific angular momentum, normal to the orbital plane.
        let h = position.cross(velocity);
        // Node vector, pointing at the ascending node.
        let n = DVec3::Z.cross(h);

        // Eccentricity vector points at periapsis and has magnitude e.
        let e_vec = ((v2 - mu / r) * position - position.dot(velocity) * velocity) / mu;
        let eccentricity = e_vec.length();

        // Vis-viva: v² = µ(2/r − 1/a).
        let specific_energy = v2 / 2.0 - mu / r;
        let semi_major_axis = if specific_energy.abs() < EPS {
            f64::INFINITY // parabolic
        } else {
            -mu / (2.0 * specific_energy)
        };

        let h_len = h.length();
        let inclination = if h_len > EPS {
            (h.z / h_len).clamp(-1.0, 1.0).acos()
        } else {
            0.0
        };

        let equatorial = n.length() < EPS;
        let circular = eccentricity < EPS;

        let longitude_of_ascending_node = if equatorial {
            0.0
        } else {
            wrap_tau(n.y.atan2(n.x))
        };

        let argument_of_periapsis = match (equatorial, circular) {
            (_, true) => 0.0,
            (true, false) => wrap_tau(e_vec.y.atan2(e_vec.x) * h.z.signum()),
            (false, false) => {
                let angle = angle_between(n, e_vec);
                if e_vec.z < 0.0 {
                    wrap_tau(-angle)
                } else {
                    angle
                }
            }
        };

        // True anomaly, measured from whichever reference we just chose.
        let true_anomaly = if circular {
            let reference = if equatorial { DVec3::X } else { n };
            let angle = angle_between(reference, position);
            if h.dot(reference.cross(position)) < 0.0 {
                wrap_tau(-angle)
            } else {
                angle
            }
        } else {
            let angle = angle_between(e_vec, position);
            if position.dot(velocity) < 0.0 {
                wrap_tau(-angle)
            } else {
                angle
            }
        };

        Self {
            semi_major_axis,
            eccentricity,
            inclination,
            longitude_of_ascending_node,
            argument_of_periapsis,
            mean_anomaly: mean_anomaly_from_true(true_anomaly, eccentricity),
        }
    }

    /// Samples `count` points along the orbit, one full revolution, in the
    /// primary's frame.
    ///
    /// Points are evenly spaced in eccentric anomaly rather than in time, which
    /// keeps the resolution near periapsis where the curvature is highest.
    /// Used to draw orbit tracks.
    pub fn sample(&self, count: usize) -> Vec<DVec3> {
        let a = self.semi_major_axis;
        let e = self.eccentricity;
        let b = self.semi_minor_axis();
        let rotation = self.perifocal_to_reference();

        (0..count)
            .map(|i| {
                let eccentric = TAU * i as f64 / count as f64;
                let (sin_e, cos_e) = eccentric.sin_cos();
                rotation * DVec3::new(a * (cos_e - e), b * sin_e, 0.0)
            })
            .collect()
    }
}

/// Solves Kepler's equation `M = E − e·sin E` for the eccentric anomaly `E`.
///
/// Newton–Raphson from a starting guess that is good enough to converge in a
/// handful of iterations even at `e = 0.99`. The result is in radians.
pub fn solve_kepler(mean_anomaly: f64, eccentricity: f64) -> f64 {
    let m = wrap_pi(mean_anomaly);
    let e = eccentricity;

    // For near-circular orbits E ≈ M; for eccentric ones start at π, which is
    // the classic robust choice (the function is convex on each half).
    let mut eccentric = if e < 0.8 { m } else { PI * m.signum() };

    for _ in 0..64 {
        let f = eccentric - e * eccentric.sin() - m;
        let df = 1.0 - e * eccentric.cos();
        if df.abs() < 1e-15 {
            break;
        }
        let delta = f / df;
        eccentric -= delta;
        if delta.abs() < 1e-14 {
            break;
        }
    }
    eccentric
}

/// Converts a true anomaly to a mean anomaly, in radians.
pub fn mean_anomaly_from_true(true_anomaly: f64, eccentricity: f64) -> f64 {
    let e = eccentricity;
    if e >= 1.0 {
        return true_anomaly;
    }
    let eccentric = 2.0
        * ((1.0 - e).sqrt() * (true_anomaly / 2.0).sin())
            .atan2((1.0 + e).sqrt() * (true_anomaly / 2.0).cos());
    wrap_tau(eccentric - e * eccentric.sin())
}

/// Converts an eccentric anomaly to a true anomaly, in radians.
pub fn true_anomaly_from_eccentric(eccentric_anomaly: f64, eccentricity: f64) -> f64 {
    let e = eccentricity;
    wrap_tau(
        2.0 * ((1.0 + e).sqrt() * (eccentric_anomaly / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (eccentric_anomaly / 2.0).cos()),
    )
}

/// Wraps an angle into `[0, 2π)`.
#[inline]
pub fn wrap_tau(angle: f64) -> f64 {
    let wrapped = angle % TAU;
    if wrapped < 0.0 {
        wrapped + TAU
    } else {
        wrapped
    }
}

/// Wraps an angle into `(−π, π]`.
#[inline]
pub fn wrap_pi(angle: f64) -> f64 {
    let wrapped = wrap_tau(angle);
    if wrapped > PI { wrapped - TAU } else { wrapped }
}

fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let denominator = a.length() * b.length();
    if denominator <= 0.0 {
        return 0.0;
    }
    (a.dot(b) / denominator).clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, GM_SUN};

    fn earthlike() -> OrbitalElements {
        OrbitalElements {
            semi_major_axis: AU,
            eccentricity: 0.0167,
            inclination: 0.3,
            longitude_of_ascending_node: 1.2,
            argument_of_periapsis: 2.4,
            mean_anomaly: 0.7,
        }
    }

    #[test]
    fn kepler_equation_inverts_itself() {
        for &e in &[0.0, 0.1, 0.5, 0.9, 0.99] {
            for i in 0..100 {
                let m = wrap_pi(TAU * f64::from(i) / 100.0);
                let eccentric = solve_kepler(m, e);
                let recovered = eccentric - e * eccentric.sin();
                assert!(
                    (wrap_pi(recovered - m)).abs() < 1e-12,
                    "e={e} M={m}: got E={eccentric}"
                );
            }
        }
    }

    #[test]
    fn elements_round_trip_through_state_vectors() {
        let original = earthlike();
        let (position, velocity) = original.to_state(GM_SUN);
        let recovered = OrbitalElements::from_state(position, velocity, GM_SUN);

        assert!((recovered.semi_major_axis / original.semi_major_axis - 1.0).abs() < 1e-12);
        assert!((recovered.eccentricity - original.eccentricity).abs() < 1e-12);
        assert!((recovered.inclination - original.inclination).abs() < 1e-12);
        assert!(
            wrap_pi(recovered.longitude_of_ascending_node - original.longitude_of_ascending_node)
                .abs()
                < 1e-12
        );
        assert!(
            wrap_pi(recovered.argument_of_periapsis - original.argument_of_periapsis).abs() < 1e-12
        );
        assert!(wrap_pi(recovered.mean_anomaly - original.mean_anomaly).abs() < 1e-12);
    }

    #[test]
    fn circular_orbit_has_the_expected_speed_and_period() {
        let elements = OrbitalElements::circular(AU);
        let (position, velocity) = elements.to_state(GM_SUN);

        assert!((position.length() / AU - 1.0).abs() < 1e-12);
        // v = √(µ/r) ≈ 29.78 km/s for the Earth.
        assert!((velocity.length() - (GM_SUN / AU).sqrt()).abs() < 1e-6);
        // ... and the period is one year to within a fraction of a percent.
        let period_days = elements.period(GM_SUN) / 86_400.0;
        assert!((period_days - 365.25).abs() < 1.0, "got {period_days} days");
    }

    #[test]
    fn propagating_by_one_period_returns_to_the_start() {
        let elements = earthlike();
        let period = elements.period(GM_SUN);
        let (p0, _) = elements.to_state(GM_SUN);
        let (p1, _) = elements.propagate(GM_SUN, period).to_state(GM_SUN);
        assert!((p1 - p0).length() / AU < 1e-10);
    }

    #[test]
    fn sampled_track_stays_between_periapsis_and_apoapsis() {
        let elements = earthlike();
        for point in elements.sample(256) {
            let r = point.length();
            assert!(r >= elements.periapsis() * (1.0 - 1e-9));
            assert!(r <= elements.apoapsis() * (1.0 + 1e-9));
        }
    }
}
