# Acceptance criteria

These are behavioral gates, not implementation suggestions. `PLAN.md` is the authoritative milestone order. A milestone is incomplete until its applicable criteria pass, and no performance result weakens a correctness threshold.

## Global quality gates

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
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

M22 completed the built-in curve and generic differential-constraint surface:
independently editable points, lines/segments, circles/arcs, ellipses/conics, Bezier
curves, B-splines and NURBS; generic contact and tangency; explicit
orientation/span/winding/branch state; curvature, G2 and separately named parametric
C2 behavior; driving/reference dimensions; truthful diagnostics; and versioned
persistence. It did not complete the production host-embedding contract.

Completion of Deliverable 1 in a future production-hardening milestone additionally requires the ordinary planar CAD
constraint/dimension catalog, retained unsolved design intent separated from accepted
geometry, construction/activation semantics, typed host parameters, immutable
external 2D references, cancellation, stable persistent-ID diagnostics, stale-work
protection, documented production scale, companion drafting operations and complete
production wire/profile output.

The complete matrix must include exact, perturbed, invalid-domain, derivative, transformation, scale, branch-retention, active-bound, persistence and large sparse fixtures. A zero-speed curve jet, invalid knot vector, rational pole, escaped domain or ambiguous branch cannot produce a success-like result.

### Deliverable 2: 2D/3D rigid-body kinematics

Completion at M23 requires planar and spatial rigid bodies/features, common joints/mates, explicit assembly modes, multiple drivers, robust continuation, velocity-level queries, gauge-separated mobility and versioned persistence.

The complete matrix must include exact, perturbed, invalid-feature, tangent-Jacobian, global-transform, scale, mixed-scale, singular, branch-retention, gauge-invariance, persistence and large sparse fixtures. No accepted result may imply mass, force, reaction, collision or dynamics behavior.

### 2D Sketch Playground Alpha

M14 completes an alpha cut toward Deliverable 1, not Deliverable 1 itself. Its library scope is point, line/polyline, rectangle macro, circle, circular arc, editable quadratic/cubic Bezier; fixed/coincident/horizontal/vertical/point-on-curve/parallel/perpendicular/equal-length/equal-radius/midpoint/symmetry constraints; distance/length/radius/diameter/oriented-angle driving and reference dimensions; generic line-curve and curve-curve contact/tangency; and explicit branch state.

`SketchDocument`, `SketchSession`, commands, history, versioned serialization, curve evaluation and constraints must be reusable Rust APIs. Selection, hit testing, tool state, rendering and `localStorage` must remain web-only, and the web crate must contain no equations. Desktop and mobile must support select/box-select, compatible multi-select constraints, draw, solver-projected drag, dimension edit, delete/suppress, pan/zoom, undo/redo, JSON import/export/local autosave, confirmed prospective coincident/horizontal/vertical inference, diagnostics/conflict/DOF and retained geometry on failure.

The preceding desktop/mobile requirement records the completed M13-M14 historical
gate. Post-alpha mobile behavior is not a future acceptance target. M39/M44 establish
the desktop workbench, and cleanup M46-M50 removes the playground after direct-test
replacement; no responsive, tablet
or phone implementation is required. ADR 0029 also supersedes the historical
web-only ownership of selection, hit testing and tool state: deterministic
constraint-editing policy moves to `geosolve-constraint-editor`, while rendering,
platform events, accessibility and browser storage remain presentation-owned.

## Frozen M1-M7 regression baseline

All existing M1-M7 tests and the advanced free-radius circle/arc tangency follow-up remain
permanent mandatory regressions.

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
- Automated Rust and WASM adapter coverage remains mandatory. M13-M14 added the disposable playground and historical browser/mobile E2E as alpha acceptance consumers without making them authoritative; cleanup M46-M50 replaces retained claims with direct owning-layer tests and deletes that old infrastructure.

## M8 acceptance: contract rebaseline and representative baselines

M8 is ready for review only when every item below is objectively present. These checkboxes are acceptance criteria and do not mark `PLAN.md` complete.

The checked wording below is the preserved M8 completion record. Its then-current M8-M22
allocations are historical; `PLAN.md` now governs completed work through the approved M61 gate,
approved M62 authoring milestone, approved M63 canvas-constraint presentation, approved M64
editable-sample cleanup, approved reduced-scope M65 predictable dragging, approved scoped M66
computed-Fillet features under ADR 0031, approved M67 legacy-surface and harness cleanup, approved
M68 headless Fillet direct manipulation under ADR 0032, approved M69 Profile/Construction
semantics under ADR 0033 and approved M70 headless auto-constraint drafting under ADR 0034. M70B
has fully qualified and byte-verified F001/F002 replacements plus a historically clean 193-row
test-only H1/H2 baseline whose complete release gate and fresh publication passed. H3 historically
preserved those rows while adding four reviewed F003/F004 `DEFECT` rows; its test-only
`--require-clean` gate was deliberately red. Authorized production repairs now make the same four
stable cases `PASS`, so the current reviewed fixture is 197/197 `PASS`. Exact golden, workspace,
Clippy, formatting and relevant WASM qualification pass; clean release-candidate nomination and
publication, targeted human recheck and approval remain pending. M71 remains deferred behind it.

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

Completion record (2026-07-18): immutable analytic and homogeneous rational conics
provide finite validated third-order jets, directed domains, explicit hyperbola
branches, semantic features and CAD measurements. Runtime and persistent sketch
layers use deterministic point/vector/scalar mappings and the existing generic
contact/tangency residuals; no conic-pair equation was added. Required-scale central
differences, affine/similarity covariance, rational poles, branch retention,
circle-limit rank truthfulness, canonical JSON, accepted projection and transactional
rollback pass. Independent acceptance is capped at `1e-9`, and the web consumer
renders imported periodic ellipses through the public document evaluation API.
Post-draw arc/parabola/hyperbola trim handles and rational homogeneous-middle handles
project through public document APIs, retain explicit sweep/branch state and commit one
accepted undoable transaction; invalid targets retain the prior document and history.

## M20 acceptance: spatial joints and mates

- Axis/plane features and prismatic, cylindrical, planar and universal joints implement expected mobility.
- Distance, angle, alignment and frame-offset mates support multiple explicit drivers.
- Axis parity, winding, side and signed-volume state prevent silent mode changes.
- Each primitive passes exact, recovery, tangent-Jacobian, scale, mixed-scale and degeneracy fixtures.

Completion record (2026-07-18): complete clocked axis/plane features feed the
prismatic, cylindrical, planar and universal joints and all four documented mate
families. Analytic right-local Jacobians, required-scale and within-component
mixed-scale fixtures, independent `1e-9` validation, explicit parity/winding/side/volume
state, and gauge-separated expected mobility pass. Hinge and axial/planar translation
drivers solve simultaneously through atomic assembly-mode transactions. Representative
shaft/bearing and block/base assemblies pass exact, recovery, driver-stage, false-root
and complete retained-state rollback scenarios; monitor-only modes never enter equality
rank or physical audit rows.

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

Completion record (2026-07-19): clamped and periodic NURBS use one explicit
persisted unit gauge, homogeneous refinement and degree-local control/weight AD.
Reference-translated pairwise quotient jets and compensated normal-acceleration
curvature reject unrepresentable mixed scales without false zero values. Generic
tangent/sided-normal, signed or branch-explicit magnitude curvature, G0/G1/G2 and
rate-explicit parametric C2 rows pass central differences and are independently
recomputed from immutable candidate jets before commit. Canonical persistence,
transactional gauge/refinement/transition/deletion, 48-case properties and the
1,000-control/128-contact sparse corpus pass with the complete locked workspace,
WASM, rustdoc, benchmark compilation and release Trunk gates.

## M23 acceptance: kinematic completion

- Adaptive and pseudo-arclength continuation preserve explicit planar/spatial modes with branch-boundary events and hysteresis.
- Multiple-driver velocity requests distinguish determinate, underdetermined and inconsistent outcomes.
- Body/feature velocities and optional motion/nullspace bases validate differentiated equations.
- Planar mechanisms embedded in 3D agree with planar oracles.
- Complete linkage persistence, fuzz/property, differential-oracle and sparse performance suites pass.

Completion record (2026-07-19): spatial continuation and its mode-event
follow-up are implemented under ADR 0016. Hinge and translation drivers use active
scalar forms of their existing equations; private gauges, adaptive retry and
ephemeral pseudo control feed only separately re-solved ordinary physical samples.
Typed predictor/corrected endpoint events cover source false roots, explicit mode
monitors and canonical hinge cuts with accepted enter/leave hysteresis. Explicit
parity, side, orientation and principal-cut changes lower to one revision-checked
clone/solve/validate/swap transaction. Grounded/floating shaft motion, an embedded
spatial slider-crank and cut/side fixtures cover folds, required scales,
common-left `SE(3)`, dense/sparse parity, rollback and no implicit winding change.
Multi-driver spatial velocity additionally reuses executable parameter columns
and accepted component rank thresholds, distinguishes determinate modulo gauge,
underdetermined and inconsistent outcomes, and independently validates body,
feature and coordinate fields plus optional physical nullspace bases under ADR
0017. The displacement-driven L3 additionally matches compatibility and persistent
planar position/velocity oracles at all required scales under an arbitrary static
`SE(3)` embedding. A non-planar universal ring, macro/micro stage-tool stack and
258-coordinate sparse fixed-frame chain add closed-chain chirality/mobility,
required scale extremes and validated `SparseQr` step coverage without replacing
dense rank authority. ADR 0018 adds canonical versioned spatial documents,
persistent body/feature/source/coordinate/monitor IDs, complete accepted pose,
driver/gauge/mode/hysteresis state, deterministic fresh runtime remapping and
atomic failed-import rollback. Full-catalog and malformed persistence tests pass.
Generated scale/common-left `SE(3)` and byte-mutation properties, a 36-case
analytic position/velocity oracle and the explicit 256-moving-body release `Auto`
crossover complete the final linkage corpus without weakening independent
residual validation or dense rank authority. M23 acceptance is complete.

## M24 acceptance: sketch extension and embedding foundation

- Sketch JSON version 1 has a private frozen wire DTO and strict explicit version dispatch without canonical output changes.
- Persistent element and source-owner APIs cover every document object and audit source without runtime/core identity joins.
- `SketchAttributes<T>` remains document-scoped, typed, solver-independent and outside canonical sketch JSON.
- Foreign/wrong-kind targets reject; dormant values survive document history until explicit cleanup.

Completion record (2026-07-20): ADR 0019 freezes exact version-1 JSON behind a
private DTO and explicit strict dispatch, adds typed persistent element/raw-ID and
source-owner joins, and keeps generic application attributes in a document-bound
sidecar with host-owned persistence/history. Complete A8 identity coverage,
foreign/wrong-kind rejection, real delete/undo/redo dormancy, solver isolation and
an exact golden payload pass with all locked native, WASM, rustdoc, benchmark and
release Trunk gates. M24 acceptance is complete.

## M25-M28 acceptance: advanced sketch constructions

- Supporting-line and translated-segment offsets have separately truthful equations, rank and DOF.
- Point-defined entity mirrors compose explicit ordinary constraints, and existing oriented angles remain branch-explicit.
- Public line arrangements split crossings and publish visual-only bounded faces without persistent region semantics.
- Fillets progress from an independently validated line-line vertical slice to common-jet generic curves and explicit parent trim views.
- Every new row has audit, derivative, transformation, scale, persistence, malformed-input and rollback coverage.

M25 acceptance is complete: separately named offsets publish independently
validated rows and truthful mobility; point-defined mirrors remain ordinary
constraint constructions through coordinated B-spline refinement; directed angles
retain explicit branch state; and strict sketch v1 input migrates to canonical v2.

M26 acceptance is complete: public read-only line/polyline arrangements use only
explicit identity/coincidence topology, split crossings and T-junctions ephemerally,
publish deterministic finite contours with exact source-span intervals, and create
no persistent regions or equations. Overlap, uncertainty, inconsistent unsolved
coincidences and overflow/resource limits fail closed with typed status. The
pointer-transparent WASM overlay leaves selection, history, autosave and canonical
JSON unchanged under desktop/mobile browser automation.

M27 acceptance is complete: persistent line-line fillets solve four audited
common-jet center/contact rows with explicit parent normal sides, endpoint order,
sweep and driving/reference radius. Accepted arc endpoints are derived from solved
strict-interior contacts and independently revalidated with center-normal,
tangency, radius, side, order and sweep oracles before publication. Sketch JSON v3
round-trips behind frozen v1/v2 input DTOs; parent edits, branch edits, suppression,
history, explode/ownership behavior and failed-domain rollback retain truthful
state. Every side/order/sweep combination passes under similarity transforms at
scales `1e-6`, `1` and `1e6`; parallel, near-parallel, escaped, zero-radius and
non-finite inputs reject without allocation or mutation. Parent trim views and
generic curve-family incidence remain explicitly assigned to M28.

