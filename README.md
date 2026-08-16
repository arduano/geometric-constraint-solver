# GeoSolve

**[Open the public GeoSolve Sketch workbench](https://arduano.github.io/geometric-constraint-solver/)**

A GPL-3.0-or-later, pure-Rust geometric constraint solver for:

- CAD-style 2D sketches;
- planar rigid-body mechanisms embedded in arbitrary 3D workplanes;
- spatial rigid-body assembly and linkage kinematics.

The validated baseline includes a nonlinear equality solver with strict residual validation,
dense-authoritative rank/DOF and source diagnostics, indexed sparse hard steps, structural
matching, cross-component strict priorities, adaptive natural and explicit pseudo-arclength planar
and spatial linkage continuation, typed spatial branch events with hysteresis and explicit mode
changes, persistent gauge-separated planar and spatial linkage sessions, multi-driver spatial
velocity with physical feature fields and optional motion bases, the common independently validated
spatial joint/mate and position-driver catalog, complete 2D sketch geometry through gauge-separated
NURBS, generic tangency, curvature, G2 and separately named parametric C2 continuity, associative
signed line offsets, ordinary-constraint point-defined mirrors, strict sketch-v1-to-v4 migrations,
certified read-only all-family visual profile analysis, common-jet generic fillets, differentiable
associated output arcs, multi-interval visible parent topology, shared `SE(2)`/`SE(3)` manifold
state, accepted hard-equality sensitivity, typed application attribute sidecars, separate retained-
design/attempt/accepted sketch views, cooperative cancellation and deterministic operation
controls, stable persistent-ID sketch diagnostics, the complete preserved alpha relation/dimension/
branch-action surface, exact-input prepared jobs with compare-and-swap publication, dependency-local
retained sketch solving with revision-local profile caches and bounded rank evidence, separate
equation-free deterministic sketch-operations and production-topology companions, and the completed
disposable 2D Sketch Playground Alpha over reusable Rust APIs. The kinematic product and the sketch
curve/differential surface are complete for the `0.2.0` supported preview. M33-M44 establish the
production-embedding contracts, ordinary CAD semantics, headless editor and host-state workbench.
M45 records the cleanup investigation without human approval; M46-M53 replace and purge old
browser E2E/legacy UI, consolidate one workbench and complete approved post-cleanup human UAT;
M54 completes stable diagnostics, M55 completes early alpha action parity, M56 completes prepared
concurrency, M57 completes incremental production-scale solving, M58 completes the operations
companion, M59 completes production topology and M60 completes the directly qualified advanced
workbench plus versioned desktop workspace. M61 adds movable representative mechanisms, full
Bezier/conic/NURBS workbench authoring and canvas camera inspection without restoring the deleted
demo, and is complete with supervising-human approval. M62 completes approved CAD-style constraint
and dimension authoring, M63 completes approved geometry-anchored canvas constraint/dimension
presentation, M64 completes the approved editable sample-library cleanup and M65 completes approved
predictable bounded dragging. M66 completes the explicitly approved computed-2D-Fillet feature and
authoring cut, with radius-drag/branch-choice interaction retained as known limitation
`M66-KL001`. Its earlier unapproved Fillet/Offset/Mirror candidate is archived at
`origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`); the completed M25 offset constraints
and M58 Mirror companion remain available. M67 completed the approved legacy UI,
qualification-harness and dead-code cleanup. M68 completed and received supervising-human approval
for the ADR 0032 headless Fillet direct-manipulation cut: branch-preserving radius rails, typed
headless contact metadata/internal continuation, explicit retention/local-alternative actions,
Current-only coordinator transactions and pointer capture. M69 completed and received
supervising-human approval for ADR 0033's Profile/Construction semantics, including computed
Fillet-hidden construction provenance and headless role-aware picking. M70 completed and received
supervising-human approval for ADR 0034's headless auto-constraint drafting milestone, including
the `M70-F001` Circle-through-point repair, replacement qualification/publication and scoped UAT.
M70B is complete under the supervising human's requested scoped sign-off. It packages freshly
encoded authoritative
application-workspace v5 bytes as strict, size-limited `GEOSOLVE_REPRO_V1` text for copy/paste
failure handoff, then validates and reconstructs a complete coordinator before replacing live
state. F001-F005 retain owning-layer regressions; M70B closed on its 198/198 `PASS` golden, while
M71 extends the current canonical inventory to 234/234 `PASS`. Clean
closing source `48e3cc3` passes the complete release gate with the final multi-feature transaction
and finite-arc transport regressions; its release bytes matched the historical byte-verified
M70B-F005 candidate. M71 is complete under amended ADR 0035 for six ordinary retained drafting
definitions:
point-pair Horizontal/Vertical, native line/polyline midpoint-axis Horizontal/Vertical,
Concentric and Collinear. M71-F003 through M71-F006 are resolved; clean product source
`f8a45ae7b355ab9874bf268c9950e369814e8432` passes the complete release gate and its immutable
F005/F006 replacement is byte-verified at the published endpoint. The supervising human accepted
the scoped U1-U5 review and explicitly closed M71 on 2026-08-14.
M72's public-workbench bulk fixes and GitHub Pages release are complete and explicitly approved.
The final accepted workbench is live at
`https://arduano.github.io/geometric-constraint-solver/`; its deployed seven-file artifact and
two-size Chromium contract are exactly verified. M73's behavior-preserving retained-authoring
semantic consolidation is complete, approved and publicly verified. M74's intrinsic reference
geometry and production-style desktop polish are complete under the supervising caller's scoped
close decision and exact final GitHub Pages verification. Separate hands-on UAT remains deferred,
not passed, and transfers with any findings to active M75. M75's shared headless Select resolver,
hover/click ownership parity and stale-hover invalidation passed an initial clean immutable
nomination. M75-F001 then exposed discarded active-authoring moves; the domain-aware authoring
hover/click correction is implemented and focused-qualified. Replacement immutable nomination,
combined human UAT and M75 Pages publication remain pending.
Physics, collision and a production rendering system remain out of scope.

