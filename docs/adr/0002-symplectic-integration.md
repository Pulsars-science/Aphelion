# 2. Symplectic integration by default

**Status:** Accepted · 2026-08-13

## Context

Orbits are integrated for millions of steps. The obvious default is classical
fourth-order Runge–Kutta: familiar, well documented, and more accurate per step
than anything of lower order.

But single-step accuracy is the wrong measure. What matters over long runs is
whether the error accumulates.

A *symplectic* integrator preserves phase-space volume and therefore conserves a
slightly perturbed Hamiltonian exactly. The consequence is that its energy error
oscillates around zero forever instead of drifting.

Measured over a century of the inner Solar System at `dt` = 1 day:

| Integrator | Worst \|ΔE/E\| | Final \|ΔE/E\| | At 50 yr |
|---|---|---|---|
| Velocity Verlet | 8.3e-5 | 2.6e-6 | 4.9e-5 |
| Yoshida 4 | 1.1e-6 | 4.0e-8 | 8.8e-7 |
| Runge–Kutta 4 | 2.6e-5 | 2.6e-5 | 1.3e-5 |

RK4's error at 100 years is exactly twice its error at 50: linear in elapsed
time. The symplectic schemes end far below their own worst excursion.

## Decision

Default to **velocity Verlet**. Offer **Yoshida 4** for long or tight
integrations, semi-implicit Euler as a baseline, and RK4 for comparison.

Verlet is implemented in kick–drift–kick form and recomputes the acceleration on
both kicks rather than caching it from the previous step. That costs one extra
force evaluation and keeps the scheme correct when velocity-dependent terms —
the relativistic correction — are enabled.

Expose the relative energy error in the UI, with a plain-language verdict.

## Consequences

**Good**

- Century-scale integrations are meaningful rather than decorative.
- Users can see the quality of what they are looking at, and watch it change as
  they move the accuracy slider.
- The comparison is a test, not a claim: `runge_kutta_energy_drifts_while_yoshida_only_oscillates`
  fails if the property stops holding.

**Bad**

- Verlet is only second order, so it needs a smaller step than RK4 for the same
  accuracy over a short span.
- Recomputing the acceleration doubles the force evaluations per Verlet step
  relative to the cached formulation.
- Symplectic schemes assume a fixed step size. Adaptive stepping breaks the
  guarantee, which is why it is not in 0.1 despite eccentric orbits wanting it.

**Follow-up**

Hierarchical or symplectic-corrector time-stepping, which would resolve tight
orbits without giving up the conservation property.