M28 acceptance is complete: canonical JSON v4 adds one equation-free persistent
visible interval per stable support span and common-jet `CurveCurveFillet`
associations over every regular line, circle/arc, ellipse/conic, Bezier, B-spline
and NURBS family. Four offset rows plus two output-radial rows pass finite
differences across all 105 unordered family pairs; associated output-arc point,
contact, tangency, curvature and continuity incidence differentiates both endpoint
angles. Independent validation rejects zero speed, poles, escaped or ambiguous
local roots, unresolved `1 - side*radius*curvature`, parallel offset intersections
and malformed ownership. Periodic winding, required scales/transforms, persistent
trim motion, suppression, explosion, history, migration and rollback pass. Frozen
v1-v3 languages reject v4 syntax, while legacy v3 fillets migrate visibly
untrimmed. Public visible intervals drive the separate WASM renderer, hit testing,
selection and profile analysis; desktop/mobile browser gates pass without adding
equations to the web crate. Arbitrary multi-fragment trimming is not claimed.

## M29 acceptance: release hardening

- Public APIs expose domain and audit behavior without accidental compiler/core internals.
- Versioned serialization migrations, malformed-document tests and round trips pass.
- Crate documentation and complete examples cover both deliverables.
- SemVer, changelog, deprecation, licence and attribution policies are complete.
- Supported scale/performance envelopes are recorded from reproducible benchmarks.
- Fuzzing finds no panic, non-finite accepted state or false success.
- Native, locked WASM smoke and all prior acceptance suites pass.

Completion record (2026-07-21): the `0.1.0` compatibility policy classifies
persistent domain workflows, legacy compatibility facades and explicitly unstable
advanced compiler/runtime diagnostics; defines lockstep SemVer, Rust `1.89` MSRV,
deprecation and schema-retention rules; and freezes sketch v1-v4 plus planar/spatial
v1 support. Crate guides and runnable sketch/planar/spatial examples cover accepted
solve, audit, edit, velocity and canonical restore workflows. Package archives carry
the GPL text and README, dependency licences pass `cargo-deny`, and faer's bundled
MPL/BSD notices are recorded separately.

The M29 mutation corpus applies more than 2,000 deterministic byte/extreme-value
cases across all persisted domains under panic guards; any surviving session must
be canonical, finite, independently `HardValidity::Valid` and within `1e-9`.
The documented release runner passes full native acceptance, warnings-denied docs,
locked WASM, package contents, native/browser performance, the 1,536-coordinate
spatial release fixture and desktop/mobile Chromium. The browser exposes the M28
trimmed-fillet UAT scenario plus the release/schema/scale/legal contract without
owning equations or document semantics. M29 acceptance is complete.

## M30 acceptance: interactive construction and NURBS UAT

- Supporting-line offset starts with two equality DOF; target endpoint drag changes accepted axial position or length while retaining signed offset and direction state.
- Exact translated offset starts with one rotational DOF; source drag preserves exact endpoint translation on the target.
- Entity-mirror drag moves its ordinary reflected counterpart through public symmetry rows and survives history/persistence.
- Directed-angle drag crosses the principal cut without changing explicit orientation; target/orientation/mode edits are transactional.
- M27 and M28 fillet labs start with documented free-radius motion and atomically update contacts, output arc and M28 visible intervals.
- NURBS labs expose controls, non-gauge weights, gauge selection, knot insertion, local support, periodic span/winding and differential continuity through public APIs.
- Every lab publishes finite independently valid geometry at normalized hard residual `<= 1e-9`; rejected edits retain the accepted scene.
- Desktop E2E proves accepted geometry movement and focused editor transactions; mobile smoke loads every scene without overflow.

Completion record (2026-07-21): twelve public scenarios publish exact expected DOF,
focused instructions and primary drag identities. Seven native lifecycle tests and
the focused desktop/mobile Chromium suite prove actual accepted movement for offsets,
mirror, angle, M27/M28 fillets and NURBS controls plus persistence/history/rollback.
All browser operations remain public document transactions and M30 acceptance is
complete.

## M31 acceptance: all-family visual profiles

- Every built-in planar curve family can contribute bounded visible intervals to the arrangement without shape inference.
- Pair and self intersections are isolated with bounded family-specific methods; a root is published only when its parameter enclosure and transverse local topology are resolved.
- Tangency, positive-length overlap, poles, zero speed, unresolved multiple roots and exhausted work produce typed `Truncated` or `Skipped` evidence rather than false `Complete`.
- Directed curved half-edges use actual source tangents, preserve traversal-ordered source parameters and never weld unrelated coordinate-equal endpoints by proximity.
- Area and containment use analytic or interval-enclosed curve integrals/tests; orientation and visual area publish only when their uncertainty excludes ambiguity.
- Explicit fillet ownership may weld trim/output endpoints after fresh position validation; suppression/explosion semantics remain truthful.
- All-family pair/self, required-scale, transform, large-translation, nesting, budget and malformed-geometry fixtures pass with canonical JSON unchanged.
- Browser overlays evaluate public returned source intervals, remain pointer-transparent and display scope/status/issues/budget evidence.

Completion record (2026-07-21): ADR 0024 is implemented with bounded linear,
circular, analytic-conic, polynomial and homogeneous rational pieces plus pure-Rust
outward interval arithmetic, certified transcendental/angle bounds and
interval-Newton/Krawczyk root isolation. Exact source parameters, periodic winding,
endpoint identities and fillet ownership drive topology; tangency, overlap, poles,
zero speed, unresolved roots, ambiguous tangent/containment decisions and exhausted
budgets fail closed. Area signs and containment publish only from resolved analytic
or interval enclosures, and ambiguous components cannot contaminate disjoint clean
faces.

Thirty-one M31 tests cover all 120 family pairs, self-intersections, required
scales/transforms, periodic seams, nesting, malformed geometry, ownership,
persistence neutrality and budgets. Post-completion UAT adds accepted-residual
endpoint/contact splits, certified artificial-boundary root retries, proof-based
duplicate root merging and NURBS refinement regressions. Six reusable browser scenes
pass focused desktop/mobile Chromium checks for public-source rendering, movable
fillet closure, exact NURBS control authoring, self-root enclosures, metadata,
pointer transparency and responsive layout. Format/diff, warnings-denied locked
workspace Clippy, full locked workspace tests, locked WASM and release Trunk pass.
M31 acceptance is complete.

## M32 acceptance: post-expansion release

- The complete construction/NURBS/profile UAT catalog is discoverable and has concise expected-motion instructions.
- A copied text capsule reproduces canonical geometry and profile budgets, while corrupt or oversized capsule input is rejected atomically.
- Mutation tests cover new command payloads and all profile families without panic, non-finite publication or false success/completeness.
- Updated scale/performance envelopes pass without weakening correctness or completeness policy.
- One release-gate command passes all native, documentation, package/licence, locked WASM and desktop browser suites.

Completion record (2026-07-22): `0.2.0` consolidates all M30/M31 UAT metadata,
deterministic scene capsules, exact retained-state desktop failure coverage, a two-test
command/profile mutation corpus and the native/browser envelope in
`docs/M32_SCALE_PERFORMANCE.md`. Clean candidate `8d6f648` passed the complete
`scripts/release-gate.sh` command: format/diff, warnings-denied Clippy/rustdoc, full
locked workspace tests, locked WASM, benchmark compilation, explicit mutation and
timing gates, the 1,536-coordinate spatial release case, licences, package contents,
release Trunk and full Chromium on the first invocation. The completion-status commit
is re-gated cleanly before M33 implementation. M32 acceptance is complete.

## M33 acceptance: CAD engine contract

- Accepted ADRs define design/accepted state, immutable host inputs, cancellation/concurrency, companion boundaries and draft-v5 policy.
- The complete feature/relation/dimension applicability matrix and representative production workloads are deterministic and reviewable.
- No target-only concept is exposed as implemented behavior and all frozen M1-M32 contracts remain green.

Completion record (2026-07-23): ADRs 0025-0028, the machine-checked 15-family/
38-relation/37-dimension capability matrix, six exact current-v4 workload signatures
and 24 Criterion measurement cases freeze the production embedding contract without
adding target APIs or changing v1-v4. Cancellation latency remains explicitly
unavailable until M35. A release-gate-generated spatial seed additionally hardened
bitwise-idempotent accepted `Pose3` reconstruction without relaxing exact velocity or
continuation snapshots. Clean candidate `5cd7cb6` passes the complete native,
documentation, package/licence, locked WASM, performance and Chromium release gate;
M33 acceptance is complete.

## M34 acceptance: retained design and accepted state

- Design, attempted candidate and accepted solved state have separate identities and revisions.
- Structurally valid unsolved intent is repairable and persistable without changing the last independently validated accepted state.
- Malformed or non-finite design never enters retained intent, and attempted geometry is never labeled accepted.

Completion record (2026-07-23): the additive `RetainedSketchDocumentSession`
publishes typed design, attempt and accepted identities, exact implemented attempt
inputs, separate attempt/accepted runtime mappings and optional finite candidate
geometry. Conflicting and failed-unsuppression revisions remain editable and repair
through ordinary transactions while older accepted document/geometry/audit bytes stay
unchanged. Invalid design transactions allocate no revision. Separate canonical v4
design/accepted graphs plus host-owned revision high-water metadata restore without
freezing draft v5 or reusing identity. Initial conflict, all-family candidates,
topology divergence, underconstrained parent warm starts, persistence and repair
regressions pass with the complete locked workspace, WASM, rustdoc, Trunk and release
gates. M34 acceptance is complete.

## M35 acceptance: cancellation and operation control

- Cancellation and deterministic work exhaustion are distinct from solve/profile failure and convergence.
- Cancellation at every documented checkpoint commits nothing and retains accepted state bitwise.
- Native and single-threaded WASM consumers observe the same operation outcome semantics.

Completion record (2026-07-24): public core and sketch operation-control APIs carry a
monotonic cancellation token, deterministic overflow-safe limits, typed stop reasons
and exact reports through lowering, compilation, nonlinear/hierarchy work,
factorization, rank, diagnostics, validation, profiles and session publication.
Controlled mutations use scratch state and a final pre-commit checkpoint, so all
cancelled/exhausted paths retain accepted bytes and revisions. Controlled dense
kernels reject either dimension above the documented M35 256 limit before execution;
20-run release probes bound the measured profile, QR and rank-SVD checkpoint windows
in `docs/M35_CANCELLATION_LATENCY.md`. Nineteen focused regressions plus the complete
locked native, WASM, rustdoc, Trunk and release gates pass. M35 acceptance is complete.

## M36 acceptance: semantic features and scalars

- Typed point, direction, support, curve and scalar references validate and persist through stable domain IDs.
- Composite relations retain one semantic source and complete structured audit evidence.
- No coordinate proximity or public plugin callback selects operand or branch meaning.

Completion record (2026-07-25): M36 adds closed, serializable references for points,
centers, endpoints, owning-curve plus persistent-point controls (including
refinement-stable B-spline/NURBS controls), directions, line supports, curve spans and
scalar properties. Fixed/equal scalar sources live in one strict document-bound
catalog, reserve identities through the document allocator, carry explicit
unit/domain/support/neighborhood/branch provenance and lower deterministically to
separate raw and normalized hard-row values/Jacobians with complete structured audit.
Normalized scalar-row derivatives pass central finite differences at all required
scales; malformed topology/neighborhoods, sibling/document identity collisions,
persistence, cancellation/work exhaustion and tampered public row/audit evidence have
focused regressions. The catalog is serialized separately from frozen sketch v1-v4,
and M37/M38 catalog and measurement behavior is not implied. M36 acceptance is
complete.

## M37 acceptance: standard constraint catalog

- Concentric, collinear, point-pair horizontal/vertical, block/fix, point symmetry and broadened equal/symmetry relations cover the frozen applicability matrix.
- Every multi-root relation carries explicit side/orientation/containment state.
- Every new residual passes finite differences, transformations, required scales, persistence, cancellation and rollback.

## M38 acceptance: dimensions and measurements

- Relative coordinates, datum coordinates, point/line and line/line spacing, angle, sweep, arc length and conic dimensions have consistent driving/reference measurements.
- Persistent measurements carry typed units, provenance and finite independently evaluated values.
- Generic path length cannot succeed when value, derivative or work bounds are incomplete.

M38 acceptance is complete. Focused regressions also prove explicit positive/negative
angle winding and reject stale or foreign persistent-measurement provenance without
mutation.

## M39 acceptance: CAD workbench foundation

- The desktop shell provides core authoring, selection, sketch tree, inspector, glyphs, dimensions, status and problems through public sketch APIs.
- Accepted, unsolved, solving, solved-preview and rejected views are explicit and synchronized.
- Automated desktop E2E passes; mobile and responsive behavior are not tested or claimed.

