//! What to draw, in simulation units.
//!
//! A [`Scene`] is a plain description handed to the renderer each frame: world
//! positions in metres, radii in metres, colours in linear RGB. Nothing here
//! knows about wgpu, and nothing here is retained between frames — building one
//! from a [`Simulation`] is cheap.

use aphelion_core::{Body, BodyId, DVec3, Simulation};

/// One body, positioned and oriented for this frame.
#[derive(Debug, Clone, Copy)]
pub struct BodyInstance {
    /// Centre, in metres.
    pub position: DVec3,
    /// Radius to draw, in metres.
    ///
    /// Already includes any exaggeration factor: the renderer draws exactly
    /// what it is given.
    pub radius: f64,
    /// Colour, as linear RGB.
    pub color: [f32; 3],
    /// Whether the body emits light rather than reflecting it.
    pub emissive: bool,
    /// Rotation about the body's own axis, in radians.
    pub spin: f64,
    /// Obliquity of that axis, in radians.
    pub axial_tilt: f64,
}

/// A path drawn as connected line segments — an orbit track, a trajectory
/// trail, a reference grid.
#[derive(Debug, Clone)]
pub struct Track {
    /// Points along the path, in metres.
    pub points: Vec<DVec3>,
    /// Colour, as linear RGB with alpha.
    pub color: [f32; 4],
    /// Whether to join the last point back to the first.
    pub closed: bool,
}

/// Largest fraction of an orbit a body may be drawn to fill.
///
/// A quarter puts the Sun's disc comfortably inside Mercury's orbit — the
/// familiar textbook proportion — while still leaving it clearly the largest
/// thing on screen.
const ORBIT_CLEARANCE: f64 = 0.25;

/// How much to exaggerate body radii when drawing.
///
/// At true scale the planets are invisible: the Earth is one ten-thousandth of
/// an AU across, well under a pixel from anywhere you can see its orbit from.
/// Some exaggeration is therefore the only way to see anything — but a single
/// multiplier applied uniformly does not work either, because the bodies do not
/// start at comparable sizes. The Sun is 109 times the Earth's radius, so a
/// factor that makes the Earth visible makes the Sun 4.6 AU across, swallowing
/// the entire inner system.
///
/// Hence [`RadiusScale::clamp_to_orbits`]: no body may be drawn large enough to
/// engulf the nearest orbit around it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusScale {
    /// Multiplier applied to every body's true radius.
    pub factor: f64,
    /// Whether to stop a body growing past the orbit it would otherwise
    /// swallow.
    ///
    /// The limit is the periapsis of the body's own orbit — the path it travels
    /// and must stay inside — or, for a body with no parent, of the closest
    /// orbit around it. In the Solar System at a factor of 1000 that binds on
    /// exactly two bodies: the Sun, which is held inside Mercury's orbit, and
    /// the Moon, which is held inside its own. Every planet keeps the full
    /// factor, and therefore its true size relative to the other planets.
    pub clamp_to_orbits: bool,
}

impl Default for RadiusScale {
    fn default() -> Self {
        Self::true_scale()
    }
}

impl RadiusScale {
    /// Geometry true to life: no exaggeration, and nothing to clamp.
    pub fn true_scale() -> Self {
        Self {
            factor: 1.0,
            clamp_to_orbits: true,
        }
    }

    /// Exaggerated by `factor`, held inside the orbits.
    pub fn exaggerated(factor: f64) -> Self {
        Self {
            factor,
            clamp_to_orbits: true,
        }
    }

    /// The radius to draw a body of true radius `radius`, in metres.
    ///
    /// `orbit_limit` is the periapsis of the nearest orbit that must stay
    /// clear, or [`f64::INFINITY`] if there is none.
    ///
    /// A body is never drawn *smaller* than it really is: the clamp only ever
    /// takes back exaggeration, so true scale stays true.
    pub fn apply(self, radius: f64, orbit_limit: f64) -> f64 {
        let exaggerated = radius * self.factor;
        if !self.clamp_to_orbits {
            return exaggerated;
        }
        exaggerated.min(ORBIT_CLEARANCE * orbit_limit).max(radius)
    }

    /// Whether drawing a body of this radius would hit the clamp.
    pub fn clamps(self, radius: f64, orbit_limit: f64) -> bool {
        self.clamp_to_orbits && radius * self.factor > ORBIT_CLEARANCE * orbit_limit
    }
}

/// Everything the renderer needs for one frame.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// Bodies to draw.
    pub bodies: Vec<BodyInstance>,
    /// Paths to draw.
    pub tracks: Vec<Track>,
    /// Position of the single light source, in metres.
    pub light: DVec3,
    /// Fraction of a body's colour visible on its night side, in `0..=1`.
    ///
    /// Physically this should be nearly zero. A little ambient keeps unlit
    /// hemispheres from reading as holes in the screen.
    pub ambient: f32,
}

impl Scene {
    /// An empty scene with a light at the origin.
    pub fn new() -> Self {
        Self {
            ambient: 0.03,
            ..Self::default()
        }
    }

