//! Application state: the simulation, the camera, and the dials on both.
//!
//! Deliberately free of winit, wgpu and egui, so the whole behaviour of the app
//! can be driven and tested without opening a window.

use aphelion_core::constants::{DAY, YEAR};
use aphelion_core::{BodyId, DVec3, Integrator, Simulation};
use aphelion_gfx::{OrbitCamera, RadiusScale, Scene, display_radius};

/// Everything the user can change that is not a law of physics.
///
/// The booleans really are independent toggles rather than a state machine in
/// disguise — every combination of them is meaningful — so clippy's advice to
/// collapse them into an enum would lose information rather than add any.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct Controls {
    /// Whether time is stopped.
    pub paused: bool,

    /// Simulated seconds per real second.
    ///
    /// One day per second is a good default: fast enough to watch the inner
    /// planets move, slow enough to follow the Moon.
    pub time_scale: f64,

    /// Integrator steps per orbit of the fastest body.
    ///
    /// The accuracy dial. Sixty is watchable, a few hundred is accurate, and
    /// the cost is linear.
    pub steps_per_orbit: f64,

    /// Upper bound on integrator steps in a single frame.
    ///
    /// Without it, winding the time scale up far enough turns one frame into an
    /// unbounded amount of work and the window stops responding. When the cap
    /// bites, time simply runs slower than asked — see
    /// [`Update::throttled`].
    pub max_steps_per_frame: u32,

    /// Display-only exaggeration of every body's radius.
    pub radius_scale: f64,

    /// Whether an exaggerated body is held clear of the nearest orbit.
    ///
    /// Off, a factor of 1000 makes the Sun 4.6 AU across and the inner system
    /// disappears inside it. See [`RadiusScale::clamp_to_orbits`].
    pub clamp_body_size: bool,

    /// Whether the control panel is open.
    pub panel_open: bool,

    /// Whether to draw orbit tracks.
    pub show_orbits: bool,

    /// Body the camera looks at, if any.
    pub focus: Option<BodyId>,

    /// Whether the camera follows its focus as the body moves.
    pub follow: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            paused: false,
            time_scale: DAY,
            steps_per_orbit: 360.0,
            max_steps_per_frame: 20_000,
            // True scale is astronomically correct and visually useless: the
            // Earth would be a fifth of a pixel from anywhere you can see its
            // orbit. A thousandfold makes the planets legible while leaving the
            // orbits where they belong.
            radius_scale: 1000.0,
            clamp_body_size: true,
            panel_open: true,
            show_orbits: true,
            focus: None,
            follow: true,
        }
    }
}

/// What a call to [`AppState::update`] actually did.
#[derive(Debug, Clone, Copy, Default)]
pub struct Update {
    /// Integrator steps taken this frame.
    pub steps: u32,
    /// Simulated seconds advanced this frame.
    pub simulated: f64,
    /// Whether [`Controls::max_steps_per_frame`] capped the work, meaning time
    /// ran slower than the requested scale.
    pub throttled: bool,
}

/// The running application, minus everything platform-specific.
pub struct AppState {
    /// The system being integrated.
    pub sim: Simulation,
    /// The camera controller.
    pub camera: OrbitCamera,
    /// User settings.
    pub controls: Controls,
    /// Scene rebuilt each frame for the renderer.
    pub scene: Scene,
    /// Each body's mass as the system was loaded, in kilograms.
    ///
    /// Kept so the UI can offer a mass slider whose range is anchored to the
    /// real value. Anchoring it to the *current* mass instead would make the
    /// range slide under the cursor as you drag it.
    pub reference_masses: Vec<f64>,
    /// Outcome of the most recent update, for the UI to report.
    pub last_update: Update,
    /// Simulated time owed but not yet integrated, in seconds.
    ///
    /// The integrator only moves in whole steps, and a frame almost never asks
    /// for a whole number of them. Rounding up each frame would make the clock
    /// run fast — badly so at low time scales, where one step can be longer
    /// than the whole frame's worth of simulated time. Carrying the remainder
    /// forward keeps the long-run rate exactly as requested.
    time_debt: f64,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Starts on the Solar System at J2000.0, framed on the Sun.
    pub fn new() -> Self {
        let sim = aphelion_data::solar_system();
        let mut state = Self {
            camera: OrbitCamera {
                distance: 4.0 * aphelion_core::constants::AU,
                pitch: 0.5,
                max_distance: 200.0 * aphelion_core::constants::AU,
                ..OrbitCamera::default()
            },
            controls: Controls::default(),
            scene: Scene::new(),
            last_update: Update::default(),
            reference_masses: sim.bodies().iter().map(|body| body.mass).collect(),
            time_debt: 0.0,
            sim,
        };
        state.focus_on(state.sim.find("Sun"));
        state.rebuild_scene();
        state
    }