M39 acceptance is complete. The focused fresh-profile desktop E2E covers authoring,
selection synchronization, accepted-value dimension annotations, accepted and rejected
reload behavior, retained accepted geometry and the isolated legacy developer route.

## M40 acceptance: mechanically qualified core interaction and human UAT 1

- M40.1 fixes the one-way `geosolve-constraint-editor` to `geosolve-sketch` dependency
  and assigns no equations, accepted-state authority, rendering or platform state to
  the editor.
- M40.2 natively proves deterministic accepted scene, persistent point/span picking,
  point priority, ordered modifier selection, basic relation applicability and the
  exact click/drag threshold without DOM hit geometry.
- M40.3 natively enumerates every draft stage, snap proposal, projected-drag preview,
  completion, cancellation, invalid input, modifier, pointer and threshold boundary.
- M40.4 natively proves retained design/attempt/accepted identity, action
  applicability, dimensions, delete/suppression, history, diagnostics, stale revisions
  and transactional rollback through public sketch APIs.
- M40.5 makes the desktop workbench a thin event/render/storage adapter and removes
  duplicate interaction policy rather than retaining fallback paths.
- M40.6 provides deterministic/model-based native transition coverage, native/WASM
  parity and focused browser adapter tests for every objective scorecard action.
- M40.7 begins only after M40.1-M40.6 pass; the supervising human then completes the
  prepared 30-45 minute usability scorecard.
- No unresolved correctness, data-loss, misleading-state or basic interaction blocker
  remains; every objective finding has a headless regression before human recheck.

M40.1-M40.6 automated acceptance is complete. The checked-in corpus, generated model,
canonical native/release-WASM report and all-covered machine matrix qualify every
objective scorecard action, while focused Chromium platform evidence passes 14/14.
M40 is complete as of 2026-07-26: the supervising human approved M40.7 after the
mechanically requalified UAT-C1-F4 and UAT-C1-F5 remediations.

### Post-M40 reusable interaction acceptance rule

- New semantic drafting assistance must be implemented and deterministically tested in
  `geosolve-constraint-editor`, including state retained across pointer samples, snap or
  hover identity memory, candidate ranking, tolerance boundaries, guides, previews and
  commit/cancel consequences.
- Browser/native/3D hosts may translate platform events and map a ray or pointer onto a
  sketch plane, then render editor DTOs. They may not maintain a parallel geometric
  interaction state machine or derive their own assistance candidates.
- Prospective inference remains non-mutating until explicit confirmation submits a
  typed headless effect/document edit.
- Every future browser assertion for such behavior requires a native headless replay
  proving the same transition first. This rule is prospective and does not reopen the
  mechanically completed M40.1-M40.6 scope.

## M41 acceptance: construction roles and activation

- Construction geometry participates in solving but is excluded from production profiles under the default declared scope.
- Effective activation and every inactivity reason are explicit and dependency-safe.
- Inactive branch/contact/topology state survives reactivation without coordinate inference.

Passed 2026-07-27: focused role/activation, constrained-construction, dependency-closure,
retained-lifecycle, frozen-persistence and exact discrete-state regressions passed with
independent review and all global native/WASM quality gates.

## M42 acceptance: typed host parameters

- Immutable revisioned parameter batches can drive several targets without adding solver variables.
- Inputs and output proposals have typed units, persistent identity, provenance and stale-commit protection.
- GeoSolve owns no formula parser, host dependency graph or configuration evaluator.

Passed 2026-07-27: canonical typed batches, shared driving targets, activation-first
resolution, declared dimensionless fixed-scalar targets, stamped output proposals,
deterministic persistence/evidence and atomic stale/invalid/cancelled retention passed
focused and global native/WASM qualification with independent review.

## M43 acceptance: external references

- Solving consumes one immutable finite 2D snapshot set with exact revision and digest evidence and no host callback.
- Missing, stale, malformed or topology-incompatible references retain unsolved intent and accepted geometry truthfully.
- Rebinding and topology transitions are explicit; proximity never repairs identity.

Passed 2026-07-27: canonical bounded point/directed-line snapshot sets, exact attempt
and accepted stamps, fixed-coefficient native solving, typed unavailable-reference
closure, atomic failure retention, explicit topology rebinding, complete audit evidence
and diagnostic reproduction passed focused and global native/WASM/browser qualification
with independent review.

## M44 acceptance: host-state workbench

- Construction, activation, parameter, external-reference and dual-state workflows are available through the desktop consumer.
- Browser state identifies all design/input/accepted revisions and never exposes internal scalars as user parameters accidentally.
- Fresh-profile automated E2E proves every advertised state and recovery path before UAT 2.

Passed 2026-07-27: coordinator and workbench fixtures expose
typed role/activity, complete parameter batches and proposals, immutable external status
and explicit rebinding, three lifecycle identities and accepted-only scene/profile/audit
evidence. Locked native/WASM/release checks, preserved M40 browser coverage (14/14) and
fresh-profile M44 coverage (6/6) passed with static equation/callback boundary checks.
The carry-forward full M14 browser suite has no complete
post-correction pass. Separate runs produced tower burst-drag overruns (`130 ms` and
`118 ms` against the unchanged `100 ms` budget) and a CDP mouse-event dispatch timeout.
The reproducible overrun was localized to unconditional all-active M41 dependency
propagation and corrected by skipping dependency closure when no direct inactivity reason
exists. Five isolated post-correction tower runs (`62`, `42`, `37`, `39`, `44 ms`) were
within budget and focused M41/native tower/release build checks passed, but the full rerun
was stopped by the supervising user before a final result. The budget and historical
assertions remain unchanged. Further flaky full-M14 work is explicitly deferred to the
M45 UAT preparation window. During that preparation, the release WASM build, focused
103-test web suite and all six fresh-profile M44 groups passed again, including the
checksummed M45 finding package. The supervising user then explicitly authorized avoiding
the costly legacy full-M14 suite and fast-tracking to M45 based on focused happy-path
qualification. This closes M44 without converting any incomplete M14 run into passing
evidence or weakening its preserved gates.

## M45 acceptance: cleanup investigation and UAT-point capture

- All intended host-semantics UAT points are preserved independently of the temporary fixture.
- Every legacy inline/E2E group is classified by authoritative owner or explicit retirement.
- Human UAT is not claimed; the host-semantics human gate is relocated to post-cleanup M53.

## M46 acceptance: direct-test ownership freeze

- Every M14/M40/M44 E2E assertion has a named direct Rust unit/integration owner or reviewed retirement reason.
- Direct tests target domain, editor, presentation, persistence or WASM-adapter contracts without Chromium, CDP, HTTP serving, DOM scraping, screenshots or wall-clock browser timing.
- No new E2E or source-substring policy scan is introduced, and workspace formatting/Clippy gates are clean.

Completion record (2026-07-27): the final ledger assigns every M14/M40/M44 group,
static scan and legacy inline assertion to an existing or named proposed direct Rust owner
or reviewed retirement. Finding capture is test/UAT-only, M46 deletes nothing, and an
independent read-only review found no ownership or deletion-gate blocker. Formatting/diff,
shell syntax, warnings-denied workspace Clippy, the complete locked all-feature workspace
suite and locked WASM check pass without browser automation.

## M47 acceptance: focused host-state replacement and M44 purge

- Five small fixture groups preserve all former M44 contracts and ten recorded UAT points without becoming canonical or persisted product state.
- Typed finding evidence is tested deterministically over public domain/audit APIs.
- The broad fixture, fixture-only controls, `e2e/m44.mjs` and its browser infrastructure are deleted.

Completion record (2026-07-28): five direct Rust groups in `panels.rs`, `scene.rs` and
`evidence.rs` preserve the six M44 contracts and all ten archived UAT points through typed
public domain/editor inputs, accepted-only rendering and deterministic checksummed capture.
The broad `HostState` composition, its fixture actions/markers and `e2e/m44.mjs` are absent.
Focused M41-M43 tests, 101 demo-web tests, the complete locked all-feature workspace suite,
formatting/diff, warnings-denied Clippy and the locked all-feature WASM check pass without
browser automation. M47 acceptance is complete.

## M48 acceptance: direct workbench qualification and M40 purge

- The native editor corpus/golden remains authoritative; retained presentation, persistence and WASM-adapter claims have direct tests.
- Browser-only DOM/layout/reload/download/focus claims are explicitly retired rather than imitated at the wrong layer.
- `e2e/m40.mjs`, `serve-m40.sh`, source scans and M40 browser infrastructure are deleted.

Completion record (2026-07-28): the unchanged 53-test editor suite passes its M40 corpus,
canonical golden and completeness oracle. Pure workbench tests directly cover construction
effects, coordinate normalization, shared persistent identity, glyph/dimension DTOs,
lifecycle/problem/redundancy presentation, the production persistence codec and fallback,
semantic markup, deterministic evidence serialization and exact adapter report/checksum
parity. The M40 script, serving script, runtime markers/actions and browser-only qualification
mechanics are absent; delivery-only claims are recorded as retired in
`docs/M48_IMPLEMENTATION.md`. The 111-test all-feature demo-web suite, complete locked
all-feature workspace suite, formatting/diff, warnings-denied Clippy, locked all-feature WASM
check and release Trunk build pass without browser automation. M48 acceptance is complete.

## M49 acceptance: legacy semantic extraction

- Every retained class-A/M14 claim passes in sketch, linkage, editor, persistence or focused presentation tests.
- Duplicate claims are confirmed against native owners; legacy-only UI/mobile/layout/timing/download claims have reviewed retirement entries.
- No retained assertion depends on the playground runtime or `e2e/m14.mjs`.

Completion record (2026-07-28): all 13 M14 browser groups and all 92 legacy inline tests
are reconciled in `docs/M49_IMPLEMENTATION.md` with zero unowned claims. Retained geometry,
continuation, transaction, inference, accepted-snapshot and diagnostic semantics pass through
direct sketch, linkage, editor and focused workbench owners; browser delivery, layout,
timing, download and private adaptive-render claims have explicit retirement rationales.
Formatting/diff, the complete locked all-feature workspace suite, warnings-denied workspace
Clippy, the all-feature WASM check and release Trunk build pass without browser automation.
Independent read-only verification confirmed that no retained assertion depends on the old
runtime and that M49 leaves the M14 E2E/application purge exclusively to M50. M49 acceptance
is complete.

## M50 acceptance: old E2E and legacy application purge

- All old E2E scripts and Chromium/CDP/server/profile/download infrastructure are absent.
- `#/dev/lab`, the playground/frozen legacy application, hidden DOM, obsolete CSS, legacy persistence glue and legacy-only tests are absent.
- Repository search, direct native/WASM tests, formatting and warnings-denied Clippy prove the deletion boundary without browser automation.

Completion record (2026-07-28): the final M14 E2E script/directory, playground route/runtime,
hidden DOM and obsolete CSS, legacy-only tests and persistence glue, serving scripts and
release-gate browser invocation are absent. One workbench remains over public domain/editor
APIs; dead dependencies and browser-only features are pruned. The focused editor and demo-web
suites, complete locked all-feature workspace suite, formatting/diff, warnings-denied workspace
Clippy, all-feature WASM check and release Trunk build pass without browser automation or
serving. Reviewed source/script searches and independent read-only verification confirm the
deletion boundary. M50 acceptance is complete.

## M51 acceptance: single-workbench consolidation

- One workbench remains, with minimal platform glue and directly testable presentation/persistence/evidence transformations.
- Dead dependencies, compatibility shims, stale docs and cleanup-only fixtures are removed.
- Direct tests are the sole automated qualification path and native/WASM checks pass.

Completion record (2026-07-28): one workbench and one WASM startup path remain. M51 removed the
design-only browser-storage migration, duplicate M40 editor-report adapter and M40 JSON/SVG
evidence package; retained directly tested workspace persistence, presentation/effect
transformations and deterministic typed host evidence; and removed the stale M32 distribution
copy. Focused editor/demo-web suites, the complete locked all-feature workspace suite,
formatting/diff, warnings-denied workspace Clippy, all-feature WASM check and release Trunk build
pass without browser automation or serving. Independent read-only verification found no
functional loss or unowned survivor contract. M51 acceptance is complete.

## M52 acceptance: post-cleanup host UAT candidate

- Minimal disposable UAT composition covers all ten host-semantics points without reintroducing product fixture state.
- Instructions and finding evidence are deterministic products of public domain/audit APIs.
- All objective claims pass direct automated qualification; only human judgment remains.

