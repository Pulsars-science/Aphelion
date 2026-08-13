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
    /// `radius_scale` exaggerates every body's radius. At `1.0` the geometry is
    /// true to scale, which means the planets are sub-pixel from anywhere you
    /// can see the whole system from — accurate, and not much to look at. The
    /// light is placed on the most luminous body.
    ///
    /// Existing contents are cleared first.
    pub fn build_from(&mut self, sim: &Simulation, radius_scale: f64) {
        self.clear();

        let elapsed = sim.epoch().seconds();
        for (index, body) in sim.bodies().iter().enumerate() {
            let id = BodyId(index);
            self.bodies.push(BodyInstance {
                position: sim.position(id),
                radius: body.radius * radius_scale,
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

    #[test]
    fn a_scene_built_from_a_system_has_one_instance_per_body() {
        let sim = aphelion_data::solar_system();
        let mut scene = Scene::new();
        scene.build_from(&sim, 1.0);

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
        scene.build_from(&sim, 100.0);

        let earth = sim.find("Earth").unwrap();
        let expected = sim.body(earth).unwrap().radius * 100.0;
        assert!((scene.bodies[earth.index()].radius - expected).abs() < 1.0);
    }

    #[test]
    fn every_orbiting_body_gets_a_track_around_its_own_parent() {
        let sim = aphelion_data::solar_system();
        let mut scene = Scene::new();
        scene.build_from(&sim, 1.0);
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
