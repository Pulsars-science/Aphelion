//! The simulation itself: bodies, state, clock and the stepping loop.

use glam::DVec3;

use crate::body::{Body, BodyId};
use crate::integrator::{Integrator, Scratch};
use crate::kepler::OrbitalElements;
use crate::nbody::{self, State};
use crate::params::SimulationParams;
use crate::time::Epoch;

/// A gravitating system being integrated forward in time.
///
/// Bodies carry their unchanging description ([`Body`]); their positions and
/// velocities live in a packed [`State`] the integrator walks over.
///
/// # Example
///
/// ```
/// use aphelion_core::{Body, BodyKind, OrbitalElements, Simulation, constants::*};
///
/// let mut sim = Simulation::new();
/// let sun = sim.add_body(
///     Body::new("Sun", BodyKind::Star, SOLAR_MASS, SOLAR_RADIUS),
///     Default::default(),
///     Default::default(),
/// );
/// let earth = sim.add_orbiting(
///     Body::new("Earth", BodyKind::Planet, EARTH_MASS, EARTH_RADIUS),
///     sun,
///     &OrbitalElements::circular(AU),
/// );
///
/// sim.advance(days(30.0), 3600.0);
/// let elements = sim.elements_of(earth).unwrap();
/// assert!((to_au(elements.semi_major_axis) - 1.0).abs() < 1e-3);
/// ```
#[derive(Debug, Clone)]
pub struct Simulation {
    bodies: Vec<Body>,
    state: State,
    /// Effective masses, i.e. `body.mass * params.mass_scale`. Kept separate so
    /// the force loop reads one flat array.
    masses: Vec<f64>,
    scratch: Scratch,
    params: SimulationParams,
    /// Index of the most massive body, used as the centre for the relativistic
    /// correction. Recomputed whenever the body set changes.
    dominant: Option<usize>,
    /// Which time-stepping scheme to use.
    pub integrator: Integrator,
    epoch: Epoch,
    reference_energy: Option<f64>,
    steps_taken: u64,
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulation {
    /// An empty simulation at J2000.0, with default parameters and the default
    /// integrator.
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            state: State::new(),
            masses: Vec::new(),
            scratch: Scratch::new(),
            params: SimulationParams::default(),
            dominant: None,
            integrator: Integrator::default(),
            epoch: Epoch::J2000,
            reference_energy: None,
            steps_taken: 0,
        }
    }

    // ---------------------------------------------------------------- setup

    /// Adds a body at an explicit position and velocity, and returns its
    /// handle.
    pub fn add_body(&mut self, body: Body, position: DVec3, velocity: DVec3) -> BodyId {
        let mass = body.mass * self.params.mass_scale;
        self.bodies.push(body);
        self.masses.push(mass);
        let index = self.state.push(position, velocity);
        self.scratch.resize(self.bodies.len());
        self.refresh_dominant();
        self.reference_energy = None;
        BodyId(index)
    }

    /// Adds a body on a Keplerian orbit around `primary`, and returns its
    /// handle.
    ///
    /// The elements are interpreted relative to the primary's current state, so
    /// a moon added after its planet ends up in the right place automatically.
    /// The two-body gravitational parameter used is `G(M_primary + m_body)`.
    ///
    /// The new body records `primary` as its [`Body::parent`], which is what
    /// lets the UI draw its orbit in the primary's frame.
    ///
    /// # Panics
    ///
    /// Panics if `primary` is not a body of this simulation.
    pub fn add_orbiting(
        &mut self,
        body: Body,
        primary: BodyId,
        elements: &OrbitalElements,
    ) -> BodyId {
        let primary_index = primary.index();
        assert!(
            primary_index < self.bodies.len(),
            "primary {primary:?} is not part of this simulation"
        );

        let g = self.params.gravitational_constant();
        let mu = g * (self.masses[primary_index] + body.mass * self.params.mass_scale);
        let (relative_position, relative_velocity) = elements.to_state(mu);

        let position = self.state.positions[primary_index] + relative_position;
        let velocity = self.state.velocities[primary_index] + relative_velocity;

        let mut body = body;
        body.parent = Some(primary);
        self.add_body(body, position, velocity)
    }

    // ------------------------------------------------------------ accessors

    /// Every body's static description, in handle order.
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// The description of one body.
    pub fn body(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id.index())
    }

    /// Mutable access to one body's presentation: name, colour, radius, spin.
    ///
    /// Mass is deliberately not settable this way — it feeds a derived array —
    /// use [`Simulation::set_mass`] instead.
    pub fn body_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        self.bodies.get_mut(id.index())
    }

    /// Changes a body's mass, in kilograms, and refreshes everything derived
    /// from it.
    ///
    /// This is one of the headline "what if" dials: make Jupiter ten times
    /// heavier and watch the inner system destabilise.
    pub fn set_mass(&mut self, id: BodyId, mass: f64) {
        let index = id.index();
        if index >= self.bodies.len() {
            return;
        }
        self.bodies[index].mass = mass;
        self.masses[index] = mass * self.params.mass_scale;
        self.refresh_dominant();
        self.reference_energy = None;
    }

    /// Looks a body up by name, case-insensitively.
    pub fn find(&self, name: &str) -> Option<BodyId> {
        self.bodies
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
            .map(BodyId)
    }

    /// Number of bodies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether the simulation holds no bodies.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// The packed dynamical state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Position of a body, in metres.
    ///
    /// # Panics
    ///
    /// Panics if `id` is not a body of this simulation.
    pub fn position(&self, id: BodyId) -> DVec3 {
        self.state.positions[id.index()]
    }

    /// Velocity of a body, in m·s⁻¹.
    ///
    /// # Panics
    ///
    /// Panics if `id` is not a body of this simulation.
    pub fn velocity(&self, id: BodyId) -> DVec3 {
        self.state.velocities[id.index()]
    }

    /// Effective masses currently used by the force loop, in kilograms.
    ///
    /// These include [`SimulationParams::mass_scale`].
    pub fn masses(&self) -> &[f64] {
        &self.masses
    }

    /// Current simulation time.
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Overrides the simulation clock without touching the state.
    pub fn set_epoch(&mut self, epoch: Epoch) {
        self.epoch = epoch;
    }

    /// Number of integrator steps taken since construction.
    pub fn steps_taken(&self) -> u64 {
        self.steps_taken
    }

    /// The tunable parameters.
    pub fn params(&self) -> &SimulationParams {
        &self.params
    }

    /// Replaces the tunable parameters.
    ///
    /// Changing [`SimulationParams::mass_scale`] rescales the effective masses
    /// immediately, and any change to gravity or mass invalidates the energy
    /// reference — the system is now a different one, so drift is measured from
    /// here on.
    // Exact comparison is deliberate: we only want to rebuild derived state
    // when a dial actually moved, and any movement at all counts.
    #[allow(clippy::float_cmp)]
    pub fn set_params(&mut self, params: SimulationParams) {
        let physics_changed = params.gravity_scale != self.params.gravity_scale
            || params.mass_scale != self.params.mass_scale
            || params.softening != self.params.softening;
        self.params = params;
        if physics_changed {
            self.refresh_masses();
            self.reference_energy = None;
        }
    }

    /// Mutates the parameters through a closure, refreshing derived state.
    pub fn update_params(&mut self, f: impl FnOnce(&mut SimulationParams)) {
        let mut params = self.params;
        f(&mut params);
        self.set_params(params);
    }

    // ------------------------------------------------------------ stepping

    /// Advances the simulation by exactly one step of `dt` seconds.
    ///
    /// `dt` may be negative to integrate backwards; the symplectic schemes are
    /// time-reversible, so rewinding is well behaved.
    pub fn step(&mut self, dt: f64) {
        if self.bodies.is_empty() || dt == 0.0 {
            return;
        }
        self.scratch.resize(self.bodies.len());

        let g = self.params.gravitational_constant();
        let softening = self.params.softening;
        let relativity = self.params.relativistic_correction;
        let dominant = self.dominant;
        let masses = &self.masses;

        self.integrator
            .step(&mut self.state, dt, &mut self.scratch, |pos, vel, out| {
                nbody::accelerations(pos, masses, g, softening, out);
                if relativity && let Some(central) = dominant {
                    nbody::add_relativistic_correction(pos, vel, masses, g, central, out);
                }
            });

        self.epoch += dt;
        self.steps_taken += 1;
    }

    /// Advances by `duration` seconds, splitting it into steps no longer than
    /// `max_dt`.
    ///
    /// This is what a render loop calls: it decouples the frame rate from the
    /// integration step, so the physics stays accurate whether the frame took
    /// 4 ms or 40 ms. Returns the number of steps taken.
    ///
    /// # Panics
    ///
    /// Panics if `max_dt` is not strictly positive.
    pub fn advance(&mut self, duration: f64, max_dt: f64) -> u32 {
        assert!(max_dt > 0.0, "max_dt must be positive, got {max_dt}");
        if duration == 0.0 {
            return 0;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (duration.abs() / max_dt).ceil() as u32;
        let dt = duration / f64::from(steps);
        for _ in 0..steps {
            self.step(dt);
        }
        steps
    }

    /// A step size that resolves the fastest orbit in the system.
    ///
    /// Returns roughly 1/`steps_per_orbit` of the shortest orbital period,
    /// which is the practical way to pick `max_dt` for [`Simulation::advance`].
    /// Sixty steps per orbit is a reasonable floor for Velocity Verlet; a few
    /// hundred is comfortable.
    pub fn suggested_timestep(&self, steps_per_orbit: f64) -> f64 {
        let g = self.params.gravitational_constant();
        let shortest = self
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                let parent = body.parent?.index();
                let mu = g * (self.masses[parent] + self.masses[index]);
                let relative_position = self.state.positions[index] - self.state.positions[parent];
                let relative_velocity =
                    self.state.velocities[index] - self.state.velocities[parent];
                let elements =
                    OrbitalElements::from_state(relative_position, relative_velocity, mu);
                elements.is_bound().then(|| elements.period(mu))
            })
            .fold(f64::INFINITY, f64::min);

        if shortest.is_finite() {
            shortest / steps_per_orbit
        } else {
            crate::constants::DAY
        }
    }

    // ----------------------------------------------------------- diagnostics

    /// Total mechanical energy, in joules.
    pub fn total_energy(&self) -> f64 {
        nbody::total_energy(
            &self.state,
            &self.masses,
            self.params.gravitational_constant(),
            self.params.softening,
        )
    }

    /// Total angular momentum about the origin, in kg·m²·s⁻¹.
    pub fn angular_momentum(&self) -> DVec3 {
        nbody::angular_momentum(&self.state, &self.masses)
    }

    /// Relative energy error since the reference was last taken.
    ///
    /// This is the single most useful number for judging whether the step size
    /// is small enough. Below `1e-9` the integration is excellent; above `1e-3`
    /// the trajectories should not be trusted.
    ///
    /// The reference is captured on the first call, and reset whenever the
    /// bodies or the physical parameters change.
    pub fn energy_drift(&self) -> f64 {
        match self.reference_energy {
            Some(reference) if reference != 0.0 => (self.total_energy() - reference) / reference,
            _ => 0.0,
        }
    }

    /// Captures the current energy as the reference for [`Self::energy_drift`].
    pub fn reset_energy_reference(&mut self) {
        self.reference_energy = Some(self.total_energy());
    }

    /// Centre of mass of the system, in metres.
    pub fn barycentre(&self) -> DVec3 {
        nbody::barycentre(&self.state.positions, &self.masses)
    }

    /// Moves to the barycentric inertial frame: the centre of mass sits at the
    /// origin and stays there.
    ///
    /// Worth calling once after building a system. Otherwise the whole thing
    /// drifts off across the screen, because giving the Sun zero velocity while
    /// the planets all orbit it leaves the barycentre moving.
    pub fn recentre_on_barycentre(&mut self) {
        let offset = nbody::barycentre(&self.state.positions, &self.masses);
        let velocity_offset = nbody::barycentre_velocity(&self.state.velocities, &self.masses);
        self.state.shift_frame(offset, velocity_offset);
        self.reference_energy = None;
    }

    /// The osculating orbital elements of a body around its
    /// [parent](Body::parent) — the ellipse it would follow from here if every
    /// other body vanished.
    ///
    /// Returns `None` for a body with no parent.
    pub fn elements_of(&self, id: BodyId) -> Option<OrbitalElements> {
        let index = id.index();
        let parent = self.bodies.get(index)?.parent?.index();
        let g = self.params.gravitational_constant();
        let mu = g * (self.masses[parent] + self.masses[index]);
        Some(OrbitalElements::from_state(
            self.state.positions[index] - self.state.positions[parent],
            self.state.velocities[index] - self.state.velocities[parent],
            mu,
        ))
    }

    /// The two-body gravitational parameter `G(M_primary + m_body)` governing a
    /// body's orbit around its [parent](Body::parent), in m³·s⁻².
    ///
    /// Returns `None` for a body with no parent.
    pub fn mu_of(&self, id: BodyId) -> Option<f64> {
        let index = id.index();
        let parent = self.bodies.get(index)?.parent?.index();
        Some(self.params.gravitational_constant() * (self.masses[parent] + self.masses[index]))
    }

    /// Current osculating orbital period of a body around its parent, in
    /// seconds.
    ///
    /// Returns `None` for a body with no parent, and [`f64::INFINITY`] for one
    /// on an escape trajectory.
    pub fn period_of(&self, id: BodyId) -> Option<f64> {
        let mu = self.mu_of(id)?;
        Some(self.elements_of(id)?.period(mu))
    }

    /// The same, but around an explicitly chosen primary.
    ///
    /// # Panics
    ///
    /// Panics if either handle is not a body of this simulation.
    pub fn elements_relative_to(&self, id: BodyId, primary: BodyId) -> OrbitalElements {
        let (index, parent) = (id.index(), primary.index());
        let mu = self.params.gravitational_constant() * (self.masses[parent] + self.masses[index]);
        OrbitalElements::from_state(
            self.state.positions[index] - self.state.positions[parent],
            self.state.velocities[index] - self.state.velocities[parent],
            mu,
        )
    }

    // -------------------------------------------------------------- internal

    fn refresh_masses(&mut self) {
        let scale = self.params.mass_scale;
        self.masses.clear();
        self.masses
            .extend(self.bodies.iter().map(|body| body.mass * scale));
        self.refresh_dominant();
    }

    fn refresh_dominant(&mut self) {
        self.dominant = self
            .masses
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::BodyKind;
    use crate::constants::{AU, EARTH_MASS, EARTH_RADIUS, SOLAR_MASS, SOLAR_RADIUS, YEAR, days};

    fn sun_and_earth() -> (Simulation, BodyId, BodyId) {
        let mut sim = Simulation::new();
        let sun = sim.add_body(
            Body::new("Sun", BodyKind::Star, SOLAR_MASS, SOLAR_RADIUS),
            DVec3::ZERO,
            DVec3::ZERO,
        );
        let earth = sim.add_orbiting(
            Body::new("Earth", BodyKind::Planet, EARTH_MASS, EARTH_RADIUS),
            sun,
            &OrbitalElements::circular(AU),
        );
        sim.recentre_on_barycentre();
        sim.reset_energy_reference();
        (sim, sun, earth)
    }

    #[test]
    fn earth_returns_to_its_starting_point_after_one_orbit() {
        // The dominant error after one revolution is a phase error along the
        // orbit, of order (ω·dt)^order · a. With a half-day step that is a few
        // 1e-4 AU for Verlet and utterly negligible for Yoshida — which is the
        // whole argument for offering a fourth-order scheme.
        for (integrator, tolerance_au) in [
            (Integrator::VelocityVerlet, 1e-3),
            (Integrator::Yoshida4, 1e-6),
        ] {
            let (mut sim, _, earth) = sun_and_earth();
            sim.integrator = integrator;
            let period = sim.period_of(earth).unwrap();
            let start = sim.position(earth);

            sim.advance(period, days(0.5));

            let error = (sim.position(earth) - start).length() / AU;
            assert!(
                error < tolerance_au,
                "{}: drifted {error:e} AU after one orbit",
                integrator.name()
            );
        }
    }

    #[test]
    fn energy_and_angular_momentum_are_conserved() {
        let (mut sim, _, _) = sun_and_earth();
        let momentum = sim.angular_momentum();

        sim.advance(10.0 * YEAR, days(1.0));

        assert!(
            sim.energy_drift().abs() < 1e-9,
            "energy drifted by {:e}",
            sim.energy_drift()
        );
        let momentum_error = (sim.angular_momentum() - momentum).length() / momentum.length();
        assert!(momentum_error < 1e-12, "momentum error {momentum_error:e}");
    }

    #[test]
    fn the_barycentre_stays_put() {
        let (mut sim, _, _) = sun_and_earth();
        sim.advance(5.0 * YEAR, days(1.0));
        // Well under the Sun's radius, i.e. the system has not wandered off.
        assert!(sim.barycentre().length() < 1e6);
    }

    #[test]
    fn stronger_gravity_shortens_the_year() {
        let (mut sim, sun, earth) = sun_and_earth();
        let mu = |sim: &Simulation| {
            sim.params().gravitational_constant()
                * (sim.masses()[sun.index()] + sim.masses()[earth.index()])
        };
        let before = sim.elements_of(earth).unwrap().period(mu(&sim));

        sim.update_params(|p| p.gravity_scale = 4.0);
        let after = sim.elements_of(earth).unwrap().period(mu(&sim));

        // The orbit is no longer circular — the Earth is now far too slow for
        // the stronger pull — but its period must have dropped.
        assert!(after < before, "period went from {before} to {after}");
    }

    #[test]
    fn reversing_time_undoes_the_integration() {
        let (mut sim, _, earth) = sun_and_earth();
        let start = sim.position(earth);
        sim.advance(days(200.0), days(0.25));
        sim.advance(-days(200.0), days(0.25));
        assert!((sim.position(earth) - start).length() / AU < 1e-9);
    }

    #[test]
    fn suggested_timestep_resolves_the_orbit() {
        let (sim, _, _) = sun_and_earth();
        let dt = sim.suggested_timestep(360.0);
        assert!(dt > 0.0 && dt < days(2.0), "got {dt} s");
    }
}