Completion record (2026-07-28): four fixed-identity in-memory fixtures and ten deterministic typed
instructions/actions cover the preserved host-semantics points in the sole workbench. Dedicated
M52 evidence omits canonical fixture documents while retaining typed inputs, identities, revisions,
lifecycle and accepted/attempted audit. The production-used sidecar state directly proves blocked
ordinary actions and saves, unchanged exit state and persistence-codec reload. Focused and full
native tests, formatting/diff, warnings-denied workspace Clippy, WASM check and release Trunk build
pass without browser automation or serving. Independent read-only verification passed. M52
acceptance is complete. M53 subsequently received supervising-human approval.

## M53 acceptance: human UAT 2

Completion note (2026-07-28): the supervising human rated every M53-S5 scorecard area Pass,
reported no concern or blocker and explicitly approved the milestone. The candidate identity,
finding dispositions and approval record are retained in `docs/M53_UAT.md`.

- The supervising human completes and explicitly approves the selector-led 35-50 minute
  host-semantics scorecard.
- Exactly eight typed scenarios cover preserved host-semantics points P1-P10 and error-presentation
  points P11-P12 under **Geometry intent**, **Host-owned inputs**, **Truth & evidence** and
  **Error attribution**; selecting, switching and resetting reconstruct
  deterministic ephemeral state, while global evidence capture and exit preserve the ordinary
  workspace.
- The selected-scenario guide exposes its description, objective points, human questions, steps,
  expected outcome and recent transcript/evidence. Direct tests qualify catalog completeness and
  isolation; grouping and presentation add no browser-owned domain logic.
- Nested scenario groups open immediately as right-expanding flyouts on hover or keyboard focus;
  narrow layouts keep every branch reachable inline without restoring per-group disclosure state.
- The headless editor exposes only current failed/rejected attempts as structured persistent-ID
  problem metadata. Defensible source owners and visible operands are targeted through attempted
  mappings and document dependencies; unattributable failures remain explicitly global.
- The accepted canvas remains authoritative beneath a separate overlay. Targeted points, curves,
  constraints and dimensions receive highlights and focusable tooltip markers; a global marker is
  used when scope is global or no target resolves. Marker interaction never mutates selection.
- Construction, suppression, parameters, external references and unsolved-state recovery are understandable and trustworthy.
- No unresolved ownership, stale-data, recovery or state-trust blocker remains; objective findings have direct regressions.
- Every UAT request/finding has a durable identifier, classification, disposition and applicable
  regression/requalification plus human retest; deferred future scope retains an explicit roadmap
  owner.

## M54-M69 acceptance

M54-M69 are the completed post-M53 sequence. M62 received explicit supervising-human approval on
2026-07-29, M63 and M64 received explicit supervising-human approval on 2026-07-30, and M65
received focused supervising-human approval on 2026-08-01. M66 received explicit scoped
supervising-human approval on 2026-08-08. M67 received explicit supervising-human approval on
2026-08-08, while M68 and M69 received explicit supervising-human approval on 2026-08-09. No old
browser E2E qualification may return.

## M65 acceptance: predictable bounded projected dragging

Status: complete and explicitly approved by the supervising human on 2026-08-01.

- [x] One gesture-start locality plan is derived from the independently accepted hard nullspace.
  Active rank and passive rank cover are explicit, and anchors are selected deterministically by
  greatest rank gain, lower mobility rank and compile order.
- [x] Locality targets are frozen from gesture-start accepted visible geometry. The selected
  cursor is the only Temporary target and the planned anchors are the only PreviousState
  Preferences; no all-point stabilizer, retry loop, sample key or sample driver controls motion.
- [x] Each non-stale pointer sample executes exactly one retained attempt from the last
  independently accepted preview. Rejection or work exhaustion preserves that complete preview,
  same-gesture recovery is supported, and stale/out-of-order samples are no-ops.
- [x] Both circle circumferences act as offset-preserving semantic handles for their own centers.
  Dragging either twin roller leaves the other center within `1e-8` across horizontal, vertical,
  diagonal and reversal paths. A visible dimension leader cannot occlude an overlapping draggable
  center/circumference, while its offset label remains selectable.
- [x] Core publishes a success-like result only after independently validating Hard rows. On the
  single-component dense path, a complete positive-Temporary residual vector is independently
  captured, re-evaluated and preserved through Preference work row-by-row within
  `max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`.
  This is only a positive-Temporary vector-reproduction floor, not a relaxed Hard or Temporary
  acceptance tolerance. Coupled-priority solving retains its existing scalar attained-level
  semantics. No path may publish post-Temporary raw drift, and accepted/no-motion reports reject
  invalid-geometry or numerical-failure termination and require successfully evaluated audit
  rows. Truthfully non-optimal secondary termination remains separate from independently valid
  Hard geometry.
- [x] Every projected sample is bounded to `16,384` each document-validation, dependency and
  lowering items; `256` nonlinear iterations, `256` factorizations and `256` rank kernels;
  `512` rejected trials; `1,024` component linearizations; `256 × 256` dense kernels;
  `512` diagnostic candidates; and `1,024` diagnostic trials.
- [x] Direct regressions cover twin-roller independence, real pointer overlaps and reject/recover;
  all four pantograph controls plus natural off-manifold guide projection; Scotch-yoke guide
  deletion and reversals; scissor jack/tower; circle offset; release/cancel/Undo/Redo; late/stale
  queued results; and ordinary authoring through workspace save/reload and subsequent editing.
- [x] Formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature WASM,
  release Trunk build and `git diff --check` pass on the same final source state.
- [x] The supervising human completes and explicitly approves the four-part
  `docs/M65_UAT.md` scorecard.

M65 does not add alternate-branch UI/search/samples, a residual family, relaxed tolerance,
weighted-priority semantics, sample-specific motion policy, worker architecture or global root
enumeration. Replacement mechanical qualification and focused U2/U3 human approval are recorded
in `docs/M65_UAT.md`; M65 is complete.

## M66 acceptance: computed 2D Fillet features

Status: complete. On 2026-08-08, the supervising human explicitly approved and closed M66 for its
mechanically qualified computed-Fillet scope, accepting `M66-KL001` as a deferred interaction
limitation. This does not claim a complete post-PF004 replay of every scripted UAT step.

The superseded solver-owned ordinary-UI build is preserved at
`origin/archive/m66-associative-fillet-2026-08-07` (`1034afc`). ADR 0031 replaces only the
ordinary workbench route. M27/M28 solver-owned Fillets,
`SketchOperationRequest::AssociativeFillet`, M58 integration and existing documents remain
accepted compatibility behavior and receive no automatic migration.

M66 is accepted only when all of the following are true:

- A separate `geosolve-sketch-features` crate owns a versioned `ComputedFeatureDocument` with
  stable feature/corner IDs, allocator non-reuse, labels, suppression and persistent
  `FilletSet` intent. Among workspace crates it depends only on `geosolve-sketch` and
  `geosolve-geometry` and adds no
  residual, solver variable, constraint or dimension.
- One Apply persists one set of grouped corners and one shared finite positive radius. A later
  Apply creates a separate set. Intent includes exact source spans, picked parameters,
  neighborhoods/winding, normal sides, retained endpoints, endpoint order and sweep; generated
  arcs/fragments are never persisted.
- Evaluation consumes one exact independently accepted sketch snapshot and publishes an exact-
  stamped computed snapshot. Output IDs are revision-local; stable provenance maps them to
  persistent set/corner identity and source intervals. Result containers admit variable output
  cardinality without implementing Offset.
- Every published output independently validates finite geometry, tangency, radius, domains,
  retained sides, branch/order/sweep state and offset regularity. M66 accepts affine/affine and
  affine/non-affine corners and returns a typed unsupported failure for two non-affine parents
  without narrowing M28.
- Opposite endpoints of one shared span may be claimed by different sets. Duplicate, crossed or
  consumed endpoint intervals fail every participating set deterministically. One invalid corner
  fails its complete set; unrelated sets remain current. Evaluation never mutates
  `DocumentCurveTrimView`.
- Multi-corner preselection remains grouped, repeated picks accumulate corner targets and reverse
  selection canonicalizes deterministically. Numeric input or a preview arc/radius grip edits the
  shared radius. Apply/Enter commits without a final radius-confirmation click.
- Blank optional host radius input retains the initialized or remembered positive radius. First and
  second line picks, point corners, overlapping candidates and high-valence junctions resolve
  deterministically under finite work. Pick and option transitions publish only together with a
  freshly `Current` whole-feature preview; rejection preserves the prior state and preview.
- Native curves, computed source fragments and generated Fillet arcs use bounded headless
  tessellation with a smooth curved baseline. Inflected curves remain rendered and pickable even
  when their parameter midpoint lies exactly on the endpoint chord; straight spans remain minimal.
- The stable ordinary sample leaf **Samples → Curves & constructions → 2D Fillet playground**
  provides editable multi-corner/sequential and short-middle conflict polylines beside fixed
  line-line, line-circle, line-quadratic-Bezier and high-valence reference specimens. Direct
  screen/coordinator tests cover their intended authoring, ambiguity and rejected-pick recovery;
  the sample owns no guide, read-only state or alternate coordinator.
- Native browser text-selection and element-drag defaults are suppressed only within the SVG
  canvas. Fillet option inputs and other HTML remain selectable/editable. This adapter contract is
  directly tested without restoring or claiming browser E2E qualification.
- A painted computed-preview arc owns radius pointer-down even where its native parent support is
  also within tolerance. Stable painted-item metadata is only a hint: the coordinator requires the
  exact held whole-feature preview and current scene provenance, and the headless editor
  independently hits the named owner. Stale/foreign owners and any second radius press reject
  state-neutrally; the original gesture remains usable, and Shift/Control/Command cannot toggle its
  owner away. Ordinary pointer selection keeps its existing modifier behavior.
- A generated arc selects stable corner/set provenance. Dragging it changes only the shared feature
  radius; deleting it removes its corner, deleting the final corner removes the set and suppression
  is set-wide. Generated geometry is not a sketch-constraint operand. Every native source point
  remains selectable and draggable.
- A valid sketch edit commits even when a feature becomes invalid. The failed set publishes no
  stale output and emits feature/corner/source-attributed errors, using global scope only when
  attribution is unsafe. Source motion may recover it; source deletion is a repairable failure and
  Undo restores the same intent identities.
- Shared-radius edits change no sketch input/accepted identity, accepted coordinate, residual,
  numerical rank or reported DOF. Direct tests compare those facts, not only rendered geometry.
- Exact CAS covers complete sketch input/accepted identity, feature revision/digest and evaluator
  policy. Stale, cancelled or exhausted evaluation cannot publish. Undo/Redo/reload preserve
  intent and stable IDs while regenerating fresh output IDs.
- Application workspace v4 stores the separately versioned feature sidecar beside the unchanged
  canonical-v4/draft-v5 sketch payload. Workspace v1-v3 migrate to an empty feature document and
  do not reinterpret existing M28 Fillets.
- The ordinary workbench route creates no M28 association, trim view, radius scalar, Driving/
  Reference choice, radius dimension or sketch constraint. It presents a **Features** tree,
  computed-arc/radius interaction and attributed failures from reusable metadata.
- Base-only profile/fill output is withheld with a typed “computed geometry not yet included”
  status when active computed geometry would make it misleading. M66 exposes no production or
  visual-profile consumption of computed output.
- Direct regressions cover the four-point/three-span two-corner batch; reverse selection;
  sequential/batch visible parity; conflict and recovery; every source-point drag; source deletion
  and Undo; independent deletion/suppression; Undo/Redo/reload; stale CAS; cancellation;
  exhaustion; allocator non-reuse; output-ID invalidation; and variable output count.
- M27/M28/M30/M58 compatibility remains green. Formatting, warnings-denied Clippy, locked
  all-feature workspace tests, all-feature demo-web WASM, release Trunk and `git diff --check`
  pass on one nominated post-pivot source.
- The supervising human explicitly approves the rewritten `docs/M66_UAT.md` under a scoped close
  decision. U1-U5 are accepted under that decision; `M66-PF001` through `M66-PF004` are closed by
  direct regression evidence rather than represented as individually repeated human tests. No
  correctness, data-loss, stale-output, source-editability or ordinary multi-corner Fillet blocker
  remains within the accepted scope.

Accepted known limitation `M66-KL001` — radius-drag and branch-choice interaction: radius drag
measures pointer distance from the held/old arc center while evaluation moves the center and
contacts, so tracking can drift or feel inverted. Post-placement contact/root, retained-parent
direction and alternate-arc choices lack intuitive controls, especially for line-circle Fillets.
Numeric radius editing, explicit persisted branch state, independent validation, rollback and
sketch-state invariance remain correct. The playground line-circle specimen starts at radius `0.5`,
near a branch fold. At M66 close, a future cut was left unassigned; completed M68 now owns the
headless one-dimensional radius rail, frozen absolute branch intent, typed contact metadata and its
internal continuation seam, retention/continuation actions, bounded local-alternative previews and
friendlier specimen while retaining the fold as a regression fixture. M68 closes `M66-KL001`; it
does not reopen M66 or assign the work retroactively to M67.