    /// Removes everything, keeping the allocations for the next frame.
    pub fn clear(&mut self) {
        self.bodies.clear();
        self.tracks.clear();
    }

    /// Fills the scene from a simulation.
    ///
    /// The light is placed on the most luminous body. Existing contents are
    /// cleared first.
    pub fn build_from(&mut self, sim: &Simulation, scale: RadiusScale) {
        self.clear();

        let limits = orbit_limits(sim);
        let elapsed = sim.epoch().seconds();
        for (index, body) in sim.bodies().iter().enumerate() {
            let id = BodyId(index);
            self.bodies.push(BodyInstance {
                position: sim.position(id),
                radius: scale.apply(body.radius, limits[index]),
                color: body.color,
                emissive: body.kind.is_luminous(),
                spin: spin_at(body, elapsed),
                axial_tilt: body.axial_tilt,
            });
        }

        self.light = sim
            .bodies()
            .iter()
            .enumerate()
            .find(|(_, body)| body.kind.is_luminous())
            .map_or(DVec3::ZERO, |(index, _)| sim.position(BodyId(index)));
    }

    /// Appends one orbit track per body that has a parent, sampled from the
    /// body's current osculating ellipse.
    ///
    /// The track is drawn in the parent's frame, so a moon's track follows its
    /// planet around instead of smearing across the whole system. Unbound
    /// bodies are skipped — there is no closed curve to draw.
    ///
    /// `samples` controls smoothness; 256 is plenty for a screen-filling orbit.
    pub fn add_orbit_tracks(&mut self, sim: &Simulation, samples: usize, alpha: f32) {
        for (index, body) in sim.bodies().iter().enumerate() {
            let Some(parent) = body.parent else { continue };
            let id = BodyId(index);
            let Some(elements) = sim.elements_of(id) else {
                continue;
            };
            if !elements.is_bound() {
                continue;
            }

            let origin = sim.position(parent);
            let points = elements
                .sample(samples)
                .into_iter()
                .map(|point| origin + point)
                .collect();

            let [r, g, b] = body.color;
            self.tracks.push(Track {
                points,
                color: [r, g, b, alpha],
                closed: true,
            });
        }
    }
}

/// For every body, the periapsis of the nearest orbit that must stay clear of
/// it, in metres — or [`f64::INFINITY`] where there is none.
///
/// For a body with a parent that is its own orbit: drawn any larger and it
/// swallows the path it travels on. For a body with no parent — a star — it is
/// the closest orbit around it, which is what keeps the Sun out of Mercury's
/// way.
///
/// Periapsis rather than the current separation, so an eccentric orbit does not
/// make its primary pulse once per revolution.
///
/// One pass over the bodies, so this is cheap enough to redo every frame and
/// therefore responds immediately when the user changes a mass or gravity.
pub fn orbit_limits(sim: &Simulation) -> Vec<f64> {
    let mut limits = vec![f64::INFINITY; sim.len()];

    for (index, body) in sim.bodies().iter().enumerate() {
        let Some(parent) = body.parent else { continue };
        let Some(elements) = sim.elements_of(BodyId(index)) else {
            continue;
        };
        if !elements.is_bound() {
            continue;
        }

        let periapsis = elements.periapsis();
        // The body must fit inside its own orbit...
        limits[index] = limits[index].min(periapsis);
        // ... and a body with nothing of its own to fit inside is instead held
        // clear of whatever orbits it.
        let parent_index = parent.index();
        if sim.bodies()[parent_index].parent.is_none() {
            limits[parent_index] = limits[parent_index].min(periapsis);
        }
    }

    limits
}

/// The radius a single body would be drawn at, in metres.
///
/// Same answer as [`Scene::build_from`] gives, for callers that need one body
/// without building a scene — the camera, which must not let the viewer zoom
/// inside an exaggerated planet.
pub fn display_radius(sim: &Simulation, id: BodyId, scale: RadiusScale) -> f64 {
    let Some(body) = sim.body(id) else {
        return 0.0;
    };
    let limit = orbit_limits(sim)
        .get(id.index())
        .copied()
        .unwrap_or(f64::INFINITY);
    scale.apply(body.radius, limit)
}

/// Rotation angle of a body about its own axis at time `elapsed`, in radians.
///
/// A zero or non-finite period means the body does not turn.
fn spin_at(body: &Body, elapsed: f64) -> f64 {
    if body.rotation_period == 0.0 || !body.rotation_period.is_finite() {
        return 0.0;
    }
    std::f64::consts::TAU * elapsed / body.rotation_period
}

#[cfg(test)]
mod tests {
    use super::*;
    use aphelion_core::constants::to_au;

