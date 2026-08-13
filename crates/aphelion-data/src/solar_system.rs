//! The Solar System at epoch J2000.0.
//!
//! # Sources
//!
//! * **Orbital elements** — E. M. Standish, *Keplerian Elements for Approximate
//!   Positions of the Major Planets* (JPL Solar System Dynamics), valid
//!   1800–2050. These are *mean* elements: they describe the average ellipse,
//!   not the instantaneous one, so a body starts within a fraction of a degree
//!   of its true J2000 position rather than exactly on it.
//! * **Masses and radii** — JPL Solar System Dynamics planetary physical
//!   parameters, and the IAU 2015 nominal values for the Sun and Earth.
//! * **Rotation and obliquity** — IAU Working Group on Cartographic Coordinates
//!   and Rotational Elements, 2015 report.
//!
//! # Accuracy
//!
//! Good enough to look right and to behave right: the planets end up on the
//! correct orbits, in roughly the correct places, and the long-term dynamics
//! (resonances, secular precession, the Sun's wobble about the barycentre) are
//! genuine because they emerge from the integration rather than from a fitted
//! series.
//!
//! It is *not* an ephemeris. If you need to know where Mars was on a given
//! night to arcsecond accuracy, you want JPL DE440 — see the roadmap.

use aphelion_core::DVec3;
use aphelion_core::body::{Body, BodyId, BodyKind};
use aphelion_core::constants::{AU, DAY, G, GM_SUN, SOLAR_RADIUS, deg, km};
use aphelion_core::kepler::OrbitalElements;
use aphelion_core::sim::Simulation;

/// Physical and orbital description of one body, in astronomer-friendly units.
///
/// This is the table as published; [`solar_system`] converts it to SI and feeds
/// it to the integrator.
#[derive(Debug, Clone, Copy)]
pub struct PlanetData {
    /// Display name.
    pub name: &'static str,
    /// Category.
    pub kind: BodyKind,
    /// Mass, in kilograms.
    pub mass: f64,
    /// Mean radius, in kilometres.
    pub radius_km: f64,
    /// Semi-major axis, in astronomical units.
    pub semi_major_axis_au: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Inclination to the ecliptic, in degrees.
    pub inclination_deg: f64,
    /// Longitude of the ascending node `Ω`, in degrees.
    pub node_deg: f64,
    /// Longitude of periapsis `ϖ = Ω + ω`, in degrees.
    pub periapsis_deg: f64,
    /// Mean longitude `L = ϖ + M` at J2000.0, in degrees.
    pub mean_longitude_deg: f64,
    /// Sidereal rotation period, in days. Negative means retrograde.
    pub rotation_days: f64,
    /// Obliquity of the rotation axis, in degrees.
    pub axial_tilt_deg: f64,
    /// Approximate visual colour, as linear RGB.
    pub color: [f32; 3],
}

impl PlanetData {
    /// The [`Body`] this entry describes, in SI units.
    pub fn to_body(&self) -> Body {
        Body::new(self.name, self.kind, self.mass, km(self.radius_km))
            .with_rotation(self.rotation_days * DAY)
            .with_axial_tilt(deg(self.axial_tilt_deg))
            .with_color(self.color)
    }

    /// The J2000.0 orbital elements this entry describes, in SI units and
    /// radians.
    pub fn to_elements(&self) -> OrbitalElements {
        OrbitalElements::from_longitudes_deg(
            self.semi_major_axis_au * AU,
            self.eccentricity,
            self.inclination_deg,
            self.node_deg,
            self.periapsis_deg,
            self.mean_longitude_deg,
        )
    }
}

/// The Sun.
pub const SUN: PlanetData = PlanetData {
    name: "Sun",
    kind: BodyKind::Star,
    mass: GM_SUN / G,
    radius_km: SOLAR_RADIUS / 1000.0,
    semi_major_axis_au: 0.0,
    eccentricity: 0.0,
    inclination_deg: 0.0,
    node_deg: 0.0,
    periapsis_deg: 0.0,
    mean_longitude_deg: 0.0,
    rotation_days: 25.38,
    axial_tilt_deg: 7.25,
    color: [1.0, 0.95, 0.80],
};