M66 explicitly excludes Offset implementation/UI/placeholders, computed-on-computed chaining,
Bake/Explode, production/profile consumption, automatic M28 migration, a canonical sketch-schema
change, global root enumeration, browser E2E, `/#/dev/lab` and mobile behavior.

## M67 acceptance: legacy surface and harness cleanup

Status: complete and explicitly approved by the supervising human on 2026-08-08.

M67 is accepted only when all of the following are true:

- The sole workbench no longer renders Production topology, Host-state evidence or Accepted
  redundancy developer cards. Problems, canvas error attribution, selection/branch/dimension/
  computed-feature editors, authoring, editable Samples, history, camera and persistence remain.
- There is exactly one WASM startup and one workbench root. No playground root, router,
  `/#/dev/lab` runtime, browser E2E, guided scenario harness or misleading hash brand link exists.
- All fourteen former M40 transition cases have reviewed dispositions. Every retained executed
  semantic has a named current test owner; nonexecuted browser-delivery labels and the seeded
  schedule/digest format are explicitly retired. The production qualification runner,
  browser-evidence matrix, JSON corpus/golden report and doc-hidden evidence API are gone rather
  than replaced by another generic harness.
- The unused generic local-AD prototype and normalized-tangent fused-Jacobian branch are absent.
  Live Pose2/Pose3 local-difference AD and finite-difference tests remain; solver equations,
  independent success validation and priority semantics are unchanged.
- Audited orphan selectors, unused sketch lowering helpers, a duplicate defaults test and exactly
  duplicated release-gate invocations are removed. Capability-owning M49 regressions, performance
  probes, architecture boundaries and persistence migrations remain. The M32 supporting-offset
  timing witness observes movement of the edited endpoint rather than requiring unrelated passive
  motion from a valid two-DOF solve.
- Reusable topology, lifecycle, redundancy, diagnostic and audit domain APIs retain their direct
  owning-layer regressions even though raw developer cards no longer consume them in demo-web.
- Formatting, warnings-denied Clippy, locked all-feature workspace tests, forced dead-code review,
  all-feature WASM, rustdoc, benchmark compilation, licence/package checks, release Trunk and Git
  hygiene pass on one nominated source.
- The supervising human explicitly approves all four areas in `docs/M67_UAT.md`.

The 2026-08-08 close decision accepts all four focused UAT areas with no new M67 finding recorded.
M68 subsequently completed and received explicit approval under ADR 0032; that later scope does
not reopen M67.

M67 changes no residual family, branch/orientation choice, tolerance, persistence language,
computed-Fillet behavior, Offset/Mirror capability or mobile support claim. Historical milestone
and ADR records remain evidence rather than live endpoint instructions.

## M68 acceptance: headless Fillet direct manipulation

Status: complete and explicitly approved by the supervising human on 2026-08-09. ADR 0032's
implementation, focused direct qualification, complete release qualification, frozen candidate
publication and focused human UAT all pass.

M68 is accepted only when all of the following are true:

- `geosolve-sketch-features` continues a completed Fillet corner from its exact absolute accepted
  intent. Radius continuation preserves normal sides, retained endpoints, contact
  neighbourhoods/windings, endpoint order, sweep and local root; relative authoring booleans do
  not reconstruct completed corners.
- A regular corner exposes a finite one-dimensional radius rail derived from the analytic
  offset-intersection centre sensitivity. Both parent expressions agree within a documented
  scale-aware tolerance, and central finite differences independently qualify orthogonal, acute
  and reversed line-line, line-circle and line-Bezier cases under transforms/scales and forward/
  reverse motion. Non-finite, singular and ill-conditioned rails reject.
- Pointer motion projects onto the frozen pointer-down rail. Tangential motion is a no-op and
  there is no arbitrary radius clamp beyond finite positive radius, parent domains and valid
  same-branch geometry. A shared-radius edit previews every corner atomically and identifies all
  affected arcs.
- A branch fold, offset singularity, domain limit or loss of regularity retains the exact last
  `Current` result with a typed reason. Radius dragging and numeric editing never switch roots
  implicitly. Contact reseeding, retained-direction changes and complementary/local alternatives
  are explicit actions bounded to the selected native parents and persisted neighbourhoods;
  tied candidates report ambiguity rather than guessing.
- One atomic feature-set mutation can publish an accepted radius and replacement absolute corner
  configuration in one feature revision and one history step without changing stable
  feature/corner IDs. Workspace v4 and the separately versioned feature document require no
  schema migration.
- `geosolve-constraint-editor` owns idle, radius-drag, named-parent contact-drag and branch-preview
  state, including exact stamps, pointer ID, owner, origin configuration, frozen rail and last
  current preview token/sample. Authoring, published drag and direct numeric edits use the same
  Current-only transaction. Invalid release, cancellation, stale/exhausted work, foreign or
  second pointers and camera cancellation publish nothing and create no history.
- Stable model-space DTOs/actions describe the one central radius grip/spoke/rail, typed named-
  contact metadata, solid retained-direction arrows, outlined alternatives and dashed
  complementary/local previews. Named-contact state and its continuation seam remain headless;
  there is no endpoint contact dot, canvas hit zone or compact-panel contact control. Canvas hover,
  canvas click and the compact accessible panel use the same branch-action IDs, applicability and
  disabled reasons.
- Every advertised branch action survives exact replacement and complete cloned-feature-document
  evaluation. A locally resolvable but composition-invalid action is omitted. A validated visible
  arrow outranks an overlapping Fillet radius surface, the visible central grip remains
  authoritative where it covers an arrow, and the generated arc/radius surface outranks native
  support. Hover and click resolve the same unique headless-nearest action independently of SVG
  paint order.
- Full-period closed parents retain their complete native presentation and expose no meaningless
  retained-direction action. Arcs, bounded/open parents and explicitly open periodic views retain
  source-fragment trimming.
- Painted SVG identity remains a hint. Independent exact owner, provenance and model-space
  proximity checks preserve `M66-PF004`; stale/foreign painted owners cannot mutate selection,
  authoring, preview or history.
- The workbench captures and releases the initiating pointer for point, Fillet and pan gestures.
  Release/cancel outside the SVG cannot strand interaction. A camera change cancels/restores a
  live Fillet manipulation first, while pan/zoom remain usable during collection and inspection.
  Rendering, accessibility, overlay layout and browser-default suppression remain thin-adapter
  concerns with direct Rust/WASM presentation tests, not a restored browser E2E suite. Automatically
  exposed problem detail is a bounded non-intercepting canvas overlay and cannot resize the canvas
  or change pointer-to-model mapping during a gesture.
- One friendly line-circle specimen supports ordinary manipulation away from a fold, and the
  existing radius-`0.5` fold configuration remains a distinct stress specimen. Both are ordinary
  editable save-like scenes with no guide, protected state or alternate coordinator.
- Direct feature tests cover rail derivatives, same-branch continuation and bounded alternatives;
  direct editor tests exhaust pointer/action matrices, sampling/zoom invariance, invalid recovery,
  authoring/published/numeric parity, one-step history and Undo/Redo/reload. A bounded transition
  model proves no unaccepted preview can publish or survive cancellation.
- Every feature-edit test asserts unchanged native sketch identity, coordinates, independently
  validated residuals, numerical rank and DOF. `M66-PF001` through `M66-PF004`, M27/M28/M30/M58
  compatibility and the independent solver success contract remain green.
- Formatting, warnings-denied Clippy, locked all-feature workspace tests, WASM, rustdoc,
  benchmark/licence/package checks, release Trunk, static single-workbench inventory and Git
  hygiene pass on one nominated candidate.
- A fresh release candidate is served through Tailscale and the supervising human explicitly
  approves every area in `docs/M68_UAT.md`.

The 2026-08-09 close decision accepts M68-U1 through M68-U6 and resolved findings `M68-F001`
through `M68-F005` with no new blocker recorded. It records explicit approval of frozen candidate
`edffb8a` without inventing a separate exhaustive replay of every scripted step. M68 is closed.

## M69 acceptance: Profile and construction geometry semantics

Status: complete and explicitly approved by the supervising human on 2026-08-09. ADR 0033's
implementation, focused direct qualification, complete release qualification, frozen candidate
publication and focused human UAT all pass.

- Persistent Construction remains ordinary solver-active, constrainable curve geometry and is
  excluded only by the existing default Profile/topology scope.
- Atomic role authoring/conversion changes no accepted coordinates, residuals, rank, DOF or branch
  state and records one history step for any selected curve batch.
- Successful open-parent Fillets publish finite, contained, non-overlapping discarded complements
  as a separate implicit-construction collection with exact native source/corner/endpoint
  provenance. They never become effective edges, persistent entities or independent constraint
  operands.
- Full-period parents, failed/suppressed/conflicting features and stale/interrupted/invalid output
  publish no discarded construction fragments.
- An implicit-fragment hit returns the full native curve identity and picked parameter. Selection,
  hover, role editing, Delete, constraints and dimensions therefore address that complete curve.
- `All`, `Profile` and `Construction` scopes and explicit/implicit visibility are headless and
  apply consistently to hover, selection, dragging, snapping and ordinary/Fillet authoring.
  Profile wins a cross-role overlap within one CSS pixel without making Construction inaccessible.
- Existing workspace v4 round-trips persistent roles through its canonical-v4/draft-v5 choice;
  M69 adds no sketch, feature or workspace persistence version.
- Focused native/WASM owners, the complete release gate and explicit supervising-human approval of
  `docs/M69_UAT.md` pass before closure.

The 2026-08-09 close decision accepts M69-U1 through M69-U5 with no new finding or blocker
recorded. It records explicit approval of frozen candidate
`567141776c78178022f6123cbb399599ba713c62` without inventing a separate exhaustive replay of every
scripted step. M69 is closed. M70 subsequently completed the acceptance criteria below and is also
closed.

M69 explicitly excludes persistent point roles, canonical sketch v5, workspace migration,
marquee/cycling/search additions, Offset/Mirror UI, computed-on-computed chaining, Bake/Explode,
computed-feature production-topology consumption, new residuals, browser E2E, mobile behavior and
legacy UI.

## M70 acceptance: headless auto-constraint drafting intelligence

Status: complete and explicitly approved by the supervising human on 2026-08-10. Implementation,
focused direct qualification, integrated release qualification, frozen replacement-candidate
publication, served-byte verification and scoped human UAT all pass. `M70-F001` is resolved.

M70 is accepted only when all of the following are true:

- `geosolve-constraint-editor` owns semantic anchors, stage-local wake/reference memory, candidate
  generation/ranking, hysteresis, guide and adjusted-preview DTOs, suppression, commit/cancel
  consequences and retained replay. No browser/native host reconstructs those decisions.
- Validated policy independently controls guide publication, coordinate adjustment and durable
  relation creation per inference family where those choices are semantically coherent.
  Persistent-point identity reuse cannot persist without adjustment because it is structural
  operand reuse, not a solver relation; that invalid combination is rejected. Default inclusive
  enter/leave thresholds are `8/12 px`
  for points/midpoints, `10/14 px` for curves and `4/6 degrees` for directions. Configured policy
  has hard ceilings of 32 candidates and eight remembered references; default scene-query bounds
  are 4,096 semantic anchors and 16,384 tessellation chords.
- Candidate generation stops as soon as the first unique bundle proves the configured candidate
  bound insufficient. It reports the first proven lower bound, returns raw unadjusted coordinates
  with no candidate/guide prefix and acquires no wake state. A scene-anchor/chord bound likewise
  returns one typed scene-limit result rather than a truncated semantic prefix. The complete
  derived candidate, guide, reference, ranking and raw/adjusted screen/model output is independently
  finite-validated before any identities or state publish; derived overflow is `InvalidFrame` and
  leaves the engine transactionally unchanged.
- Every `ConstructionPoint`-backed stage receives positional inference. Directional inference is
  limited to real newly authored line/polyline spans; raw-coordinate-only stages retain their
  existing behavior.
- Existing persistent-point inference reuses that exact point identity. It creates neither a
  duplicate point nor a redundant Coincident source. Reuse inside another construction is encoded
  directly in its point operand; a standalone Point-tool confirmation of an already-existing point
  emits no construction plan and is a history-neutral no-op.
- The Circle circumference stage is a radius sample, not a point operand. Within point tolerance of
  an existing persistent point, including a line endpoint, it proposes **Circle through point** and
  atomically commits PointOnCurve(existing point, created circle). It creates no hidden rim point;
  semantic midpoints and arbitrary line interiors are ineligible, and no contact or tangency is
  inferred from a line interior.
- PointOnCurve supports native line, circle/arc, Bezier, conic, B-spline and NURBS families and
  carries explicit span, accepted parameter/domain, winding and contact neighbourhood metadata.
