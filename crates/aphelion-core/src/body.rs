//! Description of the bodies that populate a simulation.

/// Handle to a body inside a [`Simulation`](crate::Simulation).
///
/// It is an index, so it is only meaningful for the simulation that produced
/// it, and only stays valid as long as no body is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(pub usize);

impl BodyId {
    /// The underlying index.
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}

/// Broad classification of a body, used for filtering and presentation.
///
/// It carries no dynamical meaning: a moon and a planet obey exactly the same
/// equations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BodyKind {
    /// Self-luminous body; acts as a light source when rendering.
    Star,
    /// Planet orbiting a star.
    #[default]
    Planet,
    /// Dwarf planet (Pluto, Ceres, Eris…).
    DwarfPlanet,
    /// Natural satellite of a planet or dwarf planet.
    Moon,
    /// Minor body of the asteroid population.
    Asteroid,
    /// Icy minor body on a typically eccentric orbit.
    Comet,
    /// Artificial object.
    Spacecraft,
}

impl BodyKind {
    /// Whether bodies of this kind emit their own light.
    #[inline]
    pub fn is_luminous(self) -> bool {
        matches!(self, BodyKind::Star)
    }

    /// Human-readable name of the category.
    pub fn label(self) -> &'static str {
        match self {
            BodyKind::Star => "star",
            BodyKind::Planet => "planet",
            BodyKind::DwarfPlanet => "dwarf planet",
            BodyKind::Moon => "moon",
            BodyKind::Asteroid => "asteroid",
            BodyKind::Comet => "comet",
            BodyKind::Spacecraft => "spacecraft",
        }
    }
}

/// Everything about a body that does *not* change as the simulation advances.
///
/// Position and velocity live separately, in [`State`](crate::State), so that
/// the integrator can work on tightly packed arrays.
#[derive(Debug, Clone)]
pub struct Body {
    /// Display name, e.g. `"Jupiter"`.
    pub name: String,
    /// Category of the body.
    pub kind: BodyKind,
    /// Rest mass, in kilograms.
    pub mass: f64,
    /// Mean (volumetric) radius, in metres.
    pub radius: f64,
    /// Sidereal rotation period, in seconds. Negative means retrograde.
    ///
    /// Zero disables rotation.
    pub rotation_period: f64,
    /// Obliquity: angle between the rotation axis and the orbital plane
    /// normal, in radians.
    pub axial_tilt: f64,
    /// Approximate visual colour, as linear (not sRGB-encoded) RGB in `0..=1`.
    pub color: [f32; 3],
    /// The body this one is usually described as orbiting.
    ///
    /// Purely informational — gravity is always computed between every pair —
    /// but it lets the UI group moons under their planet and draw orbit tracks
    /// in the right frame.
    pub parent: Option<BodyId>,
}

impl Body {
    /// Creates a body with default rotation, no tilt and a neutral grey colour.
    pub fn new(name: impl Into<String>, kind: BodyKind, mass: f64, radius: f64) -> Self {
        Self {
            name: name.into(),
            kind,
            mass,
            radius,
            rotation_period: 0.0,
            axial_tilt: 0.0,
            color: [0.7, 0.7, 0.7],
            parent: None,
        }
    }

    /// Sets the sidereal rotation period, in seconds (negative = retrograde).
    #[must_use]
    pub fn with_rotation(mut self, period_seconds: f64) -> Self {
        self.rotation_period = period_seconds;
        self
    }

    /// Sets the obliquity, in radians.
    #[must_use]
    pub fn with_axial_tilt(mut self, radians: f64) -> Self {
        self.axial_tilt = radians;
        self
    }

    /// Sets the display colour, as linear RGB in `0..=1`.
    #[must_use]
    pub fn with_color(mut self, color: [f32; 3]) -> Self {
        self.color = color;
        self
    }

    /// Standard gravitational parameter `GM` of this body, in m³·s⁻².
    ///
    /// `g` is the gravitational constant actually in use, which may differ from
    /// [`constants::G`](crate::constants::G) when the user has scaled gravity.
    #[inline]
    pub fn mu(&self, g: f64) -> f64 {
        g * self.mass
    }

    /// Mean density, in kg·m⁻³. Returns 0 for a point mass.
    pub fn density(&self) -> f64 {
        if self.radius <= 0.0 {
            return 0.0;
        }
        let volume = 4.0 / 3.0 * std::f64::consts::PI * self.radius.powi(3);
        self.mass / volume
    }

    /// Surface gravity, in m·s⁻². Returns 0 for a point mass.
    pub fn surface_gravity(&self, g: f64) -> f64 {
        if self.radius <= 0.0 {
            return 0.0;
        }
        g * self.mass / (self.radius * self.radius)
    }

    /// Escape velocity at the surface, in m·s⁻¹. Returns 0 for a point mass.
    pub fn escape_velocity(&self, g: f64) -> f64 {
        if self.radius <= 0.0 {
            return 0.0;
        }
        (2.0 * g * self.mass / self.radius).sqrt()
    }
}