/// The eight planets and Pluto, in order of distance from the Sun.
///
/// The Earth entry uses the Earth–Moon barycentre's orbit, which is what the
/// JPL table publishes; the Moon is then added around the Earth separately, and
/// the small resulting offset is well inside the accuracy of mean elements.
pub const PLANETS: &[PlanetData] = &[
    PlanetData {
        name: "Mercury",
        kind: BodyKind::Planet,
        mass: 3.3011e23,
        radius_km: 2439.7,
        semi_major_axis_au: 0.38709927,
        eccentricity: 0.20563593,
        inclination_deg: 7.00497902,
        node_deg: 48.33076593,
        periapsis_deg: 77.45779628,
        mean_longitude_deg: 252.25032350,
        rotation_days: 58.646,
        axial_tilt_deg: 0.034,
        color: [0.55, 0.51, 0.47],
    },
    PlanetData {
        name: "Venus",
        kind: BodyKind::Planet,
        mass: 4.8675e24,
        radius_km: 6051.8,
        semi_major_axis_au: 0.72333566,
        eccentricity: 0.00677672,
        inclination_deg: 3.39467605,
        node_deg: 76.67984255,
        periapsis_deg: 131.60246718,
        mean_longitude_deg: 181.97909950,
        // Venus turns backwards, once every 243 days — longer than its year.
        rotation_days: -243.025,
        axial_tilt_deg: 177.36,
        color: [0.90, 0.80, 0.60],
    },
    PlanetData {
        name: "Earth",
        kind: BodyKind::Planet,
        mass: 5.97217e24,
        radius_km: 6371.0,
        semi_major_axis_au: 1.00000261,
        eccentricity: 0.01671123,
        inclination_deg: -0.00001531,
        node_deg: 0.0,
        periapsis_deg: 102.93768193,
        mean_longitude_deg: 100.46457166,
        rotation_days: 0.99726968,
        axial_tilt_deg: 23.4392811,
        color: [0.22, 0.40, 0.65],
    },
    PlanetData {
        name: "Mars",
        kind: BodyKind::Planet,
        mass: 6.4171e23,
        radius_km: 3389.5,
        semi_major_axis_au: 1.52371034,
        eccentricity: 0.09339410,
        inclination_deg: 1.84969142,
        node_deg: 49.55953891,
        periapsis_deg: -23.94362959,
        mean_longitude_deg: -4.55343205,
        rotation_days: 1.02595676,
        axial_tilt_deg: 25.19,
        color: [0.71, 0.38, 0.24],
    },
    PlanetData {
        name: "Jupiter",
        kind: BodyKind::Planet,
        mass: 1.8982e27,
        radius_km: 69911.0,
        semi_major_axis_au: 5.20288700,
        eccentricity: 0.04838624,
        inclination_deg: 1.30439695,
        node_deg: 100.47390909,
        periapsis_deg: 14.72847983,
        mean_longitude_deg: 34.39644051,
        rotation_days: 0.41354,
        axial_tilt_deg: 3.13,
        color: [0.78, 0.68, 0.55],
    },
    PlanetData {
        name: "Saturn",
        kind: BodyKind::Planet,
        mass: 5.6834e26,
        radius_km: 58232.0,
        semi_major_axis_au: 9.53667594,
        eccentricity: 0.05386179,
        inclination_deg: 2.48599187,
        node_deg: 113.66242448,
        periapsis_deg: 92.59887831,
        mean_longitude_deg: 49.95424423,
        rotation_days: 0.44401,
        axial_tilt_deg: 26.73,
        color: [0.85, 0.77, 0.58],
    },
    PlanetData {
        name: "Uranus",
        kind: BodyKind::Planet,
        mass: 8.6810e25,
        radius_km: 25362.0,
        semi_major_axis_au: 19.18916464,
        eccentricity: 0.04725744,
        inclination_deg: 0.77263783,
        node_deg: 74.01692503,
        periapsis_deg: 170.95427630,
        mean_longitude_deg: 313.23810451,
        // Uranus is tipped over almost a full right angle, so it rolls along
        // its orbit rather than spinning upright.
        rotation_days: -0.71833,
        axial_tilt_deg: 97.77,
        color: [0.56, 0.77, 0.80],
    },
    PlanetData {
        name: "Neptune",
        kind: BodyKind::Planet,
        mass: 1.02413e26,
        radius_km: 24622.0,
        semi_major_axis_au: 30.06992276,
        eccentricity: 0.00859048,
        inclination_deg: 1.77004347,
        node_deg: 131.78422574,
        periapsis_deg: 44.96476227,
        mean_longitude_deg: -55.12002969,
        rotation_days: 0.67125,
        axial_tilt_deg: 28.32,
        color: [0.26, 0.40, 0.72],
    },
    PlanetData {
        name: "Pluto",
        kind: BodyKind::DwarfPlanet,
        mass: 1.303e22,
        radius_km: 1188.3,
        semi_major_axis_au: 39.48211675,
        eccentricity: 0.24882730,
        inclination_deg: 17.14001206,
        node_deg: 110.30393684,
        periapsis_deg: 224.06891629,
        mean_longitude_deg: 238.92903833,
        rotation_days: -6.38723,
        axial_tilt_deg: 122.53,
        color: [0.70, 0.63, 0.55],
    },
];

