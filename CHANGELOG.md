# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-08-13

First release. The engine works, the renderer works, and the Solar System is in
it.

### Added

**Physics (`aphelion-core`)**
- Direct `O(n²)` N-body gravity with optional Plummer softening
- Four integrators: semi-implicit Euler, velocity Verlet, Yoshida 4, Runge–Kutta 4
- Optional first post-Newtonian correction, reproducing Mercury's perihelion advance
- Keplerian orbital elements in both directions, with a Newton solver good to `e = 0.99`
- Energy, angular momentum and barycentre diagnostics; live energy-drift readout
- J2000 epochs with Julian-date and Gregorian calendar conversion
- Tunable gravity, mass, softening and display scales

**Data (`aphelion-data`)**
- The Sun, eight planets, the Moon and Pluto at J2000.0, from JPL and IAU sources
- Masses, radii, rotation periods and obliquities
- `integrator_comparison` example measuring energy conservation over a century

**Rendering (`aphelion-gfx`)**
- wgpu renderer with instanced spheres and line-list orbit tracks
- Camera-relative rendering in astronomical units, preserving `f64` precision
- Reverse-Z projection with an infinite far plane
- Orbit camera with multiplicative zoom usable across twelve orders of magnitude
- Procedural shading: limb darkening for stars, soft terminator, rim light

**Application (`aphelion-app`)**
- winit window with an egui control panel
- Accumulator-based clock, exact in the long run and capped per frame
- Body focus and follow, keyboard shortcuts, live per-body orbital readouts

[Unreleased]: https://github.com/Pulsars-science/Aphelion/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Pulsars-science/Aphelion/releases/tag/v0.1.0
