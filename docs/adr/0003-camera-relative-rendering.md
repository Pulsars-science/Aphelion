# 3. Camera-relative rendering with a reverse-Z projection

**Status:** Accepted · 2026-08-13

## Context

Two independent precision problems appear as soon as a solar system is drawn to
scale.

**Position precision.** Neptune is 4.5 × 10¹² m from the Sun. An `f32` carries
about seven significant digits, so a position in metres near Neptune is
quantised in steps of roughly 10⁵ m — larger than the planet's own radius of
2.5 × 10⁷ m by enough to make its surface visibly faceted, and enough to make it
jitter as it moves.

**Depth precision.** Showing a moon 1000 km away and a planet 30 AU away in the
same frame is a near:far ratio of about 10¹². A conventional projection with a
finite far plane distributes depth precision hyperbolically, concentrating it
near the near plane; at that ratio, everything beyond a fraction of an AU
collapses into a handful of distinguishable depth values, and z-fighting is
total.

## Decision

**Position:** keep every position in `f64` on the CPU, and convert to `f32` only
at the moment of building GPU data — *relative to the camera*, and in units of
astronomical units rather than metres.

Camera-relative conversion means the numbers reaching the GPU are small wherever
the viewer is: a subtraction of two large `f64` values, done at full precision,
leaves a small residual. Using AU as the unit buys a further eleven orders of
magnitude of headroom. The view matrix is consequently pure rotation, with the
translation already applied on the CPU.

**Depth:** use a reverse-Z projection with an infinite far plane —

```
| f/aspect  0    0     0    |
|    0      f    0     0    |
|    0      0    0    near  |
|    0      0   -1     0    |
```

— mapping the near plane to depth 1 and infinity to depth 0, paired with a
`Depth32Float` buffer, `CompareFunction::Greater` and a depth clear of `0.0`.

Floating point packs its precision near zero; reverse-Z puts the far distances
there. The hyperbolic depth distribution and the floating-point exponent
distribution cancel, giving near-uniform *relative* precision at every scale.
The infinite far plane means there is nothing to clip against, so the near plane
can be made tiny without consequence.

## Consequences

**Good**

- A moon's surface and Neptune's orbit are renderable in the same frame.
- The camera can approach a surface arbitrarily closely without the near plane
  needing adjustment.
- No far-plane clipping, and no scale-dependent tuning anywhere.
- Both properties are tested: `the_projection_is_reverse_z_with_an_infinite_far_plane`
  and `render_space_keeps_precision_at_neptune`.

**Bad**

- Every position handed to the GPU must be rebuilt when the camera moves, since
  it is expressed relative to the camera. That is one pass over the instance and
  track buffers per frame — negligible at these body counts, but it rules out
  simply leaving static geometry on the GPU.
- Anyone writing a new pipeline must remember the depth comparison is `Greater`,
  not `Less`, and that the clear value is `0.0`. Getting it wrong renders
  nothing at all, which is at least a loud failure.
- The convention is unusual enough to need the explanation that is in
  `camera.rs`.

**Rejected alternative**

Logarithmic depth, written from the fragment shader. It solves the same problem
but forces every shader to write `gl_FragDepth`, which disables early-Z on most
hardware. Reverse-Z costs nothing at runtime.
