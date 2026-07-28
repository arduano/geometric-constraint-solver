# GeoSolve

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- spatial rigid-body assembly and linkage kinematics.

The validated baseline includes a nonlinear equality solver with strict residual validation, dense-authoritative rank/DOF and source diagnostics, indexed sparse hard steps, structural matching, cross-component strict priorities, adaptive natural and explicit pseudo-arclength planar and spatial linkage continuation, typed spatial branch events with hysteresis and explicit mode changes, persistent gauge-separated planar and spatial linkage sessions, multi-driver spatial velocity with physical feature fields and optional motion bases, the common independently validated spatial joint/mate and position-driver catalog, complete 2D sketch geometry through gauge-separated NURBS, generic tangency, curvature, G2 and separately named parametric C2 continuity, associative signed line offsets, ordinary-constraint point-defined mirrors, strict sketch-v1-to-v4 migrations, certified read-only all-family visual profile analysis, common-jet generic fillets, differentiable associated output arcs, persistent visible parent trim intervals, shared `SE(2)`/`SE(3)` manifold state, accepted hard-equality sensitivity, typed application attribute sidecars, separate retained-design/attempt/accepted sketch views, cooperative cancellation and deterministic operation controls, and the completed disposable 2D Sketch Playground Alpha over reusable Rust APIs. The kinematic product and the sketch curve/differential surface are complete for the `0.2.0` supported preview. M33-M44 establish the production-embedding contracts, ordinary CAD semantics, headless editor and host-state workbench. M45 records the cleanup investigation without human approval; M46-M53 replace and purge old browser E2E/legacy UI, consolidate one workbench and complete approved post-cleanup human UAT. M54-M64 now form the executable diagnostics, early alpha action parity, concurrency, scale, operations/topology and release sequence. Physics, collision and a production rendering system remain out of scope.

## Start here

1. `START_HERE.md` — current implementation handoff and next milestone.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — active roadmap: completed M8-M53 and executable M54-M64, with M54 active.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.

## Workspace

- `geosolve-geometry` — immutable geometry, curve evaluation, validated frames/workplanes and `Pose2`/`Pose3` manifold operations; no solver state.
- `geosolve-core` — variables, residual blocks, dense/sparse Jacobian paths, nonlinear solve, structural/numerical analysis, strict hierarchy, continuation primitives and revision-stamped accepted hard linearization/sensitivity.
- `geosolve-sketch` — persistent sketch entities, conics, non-rational B-splines, generic constraints and compilation into core residuals, with reusable accepted-only and retained-design lifecycle/serialization APIs.
- `geosolve-constraint-editor` — presentation-independent accepted scene, persistent picking, selection, gestures, drafting and typed editor effects over public sketch APIs; no renderer, DOM, storage or equations.
- `geosolve-linkage` — persistent planar and spatial rigid bodies/features/sources, deterministic JSON/runtime remapping, gauge-separated mobility, common joints/mates, drivers, explicit assembly modes, independently published natural/pseudo-arclength continuation, typed hysteretic branch events/mode changes and multi-driver body/feature velocity fields with optional physical motion bases.
- `geosolve-demo-web` — separate desktop WASM/SVG diagnostic consumer without equations or authoritative document semantics; M50 removed its old playground and M51 consolidated the one directly tested workbench.

The critical design rule is: **share numerical machinery and feature evaluation, not one undifferentiated sketch/mechanism entity model.**

The post-M32 sketch north star is a Rust/WASM embeddable planar engine with retained
unsolved design intent, ordinary CAD constraints/dimensions, immutable host inputs,
cancellation, stable diagnostics and separate sketch-operation/production-topology
companions. The host continues to own expressions, B-rep projection, feature history
and application undo. Human UAT occurs at completed M40.7 and M53, plus planned M61/M63;
all objective behavior is directly automated first. The desktop demo has no future mobile
support requirement.

## Pre-1.0 API policy

Version `0.2.0` is the current supported preview; `0.1.0` was the first. Persistent domain workflows and
schema support follow `docs/API_COMPATIBILITY.md`; low-level compiler/runtime
inspection remains explicitly unstable before 1.0. Planned removals use the
documented deprecation window and all notable changes are recorded in
`CHANGELOG.md`.

## Getting started

Persistent sketch construct/solve/edit/restore:

```bash
cargo run --locked -p geosolve-sketch --example persistent_sketch
```

Planar and spatial kinematics:

```bash
cargo run --locked -p geosolve-linkage --example planar_linkage
cargo run --locked -p geosolve-linkage --example spatial_assembly
```

Each session constructor publishes only finite, independently validated accepted
geometry. Rejected edits/imports retain the previous accepted revision. Persist
document IDs and canonical JSON, never runtime or core IDs.

The tested numerical and performance envelope is in
`docs/M32_SCALE_PERFORMANCE.md`. Sketch reads schema v1-v4 and writes v4; planar
and spatial linkage currently read/write v1.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
cargo bench --locked -p geosolve-core --no-run
```

`scripts/release-gate.sh` preserves the native, package, WASM and release Trunk gates but no
longer contains a browser E2E invocation. Cleanup qualification is direct Rust/WASM testing;
current commands are listed in `START_HERE.md`, while `docs/M52_IMPLEMENTATION.md` records the
post-cleanup candidate-specific evidence.

NixOS-friendly shell:

```bash
nix-shell
```

Desktop workbench for manual inspection only:

```bash
cd crates/geosolve-demo-web
trunk serve --open
```

The web build needs the `wasm32-unknown-unknown` standard library available to the selected Rust toolchain. Old Chromium/CDP E2E has been removed after direct-test replacement; it is historical evidence, not a current qualification command.

## Licence

GPL-3.0-or-later. See `LICENSE` and `THIRD_PARTY_LICENSES.md`.
