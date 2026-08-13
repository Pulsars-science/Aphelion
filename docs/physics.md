# The physics behind Aphelion

Notes on what the simulation does, why it does it that way, and how far the
results can be trusted. Aimed at someone who wants to modify the engine or
judge its output — not a textbook.

## Frame, units and time

Everything is SI in a non-rotating inertial frame: metres, kilograms, seconds,
radians. State is `f64` throughout. At Neptune's distance (4.5 × 10¹² m) that
still resolves sub-millimetre detail, which is orders of magnitude below any
modelling error that matters here.

The reference plane is the **J2000 ecliptic**: `+x` towards the vernal equinox,
`+z` towards the ecliptic north pole. Time is counted in seconds from **J2000.0**
(2000-01-01T12:00:00 TT, JD 2451545.0), which is the epoch essentially every
modern ephemeris is published against.

Aphelion does not distinguish TT, TAI and UTC, and does not model leap seconds.
Over the spans it is used for, that difference is under a minute — invisible
next to the error in the orbits themselves.

## Forces

The acceleration of body *i* is the direct pairwise sum

$$\mathbf{a}_i = G \sum_{j \neq i} m_j \frac{\mathbf{r}_j - \mathbf{r}_i}{\left(\lVert \mathbf{r}_j - \mathbf{r}_i \rVert^2 + \varepsilon^2\right)^{3/2}}$$

evaluated in `O(n²)` using Newton's third law, so only `n(n−1)/2` pair terms are
computed. For the few hundred bodies a planetary system needs, this is the right
answer: exact, branch-free and cache-friendly. A Barnes–Hut tree only starts
paying off in the tens of thousands.

### Softening

`ε` is a **Plummer softening length**. It replaces the true separation with
`√(r² + ε²)`, which bounds the force as `r → 0`.

With `ε = 0` the law is exactly Newtonian — correct, and also fragile: a close
encounter produces an enormous acceleration that a fixed step size cannot
resolve, and the body gets slingshotted out of the system by an artefact of the
integrator rather than by physics. A few thousand kilometres of softening is a
cheap guard. It is off by default, because the honest answer should be the
default.

### Relativity

Optionally, the first post-Newtonian correction from the dominant mass is added:

$$\mathbf{a}_{1\text{PN}} = \frac{GM}{c^2 r^3}\left[\left(\frac{4GM}{r} - v^2\right)\mathbf{r} + 4(\mathbf{r}\cdot\mathbf{v})\,\mathbf{v}\right]$$

This is the Schwarzschild correction for a test particle. Applied to Mercury it
reproduces the observed **43 arcseconds per century** of perihelion advance —
the discrepancy that Newtonian gravity could not explain and that made general
relativity's reputation. For the outer planets it is negligible.

Only the single most massive body contributes. Full N-body 1PN (the
Einstein–Infeld–Hoffmann equations) is a much larger undertaking for no visible
gain at this scale.

## Integration

### Why symplectic

An orbit is integrated for millions of steps, so what matters is not the error
of one step but whether that error *accumulates*.

A **symplectic** integrator preserves phase-space volume. The consequence is
that it conserves a slightly perturbed Hamiltonian *exactly*, so the true energy
error oscillates around zero forever instead of drifting. A non-symplectic
scheme of the same nominal order will be more accurate over one orbit and far
worse over ten thousand.

Measured over a century of the inner Solar System:

| Integrator | Order | Evals/step | Worst \|ΔE/E\| | Final \|ΔE/E\| | At 50 yr |
|---|---|---|---|---|---|
| Semi-implicit Euler | 1 | 1 | 1.4e-3 | 5.0e-4 | 1.3e-3 |
| Velocity Verlet | 2 | 2 | 8.3e-5 | 2.6e-6 | 4.9e-5 |
| Yoshida 4 | 4 | 6 | 1.1e-6 | 4.0e-8 | 8.8e-7 |
| Runge–Kutta 4 | 4 | 4 | 2.6e-5 | 2.6e-5 | 1.3e-5 |

(`dt` = 1 day; reproduce with
`cargo run --release -p aphelion-data --example integrator_comparison`.)

Read the last two columns together. For the symplectic schemes the final error
is far below the worst excursion — they wander and come back. For RK4 the two
are identical, and the error at 100 years is exactly twice the error at 50: a
one-way drift, linear in elapsed time.

