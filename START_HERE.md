# GeoSolve active handoff

## Objective

Build two production library deliverables on the validated M1-M7 baseline:

1. generic 2D CAD sketch constraints, including advanced parametric curves and tangency/continuity;
2. 2D and 3D rigid-body kinematics for linkages and CAD assemblies.

This is a kinematics project, not a physics engine. UI work is not on the active roadmap.

## Read first

1. `AGENTS.md`
2. `PLAN.md`
3. `ARCHITECTURE.md`
4. `ACCEPTANCE.md`
5. `docs/SCENARIOS.md`
6. `REFERENCES.md`
7. `docs/adr/0001-*.md` through `docs/adr/0009-*.md`

`PLAN.md` is the authoritative execution order. `OVERNIGHT_REPORT.md` is a historical M1-M4 record, not current status.

## Current state

M0-M9 and the advanced free-radius circle/arc tangency follow-up are complete. M0-M7 form the frozen domain baseline; M8-M9 establish the production contracts, representative benchmarks, component-local linearization, local AD, and numerical status/rank policy.

The next milestone is **M10: persistent solve sessions and first-class bounds**.

Do not skip directly to splines, NURBS, sparse solving or spatial joints. M10-M12 complete the shared numerical foundation those features require.

M8 is a contract and benchmark-baseline milestone. It does not implement the M9 linearization, M10 session/bounds, M11 manifold or M12 sparse targets described in the accepted documents.

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

Run the relevant Trunk build when shared public APIs or the WASM consumer change. The browser remains a smoke consumer, not a product gate for new interaction design.
