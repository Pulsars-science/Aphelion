# Contributing to Aphelion

Thanks for being here. Aphelion is a small project and every kind of
contribution helps — a typo fix, a better comment, a bug report with a repro, a
new integrator.

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Getting set up

```bash
git clone https://github.com/Pulsars-science/Aphelion.git
cd Aphelion
cargo test --workspace
cargo run --release
```

That is the whole setup. There is no code generation step, no submodules and no
system dependencies beyond a working GPU driver.

If `cargo run` fails to find an adapter, try forcing a backend:

```bash
WGPU_BACKEND=vulkan cargo run --release   # or metal, dx12, gl
RUST_LOG=aphelion=debug,wgpu=warn cargo run --release
```

## Branches

Two long-lived branches:

| Branch | What it is |
|---|---|
| `main` | Always releasable. Tagged releases come from here. Protected. |
| `develop` | Integration branch. Everything lands here first. |

Everything else is short-lived and branches from `develop`:

```
feat/<short-name>     a new capability          feat/barnes-hut
fix/<short-name>      a bug fix                 fix/moon-track-frame
docs/<short-name>     documentation only        docs/physics-notes
perf/<short-name>     performance work          perf/force-loop-simd
refactor/<short-name> no behaviour change       refactor/split-renderer
```

A hotfix against a release branches from `main` as `hotfix/<name>` and merges
back into both `main` and `develop`.

```
main     ─────●──────────────────────●────────▶  (tags: v0.1.0, v0.2.0)
               \                    /
develop  ───●───●────●────●────●───●──────────▶
                 \        \    /
feat/…            ●────────●──
```

## Making a change

1. Branch from `develop`.
2. Make the change. Keep it focused — one concern per pull request.
3. Make sure the checks below pass.
4. Open a pull request against `develop`, describing *why* as well as *what*.

### Before you push

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs exactly these on Linux, macOS and Windows, plus a docs build. The
workspace is currently warning-free; please keep it that way.

### The minimum supported Rust version

The MSRV lives in one place: `rust-version` in the workspace `Cargo.toml`. CI
reads it from there, so raising it is a one-line change. Do not repeat it in the
workflow.

### Commit messages

[Conventional Commits](https://www.conventionalcommits.org/):

```
feat(core): add Barnes-Hut tree for large systems
fix(gfx): correct depth compare for reverse-Z
docs(readme): explain the energy drift readout
perf(core): halve force-loop allocations
test(data): check Kepler's third law against published periods
```

Scopes are the crate names without the prefix: `core`, `data`, `gfx`, `app`, or
`ci`, `docs`, `deps`.

## What review looks for

**Correctness first.** This is a physics project. A number that is subtly wrong
is worse than a feature that is missing, because it is much harder to notice.
Anything touching the physics should come with a test that would fail if the
maths were wrong — not just one that exercises the code path.

**Explain the non-obvious.** Comments should say *why*, not *what*. The
codebase's own examples: why the projection is reverse-Z, why the Yoshida
coefficients are what they are, why the calendar conversion avoids Julian dates.
If a reader would reasonably ask "why is it done this way?", answer it in the
code.

**Cite your sources.** New constants, orbital elements or physical parameters
need a reference in a comment — JPL, IAU, CODATA, or a paper.

**Keep the layers apart.** `aphelion-core` must not learn about graphics;
`aphelion-gfx` must not learn about windowing. That separation is what lets the
physics be tested headlessly and the renderer be reused.

**Public items are documented.** `missing_docs` is a warning in every library
crate, and CI treats warnings as errors.

## Tests

Unit tests live next to the code, in a `mod tests`. Prefer tests that assert
something physically meaningful:

```rust
// Good: fails if the dynamics or the data are wrong.
assert!((sim.period_of(earth).unwrap() / YEAR - 1.0).abs() < 5e-3);

// Weak: passes even if the integrator is nonsense.
assert_eq!(sim.len(), 11);
```

Long integrations are welcome as tests — the century-long stability check runs
in well under a second in release mode — but keep the debug-mode runtime
reasonable.

## Reporting bugs

Open an issue with:

- what you did, what happened, what you expected;
- OS, GPU and `rustc --version`;
- for rendering problems, the first few lines of
  `RUST_LOG=aphelion=debug,wgpu=warn cargo run --release`, which name the
  adapter and backend;
- for physics problems, the parameters you had set and the reported energy
  drift.

## Proposing features

Open an issue first for anything substantial, so the design can be discussed
before the code exists. The [roadmap](README.md#roadmap) lists what is already
planned; picking something from it is the easiest way in.

Architectural decisions are recorded in [`docs/adr/`](docs/adr/). If your change
reverses or complicates one of them, say so in the pull request.

## Licence

By contributing you agree that your work is dual-licensed under MIT and
Apache-2.0, matching the project.