## Start here

1. `START_HERE.md` — current implementation handoff and next milestone.
2. `ARCHITECTURE.md` — crate boundaries, mathematical model, and API direction.
3. `PLAN.md` — authoritative roadmap, with M74 closed/public and M75 active on replacement UAT.
4. `ACCEPTANCE.md` — objective completion gates.
5. `REFERENCES.md` — libraries and reference implementations.
6. `docs/SCENARIOS.md` — canonical end-to-end scenarios.
7. `docs/M72_GOALS.md` — completed public-workbench bulk-fix and Pages-release scope.
8. `docs/M72_IMPLEMENTATION.md` and `docs/M72_UAT.md` — closing evidence and approved review.
9. `docs/M73_GOALS.md` — completed retained-authoring consolidation and public-release scope.
10. `docs/M74_GOALS.md`, `docs/M74_IMPLEMENTATION.md` and `docs/M74_UAT.md` — completed scoped
    closure, final public-release evidence and explicitly deferred hands-on scorecard.
11. `docs/M75_GOALS.md`, `docs/M75_IMPLEMENTATION.md` and `docs/M75_UAT.md` — active hover/click
    ownership milestone, M75-F001 replacement and pending combined human scorecard.

## Workspace

- `geosolve-geometry` — immutable geometry, curve evaluation, validated frames/workplanes and `Pose2`/`Pose3` manifold operations; no solver state.
- `geosolve-core` — variables, residual blocks, dense/sparse Jacobian paths, nonlinear solve, structural/numerical analysis, strict hierarchy, continuation primitives and revision-stamped accepted hard linearization/sensitivity.
- `geosolve-sketch` — persistent sketch entities, conics, non-rational B-splines, generic constraints and compilation into core residuals, with reusable accepted-only and retained-design lifecycle/serialization APIs, host-scheduled prepared jobs, exact-input compare-and-swap publication and dependency-local retained runtime updates.
- `geosolve-sketch-ops` — deterministic equation-free split/trim/extend/construction proposals over complete stamped sketch snapshots; no residuals, solver state or private publication path.
- `geosolve-sketch-topology` — read-only revision-stamped production wires, regions, holes and exact source provenance with explicit bounded completeness; no solver or B-rep state.
- `geosolve-sketch-features` — separately versioned persistent computed-feature intent and independently validated revision-local output over exact accepted sketch snapshots; no residuals, solver variables, canonical sketch schema or B-rep state.
- `geosolve-constraint-editor` — presentation-independent accepted scene, persistent picking, selection, gestures, constraint/dimension and computed-feature authoring, and typed editor effects over public sketch/feature APIs; no renderer, DOM, storage or equations.
- `geosolve-linkage` — persistent planar and spatial rigid bodies/features/sources, deterministic JSON/runtime remapping, gauge-separated mobility, common joints/mates, drivers, explicit assembly modes, independently published natural/pseudo-arclength continuation, typed hysteretic branch events/mode changes and multi-driver body/feature velocity fields with optional physical motion bases.
- `geosolve-demo-web` — separate desktop WASM/SVG consumer without equations or authoritative
  document semantics; M50 removed its old playground, M51 consolidated the one directly tested
  workbench, M60 added public advanced-operation/topology presentation plus the versioned workspace
  envelope, and M67 removed its raw developer evidence/topology cards while preserving the domain
  APIs beneath them.

The critical design rule is: **share numerical machinery and feature evaluation, not one undifferentiated sketch/mechanism entity model.**

The post-M32 sketch north star is a Rust/WASM embeddable planar engine with retained
unsolved design intent, ordinary CAD constraints/dimensions, immutable host inputs,
cancellation, stable diagnostics and separate sketch-operation/production-topology
companions. The host continues to own expressions, B-rep projection, feature history
and application undo. M40.7, M53 and M61-M74 have explicit acceptance dispositions. New milestones
normally end in hands-on UAT after objective automation; M74 records an explicit scoped exception
that defers its unexecuted scorecard without calling it passed. The desktop demo has no future
mobile support requirement.

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
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
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