### The schemes

**Semi-implicit Euler** — update velocity first, then position with the *new*
velocity. That ordering is the whole trick; it costs nothing and makes plain
Euler symplectic. Included as a baseline, not for use.

**Velocity Verlet** (default) — kick–drift–kick. Second order, symplectic,
time-reversible. The workhorse of celestial mechanics. Aphelion recomputes the
acceleration on both kicks rather than caching it, which costs one extra
evaluation but keeps the scheme correct when velocity-dependent terms (the
relativistic correction) are switched on.

**Yoshida 4** — three Verlet steps composed with coefficients

$$w_1 = \frac{1}{2 - 2^{1/3}}, \qquad w_0 = \frac{-2^{1/3}}{2 - 2^{1/3}}$$

so that `w₁ + w₀ + w₁ = 1` and the third-order error cancels between the forward
and backward sub-steps. Six evaluations per step, error falling as `dt⁴` — at
equal accuracy, usually cheaper than Verlet.

**Runge–Kutta 4** — classical, not symplectic. Accurate over short spans and the
cleanest choice for strongly velocity-dependent forces, but its energy error
grows without bound. Offered for comparison and for anyone who needs it.

### Step size

`Simulation::suggested_timestep(steps_per_orbit)` returns a fraction of the
*shortest* orbital period in the system, since that is what sets the stability
limit. With the Moon present, that is about 27 days, so 360 steps per orbit puts
`dt` near 1.8 hours.

Sixty steps per orbit is watchable; a few hundred is accurate; cost is linear.

## Diagnostics

Two conserved quantities are exposed, and they fail differently — which is what
makes having both useful.

**Total energy** is *not* conserved exactly by any of the schemes, so
`Simulation::energy_drift()` is the practical quality readout:

| \|ΔE/E\| | Verdict |
|---|---|
| < 1e-9 | excellent |
| < 1e-6 | good |
| < 1e-3 | coarse — trajectories are qualitatively right |
| > 1e-3 | do not trust the result |

**Angular momentum** *is* conserved to machine precision by every scheme here,
because the pairwise forces are central. That makes it an independent check: if
angular momentum moves, the bug is in the force loop, not the step size.

## Initial conditions

Bodies start from JPL's mean Keplerian elements for J2000.0 (Standish,
*Keplerian Elements for Approximate Positions of the Major Planets*), converted
to state vectors by solving Kepler's equation.

These are **mean** elements — the average ellipse, not the instantaneous one —
so each planet begins within a fraction of a degree of where it truly was at
J2000, not within an arcsecond. That is a deliberate trade: mean elements are a
small public-domain table, whereas true accuracy means shipping a DE440 kernel.

The system is then shifted into the **barycentric frame**. Without that step,
giving the Sun zero velocity while every planet orbits it leaves the centre of
mass moving, and the whole system slides off the screen.

### What emerges

Nothing below is scripted. All of it falls out of the integration, which is the
best available evidence that the engine is doing its job:

- Kepler's third law, reproducing published orbital periods to better than 0.5%;
- the Sun's wobble about the barycentre — more than a solar radius, driven
  chiefly by Jupiter;
- secular perturbation between the planets;
- Mercury's relativistic perihelion advance, when that correction is enabled.

## Known limitations

- **Point masses only.** No `J2` oblateness, no tidal forces, no radiation
  pressure, no Yarkovsky effect.
- **No collisions.** Bodies pass through one another. Softening is the only
  mitigation offered.
- **Fixed step size.** A highly eccentric orbit is resolved by the same `dt` at
  aphelion as at perihelion, which is wasteful at one end and marginal at the
  other. Adaptive and hierarchical stepping are on the roadmap.
- **Not an ephemeris.** For observational work, use JPL Horizons or a SPICE
  kernel.

## References

- E. M. Standish, *Keplerian Elements for Approximate Positions of the Major
  Planets*, JPL Solar System Dynamics.
- H. Yoshida, *Construction of higher order symplectic integrators*, Physics
  Letters A 150 (1990) 262–268.
- J. Wisdom & M. Holman, *Symplectic maps for the n-body problem*, Astronomical
  Journal 102 (1991) 1528.
- Archinal et al., *Report of the IAU Working Group on Cartographic Coordinates
  and Rotational Elements: 2015*.
- CODATA 2018 recommended values of the fundamental physical constants.
