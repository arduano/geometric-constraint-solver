# GeoSolve

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- a deliberate future path to spatial mechanisms.

This repository currently contains a buildable workspace scaffold and a concrete implementation plan for delegation to an OpenCode agent. The browser crate already renders primitive hardcoded SVG scenes alongside human-readable residual-equation templates; it does **not** yet solve them or show evaluated residual values.

## Start here

1. `START_HERE.md` — concise OpenCode handoff and first assignment.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — ordered milestones and their required outputs.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.

## Workspace

- `geosolve-geometry` — immutable geometry, workplanes, `Pose2`; no solver state.
- `geosolve-core` — variables, residual blocks, Jacobians, nonlinear solve, rank analysis.
- `geosolve-sketch` — sketch entities/constraints and compilation into core residuals.
- `geosolve-linkage` — rigid bodies, joints, drivers, assembly modes and continuation.
- `geosolve-demo-web` — separate WASM/SVG demonstration crate with hardcoded scenes and an equation-audit panel.

The critical design rule is: **share numerical machinery and feature evaluation, not one undifferentiated sketch/mechanism entity model.**

## Development

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

NixOS-friendly shell:

```bash
nix-shell
```

Browser scaffold:

```bash
cd crates/geosolve-demo-web
trunk serve --open
```

The web build needs the `wasm32-unknown-unknown` standard library available to the selected Rust toolchain.

## Licence

GPL-3.0-or-later. See `LICENSE`.
