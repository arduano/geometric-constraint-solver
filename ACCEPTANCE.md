# Acceptance criteria

These are behavioral gates, not implementation suggestions. `PLAN.md` is the authoritative milestone order. A milestone is incomplete until its applicable criteria pass, and no performance result weakens a correctness threshold.

## Global quality gates

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
```

Run `(cd crates/geosolve-demo-web && trunk build --release)` when a shared public API or the WASM consumer changes.

Every milestone also requires:

- no non-finite value in an accepted state, residual, Jacobian, factorization input or report;
- deterministic result, source, component and diagnostic ordering for identical input and accepted state;
- no `unsafe` code and no native solver FFI;
- `GPL-3.0-or-later` metadata on all public crates;
- independent hard-residual and domain/branch validation before any success-like result or accepted-state commit;
- transactional retention of the previous finite accepted state on rejection;
- an analytic/local-AD versus central finite-difference test for every residual implementation;
- a structured, finite and valid audit descriptor for every executable residual row.

## Numerical policy and milestone ownership

The following rules apply through the frozen M1-M8 baseline unless a scenario explicitly documents a stricter or conditioning-justified alternative:

- residuals and tangent columns are normalized before convergence and rank decisions;
- accepted maximum normalized hard residual is `<= 1e-9`;
- analytic/local-AD Jacobian relative error is `<= 1e-6` away from singular or nondifferentiable states;
- model scales `1e-6`, `1` and `1e6` preserve topology, branch labels, rank/mobility classification and source diagnosis;
- numerical rank uses the implemented component-local relative threshold, and right nullity is exposed as local DOF;
- top-level `Converged` retains the baseline coupling described in `ARCHITECTURE.md`, while hard validation fields remain independently inspectable.

Starting at M9, and not as a retroactive M8 gate:

- hard validity, hard nonlinear termination and every secondary optimization status are independent report fields;
- each component uses the machine-floor numerical rank threshold in `ARCHITECTURE.md`;
- numerical left and right nullity are both reported;
- near-singular warnings use the documented band without silently changing rank or convergence.
- the M9 rank contract governs core equality/position reports; starting at M17, linkage velocity consumes the same accepted component-local hard linearization and rank thresholds.

Starting at M10, active-bound mobility and one-sided feasible motion are mandatory. Starting at M16, structural matching and deterministic under/well/over/mixed partitions are mandatory. A target rule does not make the frozen baseline fail before its assigned milestone.

## Product gates

### Deliverable 1: 2D CAD sketches

Completion at M22 requires independently editable points, lines/segments, circles/arcs, ellipses/conics, Bezier curves, B-splines and NURBS; generic contact and tangency; explicit orientation/span/winding/branch state; curvature, G2 and separately named parametric C2 behavior; driving/reference dimensions; truthful diagnostics; and versioned persistence.

The complete matrix must include exact, perturbed, invalid-domain, derivative, transformation, scale, branch-retention, active-bound, persistence and large sparse fixtures. A zero-speed curve jet, invalid knot vector, rational pole, escaped domain or ambiguous branch cannot produce a success-like result.

### Deliverable 2: 2D/3D rigid-body kinematics

Completion at M23 requires planar and spatial rigid bodies/features, common joints/mates, explicit assembly modes, multiple drivers, robust continuation, velocity-level queries, gauge-separated mobility and versioned persistence.

The complete matrix must include exact, perturbed, invalid-feature, tangent-Jacobian, global-transform, scale, mixed-scale, singular, branch-retention, gauge-invariance, persistence and large sparse fixtures. No accepted result may imply mass, force, reaction, collision or dynamics behavior.

### 2D Sketch Playground Alpha

M14 completes an alpha cut toward Deliverable 1, not Deliverable 1 itself. Its library scope is point, line/polyline, rectangle macro, circle, circular arc, editable quadratic/cubic Bezier; fixed/coincident/horizontal/vertical/point-on-curve/parallel/perpendicular/equal-length/equal-radius/midpoint/symmetry constraints; distance/length/radius/diameter/oriented-angle driving and reference dimensions; generic line-curve and curve-curve contact/tangency; and explicit branch state.

`SketchDocument`, `SketchSession`, commands, history, versioned serialization, curve evaluation and constraints must be reusable Rust APIs. Selection, hit testing, tool state, rendering and `localStorage` must remain web-only, and the web crate must contain no equations. Desktop and mobile must support select/box-select, compatible multi-select constraints, draw, solver-projected drag, dimension edit, delete/suppress, pan/zoom, undo/redo, JSON import/export/local autosave, confirmed prospective coincident/horizontal/vertical inference, diagnostics/conflict/DOF and retained geometry on failure.

The preceding desktop/mobile requirement records the completed M13-M14 gate. Post-alpha, the playground is a desktop-first diagnostic instrument rather than a production UI: it must remain effective for inspecting accepted geometry, audit, rank, branch and failure claims, while mobile compatibility is best-effort and non-gating.

## Frozen M1-M7 regression baseline

All existing M1-M7 tests and the advanced free-radius circle/arc tangency follow-up remain mandatory through M24.

### Core representation and solver

- Stable variable/residual/source IDs survive unrelated insertion and removal; packed order is deterministic.
- Scalar, `Vec2` and baseline `Pose2` blocks apply local increments and scales correctly.
- Residual incidence assembles heterogeneous blocks into correct normalized ranges.
- Invalid dimensions, scales, geometry, NaN and Inf reject before factorization.
- Exact, underdetermined, overdetermined, duplicate and contradictory systems report independently validated geometry and truthful rank/diagnostics.
- One equation in two scalar variables reports right nullity/local DOF `1`.
- Duplicate rows report deterministic redundancy candidates; contradictions never report `Converged`.
- Configuration-dependent rank loss is independent from nonlinear termination.
- Iteration-limit, stagnation and invalid-trial paths retain the last finite accepted state.
- Hard, temporary and preference objectives retain strict lexicographic semantics rather than undocumented weighted least squares.

### Adversarial verification

- Constructed-rank property systems recover known valid solutions and report known rank/nullity with reproducible seeds.
- Construct-valid nonlinear fixtures recover from documented perturbations.
- Translation, rotation, scale and insertion-order metamorphic tests preserve semantic results.
- Trace invariants show accepted hard cost is non-increasing within documented roundoff and rejected states are never committed.
- Benchmarks remain separate from correctness tests.

### Decomposition and diagnostics

- Disconnected components solve independently.
- Editing one component leaves another unchanged within `1e-12` and reuses it with zero iterations.
- Fixed and alias elimination retain independent validation of eliminated rows.
- Source diagnostics name a high-level source once even when it emits several rows.
- Underconstraint, redundancy and singularity may coexist and are not encoded only in one termination enum.
- `S2 conflicting rectangle` fails hard validation and names the incompatible width dimensions within the baseline bounded candidate algorithm.

Baseline candidate vectors do not yet carry M8 completeness metadata. Until the M10 report transition, an empty conflict/redundancy vector must be described as “no candidates reported”, not as a complete proof that none exist.

### Sketch scenarios

- S1 returns hard-valid geometry with local DOF `1`; temporary dragging moves only along permitted motion and release preserves nearby accepted geometry.
- Driving dimensions add equations; reference dimensions add none and report solved measurements.
- S3 external tangency reaches `(3, 0)` and explicit internal A-contains-B reaches `(1, 0)` on the retained positive-x branch.
- Bounded segment/arc contacts accept valid interior or endpoint roots and reject true escape transactionally.
- Line-circle and circle-arc tangency retain explicit side, radial, contact-neighborhood and periodic branch state.
- Free-radius circle/arc tangency reports exactly two local DOF and solves radius plus both contact parameters.
- Zero radii, collapsed directions, zero derivatives and unresolved mixed-scale ambiguity never become success-like domain results.

### Linkage scenarios

- L1/L2 closure residuals are `<= 1e-9`, preserve opposite open/crossed orientation signs and never silently change assembly mode.
- L3 revolute, guide and orientation equations validate at position and velocity level.
- Driver sweeps warm-start from the prior accepted state and retain ground unchanged.
- Near-toggle fixtures raise rank/singularity or conditioning diagnostics without committing a branch jump.

### WASM smoke consumer

- The separate crate builds for `wasm32-unknown-unknown` without a backend.
- It constructs S1-S3 and L1-L3 through public domain APIs and contains no duplicate equations.
- Geometry and audit values always come from the same accepted state.
- It displays termination, hard residual, rank/DOF, branch and candidate diagnostics, plus grouped source audit rows.
- It preserves prior valid geometry visibly after failed edits.
- Automated WASM and desktop browser diagnostic coverage remains through M24. M13-M14 added the disposable playground, including its historical mobile checks, as an alpha acceptance consumer without making it authoritative.

## M8 acceptance: contract rebaseline and representative baselines

M8 is ready for review only when every item below is objectively present. These checkboxes are acceptance criteria and do not mark `PLAN.md` complete.

The checked wording below is the preserved M8 completion record. Its then-current M8-M22 allocations are historical; the user-approved M10+ execution numbering is now M10-M24.

- [x] `ARCHITECTURE.md` and this file describe both product deliverables and allocate target behavior across M8-M22 without presenting a target as implemented baseline behavior.
- [x] Hard validity is specified independently from hard nonlinear termination, secondary optimum status, rank and structural class, including the baseline-to-target report transition.
- [x] The rank contract defines normalized component-local thresholding, a machine floor, numerical left/right nullity, structural under/well/over/mixed classification, near-singular warnings, active-bound mobility and gauge/internal mobility separation.
- [x] Every rank feature states whether it exists in the M1-M8 baseline or whether M9, M10 or M12 makes it mandatory.
- [x] Bounded redundancy/conflict reporting requires `Complete`, `Truncated` or `Skipped`, configured and consumed budgets, a reason when incomplete, and no complete inference from an empty skipped/truncated candidate list.
- [x] ADRs 0005-0009 are present with `Status: accepted` and make concrete decisions for component-local AD, pose manifolds, persistent sessions/bounds, the sketch design graph and grounding/gauge separation.
- [x] All ADRs preserve pure Rust, `unsafe_code = "forbid"`, internal traits initially, explicit branch state outside AD and the no-physics boundary where applicable.
- [x] Historical roadmap language in `README.md`, `START_HERE.md`, `REFERENCES.md`, `docs/SCENARIOS.md` and `OVERNIGHT_REPORT.md` is consistent with M1-M7 frozen baseline and M8 as the then-next milestone.

### Required benchmark artifacts

- [x] Existing `crates/geosolve-core/benches/small_dense.rs` remains registered and unchanged in scope.
- [x] `crates/geosolve-core/benches/representative_sparse.rs` is a Criterion harness registered with `harness = false` in `crates/geosolve-core/Cargo.toml`.
- [x] Benchmark support constructs deterministic finite residuals with analytic Jacobians and complete valid audit rows; a shared-code test checks central finite differences and accepted reports.
- [x] CAD-like cases contain exactly 100, 1,000 and 10,000 normalized tangent variables as 50, 500 and 5,000 `Vec2` blocks. Components contain at most 10 blocks, giving exactly 5, 50 and 500 sparse-incidence chains.
- [x] Linkage-like cases contain documented near sizes 99, 999 and 9,999 normalized tangent variables as 33, 333 and 3,333 `Pose2` blocks. Components contain at most 11 blocks, giving exactly 3, 31 and 303 local-incidence chains.
- [x] All six cases expose equal hard-row and tangent-variable counts, bounded local incidence, deterministic perturbations and no random input.
- [x] Criterion groups are exactly `representative_definition_compile`, `representative_linearization_assembly`, `representative_decomposition_solve_diagnostics` and `representative_component_edit_resolve`.
- [x] Each group contains `cad_like/{100,1000,10000}` and `linkage_like/{99,999,9999}` benchmark IDs and reports throughput in normalized tangent variables.
- [x] Linearization/assembly uses benchmark-local component shards because the M8 baseline public API otherwise allocates global dense columns; decomposition/solve/diagnostics and edit/re-solve use one global `Problem` and its public component cache.
- [x] Sample sizes are explicit and decrease for larger cases, with Criterion's minimum sample count retained.
- [x] Timed boundaries are exactly definition construction plus compile; assembly from precompiled shards; solve/report construction from a newly compiled problem; and edit plus re-solve from a pre-solved cached problem.
- [x] Validation, setup excluded by those boundaries, and input/output destruction occur after or outside each accumulated timed interval as far as Criterion supports.
- [x] One shared report validator checks every configured solve result for `Converged`, independent hard validation at `<= 1e-9`, expected rank/DOF and expected edit reuse.
- [x] Criterion `--test` executes those assertions at all six exact configured scales; normal `cargo test` validates representative small cases without a 10,000-variable full solve.
- [x] The normal M8 gate compiles but does not time the 10,000-variable dense baseline: `cargo bench --locked -p geosolve-core --no-run` must pass.

### M8 review commands

```bash
cargo fmt --all -- --check
git diff --check
cargo test --locked -p geosolve-core --test m8_benchmarks
cargo bench --locked -p geosolve-core --no-run
cargo bench --locked -p geosolve-core --bench representative_sparse -- --test
```

M8 does not require timed performance thresholds. It freezes reproducible benchmark definitions and measurement boundaries for later comparisons.

## M9 acceptance: canonical local linearization and AD

- [x] Fused residual/Jacobian evaluation writes into caller-provided component-local storage without global-column allocation.
- [x] Analytic and internal local-forward-AD paths agree with central finite differences to `<= 1e-6` on representative residuals.
- [x] Structured evaluation errors distinguish invalid domain, degeneracy, nondifferentiability and ambiguity.
- [x] Branch/span/winding/assembly state is not differentiated.
- [x] Reports separate hard validity, hard termination and every secondary optimization outcome.
- [x] Numerical rank uses the normalized component-local machine-floor threshold and reports left/right nullity plus a distinct near-singular warning.
- [x] Frozen accepted geometry, source ordering and audit equations remain unchanged.
- Public audit marks rows evaluated only after canonical Jacobian/fused validation while retaining fresh finite displayed values on structured derivative failure.
- Internal same-workload shape regression is required, but M9 has no timed performance threshold and does not expose private IR solely for Criterion.

## M10 acceptance: sessions, bounds and active sets

- A persistent `SolveSession` automatically tracks topology/source/state revisions and dirty components.
- `SketchSession` is the first domain consumer and keeps all sketch types out of `geosolve-core`.
- Non-structural one-component edits cannot omit dirty IDs and do not rebuild or iterate unaffected components.
- Accepted-state commits are atomic and rollback remains transactional.
- Bounds participate in step computation rather than only post-solve rejection.
- Reports distinguish equality nullity, bidirectional active-set DOF and one-sided feasible motion.
- Redundancy/conflict sections expose completeness, budget, consumed work and reason under the M8 contract.
- Endpoint contacts and positive radii report active bounds truthfully through the sketch consumer.

## M11 acceptance: persistent SketchDocument, commands and history

- Persistent external IDs survive deterministic JSON serialization/remapping independently of runtime generational keys.
- `SketchDocument` represents point/line/polyline/circle/arc topology, semantic features, dimensions, contact slots and every explicit branch field through a closed versioned graph.
- The rectangle macro expands to ordinary document geometry and constraints.
- Typed create/edit/delete/suppress and driving/reference-dimension commands update the session transactionally.
- Undo/redo records only accepted commands and reproduces accepted geometry and explicit state deterministically.
- Duplicate IDs, dangling references, unknown variants, invalid domains and non-finite JSON reject atomically.
- S1-S3 and the complete M5/M7 corpus remain semantically unchanged.

## M12 acceptance: Bezier and generic curve constraints

- Editable quadratic/cubic Bezier controls and contact parameters all appear in derivative incidence.
- Public immutable curve evaluation returns finite position and first-through-third derivatives or a typed domain/regularity failure.
- Line/polyline segment/circle/arc/Bezier combinations use common point-on-curve, contact and tangency plumbing.
- Every control/contact derivative passes central finite differences to `<= 1e-6` away from singular/nondifferentiable states.
- Endpoint orientation and all branch/span/winding/neighborhood state are explicit; cusp and zero-speed states reject.
- Every alpha document/session/command/history/serialization/curve/constraint workflow is usable without the web crate.
- The alpha library scenarios pass at uniform scales `1e-6`, `1` and `1e6` without changed IDs, branches, rank or diagnosis.

## M13 acceptance: disposable browser playground

- The browser uses only public sketch/document/session/command/history/serialization/audit APIs and contains no equation implementation.
- Select, box-select, compatible multi-select constraints, every draw tool, projected drag, dimension edit, delete/suppress and pan/zoom are functional.
- Prospective coincident/horizontal/vertical inference is visibly provisional and changes no document until explicit confirmation.
- Undo/redo, JSON import/export and local autosave operate through library commands and serialization.
- Solve status, conflict, rank/DOF and audit output correspond to the same accepted geometry; failure leaves that geometry visible.
- Pointer and touch paths are functional at representative desktop and mobile viewports.
- Selection, hit testing, tool state, rendering and `localStorage` remain web-only.

## M14 acceptance: playground hardening and alpha gate

- Browser E2E covers alpha scenarios A1-A10 from `docs/SCENARIOS.md` on desktop and mobile viewports.
- A1 constrained rectangle retains horizontal/vertical/coincident topology and solves edited width/height dimensions.
- A2 underconstrained drag follows solver-permitted motion without weakening hard constraints.
- A3 line-circle tangency and A4 free-radius circle-arc tangency retain explicit contacts, orientation/neighborhood and accepted branches.
- A5 Bezier tangent line differentiates all incident controls/contact parameters and rejects zero-speed tangency.
- A6 conflicting dimensions names both incompatible sources and retains prior geometry.
- A7 undo/redo reproduces create/edit/delete/suppress state and accepted geometry.
- A8 JSON round trip preserves every persistent ID and branch/span/winding/orientation/neighborhood field.
- A9 invalid edit/import retains the accepted document, history position, diagnostics and visible geometry.
- A10 executes the alpha corpus at uniform scales `1e-6`, `1` and `1e6` with invariant topology, branches, rank/mobility and source diagnosis.
- Malformed/unknown-version/duplicate-ID/dangling-reference/non-finite/over-limit imports fail atomically with actionable errors.
- Deterministic small/medium import, first-solve, edit/solve and render timings have documented and enforced reference-environment budgets.
- Passing M14 means 2D Sketch Playground Alpha complete; it does not claim Deliverable 1 complete.

## M15 acceptance: manifold geometry and spatial state

- [x] `SE(2)`/`SE(3)` composition, inverse, exponential, logarithm, adjoint, retraction and local difference satisfy property tests under ADR 0006.
- [x] `Pose3` stores a validated quaternion ambient representation and has tangent dimension six.
- [x] Tangent-coordinate finite differences, global-transform equivariance and quaternion-sign invariance pass.
- [x] Fixed/alias elimination and accepted-state sensitivity APIs are manifold-aware.
- [x] Invalid frames and quaternions reject before success.

Completion record (2026-07-16): exact and near-half-turn quaternion cases,
right-tangent finite differences, frame/workplane validation, transformed planar
linkage branches and body-velocity equivariance all pass. Accepted sensitivity
reuses the accepted rank threshold and independently validates the solved equation
before publishing a success-like status. Its M15 scope is reduced hard equalities
and body-local raw tangents; active bounds, secondary objectives and world-frame
conversion are not part of this acceptance claim.

## M16 acceptance: sparse structure, hierarchy and continuation

- [x] Structural matching reports declared block-envelope rank, structural left/right nullity and deterministic under/well/over/mixed partitions separately from M9 numerical rank.
- [x] Block triplet assembly and pure-Rust sparse solve agree with dense results on geometry, rank, mobility, diagnostics and branch state.
- [x] Dense fallback remains available for small or diagnostically ambiguous components.
- [x] Symbolic ordering/factorization reuse and dense/sparse crossover are demonstrated by the dedicated connected-chain Criterion probe following the M8 benchmark discipline.
- [x] Cross-component secondary objectives retain strict priority semantics.
- [x] The documented planar toggle crosses only through explicit pseudo-arclength continuation.

Completion record (2026-07-17): accepted-threshold augmented tangents, adaptive
retry, manifold pseudo-arclength control and ordinary-physical-report publication
are implemented. The displacement-driven L3 fixture stops in natural mode and
crosses explicitly in pseudo mode at scales `1e-6`, `1` and `1e6`, with
dense/sparse physical parity. Under caller-approved ADR 0012, sparse QR owns
validated damped LM steps while dense SVD remains the rank-revealing authority;
no sparse factorization success is used as rank or convergence evidence.

## M17 acceptance: planar kinematic migration

- Planar model topology, accepted state and compiled session are separate.
- Persistent body, feature and source IDs survive deterministic serialization/remapping independently of runtime generational keys.
- Body-local features are primary and velocity uses the shared accepted hard linearization.
- Physical ground and numerical gauge produce identical relative geometry under alternative gauge choices.
- A floating planar component reports three gauge DOF separately from internal mobility.
- L1-L3 and compatibility APIs remain valid.

Completion record (2026-07-17): persistent planar topology, accepted state,
body-local features, runtime mappings and gauge metadata are separate. Private
manifold gauges select coordinates for certified floating components but are absent
from the independently validated physical report, source audit and diagnostics.
Reports check and expose physical equality nullity as `gauge_dof +
internal_mobility`; alternative references preserve relative geometry and source
diagnostics at all required scales. Persistent and compatibility velocity queries
reuse accepted hard component rows, scales, rank thresholds and sensitivity solves,
then independently validate published world-frame velocities. L1-L3 remain valid.

## M18 acceptance: spatial vertical slice

- Spatial bodies and local point/frame features support physical ground and floating gauge policy.
- Fixed-frame, ball and revolute joints report expected relative mobility.
- Exact, perturbed, tangent-Jacobian, transformed/scaled, invalid-feature and rollback fixtures pass.
- Every accepted configuration is independently validated.

Completion record (2026-07-17): `SpatialAssembly` provides one `Pose3` state per
body, checked local point/frame features, physical fixed-pose grounding and private
six-DOF gauges for certified floating components. Fixed-frame, ball and explicit
aligned/opposed revolute sources report the expected rank and internal mobility in
floating and grounded assemblies. Public physical sessions exclude private gauge
rows from source mappings, audit, rank and accepted linearization. Analytic
right-tangent Jacobians, required scales and common-left `SE(3)` fixtures pass.
Independent validation is capped at `1e-9`, rejects half-turn/parity false roots and
non-finite world features, and revision-checked failed edits retain every accepted
view.

## M19 acceptance: conics

- Ellipses, elliptical arcs, rational-quadratic conics and explicit parabola/hyperbola branches have validated jets and domains.
- Affine/similarity, branch-retention and rational-pole tests pass.
- Circle-limit geometry remains valid while unobservable orientation is reported truthfully.
- Generic contact/tangency adds no geometry-pair equation implementation.

## M20 acceptance: spatial joints and mates

- Axis/plane features and prismatic, cylindrical, planar and universal joints implement expected mobility.
- Distance, angle, alignment and frame-offset mates support multiple explicit drivers.
- Axis parity, winding, side and signed-volume state prevent silent mode changes.
- Each primitive passes exact, recovery, tangent-Jacobian, scale, mixed-scale and degeneracy fixtures.

## M21 acceptance: B-splines

- Degree, control identity and knot vectors validate before evaluation.
- De Boor position and first-through-third jets pass Bezier equivalence, affine covariance and partition-of-unity oracles.
- Clamped/periodic spans have stable identities and one-sided knot policy.
- Residual incidence is limited to active local support; knot insertion preserves geometry.

## M22 acceptance: NURBS and CAD completion

- Positive weights, homogeneous jets, weight derivatives and explicit weight-gauge policy pass finite differences.
- Unit weights reproduce B-splines and canonical quadratic NURBS reproduce conics.
- Curvature, osculating radius, G2 and separately named parametric C2 constraints validate.
- Rational denominator and mixed-scale ambiguities reject truthfully.
- Complete sketch persistence, fuzz/property, differential-oracle and sparse performance suites pass.

## M23 acceptance: kinematic completion

- Adaptive and pseudo-arclength continuation preserve explicit planar/spatial modes with branch-boundary events and hysteresis.
- Multiple-driver velocity requests distinguish determinate, underdetermined and inconsistent outcomes.
- Body/feature velocities and optional motion/nullspace bases validate differentiated equations.
- Planar mechanisms embedded in 3D agree with planar oracles.
- Complete linkage persistence, fuzz/property, differential-oracle and sparse performance suites pass.

## M24 acceptance: release hardening

- Public APIs expose domain and audit behavior without accidental compiler/core internals.
- Versioned serialization migrations, malformed-document tests and round trips pass.
- Crate documentation and complete examples cover both deliverables.
- SemVer, changelog, deprecation, licence and attribution policies are complete.
- Supported scale/performance envelopes are recorded from reproducible benchmarks.
- Fuzzing finds no panic, non-finite accepted state or false success.
- Native, locked WASM smoke and all prior acceptance suites pass.

## Regression and oracle policy

- Every convergence, rank, scaling, branch or diagnostic bug gets a minimal regression scenario.
- Differential tests compare geometric validity, rank/mobility/status and branch continuity, not identical internal coordinates or iteration counts.
- SolveSpace and PlaneGCS are references/oracles, not dependencies.
- An external convergence flag is never accepted without local independent validation.
