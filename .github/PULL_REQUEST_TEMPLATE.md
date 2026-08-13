## What this changes

<!-- What does it do, and why? Link the issue if there is one: Closes #123 -->

## Type

- [ ] Bug fix
- [ ] New feature
- [ ] Performance
- [ ] Refactor (no behaviour change)
- [ ] Documentation
- [ ] Tooling / CI

## Checks

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] New public items are documented

## If this touches the physics

- [ ] There is a test that would fail if the maths were wrong
- [ ] New constants or orbital elements cite a source (JPL, IAU, CODATA, a paper)
- [ ] Energy drift over a long run is unchanged or better

<!-- If you measured it, paste the numbers: -->

## If this touches the rendering

- [ ] Checked at both extremes of scale — close to a moon, and out past Neptune
- [ ] Tested with the body-size slider at true scale (×1)

## Notes for the reviewer

<!-- Anything you are unsure about, or deliberately left out. -->
