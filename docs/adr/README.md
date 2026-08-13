# Architecture decision records

Short notes on decisions that are expensive to reverse, written when the reasons
are still fresh.

Each one states the context, the decision and the consequences — including the
bad ones. The point is not to justify the choice but to record what was known at
the time, so that a future contributor can tell the difference between "this is
load-bearing" and "nobody has got round to changing it".

If a change reverses or complicates one of these, say so in the pull request.

| # | Decision | Status |
|---|---|---|
| [0001](0001-render-stack.md) | wgpu and winit rather than a game engine | Accepted |
| [0002](0002-symplectic-integration.md) | Symplectic integration by default | Accepted |
| [0003](0003-camera-relative-rendering.md) | Camera-relative rendering with reverse-Z | Accepted |

New records: copy the shape of an existing one, take the next number, and add a
row above.
