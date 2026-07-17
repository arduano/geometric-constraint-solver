# GeoSolve

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- spatial rigid-body assembly and linkage kinematics.

The validated baseline includes a nonlinear equality solver with strict residual validation, dense-authoritative rank/DOF and source diagnostics, indexed sparse hard steps, structural matching, cross-component strict priorities, adaptive natural and explicit pseudo-arclength planar linkage continuation, persistent gauge-separated planar linkage sessions and velocity queries, a first independently validated spatial fixed-frame/ball/revolute assembly slice, 2D sketch constraints through curves and tangency, shared `SE(2)`/`SE(3)` manifold state, accepted hard-equality sensitivity, and the completed disposable 2D Sketch Playground Alpha over reusable Rust APIs. The roadmap now continues with generic production CAD curves, the broader spatial mate catalog and complete 2D/3D kinematics. Physics, collision and a production rendering system are out of scope.

## Start here

1. `START_HERE.md` — current implementation handoff and next milestone.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — active two-deliverable roadmap and ordered milestones M8-M24.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.

## Workspace

- `geosolve-geometry` — immutable geometry, curve evaluation, validated frames/workplanes and `Pose2`/`Pose3` manifold operations; no solver state.
- `geosolve-core` — variables, residual blocks, dense/sparse Jacobian paths, nonlinear solve, structural/numerical analysis, strict hierarchy, continuation primitives and revision-stamped accepted hard linearization/sensitivity.
- `geosolve-sketch` — baseline sketch entities/constraints and compilation into core residuals; M10-M12 provide the reusable document/session/command/history/serialization APIs.
- `geosolve-linkage` — persistent planar rigid bodies/features/sources, gauge-separated mobility, joints, drivers, velocity, assembly modes and continuation, plus M18 spatial bodies/local features, fixed-frame/ball/revolute sources and six-DOF gauge-separated sessions; the broader M20/M23 spatial catalog remains in this domain crate.
- `geosolve-demo-web` — separate disposable, desktop-first WASM/SVG diagnostic playground and audit consumer without equations or authoritative document semantics.

The critical design rule is: **share numerical machinery and feature evaluation, not one undifferentiated sketch/mechanism entity model.**

## Pre-1.0 API policy

Public APIs are usable but not source-stable before 1.0. Breaking source changes are limited to explicit milestone review work, documented with the owning milestone, and evolving public error/status enums are non-exhaustive. In M9 the caller-storage extension of `ResidualEvaluator` is public and unstable; the local AD formula trait, adapter and canonical component IR remain private.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
cargo bench --locked -p geosolve-core --no-run
```

NixOS-friendly shell:

```bash
nix-shell
```

Browser playground/consumer:

```bash
cd crates/geosolve-demo-web
trunk serve --open
```

The web build needs the `wasm32-unknown-unknown` standard library available to the selected Rust toolchain.

## Licence

GPL-3.0-or-later. See `LICENSE`.
