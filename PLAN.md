# Implementation plan

Implement in order. Do not start a milestone until the preceding milestone's tests and acceptance gates pass.

The plan deliberately front-loads work that is difficult to implement but objectively verifiable. **M1–M4 form the autonomous overnight block.** The first human architecture/behavior review happens after M4, before domain models and interaction semantics are expanded.

## M0 — Repository and contracts

Status: complete as of 2026-07-13. Native format/test/clippy, WASM check, and release Trunk build all pass.

Deliverables:

- Cargo workspace and GPL-3.0-or-later licence;
- separate geometry, core, sketch, linkage and WASM demo crates;
- architecture, scenarios, references and acceptance documents;
- primitive Trunk/WASM/SVG page with static geometry and equation-audit templates;
- NixOS-friendly development shell.

Gate:

- root format, clippy and native tests pass;
- demo crate checks for `wasm32-unknown-unknown` and `trunk build --release` succeeds.

---

# Autonomous overnight block

## M1 — Problem representation, audit metadata and Jacobian verification

Goal: express small nonlinear equality systems without CAD/linkage semantics.

Status: complete as of 2026-07-13.

Implement in `geosolve-core`:

- [x] Stable stores for variable blocks, residual blocks and source constraint IDs.
- [x] Variable block kinds:
   - scalar;
   - `Vec2`;
   - `Pose2` with a three-dimensional local increment.
- [x] Packed state layout mapping stable IDs to ambient/tangent ranges.
- [x] Residual categories:
   - hard;
   - temporary interaction objective;
   - previous-state/minimum-motion preference.
- [x] Residual-block API declaring incident variables, output dimension, characteristic scales, evaluation and local Jacobian blocks.
- [x] Structured audit descriptors for each high-level source and generated residual row: readable template, named bindings, category, units and scale. Audit text is metadata, not executable input.
- [x] Deterministic dense residual/Jacobian assembly.
- [x] Central finite-difference Jacobian checker with per-block error reports.
- [x] Invalid-geometry and non-finite-value rejection before linear algebra.

Implementation guidance:

- begin with hand-written analytic Jacobians for synthetic residuals;
- keep APIs internal while possible;
- retain block incidence/deterministic ordering but do not add sparse matrices;
- do not add sketch/linkage entity variants to core.

Required fixtures:

- [x] scalar quadratic residual;
- [x] two-variable distance residual;
- [x] `Pose2` transformed-point residual;
- [x] mixed block dimensions and deterministic packing;
- [x] analytic versus central finite-difference Jacobians;
- [x] NaN/Inf and invalid scale rejection.

Gate: M1 section of `ACCEPTANCE.md`.

Completion notes: dense rows are normalized by residual scale and columns by
variable tangent step scale. Integration tests cover stable generational IDs,
all required fixtures, heterogeneous matrix ranges, audit metadata, invalid
IDs/dimensions/geometry, and finite-difference relative error `<= 1e-6`.

## M2 — Dense nonlinear solver, rank and strict validation

Goal: robustly solve and classify small connected nonlinear systems.

Status: complete as of 2026-07-13.

Implement:

- [x] Dimensionless residual and step scaling.
- [x] Damped Gauss-Newton or Levenberg-Marquardt with:
   - adaptive damping;
   - accepted/rejected steps;
   - actual versus predicted reduction;
   - block-local step limits;
   - iteration, stagnation and numerical-failure termination.
- [x] Dense QR/SVD solve fallback; never rely exclusively on `J^T J`.
- [x] Numerical rank and local nullity using a documented relative tolerance.
- [x] Independent hard-residual re-evaluation after iteration.
- [x] `SolveTermination` separate from underconstraint/redundancy/singularity diagnostics in `SolveReport`.
- [x] Deterministic trace records.
- [x] Accepted-state audit snapshots containing current bindings plus raw and normalized residual values.

Required fixtures:

- [x] exactly determined linear and nonlinear systems;
- [x] underdetermined circle point with one DOF;
- [x] duplicate residual rows;
- [x] contradictory equations;
- [x] configuration-dependent rank drop;
- [x] same problems at characteristic scales `1e-6`, `1`, and `1e6`.

Gate: M2 section of `ACCEPTANCE.md`.

Completion notes: LM solves an augmented dense least-squares system with QR
and SVD fallback, commits only accepted finite states, and re-evaluates hard
rows independently before reporting convergence. Hard rows alone define the
M2 objective, rank and DOF; temporary and preference rows are validated and
audited but intentionally excluded until a documented hierarchy exists. Tests
cover accepted/rejected trace accounting, explicit accepted-state audit values,
complete-source deterministic redundancy candidates and every required M2
classification at characteristic scales `1e-6`, `1`, and `1e6`.