- A semantic line/polyline midpoint ranks ahead of generic PointOnCurve and commits the existing
  Midpoint relation. A compatible midpoint-plus-perpendicular bundle may commit together.
- Near-Horizontal/Vertical applies to new line and each live polyline span. A remembered native
  line/polyline span supports later Parallel/Perpendicular. Bare-point H/V is explicitly
  `TrackingOnly`: it neither adjusts nor persists a relation by default and is never emulated by a
  fixed coordinate, zero dimension or hidden geometry.
- Ranking is deterministic and lexicographic: applicable constraint-backed before tracking-only;
  persistent point before Midpoint before PointOnCurve; remembered Parallel/Perpendicular before
  equivalent world-axis direction; then ADR 0033 Profile/Construction priority and geometric
  error. Persistent IDs stabilize output order only; an otherwise exact tie is Ambiguous and
  cannot auto-commit.
- At most one positional and one compatible directional inference participates in a stage bundle.
  Multi-stage tools retain confirmed earlier-stage relations until their one final construction
  transaction.
- Wake is immediate and timer-free. Reference state clears after the current stage click, cancel,
  tool exit, mutation, Undo/Redo, reload, viewport/policy change or stale identity and is never
  serialized. Only eligible reusable affine spans consume bounded reference capacity; nonlinear
  contacts do not evict them, and geometry role/scope priority matches ordinary headless picking.
- Semantic suppression acquires no reference, clears active latches/guides and cannot commit a
  stale candidate. A suppressed click places the raw stage; releasing suppression recomputes from
  the current sample. Rust does not hard-code a keyboard key.
- The ordinary placement click explicitly confirms exactly the visible candidate bundle; there is
  no second Apply step. If no candidate is active, the construction remains geometry-only.
- Typed draft point/span slots allow relations to refer to persistent operands and geometry
  allocated by the same `ConstructionProposal`. The complete construction, role, contacts and
  inferred constraints form one `ConstructionCommitPlan`; direct geometry-only apply remains a
  supported compatibility path. One plan contains at most 32 inferred relations, and each
  relation is charged to the caller-controlled operation before publication.
- The retained coordinator evaluates the plan on a clone, solves once, requires fresh independent
  acceptance and rejects any newly fully or partially redundant inferred source. Only the exact
  displayed plan may publish; staleness, ambiguity, conflict, cancellation or exhaustion never
  falls through to another relation.
- Publication authority originates only from a scene authenticated against the retained session's
  exact current accepted document, design filter and `PreparedSketchInput`; caller-assembled
  document/revision/stamp combinations cannot grant it. The terminal transition retains one commit
  token, frozen plan and that exact input. Dispatch authenticates all three and rechecks the
  complete input against the live session before mutation. Compatibility/render-only scenes may
  display inference but cannot emit an inferred construction plan. A private exact seal covers the
  accepted revision, design identity, viewport, native inference curves and construction snap
  anchors: mutation before binding rejects authentication and mutation after binding revokes
  publication authority.
- Rejection leaves live document/history unchanged and the exact draft/last preview available for
  correction. Success produces one history/replay checkpoint; one Undo/Redo removes/restores the
  construction and all inferred relations together. Undo may remove those objects but never
  rewinds the persistent-object or spline-span allocator high-water; Redo, reload and divergent
  history cannot reuse retired identities. Restoring history preserves the current exact parameter
  batch and external snapshot set rather than silently reverting to defaults. Application
  workspace v5 stores the field-opaque checkpoint value, validates that it belongs to and covers
  both stored graphs, bounds and streaming-decodes spline cursor entries, and rejects inconsistent
  object/curve/span cursor relationships. A collision-free process-local epoch makes
  allocator-only retention stale to prepared CAS and distinguishes independent restored session
  incarnations. Workspace v5 strictly migrates v1-v4 by deriving their graph-visible maxima.
  Frozen sketch v1-v4 bytes and current unsupported draft-v5 bytes remain unchanged.
- ADR 0033 Profile/Construction scope, visibility, one-pixel overlap priority and implicit
  Fillet-discarded-to-native-source mapping remain unchanged. Computed Fillet arcs are not anchors.
- Direct Rust tests cover exact hysteresis boundaries, order/zoom/scale invariance, non-finite
  rejection, resource limits, every construction stage, every native curve family, line/polyline
  directions, reference lifecycle, bundles, ambiguity, suppression, stale/rejected/exhausted work,
  exact scene-publication authentication, atomic allocation/publication, Circle reverse-incidence
  without a hidden rim point or line-interior fallback, and deterministic Undo/Redo/reload/replay.
  Replacement source `3d157896c87eaf647abee1192c838100ce359ce9` passes the focused inference
  selection exactly 47/47 and 271/271 editor unit tests. Its named integration suites pass M55 17/17,
  M66 feature authoring 14/14, M66 feature authoring matrix 15/15, M69 geometry semantics 10/10
  and native M70 transition parity 1/1, without inventing one aggregate integration-suite count.
  Demo-web passes 83/83 tests, the sketch library passes 33/33 unit tests, and its M56
  prepared-work suite passes 6/6.
- Native and WASM match the shared golden transition oracle in
  `crates/geosolve-constraint-editor/tests/m70_transition_parity.rs` and
  `crates/geosolve-constraint-editor/tests/fixtures/m70_transition_parity.golden.txt`. It covers
  every M70 inference family, tracking, ambiguity, suppression/release, stale/clear lifecycle,
  atomic publication, rejection, Undo/Redo and reload. Direct workbench tests prove semantic
  suppression translation, Shift/RAF ownership without losing queued terminal projected-drag
  samples, accessible guide/glyph rendering and absence of browser-owned ranking or geometry
  calculation; no browser E2E, CDP or legacy route returns.
- One ordinary editable **Samples → Constraints & dimensions → Auto-constraint drafting
  playground** covers point reuse, curve contact, midpoint/normal, H/V, remembered parallel/
  perpendicular, suppression, ambiguity, role/scope, zoom, Undo/Redo and reload without a guide,
  protected state or alternate coordinator.
- Formatting, warnings-denied Clippy, locked all-feature workspace tests, WASM, rustdoc,
  benchmark/licence/package checks, release Trunk, static single-workbench inventory and Git
  hygiene pass on one nominated candidate. Its release distribution is byte-verified over
  Tailscale before human review. Any objective UAT repair repeats those gates on a replacement
  candidate before targeted human recheck.
- The supervising human explicitly approves every area in `docs/M70_UAT.md`; objective findings
  receive direct owning-layer regressions before any targeted recheck.

M70 adds no residual, persistent relation kind, inferred-state persistence, hidden construction
geometry, canonical sketch-schema migration, global root search or browser-owned branch policy.
Its application-workspace v5 migration is limited to host-owned identity high-water. Equality,
symmetry, concentric/quadrant, certified intersection/collinear/extension, nonlinear
tangent/normal, grid/axis, angle increment and durable arbitrary point-pair H/V inference remain
outside scope.
`M70-F001` has direct owning-layer regressions, replacement release/publication evidence and an
accepted targeted M70-U1 human recheck. The 2026-08-10 scoped close decision accepts M70-U1 through
M70-U5 without inventing an unrecorded exhaustive replay of every scripted step. M70 is closed.

## M70B acceptance: workspace reproduction handoff

Status: active during human UAT. The reproduction transport criteria remain qualified;
`M70B-F001` and `M70B-F002` retain complete replacement evidence. The historical M70B-H1/H2
continue-through-failure baseline records 193/193 passing constraint/dimension-authoring and
reachable scene-authority rows. Subsequent UAT opened `M70B-F003` in computed-Fillet authoring and
`M70B-F004` in persisted computed-feature branch traversal, dimensions absent from that baseline.
M70B-H3 historically preserved the original 193 rows byte-for-byte and appended four process-
isolated reviewed `DEFECT` rows for those findings, yielding 197 total rows. Its exact `--check`
passed while `--require-clean` intentionally failed at the test-only checkpoint. Production repair
was subsequently authorized: active explicit Coincident equivalence now owns closure-Fillet
topology, and persisted circular-plus-affine Fillets may search their complete certified explicit
tangent-orientation cell without weakening generic nonlinear or radius-continuation guards. The
same four rows retain their input fingerprints and now make the reviewed fixture 197/197 `PASS`,
SHA-256 `035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`.
Focused owner qualification, both exact golden modes, formatting, warnings-denied workspace
Clippy, locked all-feature workspace tests and the relevant WASM build pass. Clean release-
candidate nomination/publication, human review and approval remain pending.

- Copy builds a fresh deterministic application-workspace v5 snapshot from the current retained
  coordinator. It does not expose raw `localStorage`, a backup storage key, the deleted
  `GEOSOLVE_SCENE_V1` format or a second geometry/persistence schema.
- The text is one canonical `GEOSOLVE_REPRO_V1` envelope containing the exact decoded byte length,
  one zlib stream, strict unpadded URL-safe base64 and a lowercase 64-bit FNV-1a corruption
  checksum. The checksum is described only as accidental-corruption detection, never
  authentication or trust evidence.
- Encoding and decoding fail closed at independent limits of 16 MiB complete text, 12 MiB
  compressed body and 64 MiB decoded workspace. Unsupported version/codec, noncanonical decimal or
  checksum text, padded/noncanonical base64, corrupt/truncated/trailing zlib input, length mismatch,
  invalid UTF-8 and checksum mismatch return typed failures without unbounded inflation.
- Capsule decoding yields only opaque workspace JSON. `WorkspaceSnapshot::decode` must then apply
  the existing strict version/schema/migration checks, and a complete validated coordinator must be
  reconstructed before the sole live workbench is replaced. Failure at any layer retains the exact
  current coordinator, accepted scene and persisted workspace.
- A native stdin/stdout decoder can expose the bounded decoded workspace JSON to a recipient for
  diagnosis without reading browser storage or acquiring any validation/publication authority.
- Exact successful restore includes current design and accepted document payloads, accepted-current
  provenance, computed-feature intent, sketch/feature/evaluation allocator high-water and lifecycle
  revisions already owned by workspace v5. It deliberately excludes transient authoring,
  pointer/hover/selection state, camera, sample identity/guidance and native command-history cursor.
- The ordinary workbench exposes one accessible canvas-adjacent copy/paste overlay. The full text is
  always inspectable; automatic clipboard denial leaves it selected for manual copy. Paste errors
  remain visible without resizing the canvas or mutating geometry.
- Direct Rust tests own deterministic codec bytes, representative and bound-edge workspaces,
  strict malformed/corrupt/oversized rejection, complete computed-Fillet v5 round-trip and atomic
  retention after transport, workspace and coordinator failures. The same codec path must compile
  for WASM. No browser E2E or legacy route returns.
- The historical manually administered M70B-H1 oracle exhaustively inventories all sixteen
  `ResolvedConstraintKind` and five `DimensionKind` families. Every family has one deterministic
  case and eight fixed-seed variants that schedule span reversal, operand reversal and
  perturbed-recovery geometry while varying finite transform/contact input. Dimension variants
  include creation, one target edit, Undo and Redo. Each accepted row independently verifies
  finite current publication, hard validity and normalized hard residual at most `1e-9`, exact
  typed definition/branch metadata and a public geometric postcondition.
- The same oracle covers only the four actually reachable scene-authority states: current empty
  computed, current computed Fillet, current native fallback while computed output is Withheld and
  detached historical accepted presentation beneath rejected design. It verifies visible problem
  metadata and exact authentication allow/deny behavior without manufacturing an unreachable
  `ComputedSceneState::Absent` coordinator.
- Every authoring, feature and scene row runs in a separately bounded process and later rows
  continue after semantic defects, panics, timeout/hard-kill exits or harness errors. Its stable
  six-column TSV freezes the effective scheduled input fingerprint and rejects
  duplicate/unclassified rows; the checked golden must match exactly and `--require-clean` must
  fail if any row is not `PASS`.
  Dimension rows independently compare accepted measurements, target metadata and display units
  across create/edit/Undo/Redo. Endpoint-continuity rows verify path-oriented G2 curvature and
  rate-explicit Parametric-C2 derivatives, including a pre-satisfied unequal-rate witness.
  `docs/M70B_HARDENING.md` records the seed, exact commands and readable checklist. The historical
  H1/H2 baseline contains exactly 193 `PASS` rows; its scope did not include computed-Fillet operand
  collection or source-edit branch traversal and therefore did not gate later `M70B-F003` or
  `M70B-F004`.
- M70B-H2 moves those exact rows and golden bytes to milestone-neutral test/fixture/driver names,
  accepts finding IDs from later active milestones, makes the clean oracle mandatory inside the
  release gate and installs the implicitly invoked repository defect-hardening skill. The original
  H1 SHA-256 and UAT bytes remain unchanged and no compatibility alias survives.