    #[test]
    fn a_scene_built_from_a_system_has_one_instance_per_body() {
        let sim = aphelion_data::solar_system();
        let mut scene = Scene::new();
        scene.build_from(&sim, RadiusScale::true_scale());

        assert_eq!(scene.bodies.len(), sim.len());
        // The light must sit on the Sun.
        let sun = sim.find("Sun").unwrap();
        assert!((scene.light - sim.position(sun)).length() < 1.0);
        // Exactly one emissive body.
        assert_eq!(scene.bodies.iter().filter(|b| b.emissive).count(), 1);
    }

    #[test]
    fn radius_scaling_is_applied() {
        let sim = aphelion_data::solar_system();
        let mut scene = Scene::new();
        scene.build_from(&sim, RadiusScale::exaggerated(100.0));

        let earth = sim.find("Earth").unwrap();
        let expected = sim.body(earth).unwrap().radius * 100.0;
        assert!((scene.bodies[earth.index()].radius - expected).abs() < 1.0);
    }

    /// The bug this clamp exists for: at a factor of 1000 the Sun is 4.6 AU
    /// across and swallows the whole inner system, Mercury's orbit included.
    #[test]
    fn the_sun_never_swallows_mercurys_orbit() {
        let sim = aphelion_data::solar_system();
        let sun = sim.find("Sun").unwrap();
        let mercury = sim.find("Mercury").unwrap();
        let periapsis = sim.elements_of(mercury).unwrap().periapsis();

        let unclamped = RadiusScale {
            factor: 1000.0,
            clamp_to_orbits: false,
        };
        assert!(
            display_radius(&sim, sun, unclamped) > periapsis,
            "the unclamped case should reproduce the bug"
        );

        let drawn = display_radius(&sim, sun, RadiusScale::exaggerated(1000.0));
        assert!(
            drawn < periapsis,
            "drawn at {:.3} AU, Mercury reaches {:.3} AU",
            to_au(drawn),
            to_au(periapsis)
        );
    }

    /// A moon must stay inside the orbit it travels on, or it vanishes into its
    /// own track.
    #[test]
    fn a_moon_stays_inside_its_own_orbit() {
        let sim = aphelion_data::solar_system();
        let moon = sim.find("Moon").unwrap();
        let periapsis = sim.elements_of(moon).unwrap().periapsis();

        let drawn = display_radius(&sim, moon, RadiusScale::exaggerated(1000.0));
        assert!(
            drawn < periapsis,
            "drawn {drawn:e} m, orbit {periapsis:e} m"
        );
    }

    /// The clamp must bite only where it is needed. If it caught the planets
    /// too, they would lose their true sizes relative to one another — which is
    /// most of what makes the picture worth looking at.
    #[test]
    fn the_planets_keep_the_full_exaggeration() {
        let sim = aphelion_data::solar_system();
        let scale = RadiusScale::exaggerated(1000.0);

        for name in ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn"] {
            let id = sim.find(name).unwrap();
            let expected = sim.body(id).unwrap().radius * 1000.0;
            let drawn = display_radius(&sim, id, scale);
            assert!(
                (drawn / expected - 1.0).abs() < 1e-9,
                "{name} was clamped to {drawn:e} m, expected {expected:e} m"
            );
        }
    }

    /// The clamp only ever takes back exaggeration; it must never shrink a body
    /// below its real size, or true scale would stop being true.
    #[test]
    fn true_scale_is_never_clamped() {
        let sim = aphelion_data::solar_system();
        for (index, body) in sim.bodies().iter().enumerate() {
            let drawn = display_radius(&sim, BodyId(index), RadiusScale::true_scale());
            assert!(
                (drawn / body.radius - 1.0).abs() < 1e-12,
                "{} drawn at {drawn:e} m, true radius {:e} m",
                body.name,
                body.radius
            );
        }
    }

    #[test]
    fn the_clamp_can_be_turned_off() {
        let sim = aphelion_data::solar_system();
        let sun = sim.find("Sun").unwrap();
        let scale = RadiusScale {
            factor: 1000.0,
            clamp_to_orbits: false,
        };
        let expected = sim.body(sun).unwrap().radius * 1000.0;
        assert!((display_radius(&sim, sun, scale) / expected - 1.0).abs() < 1e-12);
    }

    #[test]
    fn every_orbiting_body_gets_a_track_around_its_own_parent() {
        let sim = aphelion_data::solar_system();
        let mut scene = Scene::new();
        scene.build_from(&sim, RadiusScale::true_scale());
        scene.add_orbit_tracks(&sim, 64, 0.4);

        // Everything except the Sun orbits something.
        assert_eq!(scene.tracks.len(), sim.len() - 1);
        assert!(scene.tracks.iter().all(|t| t.points.len() == 64));

        // The Moon's track must be centred on the Earth, not on the Sun: its
        // points span roughly a lunar orbit, not an astronomical unit.
        let moon = sim.find("Moon").unwrap();
        let earth_position = sim.position(sim.find("Earth").unwrap());
        let track = &scene.tracks[moon.index() - 1];
        for point in &track.points {
            let distance = (*point - earth_position).length();
            assert!(distance < 1e9, "moon track wandered {distance:e} m away");
        }
    }
}
