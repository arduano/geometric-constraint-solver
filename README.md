# GeoSolve

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- spatial rigid-body assembly and linkage kinematics.

The validated baseline includes a nonlinear equality solver with strict residual validation, dense-authoritative rank/DOF and source diagnostics, indexed sparse hard steps, structural matching, cross-component strict priorities, adaptive natural and explicit pseudo-arclength planar and spatial linkage continuation, typed spatial branch events with hysteresis and explicit mode changes, persistent gauge-separated planar and spatial linkage sessions, multi-driver spatial velocity with physical feature fields and optional motion bases, the common independently validated spatial joint/mate and position-driver catalog, complete 2D sketch geometry through gauge-separated NURBS, generic tangency, curvature, G2 and separately named parametric C2 continuity, associative signed line offsets, ordinary-constraint point-defined mirrors, strict sketch-v1-to-v3 migrations, read-only visual line-profile detection, independently validated associative line-line fillets, shared `SE(2)`/`SE(3)` manifold state, accepted hard-equality sensitivity, typed application attribute sidecars, and the completed disposable 2D Sketch Playground Alpha over reusable Rust APIs. Deliverables 1 and 2 are complete through M23; M24-M27 establish advanced embedding, linear-construction, visual-analysis and line-fillet seams, M28 generalizes fillets and explicit parent trimming, and M29 owns release hardening. Physics, collision and a production rendering system are out of scope.

## Start here

1. `START_HERE.md` — current implementation handoff and next milestone.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — active two-deliverable roadmap and ordered milestones M8-M29.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.

## Workspace

- `geosolve-geometry` — immutable geometry, curve evaluation, validated frames/workplanes and `Pose2`/`Pose3` manifold operations; no solver state.
- `geosolve-core` — variables, residual blocks, dense/sparse Jacobian paths, nonlinear solve, structural/numerical analysis, strict hierarchy, continuation primitives and revision-stamped accepted hard linearization/sensitivity.
- `geosolve-sketch` — persistent sketch entities, conics, non-rational B-splines, generic constraints and compilation into core residuals, with reusable document/session/command/history/serialization APIs.
- `geosolve-linkage` — persistent planar and spatial rigid bodies/features/sources, deterministic JSON/runtime remapping, gauge-separated mobility, common joints/mates, drivers, explicit assembly modes, independently published natural/pseudo-arclength continuation, typed hysteretic branch events/mode changes and multi-driver body/feature velocity fields with optional physical motion bases.
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