- M70B-H3 appended exactly four process-isolated `feature.fillet` rows: F003 Coincident-closure
  point and curve-pair authoring plus F004 line-circle same-cell winding-zero and seam-winding-one
  evaluation. All original 193 rows remain byte-identical. The historical test-only inventory was
  exactly 193 `PASS` plus four `DEFECT`; its passing `--check` proved checklist stability rather
  than release readiness, and `--require-clean` deliberately remained red. The repaired inventory
  keeps the same 197 case IDs and exact input fingerprints while all rows are reviewed `PASS`.
  Both exact golden modes now pass. Current fixture bytes alone do not satisfy final milestone
  acceptance: the complete clean replacement-candidate gate must also pass on the repaired source.
- Circle/arc radial Normal is explicitly centre-on-complete-supporting-line incidence. Compact
  authoring ignores the arbitrary curve-click parameter, seeds the unique affine projection from
  compatible retained accepted geometry, persists SupportingLine/Interior metadata and rejects
  bounded/local segment-containment metadata before retained mutation. It never seeds from newer
  rejected design coordinates. Both operand orders, circle/arc supports and a centre outside the
  finite segment remain independently accepted.
- A newer rejected design never erases its historical accepted canvas. The workbench may render
  that accepted document as a detached presentation scene, but it cannot bind the scene to current
  `PreparedSketchInput` or use it to publish inferred construction. Current computed output still
  fails closed rather than falling back to an authenticated native scene. Attempted/non-finite
  geometry remains non-authoritative and unpainted.
- Formatting, warnings-denied Clippy, locked all-feature workspace tests, WASM, rustdoc,
  benchmark/licence/package checks, release Trunk, single-workbench inventory and Git hygiene pass
  on one nominated source. After any test-hardening cut, its fresh frozen distribution is
  published and byte-verified before UAT.
- The supervising human completes and explicitly approves every area in `docs/M70B_UAT.md`.

M70B adds no residual, solver state, sketch constraint, branch kind, canonical sketch schema,
sample fixture or general file format. `M70B-F001` corrects how the pre-existing open Local branch
rule is represented by effective closed core bounds; it preserves the persisted branch interval
and strict independent validation. The payload is a versioned diagnostic interchange around the
application workspace and cannot publish merely because its transport checksum is valid.

`M70B-F001` acceptance additionally requires the payload-derived retained-editor regression to
reach every recorded drag target with independently valid residuals and unchanged equality
mobility, plus direct sketch evidence that effective Local bounds are strictly inside unchanged
semantic branch metadata. The replacement candidate must repeat the complete release/publication
gate before targeted human recheck.

`M70B-F002` acceptance additionally requires the supplied payload geometry to author radial Normal
through `AuthoringState -> RetainedEditorCoordinator`, publish finite independently hard-valid
geometry at normalized residual `<= 1e-9`, retain positive-radius and explicit supporting-line
semantics, and reject former bounded/Local requests without advancing design, attempt, accepted,
history or transcript identity. A thin workbench matrix must retain accepted SVG paths beneath an
unrelated rejected constraint
while proving the detached scene cannot acquire inference-publication authority, and must preserve
the current computed Fillet-preview path as exact-stamped composite geometry. Its replacement
candidate must repeat the complete release/publication gate before targeted human recheck.

The historical test-only `M70B-F003` characterization preserved the Coincident-closed open-
triangle topology, independently valid accepted geometry, both point and explicit two-span
rejection signatures, and transactional state retention. Its authorized repair is accepted at the
owning boundaries only when `SketchDocument` computes deterministic transitive representatives
from active explicit Coincident constraints, ignores suppressed relations and coordinate
proximity, and the Fillet collector uses those representatives for point incidence, same-polyline
pair eligibility and retained-endpoint hints. The positive regression must prove either coincident
closure point and both first/last-span operand orders produce one three-corner preview and publish
one Current FilletSet containing three arcs.

The historical test-only `M70B-F004` characterization preserves both supplied payload
fingerprints, finite independently hard-valid accepted sketches, exact persistent Fillet branch
metadata, the former `NoLocalRoot` states and independently viable roots strictly inside the same
certified circle cell. Its authorized repair is accepted only when both persisted evaluations are
Current without changing source, normal side, retained endpoint, endpoint order, sweep, cell or
winding. Searching a full persisted cell is permitted only for constant-curvature Circle/
CircularArc plus affine support, whose fixed-radius offset cannot fold within the certified
tangent-orientation cell. General nonlinear curves retain the narrow seed-connected guard and
radius continuation retains its fold/remote-root guard.

Historical M70B-H3 acceptance was test-only: all four Fillet rows executed in separate bounded
processes, retained stable case IDs/fingerprints and classified the two F003 plus two F004 rows as
reviewed `DEFECT`. The prior 193 H1/H2 rows remained byte-for-byte unchanged, producing exactly
197 rows with golden SHA-256
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`; `--check` passed and
`--require-clean` intentionally failed. That remains the preserved pre-repair evidence and H3
itself changed no production behavior.

Repair acceptance requires those same four rows to transition `DEFECT` to `PASS` without changing
their input fingerprints: curve-pair `input-d04adbf29c08b9bd`, point
`input-4ba571059db7afff`, lower same-cell `input-f9920c3cf170130d` and seam same-cell
`input-2da21ef04cfb4246`. The reviewed current fixture is exactly 197/197 `PASS`, SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. Focused F003 and F004 owner
qualification, exact golden `--check`/`--require-clean`, formatting, warnings-denied workspace
Clippy, locked all-feature workspace tests and the relevant WASM build pass. No full repaired-
candidate qualification is claimed until the clean release and publication gates also pass.

M71 remains deferred behind M70B.

`docs/M71_GOALS.md` is a deferred candidate backlog and confers no implementation authority.

### Superseded M66 solver-owned Fillet acceptance record

The mechanically qualified but unapproved ordinary-UI route through M28 is preserved with commit
`1034afc` at `origin/archive/m66-associative-fillet-2026-08-07`. Its `M66-F002` through
`M66-F013` tests remain compatibility evidence only. They do not satisfy active ADR 0031
acceptance, and their former human scorecard was never approved.

### M54: stable diagnostics

Status: complete (2026-07-29); direct evidence is recorded in `docs/M54_IMPLEMENTATION.md`.

- Hosts consume solve, source, component, dependency, activation, parameter and external-reference diagnostics through stable domain DTOs and persistent IDs.
- Structural/numerical rank, equality/bounded/one-sided mobility and diagnostic completeness remain separate.
- Machine-readable repair suggestions never mutate state automatically or claim globally minimal conflict proof.

### M55: alpha constraint, dimension and branch-action parity

Status: complete (2026-07-29); direct evidence is recorded in
`docs/M55_IMPLEMENTATION.md`.

- The headless editor and sole workbench expose the complete preserved M13-M14 alpha action matrix:
  fixed, coincident, horizontal, vertical, point-on-curve, parallel, perpendicular, equal-length,
  equal-radius, midpoint, symmetry, generic contact and generic tangency.
- Distance, segment-length, radius, diameter and oriented-angle dimensions support every applicable
  driving/reference form through typed public document edits.
- Tangent orientation, contact neighborhood, parameter domain, span and winding are explicit typed
  choices. No UI coordinate heuristic silently selects or changes a branch.
- Action availability and disabled reasons are headless DTOs; the web layer renders and dispatches
  them without private equations, applicability rules or core-report interpretation.
- Native editor/coordinator, direct presentation and WASM-adapter matrices cover successful,
  incompatible, redundant and rejected actions, including accepted-state retention and recovery.
- Reusable scenarios demonstrate each family without restoring the playground, `/#/dev/lab`, CDP,
  browser E2E or legacy harnesses. Mobile and complete advanced-curve authoring remain outside M55.

Completion record (2026-07-29): the headless editor publishes and executes one deterministic
13-relation/five-dimension matrix with typed selection applicability, explicit construction
choices, complete persistent contact branch edits and oriented-angle direction changes. All five
dimension identities execute in driving and reference modes. Accepted contact span/domain changes
and retained rejected tangent-orientation/winding candidates preserve persistent contact/scalar
identity, retain the prior accepted state and recover through bounded Undo.

The sole workbench consumes a reusable action identity catalog, headless disabled reasons and
branch DTOs; its glyphs and dimensions carry semantic kind metadata. The nested scenario catalog
adds `alpha-parity-catalog` and `alpha-branch-recovery` without changing ordinary workspace
persistence or restoring any retired route/harness. Focused native editor, sketch and presentation
tests, full locked workspace tests, warnings-denied Clippy, all-feature WASM and release Trunk pass.
No browser equation, branch heuristic, browser E2E or human-approval claim is part of this gate.

M55 contextual-authoring follow-up acceptance:

- The visible relation vocabulary is Lock, Coincident, Horizontal, Vertical, Parallel,
  Perpendicular, Equal, Midpoint, Symmetric, Tangent and Continuity.
- The headless editor publishes the exact persistent definition resolved for the current typed
  selection. Coincident covers point/point, point/curve and curve/curve contact; Equal covers
  line length, circular radius and branch-explicit curve curvature.
- Parallel and Perpendicular accept two lines, or one line and one regular curve with explicit
  tangent-orientation or normal-side state. Arbitrary curve-pair direction is disabled rather than
  emulated.
- Tangency, curvature and continuity retain complete explicit contact/span/domain/winding/
  neighborhood/orientation/order/rate state. No coordinate heuristic selects a discrete branch.
- Semantic-catalog-only relations remain unavailable through the retained coordinator until their
  persistence, history, prepared-input and workspace-schema ownership is frozen.
- Direct native, presentation and WASM qualification replaces the former action identities
  without restoring a legacy application or harness.

### M56: prepared jobs and concurrency

Status: complete (2026-07-29); direct evidence is recorded in
`docs/M56_IMPLEMENTATION.md`.

- Prepared jobs capture every relevant input revision and cannot mutate a session until compare-and-swap commit.
- Stale or cancelled work cannot overwrite a newer accepted state.
- The documented safe Rust ownership contract works for host-managed native workers and single-threaded WASM without `unsafe` code.

Completion record (2026-07-29): one immutable prepared stamp captures retained design, latest
attempt, accepted/high-water state, solve requests/policy, effective activation, parameter batch
and external snapshot identities. Typed edit, reattempt, parameter and external-input jobs execute
only on a captured session clone. Cancellation/work exhaustion returns no patch; completed patches
publish only through exact-stamp compare-and-swap. A moved native worker job, two out-of-order
patches, cancelled parameter work and non-default parameter/external revisions are directly
qualified. Session-bearing values are safely `Send` single-owner values; immutable metadata is
`Send + Sync`; all-feature WASM consumes the same API synchronously. No `unsafe`, lock-based solver
sharing, equation change or schema change was added.

### M57: incremental solving and scale

Status: complete (2026-07-29); direct evidence is recorded in
`docs/M57_IMPLEMENTATION.md`.

- Incremental and full-rebuild paths agree on geometry, validation, rank, branch and diagnostics.
- Parameter, reference, activation and local geometry edits dirty only their dependency closures without skipping fresh acceptance evidence.
- Published cold/warm/profile/memory/cancellation envelopes pass, and rank authority is either proved sparse or bounded honestly by a supported connected-component limit.

Completion record (2026-07-29): the retained document lifecycle preserves compatible runtime
mappings, compiled topology, core session state and component caches. A scratch compatibility
compile verifies exact runtime/equation/bound shape; changed persistent elements select transitive
source closures, and changed host parameter/external-reference payloads replace only their runtime
sources. Same-shape activation changes reuse every component; topology/source-shape changes report
and take a full rebuild. Every path still performs fresh hard-row, derivative, branch/domain,
projection and rank validation before publication. Persistent runtime joins are indexed, bounded
profile results are cached only inside one accepted revision, and execution/rank-envelope evidence
is sketch-owned. Sparse hard steps remain supported, while numerical rank is honestly dense-SVD
authoritative only for connected components at or below 256 active rows and tangent coordinates.
Ten direct regressions cover fresh parity, two- and sixteen-component locality, parameter,
external reference, activation, topology and changed-incidence fallback, profile invalidation,
storage bounds and work-exhaustion rollback.

### M58: sketch operations companion

- The companion owns no residual equations and emits deterministic public sketch transactions with identity-preserving mappings.
- Split, trim, extend, mirror, chamfer and baseline macro/pattern operations are transactional and dependency-aware.
- Multiple visible intervals never rewrite immutable support definitions or infer topology by proximity.

Completion record (2026-07-29): `geosolve-sketch-ops` prepares controlled immutable proposals
from the complete retained-design/attempt/accepted input stamp and applies them only after exact
compare-and-swap through `RetainedSketchDocumentSession::transact`. Split, break, trim, line
extension, exact point-defined mirror, line chamfer, the existing public generic-fillet command,
rectangle, regular polygon, slot and bounded linear pattern all emit ordinary public document
state plus explicit identity disposition. Unsupported exact families and incomplete accepted
geometry return typed outcomes; no curve is sampled into an approximation.

