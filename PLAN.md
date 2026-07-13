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

Implement in core:

1. Variable/residual bipartite incidence graph.
2. Connected-component decomposition.
3. Fixed-variable and exact-equality alias elimination where mathematically safe.
4. Component-level structural pattern caching.
5. Reuse of unaffected component solutions/traces.
6. Structural count summary alongside numerical rank.
7. Redundant-row/source candidates.
8. Conflict source candidates using bounded deletion/re-solve for small components.
9. Deterministic mapping from generated scalar rows back to one high-level source.
10. Audit snapshot annotations for eliminated, redundant, conflicting and singular rows/sources.

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

Implement in `geosolve-sketch`:

- point and line-segment entities;
- fixed point/coordinate;
- coincident;
- horizontal and vertical;
- point distance and segment length;
- source constraint IDs producing one or more residual rows;
- compilation to core variables/residuals;
- reference dimensions that report values without adding equations;
- temporary dragged-point target and previous-state preference.

Canonical scene: `S1 underconstrained triangle` from `docs/SCENARIOS.md`.

Web work:

- replace static triangle with solved geometry;
- implement SVG pointer drag for point C;
- show termination, validated residual, rank, DOF and iteration count;
- show live audit rows, named bindings, scales and evaluated values;
- distinguish fixed, free and actively dragged points;
- optionally add geometry↔equation highlighting if it does not delay the slice.

Gate: sketch and web criteria for S1.

### Human checkpoint B

Review drag/minimum-motion behavior, equation presentation and public sketch model before expanding the constraint library.

## M6 — Planar linkage vertical slice and driver continuation

Goal: solve rigid-body mechanisms using the same core.

Implement in `geosolve-linkage`:

- `PlaneFrame` assignment;
- rigid body with `Pose2` variable and body-local features;
- grounded body/gauge removal;
- revolute joint: transformed anchor coincidence, two rows;
- prismatic joint: transverse coincidence plus relative-axis alignment;
- weld/fixed joint;
- angular and linear drivers;
- warm-started bounded-step driver continuation;
- explicit open/crossed assembly-mode state;
- velocity solve `J_q q_dot + J_s s_dot = 0` after position convergence.

Canonical scenes:

- `L1 four-bar open`;
- `L2 four-bar crossed`;
- `L3 slider-crank`.

Web work:

- use domain constructors, not duplicate geometry;
- driver slider updates solved SVG linkage;
- display branch label and singularity warning;
- reuse equation-audit panel for joint closure and driver rows;
- preserve selected assembly mode over the safe sweep.

Gate: linkage and web criteria.

### Human checkpoint C

Review branch/assembly behavior and driver interaction before advanced continuation or spatial extension.

## M7 — CAD curves, tangency and branch semantics

Goal: cover the minimum credible CAD sketch curve/constraint set while preserving an advanced-curve path.

Entities:

- circle;
- circular arc with explicit sweep/orientation state.

Constraints/dimensions:

- point-on-line and point-on-circle;
- radius and diameter;
- parallel and perpendicular;
- equal length and equal radius;
- oriented angle;
- midpoint and symmetry;
- line-circle tangency;
- circle-circle tangency with internal/external mode;
- driving versus reference dimensions.

Requirements:

- internal curve-evaluation adapter returning position, first derivative, parameter domain and degeneracy state;
- latent scalar contact parameters for interior contacts;
- explicit branch/span/contact state;
- analytic/local-AD versus finite-difference verification;
- invalid zero-length/radius/tangent handling;
- `S3 tangent circles` browser scene;
- quadratic or cubic Bézier point-on-curve and line-tangency proof fixture before freezing a public curve trait.

Gate: complete sketch MVP matrix.

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