## M3 — Adversarial numerical verification harness

Goal: make wrong solver implementations difficult to hide before any UI or domain complexity exists.

Status: complete as of 2026-07-13.

Implement tests/tools, not new product features:

1. [x] Property-based generation of small linear systems with known rank, nullity and exact solution.
2. [x] Construct-valid nonlinear systems, perturb their state, then verify recovery from documented local basins.
3. [x] Translation, rotation and uniform-scale metamorphic tests.
4. [x] Variable/residual insertion-order permutation tests proving deterministic results and source ordering.
5. [x] Jacobian checker coverage for every synthetic residual and variable block.
6. [x] Failure injection:
   - [x] non-finite residual;
   - [x] non-finite Jacobian;
   - [x] zero/negative characteristic scale;
   - [x] singular linear solve;
   - [x] rejected steps until stagnation;
   - [x] iteration limit.
7. [x] Independent validation oracle that re-evaluates hard residuals without reusing cached iteration vectors.
8. [x] Solve trace invariant checks: accepted cost does not increase beyond policy tolerance, rejected states are not committed, and last-valid finite state is retained.
9. [x] Benchmark harness for small dense systems, recorded but not used to weaken correctness gates.

Constraints:

- prefer test-only helpers over expanding public APIs;
- do not snapshot exact iteration counts unless mathematically required;
- random/property tests use recorded deterministic seeds on failure;
- no sketch, linkage, browser interaction or sparse backend work.

Gate: M3 section of `ACCEPTANCE.md`. All randomized/property tests must be reproducible.

Completion notes: rank-by-construction properties run 32 cases for each of
exact, underdetermined and overdetermined shapes with fixed ChaCha base seed
`4d33a7419c2e5b7088d4f1036ac952ef117b8d60c4aa39e275018bc6de42f90a`,
shape-tagged final bytes, disabled failure persistence and 2,048 shrink steps.
Cases deterministically permute constructed rows and columns; failures print
the effective seed and minimized case for direct reproduction. Independent
fixture math validates returned linear, branch-sensitive circle and trace
states, including accepted-then-invalid trials. Flat audit descriptors retain
executable dense row order while grouped snapshots and redundancy candidates
follow source-store order. A direct singular least-squares test exercises the
SVD fallback. Criterion benchmarks fixed 2x2, 4x4 and 8x8 dense workloads
separately from all correctness tolerances.

## M4 — Core decomposition, elimination and source-level diagnostics

Goal: make large/disconnected systems tractable and diagnose synthetic redundancy/conflict before domain UX is involved.

Status: complete as of 2026-07-14.

Implement in core:

1. [x] Variable/residual bipartite incidence graph.
2. [x] Connected-component decomposition.
3. [x] Fixed-variable and exact-equality alias elimination where mathematically safe.
4. [x] Component-level structural pattern caching.
5. [x] Reuse of unaffected component solutions/traces.
6. [x] Structural count summary alongside numerical rank.
7. [x] Redundant-row/source candidates.
8. [x] Conflict source candidates using bounded deletion/re-solve for small components.
9. [x] Deterministic mapping from generated scalar rows back to one high-level source.
10. [x] Audit snapshot annotations for eliminated, redundant, conflicting and singular rows/sources.

Required synthetic fixtures:

- two disconnected solvable components;
- one edited and one unaffected component;
- exact variable alias chain;
- duplicate row from the same source;
- duplicate rows from separate sources;
- contradictory scalar and vector sources;
- underconstrained plus redundant system;
- configuration-dependent singularity without structural graph change.

Constraints:

- do not implement Dulmage–Mendelsohn decomposition yet unless it falls out naturally and is separately tested;
- do not introduce sparse numerical solving;
- do not freeze public domain APIs;
- conflict output is a deterministic candidate set, not a claim of globally minimal unsatisfiable core.

Gate: diagnostics/decomposition section of `ACCEPTANCE.md` using synthetic systems.

Completion notes: original incidence remains available for audit while solve
components are rebuilt after trusted core fixed/alias elimination. Dense assembly,
validation, traces, rank thresholds and diagnostics are component-scoped; cached
states are independently revalidated at the requested tolerance before zero-step
reuse. Conflict trials suppress both a source's rows and elimination semantics and
are bounded independently per failed component. Reports aggregate component rank
and DOF, distinguish fully redundant sources from sources containing dependent
rows, validate all non-objective values/Jacobians at the returned state, and retain
every audit row with explicit evaluation status. Audit annotations cover eliminated,
redundant, conflicting and conservatively singular rows/sources.

### Human checkpoint A — after M4

Review before continuing:

