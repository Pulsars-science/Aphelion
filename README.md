<h1 align="center">Aphelion</h1>

<p align="center">
  <strong>A physically real solar system you can reach into and change.</strong>
</p>

<p align="center">
  <a href="#license"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.90+" src="https://img.shields.io/badge/rust-1.90%2B-orange.svg"></a>
  <a href="https://github.com/Pulsars-science/Aphelion/actions"><img alt="CI" src="https://github.com/Pulsars-science/Aphelion/actions/workflows/ci.yml/badge.svg"></a>
  <a href="#status"><img alt="Status: alpha" src="https://img.shields.io/badge/status-alpha-yellow.svg"></a>
</p>

---

Aphelion renders the Solar System in 3D and integrates it with real Newtonian
gravity — every body pulling on every other body, every frame. Nothing is on
rails. The planets are where they are because the equations put them there.

Then it hands you the constants. Double `G`. Give Jupiter ten times its mass.
Turn on the relativistic correction and watch Mercury's perihelion creep round.
The simulation does not object; it just tells you, honestly, how well it is
still conserving energy.

> **Why "aphelion"?** The point in an orbit furthest from the Sun — where a
> body is slowest, and where you can see the whole of its path at once.

## Status

**v0.1 — alpha.** The physics engine is complete and tested; the renderer and
UI are functional; the body catalogue covers the Sun, eight planets, the Moon
and Pluto. Expect the API to move before 1.0. See the [roadmap](#roadmap).

## Quick start

```bash
git clone https://github.com/Pulsars-science/Aphelion.git
cd Aphelion
cargo run --release
```

`--release` matters: a debug build integrates roughly an order of magnitude
slower.

Requires a Rust toolchain (1.90 or newer) and a GPU with Vulkan, Metal, DX12 or
OpenGL — which in practice means anything from the last decade, integrated
graphics included.

### Controls

| Input | Action |
|---|---|
| **Drag** | Orbit the camera |
| **Scroll** | Zoom (multiplicative — usable from a moon's surface to beyond Neptune) |
| **Space** | Pause / resume |
| **`[`** / **`]`** | Halve / double the time scale |
| **`1`–`9`, `0`** | Focus a body |
| **`o`** | Toggle orbit tracks |
| **`f`** | Toggle camera follow |
| **`i`** | Cycle integrator |
| **`r`** | Reset the system |
| **Esc** | Quit |

Everything else lives in the side panel.

## What is actually simulated

This is the part worth being precise about, because "realistic" is cheap to
claim.

**Gravity is the real thing.** Every pair of bodies attracts by Newton's law,
recomputed every step — no two-body approximations, no scripted orbits. That is
why the Sun visibly wobbles around the barycentre, why Jupiter perturbs its
neighbours, and why the system is capable of genuinely falling apart if you make
it.

**Integration is symplectic.** The default is velocity Verlet; Yoshida's
fourth-order composition is available for long runs. Symplectic schemes keep the
energy error *bounded and oscillating* instead of drifting, which is what makes
century-long integrations meaningful. The UI reports the relative energy error
live, so you always know how much to trust what you are looking at.

Run the comparison yourself:

```bash
cargo run --release -p aphelion-data --example integrator_comparison
```

```text
integrator                   dt  evals        worst        final     at 50 yr
----------------------------------------------------------------------------
Velocity Verlet             1 d      2     8.265e-5     2.584e-6     4.874e-5
Yoshida 4                   1 d      6     1.135e-6     3.964e-8     8.769e-7
Runge-Kutta 4               1 d      4     2.618e-5     2.618e-5     1.309e-5
```

Note the last row. Runge–Kutta's error at 100 years is exactly twice its error
at 50 — a one-way drift. The symplectic schemes end up far closer to zero than
their own worst excursion, because they keep coming back.

**Initial conditions are real.** Bodies start from the JPL J2000.0 mean
Keplerian elements, with masses, radii, rotation periods and obliquities from
JPL and the IAU. Venus turns backwards. Uranus lies on its side. The Moon is
tidally locked.

**Optional general relativity.** The first post-Newtonian correction from the
dominant mass can be switched on. It is what accounts for the 43 arcseconds per
century of Mercury's perihelion advance that Newton alone cannot explain.

### What is not simulated

Being clear about the edges:

- **Not an ephemeris.** Mean elements put each planet within a fraction of a
  degree of its true J2000 position, not within an arcsecond. For real
  observational work you want JPL DE440 — [on the roadmap](#roadmap).
- **No non-spherical gravity.** Bodies are point masses; `J2` oblateness, tidal
  forces and radiation pressure are not modelled.
- **No collisions.** Bodies pass through each other. Plummer softening is
  offered to keep close encounters from ejecting anything.
- **Textures are procedural.** Planets are shaded solid colours with a rim light
  and faint banding, not photographic surfaces.

## The dials

| Dial | What it does |
|---|---|
| **Gravity ×G** | Scales the gravitational constant. Anything already in a circular orbit is instantly at the wrong speed — that is the interesting part. |
| **All masses** | Scales every mass at once. |
| **Individual mass** | Per body, from 1/100 to 100× the real value. |
| **Softening** | Plummer length; caps the force during close encounters so a near miss does not eject a planet. |
| **Relativity** | The 1PN correction described above. |
| **Integrator** | Euler, Verlet, Yoshida 4 or RK4 — watch the energy readout change with it. |
| **Steps / orbit** | Accuracy against cost, linearly. |
| **Body size ×** | Display only. At true scale the Earth is a fifth of a pixel from anywhere you can see its orbit. |

## Architecture

```
aphelion-core ──▶ aphelion-data ──▶ aphelion-gfx ──▶ aphelion-app
   physics          the system         renderer          window
```

| Crate | Responsibility | Depends on |
|---|---|---|
| [`aphelion-core`](crates/aphelion-core) | N-body forces, integrators, Kepler elements, epochs. No graphics. | `glam` |
| [`aphelion-data`](crates/aphelion-data) | The Solar System at J2000.0, with sources cited. | core |
| [`aphelion-gfx`](crates/aphelion-gfx) | wgpu renderer, astronomical-scale camera. No windowing. | core, `wgpu` |
| [`aphelion-app`](crates/aphelion-app) | winit window, egui panel, input. | all |

`aphelion-core` is usable on its own, headlessly, as a plain N-body library:

```rust
use aphelion_core::{Body, BodyKind, OrbitalElements, Simulation, constants::*};

let mut sim = Simulation::new();
let sun = sim.add_body(
    Body::new("Sun", BodyKind::Star, SOLAR_MASS, SOLAR_RADIUS),
    Default::default(),
    Default::default(),
);
let earth = sim.add_orbiting(
    Body::new("Earth", BodyKind::Planet, EARTH_MASS, EARTH_RADIUS),
    sun,
    &OrbitalElements::circular(AU),
);

sim.advance(years(10.0), days(1.0));

println!("{:.6} AU", to_au(sim.elements_of(earth).unwrap().semi_major_axis));
println!("energy drift: {:e}", sim.energy_drift());
```

### Two problems worth reading about

Both are documented at length in the code, because both are the kind of thing
that silently ruins an astronomy renderer:

- **Precision.** Neptune is 4.5 × 10¹² m out, and an `f32` has seven significant
  digits — a naive renderer quantises positions into steps larger than the
  planet. Aphelion keeps `f64` throughout and converts *camera-relative*, in
  astronomical units, at the last possible moment. See
  [`aphelion-gfx/src/camera.rs`](crates/aphelion-gfx/src/camera.rs).
- **Depth.** Showing a moon 1000 km away and a planet 30 AU away in one frame is
  a near:far ratio of 10¹². A reverse-Z projection with an infinite far plane
  gives near-uniform relative depth precision at every scale, and nothing to
  clip against. Same file.

## Development

```bash
cargo test --workspace        # ~40 tests, including century-long integrations
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo doc --open -p aphelion-core
```

The test suite checks physics, not just plumbing: Kepler's third law against
published orbital periods, energy and angular momentum conservation, the
symplectic-versus-RK4 drift comparison, time reversibility, and that the inner
system survives a century.

## Roadmap

**Physics**
- [ ] JPL DE440 / SPICE kernel import for true ephemeris accuracy
- [ ] Adaptive and hierarchical time-stepping for tight orbits
- [ ] Barnes–Hut tree for `n` in the thousands
- [ ] Collisions, mergers and the Roche limit
- [ ] `J2` oblateness and tidal evolution
- [ ] Spacecraft: manoeuvre nodes, transfer planning

**Rendering**
- [ ] Planet textures and normal maps
- [ ] Starfield from a real catalogue (Hipparcos)
- [ ] Atmospheric scattering, ring systems
- [ ] MSAA and bloom
- [ ] WebAssembly build

**Application**
- [ ] Save and load scenarios (RON/JSON)
- [ ] Scenario library: hot Jupiters, binary stars, the three-body problem
- [ ] Plot panel: energy, elements and separations over time
- [ ] Trajectory export to CSV
- [ ] Headless CLI for batch runs

Pick anything and open an issue — see below.

## Contributing

Contributions are welcome, from typo fixes upward. Start with
[CONTRIBUTING.md](CONTRIBUTING.md): it covers the branch model, the commit
convention, and what the review looks for.

Issues labelled [`good first issue`](https://github.com/Pulsars-science/Aphelion/labels/good%20first%20issue)
are scoped to be self-contained.

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option — the standard arrangement across the Rust ecosystem.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual-licensed as above, without any additional terms or conditions.

### Data attribution

Orbital elements and physical parameters are derived from public-domain sources:
[JPL Solar System Dynamics](https://ssd.jpl.nasa.gov/) and the reports of the
IAU Working Group on Cartographic Coordinates and Rotational Elements. Precise
citations are in
[`crates/aphelion-data/src/solar_system.rs`](crates/aphelion-data/src/solar_system.rs).

---

<p align="center">
  Built by <a href="https://github.com/Pulsars-science">Pulsars Science</a>.
  <br>
  <sub><a href="README.fr.md">Version française</a></sub>
</p>