Multi-interval visibility retains immutable support definitions, validates canonical traversal
order and non-overlap, uses exact fixed/contact boundary identities in visual profiles and freezes
constraint-owned boundaries before owner deletion. Canonical sketch v4 rejects this new state;
the hidden draft-v5 bridge remains explicitly unsupported pending a future schema-freeze
decision. Direct regressions prove deterministic
proposal mappings, stale/cancelled/exhausted/foreign-input atomicity, finite/resource bounds,
profile closure and companion dependency isolation. Native, full-workspace, WASM and release
qualification pass without a private residual, browser harness or restored legacy route.

### M59: production topology companion

- Complete output publishes revision-stamped wires, nesting, holes, orientation and exact source provenance for the declared scope.
- Tangency, overlap, touching, T-junction and self-intersection policies are explicit and bounded.
- Stale, cancelled, truncated or ambiguous output cannot be consumed as a production profile and never changes solver state.

Completion record (2026-07-29): `geosolve-sketch-topology` captures only the current
independently accepted state for the complete retained design/host input stamp. Queries declare
profile/construction and immutable external-line scope, all ambiguity policies and deterministic
candidate/output limits. Visual analysis is bounded candidate evidence, not promoted output:
eligible-source coverage, exact interval/domain provenance, parameter enclosures, fresh endpoints,
closure, signed-area orientation and output counts are independently checked before a
`TopologyProductionProfile` exists.

Complete profiles expose oriented wires and outer/hole regions with exact native visible-interval
or external binding/revision/digest/domain provenance, and must revalidate exact live-session input
before host consumption. Cancelled/work-exhausted operations and truncated/skipped topology never
carry consumable production output or mutate solve state. Fifteen direct cases cover complete,
ambiguous, bounded, stale, worker-movable, external/construction and M58 multi-interval behavior;
focused compatibility, full workspace, WASM and release Trunk qualification pass.

### M60: advanced workbench and direct qualification

Status: complete (2026-07-29); direct evidence is recorded in
`docs/M60_IMPLEMENTATION.md`.

- The M55 alpha action surface remains complete while advanced geometry and companion workflows are
  added.
- The one CAD-like desktop consumer covers advanced curves, branches, operations, diagnostics, production profiles, persistence and cancellation without private equations.
- Direct editor, presentation, persistence and WASM-adapter suites qualify every objective claim; no old playground or browser E2E returns.
- The generated UAT evidence package is deterministic; no mobile behavior is claimed.

Completion record (2026-07-29): the sole workbench directly consumes public sketch, editor,
operations and production-topology APIs. Four stable M61-ready leaves present accepted all-family
geometry, explicit periodic NURBS span/winding and knot edits, associative fillet state plus public
split/mirror/pattern proposals, and complete/incomplete/cancelled/recovered production topology.
Only complete `TopologyProductionProfile` output is labelled consumable. The ten existing M53/M55
scenario identities, complete M55 action surface and ordinary-workspace isolation remain
unchanged.

The version-2 desktop workspace envelope records explicit canonical-v4 or draft-v5 payload
encoding, round-trips M58 multi-interval state and migrates legacy workspace v1. Malformed,
unknown-version, unknown-field and unknown-encoding payloads reject. Focused editor, workbench,
operations and topology suites, warnings-denied Clippy, all relevant WASM checks, release Trunk and
the complete workspace gate pass. No equation, branch heuristic, B-rep state, browser E2E,
`/#/dev/lab` route or mobile claim was added.

### M61: human UAT 3

- Status: complete and explicitly approved by the supervising human on 2026-07-29 for the scope
  recorded in `docs/M61_UAT.md`. The original candidate was withdrawn after five UAT blockers;
  the replacement and findings `M61-F001` through `M61-F005` were directly requalified before
  closure.
- Ten representative public alpha mechanisms expose documented nonzero equality/bounded mobility,
  preselect one persistent driver, accept projected drag through the active ephemeral coordinator,
  reset exactly and never mutate ordinary workspace persistence.
- Twin-roller cam projected drag supplies the opposite roller as a headless transient stability
  target in both directions; repeated previews retain the passive accepted center within `1e-9`.
- Recursive desktop scenario branches expand to the right through the compact/linkage third level
  without flyout clipping or nested scroll overflow.
- Wheel zoom is cursor-anchored; middle-drag pan, explicit zoom controls and Fit remain available
  in ordinary and scenario modes without changing solver state.
- The sole workbench authors quadratic/cubic Beziers, ellipse/elliptical arc, rational quadratic
  conic, parabola, hyperbola and clamped/periodic NURBS through reusable headless proposals.
- Conic/NURBS options, rational gauge state and terminal topology validate atomically; advanced
  previews sample only public domain curve evaluation and contain no browser equation.
- The supervising human completed the advanced geometry/topology review and approved its scoped
  outcome.
- Advanced controls, branch transitions, associated operations, topology claims and representative interaction performance are understandable and trustworthy.
- No unresolved wrong-branch, misleading-profile, advanced-interaction or responsiveness blocker remains.

### M62: CAD-style constraint and dimension authoring

- A public presentation-independent authoring state machine consumes explicit immutable selection
  snapshots and picks; it never reads or owns application selection.
- A compatible non-empty selection applies once and returns to Select with selection preserved. An
  incompatible non-empty selection returns a typed warning without mutation. An empty selection
  enters a persistent authoring mode.
- One-operand tools apply on each pick; pair tools apply after two valid picks and remain active;
  Symmetric collects point, point and axis. Role-distinct operands normalize safely, while
  continuity and oriented-angle order remains explicit.
- Pending operands are not ordinary selection. The first Escape clears them and the second exits
  mode. External topology changes remove stale operands without panics or accidental retargeting.
- Accepted and retained-rejected transactions, plus terminal coordinator errors, clear the
  completed operand set and keep authoring active. Pre-application validation warnings retain the
  valid pending prefix for correction.
- The desktop workbench has one two-column left palette for geometry, eleven contextual relations
  and five dimensions. Constraint and dimension creation dropdowns and Apply buttons no longer
  exist in the inspector.
- Flyout options are remembered only for the current process and default to aligned tangency,
  signed equal curvature, G1 continuity, parametric-C2 rates 1/1, Driving dimensions and
  counter-clockwise angles.
- Contact domain, parameter, neighborhood, winding and tangent-orientation choices are emitted
  only for resolved definitions that own contact state. Simple line and radius relations carry no
  hidden contact metadata.
- Repeated occurrences of the same curve span retain their own picked parameters in operand order.
  Endpoint continuity pairs each endpoint parameter with its matching Start/End neighborhood.
  Direct metadata and accepted-transaction regressions cover every resolved relation family.
- A separate accepted-transaction matrix covers all five dimension authoring families without
  routing any of them through relation contact metadata.
- The complete relation and dimension matrices produce the same applications from compatible
  preselection and repeated collection, and every terminal attempt re-arms the active tool.
- Point-on-curve authoring retains exact picks across representative line, circle, Bezier and
  NURBS families. Bounded endpoint parameters default to their matching Start/End neighborhood.
- Retained-rejected authoring can be undone and retried within the active tool; dimension target
  edits pass Undo/Redo; process-local options survive tool re-entry and fresh state uses defaults.
- Canvas and tree operands use the same headless input path. Ordinary selection and point dragging
  are suppressed during authoring; pending operands and the expected next operand are visibly
  identified. Canvas pointer-down exclusively owns the parameter-bearing canvas pick; its later
  bubbled click cannot contribute the same item again, while tree clicks contribute once.
- Dimensions are created at the current independently accepted value, never at a potentially
  divergent retained-design seed. Line-angle authoring uses the acute supporting-line intersection
  angle for presentation, independent of invisible endpoint direction, while retaining its
  explicit directed solver branch. A selected dimension exposes model-unit or acute-degree target
  editing through its owned scalar, ordinary history and replay.
- Scenario mode remains read-only. No new scenario, schema, equation, residual, browser
  compatibility rule, E2E harness, mobile claim or `/#/dev/lab` route is introduced.
- Direct native state-machine/coordinator and workbench presentation tests, locked all-feature
  workspace tests, warnings-denied Clippy, all-feature WASM check and release Trunk build pass.
- The supervising human explicitly approved `docs/M62_UAT.md` in the ordinary workspace on
  2026-07-29.

### M63: canvas constraint visualization and interaction

- Accepted editor scenes expose typed constraint/dimension annotations with persistent identity,
  direct operands, finite geometry anchors, semantic kind, visibility policy and hit geometry.
- The headless editor owns separate geometry reveal context and exact pointer-proximity identity,
  including deterministic marker occurrence for multi-glyph constraints, plus annotation-first
  Select-mode picking. Constraint authoring continues to pick geometry only and one physical
  canvas click has one owner.
- Every angle dimension is always visible. Other driving dimensions are always visible;
  non-angle reference dimensions and constraint symbols appear through direct geometry context,
  their own selection/hover or targeted problem state.
- Selecting or hovering an annotation reveals and emphasizes all direct operands without adding
  them to the editable selection. Transit through a related icon set keeps siblings visible but
  highlights only the proximate occurrence; unrelated relation clusters remain hidden.
- Every persistent constraint and dimension family receives geometry-appropriate accepted-state
  placement. Shared glyph anchors use deterministic compact fan-out and leaders.
- The sole workbench renders one shared text-free CAD vector language across authoring and
  accepted canvas symbols, plus angle arcs, dimension/witness lines, values, focus, selection and
  problem states without reconstructing constraint semantics.
- Every geometry authoring tool has a distinct text-free vector symbol; sketch-tree object
  categories and canvas problem markers use representative vector symbols rather than placeholder
  letters, generic diamonds or SVG text. Genuine keyboard hints and camera controls remain
  textual.
- Three stable **M63 Canvas constraints** leaves directly exercise angle/dimension presentation,
  contextual relation symbols and crowded fan-out. Scenario state remains ephemeral.
- Direct native editor and workbench presentation tests, locked all-feature workspace tests,
  warnings-denied Clippy, WASM check and release Trunk build pass.
- The supervising human explicitly approved `docs/M63_UAT.md` on 2026-07-30.

### M64: editable sample library and harness cleanup

- The selector contains exactly 22 unique leaves under three one-level purpose groups and exposes
  no milestone-owned group names.
- Selecting a sample replaces the current coordinator, starts history at one checkpoint, fits the
  camera and saves through the ordinary versioned workspace path.
- A loaded sample is not a special mode: geometry/constraint/dimension authoring, selection,
  branch and target editing, Delete, Undo/Redo, zoom/pan and projected drag remain available.
- Every leaf independently constructs an accepted state and round-trips through
  `WorkspaceSnapshot`.
- Fixed constraints in samples can be deleted through the ordinary coordinator and restored by
  Undo.
- Four-bar coupler, pantograph and three-link drawing arm publish finite, independently validated
  hard geometry with maximum normalized hard residual `<= 1e-9`, scale-invariant IDs and
  respectively 1, 2 and 3 bidirectional degrees of freedom at `1e-6`, `1` and `1e6`.
- Generic projected drag preserves an unrelated passive freedom where feasible without
  browser/sample-specific metadata, leaves hard mobility unchanged and publishes no temporary
  target into the retained workspace request.
- Guided descriptions, scripted scenario actions, verification points, transcripts, evidence,
  reset/exit controls, alternate scenario coordinators and save suppression are absent.
- No browser E2E/CDP/server harness or `/#/dev/lab` route returns.
- Formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature WASM check
  and release Trunk build pass before UAT begins.
- `docs/M64_UAT.md` was explicitly approved by the supervising human on 2026-07-30.

## Regression and oracle policy

- Every convergence, rank, scaling, branch or diagnostic bug gets a minimal regression scenario.
- The milestone-neutral authoring/scene golden is the broad compatibility matrix, not the sole
  home for reported defects. Each reproduced defect first receives the smallest public
  owning-layer regression; the broad matrix expands only for a systemic missing family, branch,
  transform, operand-order, lifecycle or authority-state axis.
- Golden updates are reviewed input-and-classification changes, never automatic acceptance of new
  output. `--check` freezes the recorded checklist and `--require-clean` is the release closure
  gate. A passing `--check` with reviewed `DEFECT` rows proves checklist stability, not release
  readiness; the mandatory gate remains red until `--require-clean` passes.
- `.agents/skills/geosolve-harden-defect/` is the canonical intake, ownership, regression and
  qualification workflow for solver and headless-UX findings.
- Differential tests compare geometric validity, rank/mobility/status and branch continuity, not identical internal coordinates or iteration counts.
- SolveSpace and PlaneGCS are references/oracles, not dependencies.
- An external convergence flag is never accepted without local independent validation.