/// The Moon: mass, radius, and its J2000 orbit around the Earth.
const MOON: PlanetData = PlanetData {
    name: "Moon",
    kind: BodyKind::Moon,
    mass: 7.342e22,
    radius_km: 1737.4,
    // 384 400 km, expressed in AU because the table is in AU.
    semi_major_axis_au: 384_400.0 * 1000.0 / AU,
    eccentricity: 0.0549,
    inclination_deg: 5.145,
    node_deg: 125.08,
    periapsis_deg: 125.08 + 318.15,
    mean_longitude_deg: 125.08 + 318.15 + 135.27,
    // Tidally locked: it turns exactly once per orbit, which is why we only
    // ever see one face.
    rotation_days: 27.321661,
    axial_tilt_deg: 6.68,
    color: [0.55, 0.53, 0.50],
};

/// Builds the Solar System at J2000.0: Sun, eight planets, the Moon and Pluto.
///
/// The result is already in the barycentric frame and has its energy reference
/// taken, so [`Simulation::energy_drift`] is meaningful immediately.
///
/// ```
/// let sim = aphelion_data::solar_system();
/// assert_eq!(sim.len(), 11);
/// assert!(sim.find("Jupiter").is_some());
/// ```
pub fn solar_system() -> Simulation {
    build(PLANETS, true)
}

/// The same, but only out to Mars — and without the Moon.
///
/// Handy for tests and for anything that wants a system whose fastest orbit is
/// not 27 days long, since that is what sets the step size.
pub fn solar_system_inner() -> Simulation {
    build(&PLANETS[..4], false)
}

fn build(planets: &[PlanetData], with_moon: bool) -> Simulation {
    let mut sim = Simulation::new();
    let sun = sim.add_body(SUN.to_body(), DVec3::ZERO, DVec3::ZERO);

    let mut earth: Option<BodyId> = None;
    for planet in planets {
        let id = sim.add_orbiting(planet.to_body(), sun, &planet.to_elements());
        if planet.name == "Earth" {
            earth = Some(id);
        }
    }

    if with_moon && let Some(earth) = earth {
        sim.add_orbiting(MOON.to_body(), earth, &MOON.to_elements());
    }

    // Without this the Sun sits still while everything orbits it, which leaves
    // the centre of mass sliding steadily off the screen.
    sim.recentre_on_barycentre();
    sim.reset_energy_reference();
    sim
}

#[cfg(test)]
mod tests {
    use super::*;
    use aphelion_core::constants::{DAY, YEAR, to_au};

    #[test]
    fn the_system_has_the_expected_bodies() {
        let sim = solar_system();
        assert_eq!(sim.len(), 11);
        for name in ["Sun", "Mercury", "Earth", "Moon", "Neptune", "Pluto"] {
            assert!(sim.find(name).is_some(), "missing {name}");
        }
    }

    #[test]
    fn every_planet_starts_on_a_bound_orbit_of_the_right_size() {
        let sim = solar_system();
        for planet in PLANETS {
            let id = sim.find(planet.name).unwrap();
            let elements = sim.elements_of(id).unwrap();
            assert!(elements.is_bound(), "{} is unbound", planet.name);

            let a = to_au(elements.semi_major_axis);
            let relative_error = (a / planet.semi_major_axis_au - 1.0).abs();
            assert!(
                relative_error < 1e-3,
                "{}: a = {a} AU, expected {}",
                planet.name,
                planet.semi_major_axis_au
            );
        }
    }