    /// Restores the initial Solar System, keeping the camera where it is.
    pub fn reset(&mut self) {
        let camera = self.camera;
        let controls = self.controls;
        *self = Self::new();
        self.camera = camera;
        self.controls = controls;
        self.rebuild_scene();
    }

    /// Points the camera at a body, framing it sensibly.
    ///
    /// Passing `None` frames the whole system.
    pub fn focus_on(&mut self, body: Option<BodyId>) {
        self.controls.focus = body;
        let Some(id) = body else {
            self.camera.focus = DVec3::ZERO;
            self.camera.min_distance = 1e6;
            return;
        };

        // Frame against the radius actually drawn, not the true one, or the
        // camera's floor would let the viewer inside an exaggerated planet.
        let radius = display_radius(&self.sim, id, self.radius_scale()).max(1.0);
        let centre = self.sim.position(id);
        // Keep the current distance if the user has already zoomed to something
        // they like; only reframe when it makes no sense for the new target.
        let previous = self.camera.distance;
        self.camera.frame_body(centre, radius);
        if previous > self.camera.min_distance * 4.0 {
            self.camera.distance = previous.min(self.camera.max_distance);
        }
    }

    /// How body radii are currently drawn.
    pub fn radius_scale(&self) -> RadiusScale {
        RadiusScale {
            factor: self.controls.radius_scale,
            clamp_to_orbits: self.controls.clamp_body_size,
        }
    }

    /// World position the camera is currently looking at.
    pub fn focus_position(&self) -> DVec3 {
        self.controls
            .focus
            .map_or(DVec3::ZERO, |id| self.sim.position(id))
    }

    /// Advances the simulation by `real_dt` seconds of wall-clock time, then
    /// rebuilds the scene.
    ///
    /// Returns what was actually done, which is not always what was asked: see
    /// [`Controls::max_steps_per_frame`].
    pub fn update(&mut self, real_dt: f64) -> Update {
        let mut update = Update::default();

        if self.controls.paused {
            // Time owed while paused is time nobody asked for.
            self.time_debt = 0.0;
        } else if real_dt > 0.0 {
            self.time_debt += real_dt * self.controls.time_scale;

            let step = self
                .sim
                .suggested_timestep(self.controls.steps_per_orbit)
                .max(f64::MIN_POSITIVE);
            let direction = if self.time_debt < 0.0 { -1.0 } else { 1.0 };

            // Whole steps only; the rest waits for the next frame.
            let affordable = (self.time_debt.abs() / step).floor();
            let capped = affordable.min(f64::from(self.controls.max_steps_per_frame));
            update.throttled = capped < affordable;

            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = capped as u32;
            for _ in 0..steps {
                self.sim.step(direction * step);
            }
            update.steps = steps;
            update.simulated = direction * f64::from(steps) * step;
            self.time_debt -= update.simulated;

            if update.throttled {
                // Do not let unpayable debt pile up: the frame simply produced
                // less simulated time than asked for, and says so.
                self.time_debt = 0.0;
            }
        }

        if self.controls.follow {
            self.camera.focus = self.focus_position();
        }
        self.rebuild_scene();

        self.last_update = update;
        update
    }

    /// Repopulates the render scene from the current simulation state.
    pub fn rebuild_scene(&mut self) {
        let scale = self.radius_scale();
        self.scene.build_from(&self.sim, scale);
        if self.controls.show_orbits {
            self.scene.add_orbit_tracks(&self.sim, 256, 0.35);
        }
    }

    /// Simulated time requested but not yet integrated, in seconds.
    pub fn pending_time(&self) -> f64 {
        self.time_debt
    }

