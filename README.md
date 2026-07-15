# GeoSolve

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- spatial rigid-body assembly and linkage kinematics.

The validated baseline includes a nonlinear equality solver with strict residual validation, rank/DOF and source diagnostics, 2D sketch constraints through curves and tangency, and planar rigid-body linkage continuation. The active roadmap expands this foundation into generic production CAD curves and 2D/3D kinematics. Physics, collision and rendering are out of scope.

## Start here

1. `START_HERE.md` — current implementation handoff and next milestone.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — active two-deliverable roadmap and ordered milestones M8–M22.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.

## Workspace

- `geosolve-geometry` — immutable geometry, workplanes, `Pose2`; no solver state.
- `geosolve-core` — variables, residual blocks, Jacobians, nonlinear solve, rank analysis.
- `geosolve-sketch` — sketch entities/constraints and compilation into core residuals.
- `geosolve-linkage` — planar rigid bodies, joints, drivers, assembly modes and continuation; M16/M18/M21 spatial kinematics remains in this domain crate.
- `geosolve-demo-web` — separate WASM/SVG demonstration crate with hardcoded scenes and an equation-audit panel.

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

Browser smoke consumer:

```bash
cd crates/geosolve-demo-web
trunk serve --open
```

The web build needs the `wasm32-unknown-unknown` standard library available to the selected Rust toolchain.

## Licence

GPL-3.0-or-later. See `LICENSE`.