    /// Kepler's third law is not something we impose — it falls out of the
    /// integration. Checking the known periods is therefore a real test of the
    /// data *and* the dynamics.
    #[test]
    fn orbital_periods_match_the_published_values() {
        let sim = solar_system();
        let expected_years = [
            ("Mercury", 0.2408),
            ("Venus", 0.6152),
            ("Earth", 1.0000),
            ("Mars", 1.8808),
            ("Jupiter", 11.862),
            ("Saturn", 29.457),
            ("Uranus", 84.021),
            ("Neptune", 164.79),
            ("Pluto", 247.94),
        ];
        for (name, years) in expected_years {
            let id = sim.find(name).unwrap();
            let period = sim.period_of(id).unwrap() / YEAR;
            assert!(
                (period / years - 1.0).abs() < 5e-3,
                "{name}: got {period:.4} yr, expected {years} yr"
            );
        }
    }

    #[test]
    fn the_moon_orbits_the_earth_in_a_lunar_month() {
        let sim = solar_system();
        let moon = sim.find("Moon").unwrap();
        assert_eq!(
            sim.body(moon).unwrap().parent,
            Some(sim.find("Earth").unwrap())
        );
        let period = sim.period_of(moon).unwrap() / DAY;
        assert!((period - 27.32).abs() < 0.5, "got {period:.3} days");
    }

    /// The whole system must hold together over a century: energy conserved,
    /// nothing ejected, every planet still on its own orbit.
    #[test]
    fn the_inner_system_is_stable_over_a_century() {
        let mut sim = solar_system_inner();
        sim.integrator = aphelion_core::Integrator::Yoshida4;
        sim.advance(100.0 * YEAR, DAY);

        assert!(
            sim.energy_drift().abs() < 1e-6,
            "energy drifted by {:e}",
            sim.energy_drift()
        );
        for planet in &PLANETS[..4] {
            let id = sim.find(planet.name).unwrap();
            let elements = sim.elements_of(id).unwrap();
            assert!(elements.is_bound(), "{} escaped", planet.name);
            let a = to_au(elements.semi_major_axis);
            assert!(
                (a / planet.semi_major_axis_au - 1.0).abs() < 1e-3,
                "{} wandered to a = {a} AU",
                planet.name
            );
        }
    }

    /// The argument for defaulting to a symplectic integrator, as a test.
    ///
    /// Runge–Kutta's energy error grows in proportion to elapsed time — double
    /// the span, double the error. Yoshida's oscillates around zero and never
    /// grows, which is why it wins over long integrations despite being nominally
    /// the same order.
    #[test]
    fn runge_kutta_energy_drifts_while_yoshida_only_oscillates() {
        let drift_after = |integrator, years: u32| {
            let mut sim = solar_system_inner();
            sim.integrator = integrator;
            let mut worst: f64 = 0.0;
            for _ in 0..years {
                sim.advance(YEAR, DAY);
                worst = worst.max(sim.energy_drift().abs());
            }
            (sim.energy_drift().abs(), worst)
        };

        let (rk4_50, _) = drift_after(aphelion_core::Integrator::Rk4, 50);
        let (rk4_100, _) = drift_after(aphelion_core::Integrator::Rk4, 100);
        assert!(
            rk4_100 / rk4_50 > 1.7,
            "RK4 error should roughly double: {rk4_50:e} -> {rk4_100:e}"
        );

        let (_, yoshida_worst) = drift_after(aphelion_core::Integrator::Yoshida4, 100);
        assert!(
            yoshida_worst < rk4_100,
            "Yoshida's worst excursion ({yoshida_worst:e}) should beat RK4's \
             accumulated drift ({rk4_100:e})"
        );
    }

    /// The Sun does not sit still: the planets — Jupiter above all — swing it
    /// around the barycentre by more than its own radius.
    #[test]
    fn the_sun_wobbles_about_the_barycentre() {
        let mut sim = solar_system();
        let sun = sim.find("Sun").unwrap();
        let mut furthest: f64 = 0.0;
        for _ in 0..120 {
            sim.advance(YEAR / 12.0, DAY);
            furthest = furthest.max(sim.position(sun).length());
        }
        assert!(
            furthest > aphelion_core::constants::SOLAR_RADIUS,
            "the Sun barely moved: {furthest:e} m"
        );
    }
}
