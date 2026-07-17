# GeoSolve active handoff

## Objective

Build two production library deliverables on the validated M1-M7 baseline:

1. generic 2D CAD sketch constraints, including advanced parametric curves and tangency/continuity;
2. 2D and 3D rigid-body kinematics for linkages and CAD assemblies.

The completed M10-M14 cut is a 2D Sketch Playground Alpha built over reusable Rust sketch APIs. The browser UI is a disposable, non-authoritative, desktop-first diagnostic instrument for inspecting solver claims and finding behavioral defects; it is not a production UI or a third product deliverable. Mobile compatibility is best-effort and must not constrain the desktop diagnostic workflow. This remains a geometric constraint and kinematics project, not a physics engine.

## Read first

1. `AGENTS.md`
2. `PLAN.md`
3. `ARCHITECTURE.md`
4. `ACCEPTANCE.md`
5. `docs/SCENARIOS.md`
6. `REFERENCES.md`
7. `docs/adr/0001-*.md` through `docs/adr/0012-*.md`

`PLAN.md` is the authoritative execution order. `OVERNIGHT_REPORT.md` is a historical M1-M4 record, not current status.

## Current state

M0-M16 and the advanced free-radius circle/arc tangency follow-up are complete. M0-M7 form the frozen domain baseline; M8-M16 establish the production contracts, representative benchmarks, component-local linearization, local AD, numerical status/rank policy, persistent sessions, first-class bounds, the persistent sketch document/command/history layer, immutable curve jets, editable Beziers, geometry-generic curve constraints, the hardened document-backed 2D Sketch Playground Alpha, shared planar/spatial manifold state, sparse hard steps, structural matching, coupled hierarchy and robust planar continuation.

The active milestone is **M17: shared planar kinematic architecture**.

M10 proves the persistent lifecycle through `SketchSession`. M11 adds the implemented `SketchDocument` generic graph, commands/history and versioned JSON. M12 adds immutable curve jets, editable quadratic/cubic Bezier and generic curve contact/tangency. M13 delivers the disposable browser playground; M14 hardens its exact alpha scenarios, recovery behavior, files and interaction budgets. M15 adds validated `SE(2)`/`SE(3)`, right/body-local retraction, quaternion-backed `Pose3`, frames/workplanes, manifold fixed/alias behavior and revision-stamped accepted hard linearization/sensitivity APIs. M16 adds indexed/CSC assembly, structural matching, bounded symbolic reuse, validated sparse LM steps, sparse-compatible coupled hierarchy, and adaptive natural plus explicit pseudo-arclength planar continuation.

Continue in `PLAN.md` order with M17 planar model/session/gauge migration before spatial joints, conics, B-splines or NURBS. Under ADR 0012 sparse QR accelerates independently validated damped steps while dense SVD remains authoritative for rank, mobility and sensitivity. M14 completes only the 2D Sketch Playground Alpha, not Deliverable 1; final 2D/3D completion remains ordered through M24.

M8 is a contract and benchmark-baseline milestone. Its preserved completion notes and pre-rebaseline M10+ labels in older ADRs and `REFERENCES.md` record the allocation accepted at that time; `PLAN.md` is authoritative for the current M10-M24 numbering.

## Work rules

1. Complete milestones in `PLAN.md` order.
2. Keep `geosolve-sketch` and `geosolve-linkage` as separate domains over `geosolve-core`.
3. Preserve explicit branch/span/winding/assembly state.
4. Never report success without independent residual and domain validation.
5. Never weaken a tolerance or remove a regression merely to pass a gate.
6. Keep APIs private or crate-private until a milestone requires public exposure.
7. Add a finite-difference Jacobian test and structured audit descriptor for every residual.
8. Make commits only when the supervising caller permits them.

## Standard verification

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
```

Run the relevant Trunk build when shared public APIs or the WASM consumer change. Starting at M13 the browser is an alpha acceptance consumer, but equations, documents and accepted-state semantics remain authoritative only in reusable Rust APIs.