- packed variable/residual API and ownership;
- hard/temporary/preference representation;
- damping/trust policy and solve traces;
- rank/DOF semantics;
- equation-audit snapshot shape;
- elimination/decomposition behavior;
- redundancy/conflict wording and source attribution.

The overnight agent must stop here and produce `OVERNIGHT_REPORT.md`. It must not begin M5 without explicit continuation.

---

# Domain and interaction milestones

## M5 — First sketch vertical slice and live browser drag

Goal: solve a useful partially constrained sketch through the public sketch API.

Status: complete as of 2026-07-14.

Implement in `geosolve-sketch`:

- [x] Point and line-segment entities.
- [x] Fixed point/coordinate.
- [x] Coincident.
- [x] Horizontal and vertical.
- [x] Point distance and segment length.
- [x] Source constraint IDs producing one or more residual rows.
- [x] Compilation to core variables/residuals.
- [x] Reference dimensions that report values without adding equations.
- [x] Temporary dragged-point target and previous-state preference.

Canonical scene:

- [x] `S1 underconstrained triangle` from `docs/SCENARIOS.md`.

Web work:

- [x] Replace static triangle with solved geometry.
- [x] Implement SVG pointer drag for point C.
- [x] Show termination, validated residual, rank, DOF and iteration count.
- [x] Show live audit rows, named bindings, scales and evaluated values.
- [x] Distinguish fixed, free and actively dragged points.
- [ ] Optionally add geometry↔equation highlighting if it does not delay the slice.

Gate: sketch and web criteria for S1.

Completion notes: hard, temporary and preference rows are optimized in strict
lexicographic order. Secondary trials are reprojected onto the nonlinear hard
manifold and independently validated before commit. The stable-ID sketch model
compiles every supported constraint to analytic residuals with structured audit
metadata; its 20 M5 tests cover finite-difference Jacobians, recovery,
metamorphic scaling/rotation/translation, invalid geometry, reference dimensions,
explicit axis/length branch state and S1 drag/release behavior. The browser uses
the public S1 constructor and retained-state audit snapshots, supports mouse/pen/
touch pointer capture, and has 16 native rendering/interaction tests. The full
workspace gate passes with 113 tests, warnings-denied Clippy, locked WASM check
and Trunk release build. A secondary residual spanning multiple reduced hard
components remains an explicit `NumericalFailure`; S1 does not require this case.

Follow-up verification fixtures add a one-DOF horizontal rail with an
equation-free reference length and a two-DOF coincident pair. Both use the same
public sketch solve/audit path and shared mouse/pen/touch interaction. Coverage
includes projection, release continuity, off-viewport clamping and narrow-screen
pointer sizing.

### Human checkpoint B

Review drag/minimum-motion behavior, equation presentation and public sketch model before expanding the constraint library.

## M6 — Planar linkage vertical slice and driver continuation

Goal: solve rigid-body mechanisms using the same core.

Status: complete as of 2026-07-14.

Implement in `geosolve-linkage`:

- [x] `PlaneFrame` assignment.
- [x] Rigid body with `Pose2` variable and body-local features.
- [x] Grounded body/gauge removal.
- [x] Revolute joint: transformed anchor coincidence, two rows.
- [x] Prismatic joint: transverse coincidence plus relative-axis alignment.
- [x] Weld/fixed joint.
- [x] Angular and linear drivers.
- [x] Warm-started bounded-step driver continuation.
- [x] Explicit open/crossed assembly-mode state.
- [x] Velocity solve `J_q q_dot + J_s s_dot = 0` after position convergence.

Canonical scenes:

- [x] `L1 four-bar open`.
- [x] `L2 four-bar crossed`.
- [x] `L3 slider-crank`.

Web work:

- [x] Use domain constructors, not duplicate geometry.
- [x] Driver slider updates solved SVG linkage.
- [x] Display branch label and singularity warning.
- [x] Reuse equation-audit panel for joint closure and driver rows.
- [x] Preserve selected assembly mode over the safe sweep.

Gate: linkage and web criteria.

Completion notes: the linkage model uses one unwrapped `Pose2` per rigid body,
trusted elimination for grounded poses, body-local geometry, analytic joint and
driver Jacobians, and typed branch monitors. Continuation is deterministic,
warm-started and limited to two-degree canonical driver steps. L1/L2 retain
opposite orientation signs through `25..135` degrees; L3 retains its positive-x
slider branch through `15..165` degrees. Reduced velocity solves use the same
rank cutoff they report and independently validate differentiated equations.
The browser exposes live L1/L2/L3 sliders, typed branch/conditioning/velocity
diagnostics and accepted-state audit rows, and also adds canonical S2 conflict
diagnosis. The gate passes with 20 linkage tests, 23 sketch tests, 24 web tests,
144 workspace tests, warnings-denied Clippy, locked native/WASM checks and a
Trunk release build. Exact toggles reject or roll back to a finite near-toggle
state with a conditioning warning. M6 intentionally remains dense and
warm-start-only; predictor and pseudo-arclength continuation remain M8 work.

