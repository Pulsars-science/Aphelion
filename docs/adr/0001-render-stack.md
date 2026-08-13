# 1. wgpu and winit rather than a game engine

**Status:** Accepted · 2026-08-13

## Context

Aphelion needs real-time 3D on desktop, eventually on the web, from Rust. The
realistic options were a full engine (Bevy), a lower-level graphics API (wgpu
with winit), or shipping the physics engine alone and deferring the question.

The distinguishing requirement is *scale*. A solar system spans about twelve
orders of magnitude, from a 1700 km moon to a 30 AU orbit. That breaks two
assumptions general-purpose engines are built on:

- world positions fit in an `f32`;
- a near and far plane can bracket everything on screen.

Both need fixing at the level of the projection matrix and the vertex data —
below where an engine normally lets you reach.

## Decision

Build directly on **wgpu** for rendering and **winit** for windowing, with
**egui** for the interface.

`aphelion-gfx` takes any `wgpu::SurfaceTarget` and never mentions winit, so the
windowing choice is confined to `aphelion-app`.

## Consequences

**Good**

- Full control of the projection: the reverse-Z infinite-far-plane matrix in
  [ADR 3](0003-camera-relative-rendering.md) is a few lines, rather than a fight
  with an engine's camera abstraction.
- Full control of vertex data, so the `f64` → `f32` conversion happens exactly
  where it should.
- Small dependency tree and fast incremental builds.
- wgpu targets Vulkan, Metal, DX12, OpenGL and WebGPU, so the eventual web build
  needs no second renderer.
- No engine-shaped constraints on how the simulation is structured — the physics
  crate stays a plain library rather than a plugin.

**Bad**

- Considerably more code before anything appears on screen: pipelines, buffers,
  depth textures, surface reconfiguration and swap-chain recovery are all ours.
- Things an engine gives away are now on the roadmap: asset loading, texture
  streaming, shadows, post-processing.
- The wgpu/winit/egui versions must be upgraded together, since egui's
  integration crates pin both. Dependabot groups them for this reason.
- Contributors familiar with Bevy will not find familiar patterns.

**Accepted risk**

If the project later grows an editor, scene graph and asset pipeline, this
decision will look expensive. The judgement is that the scale problem is
fundamental and would have to be solved anyway, whereas the engine features are
ones the project may never need.