    /// Multiplies the time scale, clamped to a usable range.
    ///
    /// One second per second at the bottom, a century per second at the top.
    pub fn scale_time(&mut self, factor: f64) {
        self.controls.time_scale = (self.controls.time_scale * factor).clamp(1.0, 100.0 * YEAR);
    }

    /// A body's mass as the system was loaded, in kilograms.
    pub fn reference_mass(&self, id: BodyId) -> f64 {
        self.reference_masses
            .get(id.index())
            .copied()
            .unwrap_or_else(|| self.sim.body(id).map_or(1.0, |body| body.mass))
    }

    /// Cycles to the next integrator.
    pub fn cycle_integrator(&mut self) {
        let current = Integrator::ALL
            .iter()
            .position(|i| *i == self.sim.integrator)
            .unwrap_or(0);
        self.sim.integrator = Integrator::ALL[(current + 1) % Integrator::ALL.len()];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_state_has_a_populated_scene() {
        let state = AppState::new();
        assert_eq!(state.scene.bodies.len(), state.sim.len());
        assert!(!state.scene.tracks.is_empty());
        assert_eq!(state.controls.focus, state.sim.find("Sun"));
    }

    #[test]
    fn pausing_freezes_the_clock() {
        let mut state = AppState::new();
        state.controls.paused = true;
        let before = state.sim.epoch();
        let update = state.update(1.0);
        assert_eq!(update.steps, 0);
        assert_eq!(state.sim.epoch(), before);
    }

    /// The clock must run at exactly the requested rate over the long run, even
    /// though the integrator can only move in whole steps.
    #[test]
    fn the_clock_runs_at_the_requested_rate() {
        for time_scale in [1.0, 60.0, DAY, 30.0 * DAY] {
            let mut state = AppState::new();
            state.controls.time_scale = time_scale;
            let before = state.sim.epoch();

            // 600 frames at 60 fps: ten seconds of wall clock.
            for _ in 0..600 {
                state.update(1.0 / 60.0);
            }

            let expected = 10.0 * time_scale;
            let elapsed = state.sim.epoch() - before;
            // Whatever has not been integrated yet is still owed, not lost.
            let accounted = elapsed + state.pending_time();
            assert!(
                (accounted / expected - 1.0).abs() < 1e-9,
                "at {time_scale} s/s: accounted for {accounted} s, expected {expected} s"
            );
            // And the un-integrated remainder is never more than one step.
            let step = state.sim.suggested_timestep(state.controls.steps_per_orbit);
            assert!(
                state.pending_time().abs() < step,
                "owed {} s, one step is {step} s",
                state.pending_time()
            );
        }
    }

    /// The guard that keeps the window responsive when the time scale is wound
    /// past what the integrator can keep up with.
    #[test]
    fn an_extreme_time_scale_is_throttled_rather_than_hanging_the_frame() {
        let mut state = AppState::new();
        state.controls.time_scale = 1e6 * YEAR;
        state.controls.max_steps_per_frame = 500;

        let update = state.update(1.0 / 60.0);

        assert!(update.throttled, "should have hit the cap");
        assert!(update.steps <= 500, "took {} steps", update.steps);
    }

    #[test]
    fn following_a_planet_keeps_it_centred() {
        let mut state = AppState::new();
        let mars = state.sim.find("Mars").unwrap();
        state.focus_on(Some(mars));
        state.controls.follow = true;
        state.controls.time_scale = 30.0 * DAY;

        for _ in 0..30 {
            state.update(1.0);
        }

        let offset = (state.camera.focus - state.sim.position(mars)).length();
        assert!(offset < 1.0, "camera focus lagged by {offset:e} m");
    }

    #[test]
    fn cycling_the_integrator_visits_every_scheme_and_returns() {
        let mut state = AppState::new();
        let start = state.sim.integrator;
        for _ in 0..Integrator::ALL.len() {
            state.cycle_integrator();
        }
        assert_eq!(state.sim.integrator, start);
    }

    #[test]
    fn the_time_scale_stays_within_its_limits() {
        let mut state = AppState::new();
        for _ in 0..100 {
            state.scale_time(0.5);
        }
        assert!(state.controls.time_scale >= 1.0);
        for _ in 0..200 {
            state.scale_time(2.0);
        }
        assert!(state.controls.time_scale <= 100.0 * YEAR);
    }
}
