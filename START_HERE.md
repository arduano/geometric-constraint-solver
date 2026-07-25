# GeoSolve active handoff

## Objective

Build two production library deliverables on the validated M1-M7 baseline:

1. a comprehensive embeddable 2D CAD sketch engine, including the standard planar
   relation/dimension catalog, advanced parametric curves, host integration and
   production topology companions;
2. 2D and 3D rigid-body kinematics for linkages and CAD assemblies.

The completed M10-M14 cut is a 2D Sketch Playground Alpha built over reusable Rust
sketch APIs. The current browser UI is a disposable, non-authoritative diagnostic
instrument for inspecting solver claims and finding behavioral defects. M39, M44 and
M51 plan its replacement with a desktop-only CAD-like sketch workbench that remains a
demo consumer, not a production UI or third solver. Mobile support is explicitly
outside future acceptance. This remains a geometric constraint and kinematics project,
not a physics engine.

## Read first

1. `AGENTS.md`
2. `PLAN.md`
3. `ARCHITECTURE.md`
4. `ACCEPTANCE.md`
5. `docs/SCENARIOS.md`
6. `REFERENCES.md`
7. `docs/adr/0001-*.md` through `docs/adr/0024-*.md`

`PLAN.md` is the authoritative execution order. `OVERNIGHT_REPORT.md` is a historical M1-M4 record, not current status.

## Current state

M0-M37 and the advanced free-radius circle/arc tangency follow-up are complete. M0-M7 form the frozen domain baseline; M8-M32 establish the production contracts, representative benchmarks, component-local linearization, local AD, numerical status/rank policy, persistent sessions, first-class bounds, persistent sketch and linkage documents, immutable curve jets, editable Beziers, geometry-generic curve constraints, explicit analytic and homogeneous rational conics, locally supported clamped and periodic B-splines/NURBS, differential geometry and advanced CAD continuity, the hardened document-backed 2D Sketch Playground Alpha, shared planar/spatial manifold state, sparse hard steps, structural matching, coupled hierarchy, robust planar/spatial continuation, gauge-separated planar/spatial linkage sessions, multi-driver spatial velocity, completed assembly persistence/oracle/performance, typed host-attribute extension seams, associative line offsets and point-defined mirrors, strict sketch-v1-to-v4 migrations, bounded visual-only all-family profile analysis, common-jet generic fillets, differentiable associated output arcs, persistent visible parent trim intervals, deterministic diagnostic capsules, M32 mutation/performance evidence, and the versioned `0.2.0` release contract and gates. M33 freezes the production-embedding decisions, capability matrix and representative workloads; M34 implements separate retained-design, attempted-candidate and accepted-solved identities/views without changing sketch v1-v4; M35 adds typed cooperative cancellation, deterministic work controls and transactional publication boundaries; M36 adds closed typed semantic operands and fixed/equal scalar foundations; M37 completes the frozen standard planar relation catalog without changing sketch v1-v4.

The active milestone is **M38: dimensions and persistent measurements**.

After M32, the approved M33-M55 north-star roadmap turns the broad mathematical
preview into a host-usable planar engine. It adds retained unsolved design intent,
ordinary CAD relation/dimension breadth, cancellation, construction/activation,
typed host parameters, immutable external 2D references, stable diagnostics,
revision-checked jobs, incremental scale, separate operations/topology companions and
a CAD-like desktop web consumer. Human UAT is required only at M40, M45, M52 and M54
after automated qualification.

M10 proves the persistent lifecycle through `SketchSession`. M11 adds the implemented `SketchDocument` generic graph, commands/history and versioned JSON. M12 adds immutable curve jets, editable quadratic/cubic Bezier and generic curve contact/tangency. M13 delivers the disposable browser playground; M14 hardens its exact alpha scenarios, recovery behavior, files and interaction budgets. M15 adds validated `SE(2)`/`SE(3)`, right/body-local retraction, quaternion-backed `Pose3`, frames/workplanes, manifold fixed/alias behavior and revision-stamped accepted hard linearization/sensitivity APIs. M16 adds indexed/CSC assembly, structural matching, bounded symbolic reuse, validated sparse LM steps, sparse-compatible coupled hierarchy, and adaptive natural plus explicit pseudo-arclength planar continuation. M17 adds persistent planar bodies/features/sources, separate topology and accepted state, physical-ground versus numerical-gauge certification, checked gauge/internal mobility reports, and velocity queries over the accepted shared hard linearization. M18 adds spatial bodies with local point/frame features, fixed-frame/ball/revolute sources, private six-DOF floating gauges, physical audit/rank/mobility publication and transactional rollback. M19 adds explicit ellipses, directed elliptical arcs, homogeneous rational quadratics and trimmed parabola/hyperbola branches across immutable geometry, generic sketch constraints, persistence and the web consumer. M20 adds stable-clock spatial axis/plane features, the common joint/mate catalog, hinge and translation position drivers, explicit mode monitors and atomic multi-driver transactions under ADR 0013. M21 adds validated clamped and periodic non-rational B-splines, span-local jets and residual incidence, persistent semantic spans, explicit one-sided transitions, continuity diagnostics and transactional knot insertion under ADR 0014. M22 adds gauge-separated NURBS, cancellation-safe homogeneous jets/refinement, curvature and osculating measurements, generic tangent/normal/equal-curvature/G2/parametric-C2 constraints, complete sketch persistence and large/property corpora under ADR 0015, completing Deliverable 1.

Continue in `PLAN.md` order. M30 proves offsets, mirrors, directed angles, fillets
and NURBS through movable accepted scenes and focused public-API controls. M31
broadens the M26 line-only visual analysis to every built-in curve under ADR 0024;
browser tessellation is never topology evidence. M32 closes the `0.2.0` release
hardening, M33 freezes the production-embedding contracts and baselines, M34 implements
the separate retained-design, attempted-candidate and accepted-solved views, and M35
adds cooperative cancellation and deterministic operation control. M36 adds the
closed typed operand and scalar foundations required by the ordinary CAD catalog,
and M37 completes that catalog with persistent, branch-explicit standard relations.
M38 dimension and persistent-measurement behavior remains planned.
Registry publication remains outside the automated gate.

M8 is a contract and benchmark-baseline milestone. Its preserved completion notes and
pre-rebaseline M10+ labels in older ADRs and `REFERENCES.md` record the allocation
accepted at that time; `PLAN.md` is authoritative for the current M10-M55 numbering.

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