### Human checkpoint C

Review branch/assembly behavior and driver interaction before advanced continuation or spatial extension.

Approved by the supervising caller on 2026-07-14 after manual review of S1/S2 and L1/L2/L3 interactions with no visual issues.

## M7 — CAD curves, tangency and branch semantics

Goal: cover the minimum credible CAD sketch curve/constraint set while preserving an advanced-curve path.

Status: complete as of 2026-07-14.

Entities:

- [x] Circle.
- [x] Circular arc with explicit sweep/orientation state.

Constraints/dimensions:

- [x] Point-on-line and point-on-circle.
- [x] Radius and diameter.
- [x] Parallel and perpendicular.
- [x] Equal length and equal radius.
- [x] Oriented angle.
- [x] Midpoint and symmetry.
- [x] Line-circle tangency.
- [x] Circle-circle tangency with internal/external mode.
- [x] Driving versus reference dimensions.

Requirements:

- [x] Internal curve-evaluation adapter returning position, first derivative, parameter domain and degeneracy state.
- [x] Latent scalar contact parameters for interior contacts.
- [x] Explicit branch/span/contact state.
- [x] Analytic/local-AD versus finite-difference verification.
- [x] Invalid zero-length/radius/tangent handling.
- [x] `S3 tangent circles` browser scene.
- [x] Quadratic and cubic Bézier point-on-curve and line-tangency proof fixtures before freezing a public curve trait.

Gate: complete sketch MVP matrix.

Completion notes: circles and oriented circular arcs compile through a private
curve-evaluation seam with ordinary scalar radius/contact variables, structured
audits and independent accepted-state validation. Unit-direction equations avoid
short-line false convergence; bounded segment/arc contacts use explicit domains,
roundoff-only endpoint normalization and transactional rejection. Circle
tangency stores external/internal containment and center-direction state, and S3
switches deterministically between the positive-x external and internal roots.
The literal per-constraint matrix covers exact, perturbed, finite-difference,
transformed, scaled and invalid fixtures. Quadratic/cubic Béziers prove the same
private seam without exposing a generic public curve trait. The browser adds S3
mode switching, bounded-arc contact dragging and bounded line-circle tangent
gliding, all from public sketch geometry/contact/audit APIs. Arc endpoint angles
remain explicit fixed entity state in M7; only the radius and contact span solve.
The gate passes with 50 sketch tests, 33 web tests and 180 workspace tests,
warnings-denied workspace Clippy, locked native/WASM checks, WASM test
compilation, desktop/mobile browser review and a Trunk release build.

Follow-up verification adds bounded circle-arc tangency with explicit
inside/outside radial state. With the arc fixed and no circle radius dimension,
the circle center retains exactly two local DOF while its radius and two contact
parameters solve automatically. Independent feature-relative checks reject
wrong tiny radii and report mixed-scale tangencies as numerically ambiguous when
floating-point resolution cannot prove the branch relation. The eleventh browser
scene exposes two-dimensional center dragging, changing solved radii, bounded
span rejection and retained-state audits through public sketch APIs.
The expanded gate passes with 61 sketch tests, 40 web tests and 198 workspace
tests plus warnings-denied Clippy, locked WASM checks and a Trunk release build.

## M8 — Sparse scaling and continuation hardening

Goal: improve scale and singular-path behavior without changing results on the established corpus.

Implement only after profiling:

- block COO/triplet assembly and cached sparsity pattern;
- `faer` CSC/CSR conversion;
- sparse QR for least-squares/rank-sensitive components;
- optional damped normal-equation Cholesky fast path;
- recorded dense/sparse crossover policy;
- structural matching/Dulmage–Mendelsohn-style classification if justified;
- predictor-corrector continuation;
- pseudo-arclength continuation around selected linkage dead centres.

Gate:

- dense and sparse paths agree within validation tolerance on the full corpus;
- no diagnostic regression;
- documented near-toggle scenario crosses without unintended root jump.

## M9 — Pre-1.0 hardening

- stable serialization for entities, constraints and branch state;
- public API review and crate documentation;
- fuzz/property corpus for degenerate inputs;
- differential/oracle fixtures based on SolveSpace and selected PlaneGCS cases;
- benchmarks and performance baselines;
- browser smoke automation;
- licence/attribution audit;
- changelog and versioning policy.

Out of scope until after M9:

- collision/contact inequalities;
- dynamics and reaction forces;
- production-grade spline/NURBS/general-conic editing;
- global enumeration of all roots;
- general spatial joints and `SE(3)` solving.
