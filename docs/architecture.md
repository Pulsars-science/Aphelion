# Architecture

How Aphelion is put together, and why the seams are where they are.

## The layers

```
┌─────────────────────────────────────────────────────────┐
│ aphelion-app        window, input, egui panel, main loop │
│                     winit · egui · pollster              │
├─────────────────────────────────────────────────────────┤
│ aphelion-gfx        wgpu renderer, astronomical camera   │
│                     wgpu · bytemuck                      │
├─────────────────────────────────────────────────────────┤
│ aphelion-data       the Solar System at J2000.0          │
├─────────────────────────────────────────────────────────┤
│ aphelion-core       gravity, integrators, Kepler, time   │
│                     glam                                 │
└─────────────────────────────────────────────────────────┘
```

Each layer depends only on those below it. Two rules keep it that way, and both
are worth defending in review:

**`aphelion-core` knows nothing about graphics.** It is a plain N-body library
with one dependency (`glam`, for vector maths). That is what lets the physics be
tested headlessly — the century-long stability check runs in a fraction of a
second with no GPU in sight — and what would let someone use the engine for
something else entirely.

**`aphelion-gfx` knows nothing about windowing.** `Renderer::new` takes any
`wgpu::SurfaceTarget`, not a `winit::Window`. Swapping winit for something else,
or targeting a web canvas, touches only `aphelion-app`.

## Data flow, one frame

```
                       ┌──────────────┐
   wall-clock dt ─────▶│  AppState    │
                       │  ::update    │
                       └──────┬───────┘
                              │  time_debt += dt × time_scale
                              │  whole steps only, remainder carried
                              ▼
                       ┌──────────────┐
                       │ Simulation   │  positions & velocities, f64, SI
                       │ ::step ×N    │
                       └──────┬───────┘
                              │
                              ▼
                       ┌──────────────┐
                       │ Scene        │  world metres, still f64
                       │ ::build_from │  + orbit tracks from osculating elements
                       └──────┬───────┘
                              │
                              ▼
                       ┌──────────────┐
                       │ Renderer     │  f64 → f32, camera-relative, in AU
                       │ ::draw_scene │  one instanced draw + one line draw
                       └──────────────┘
```

`Scene` is the interface between simulation and rendering. It is rebuilt from
scratch every frame and holds nothing but plain data — no GPU handles, no
lifetimes, no retained state. Cheap enough not to matter, and it keeps the
renderer from reaching into the simulation.

## Key decisions

### Struct of arrays for the state

`Body` holds what does not change (name, mass, radius, colour, spin). `State`
holds packed `Vec<DVec3>` of positions and velocities. The force loop walks
those arrays linearly instead of striding over a `Vec<Body>`.

Effective masses live in a third array, `body.mass × params.mass_scale`,
recomputed when either changes, so the inner loop reads one flat `&[f64]`.

### Frames are explicit

`Renderer` does not own the main loop. `begin_frame` → `draw_scene` →
`finish_frame` lets `aphelion-app` slot the egui pass in between, drawing into
the same encoder with `LoadOp::Load`. Without that seam, the UI would need to
live inside the renderer, and the renderer would need to know about egui.

### The time accumulator

The integrator only moves in whole steps, and a frame almost never asks for a
whole number of them. Rounding up each frame makes the clock run fast — by 6%
in the default configuration, and catastrophically at low time scales, where one
step can be longer than an entire frame's worth of simulated time.

`AppState` therefore carries a debt in simulated seconds, spends it in whole
steps, and keeps the remainder. The long-run rate is then exactly the requested
one. `AppState::pending_time` exposes what is still owed, and the UI shows it
next to the step size, so stepped-looking motion at low time scales reads as an
explanation rather than a stutter.

There is also a cap: `Controls::max_steps_per_frame`. Without it, winding the
time scale far enough makes one frame unbounded work and the window stops
responding. When it bites, the frame reports `throttled` and the UI says so —
time runs slower than asked, visibly, rather than the app appearing to hang.

### Camera-relative rendering

The single most important decision in `aphelion-gfx`, covered in detail in
[`camera.rs`](../crates/aphelion-gfx/src/camera.rs) and summarised in
[physics.md](physics.md): positions stay `f64` until the last moment, then
convert relative to the camera and in units of AU. Combined with a reverse-Z
infinite projection, that is what makes a moon's surface and Neptune's orbit
renderable in the same frame.

### Orbit tracks from osculating elements

A track is not a recorded trail. Each frame, `Scene::add_orbit_tracks` reads the
body's *current* osculating elements — the ellipse it would follow from here if
every other body vanished — and samples that ellipse.

The consequence is that a track responds instantly to a change: raise gravity
and every ellipse tightens on the same frame. A recorded trail would take an
orbit to catch up. Sampling is even in eccentric anomaly rather than in time,
which concentrates points near periapsis where the curvature is highest.

Tracks are drawn in the parent's frame, so a moon's track follows its planet
instead of smearing across the system.

## Testing strategy

Tests assert physics, not plumbing.

| Layer | What is checked |
|---|---|
| `core` | Kepler equation inversion at `e` up to 0.99; element ↔ state round trip; every integrator against the analytic harmonic oscillator; symplectic energy bounds over 2000 periods; time reversibility; calendar round trip |
| `data` | Published orbital periods to 0.5%; the Moon's month; the Sun's barycentric wobble; a century of inner-system stability; Yoshida versus RK4 drift |
| `gfx` | Reverse-Z monotonicity and endpoints; render-space precision at Neptune; sphere topology; scene construction and track framing |
| `app` | Clock rate exactness with the accumulator; throttling under extreme time scales; camera follow |

The rule of thumb from [CONTRIBUTING.md](../CONTRIBUTING.md): a physics test
should fail if the *maths* is wrong, not merely if the code path is broken.

## Where things live

```
crates/
  aphelion-core/src/
    constants.rs   G, c, AU, GM☉, epochs; unit conversions
    body.rs        Body, BodyKind, BodyId
    nbody.rs       force loop, softening, 1PN, energy, momentum
    integrator.rs  Euler, Verlet, Yoshida 4, RK4, scratch buffers
    kepler.rs      orbital elements ↔ state vectors, Kepler solver
    time.rs        Epoch, Julian dates, calendar, duration formatting
    params.rs      the tunable dials
    sim.rs         Simulation: bodies, stepping, diagnostics
  aphelion-data/src/
    solar_system.rs the catalogue, with sources
  aphelion-gfx/src/
    camera.rs      Camera, OrbitCamera, render-space conversion
    mesh.rs        UV sphere, vertex layout
    scene.rs       Scene, BodyInstance, Track
    renderer.rs    wgpu device, pipelines, frames
    shaders/       body.wgsl, track.wgsl
  aphelion-app/src/
    state.rs       AppState, Controls, the time accumulator
    ui.rs          the egui panel
    main.rs        winit ApplicationHandler, input, the egui bridge
```

Architectural decisions with lasting consequences are recorded in
[`docs/adr/`](adr/).
