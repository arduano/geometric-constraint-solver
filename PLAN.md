# Implementation plan

Implement in order. Do not start a milestone until the preceding milestone's tests and acceptance gates pass.

## M0 — Repository and contracts

Status: complete as of 2026-07-13. Native format/test/clippy, WASM check, and release Trunk build all pass.

Deliverables:

- Cargo workspace and GPL-3.0-or-later licence;
- separate geometry, core, sketch, linkage and WASM demo crates;
- architecture, scenarios, references and acceptance documents;
- primitive Trunk/WASM/SVG page with hardcoded unsolved scenes;
- NixOS-friendly development shell.

Gate:

- root format, clippy and native tests pass;
- demo crate checks for `wasm32-unknown-unknown` and `trunk build --release` succeeds.

## M1 — Problem representation and Jacobian verification

Goal: express small nonlinear equality systems without CAD/linkage semantics.

Implement in `geosolve-core`:

1. Stable stores for variable blocks, residual blocks and source constraint IDs.
2. Variable block kinds:
   - scalar;
   - `Vec2`;
   - `Pose2` with a three-dimensional local increment.
3. Packed state layout mapping stable IDs to ambient/tangent ranges.
4. Residual categories:
   - hard;
   - temporary interaction objective;
   - previous-state/minimum-motion preference.
5. A residual-block API declaring incident variables, output dimension, characteristic scales, evaluation and local Jacobian blocks.
6. Deterministic dense residual/Jacobian assembly.
7. Central finite-difference Jacobian checker with per-block error reports.
8. Invalid-geometry and non-finite-value rejection before linear algebra.

Implementation guidance:

- Start with hand-written analytic Jacobians for test residuals.
- Keep the API internal until at least two sketch and two linkage residuals use it.
- Do not introduce sparse matrices yet, but retain block incidence and deterministic ordering.

M1 tests:

- scalar quadratic residual;
- two-variable distance residual;
- `Pose2` transformed-point residual;
- mixed block dimensions and deterministic packing;
- analytic versus central finite-difference Jacobians;
- NaN/Inf and invalid scale rejection.

Gate: section M1 of `ACCEPTANCE.md`.

## M2 — Dense nonlinear solve, rank and validation

Goal: robustly solve and classify small connected components.

Implement:

1. Scaled residual and step norms.
2. Damped Gauss-Newton or Levenberg-Marquardt with:
   - adaptive damping;
   - accepted/rejected steps;
   - actual/predicted reduction;
   - block-local step limits;
   - iteration and stagnation termination.
3. Dense QR/SVD linear solve fallback; do not rely exclusively on `J^T J`.
4. Numerical rank and local nullity using a documented relative tolerance.
5. Independent hard-residual validation after iteration.
6. `SolveTermination` plus independent diagnostics in `SolveReport`.
7. Deterministic trace records usable by tests and the browser diagnostics panel.

M2 canonical systems:

- exactly determined linear and nonlinear systems;
- underdetermined circle point with one DOF;
- duplicate residual rows;
- contradictory equations;
- singular system at a configuration-dependent rank drop;
- same problem scaled by `1e-6`, `1`, and `1e6`.

Stop after M2 for architecture review before expanding domain APIs.

Gate: section M2 of `ACCEPTANCE.md` plus all global quality gates.

## M3 — First sketch vertical slice and live browser drag

Goal: solve a useful partially constrained sketch through the public sketch API.

Implement in `geosolve-sketch`:

- point and line-segment entities;
- fixed point/coordinate;
- coincident;
- horizontal and vertical;
- point distance and segment length;
- high-level source constraint IDs producing one or more residual rows;
- compilation to core variables/residuals;
- reference dimensions that report values without adding equations;
- temporary dragged-point target and previous-state preference.

Canonical scene: `S1 underconstrained triangle` from `docs/SCENARIOS.md`.

Web work:

- replace the static triangle with solved geometry;
- implement SVG pointer drag for point C;
- show termination, maximum hard residual, rank, DOF and iteration count;
- visually distinguish fixed, free and actively dragged points.

Gate: sketch and web sections of `ACCEPTANCE.md` for S1.

## M4 — Decomposition and useful diagnostics

Goal: make multiple sketches/components predictable and diagnose bad constraints.

Implement:

1. Variable/residual bipartite incidence graph.
2. Connected-component decomposition.
3. Exact equality/pinned-variable elimination where safe.
4. Component-level pattern caching and unaffected-component reuse.
5. Structural count summary alongside numerical rank.
6. Redundant-row candidates mapped to source constraints.
7. Conflict candidates mapped to source constraints, initially using bounded deletion/re-solve for small components.
8. Distinguish termination from underconstraint, redundancy and singularity.

Canonical scenes:

- `S2 conflicting rectangle`;
- duplicate distance constraint as genuinely redundant;
- two disconnected sketch components where only one is edited.

Gate: diagnostics section of `ACCEPTANCE.md`.

## M5 — Planar linkage vertical slice and driver continuation

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

- scenario selector uses domain constructors, not duplicate geometry;
- driver slider updates the solved SVG linkage;
- display branch label and singularity warning;
- preserve the selected assembly mode throughout the documented safe sweep.

Gate: linkage and web sections of `ACCEPTANCE.md`.

## M6 — CAD MVP curves and branch semantics

Goal: cover the minimum credible CAD sketch primitive/constraint set.

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

- every branch-sensitive constraint serializes explicit branch state;
- every residual has analytic/local-AD versus finite-difference verification;
- zero-length and zero-radius inputs return `InvalidGeometry`, never NaN;
- add `S3 tangent circles` browser scene.

Gate: complete sketch MVP matrix in `ACCEPTANCE.md`.

## M7 — Sparse scaling and continuation hardening

Goal: improve scale without changing results on the existing corpus.

Implement only after profiling:

- block COO/triplet assembly with cached sparsity pattern;
- `faer` CSC/CSR conversion;
- sparse QR for least squares/rank-sensitive components;
- damped normal-equation Cholesky as an optional well-conditioned fast path;
- dense/sparse crossover policy recorded in solve traces;
- structural matching/Dulmage-Mendelsohn-style classification if justified;
- predictor-corrector continuation;
- pseudo-arclength continuation around selected four-bar/slider-crank dead centres.

Gate:

- dense and sparse paths agree within validation tolerance on the full scenario corpus;
- no diagnostic regression;
- branch continuation crosses the documented near-toggle scenario without an unintended root jump.

## M8 — Pre-1.0 hardening

- stable serialization format for entities, constraints and branch state;
- public API review and crate-level documentation;
- fuzz/property corpus for degenerate inputs;
- differential/oracle test fixtures based on SolveSpace and selected PlaneGCS cases;
- benchmarks and performance baselines;
- browser smoke automation;
- licence/attribution audit;
- changelog and versioning policy.

Out of scope until after M8:

- collision/contact and inequalities;
- dynamics and reaction forces;
- splines/general conics;
- global enumeration of all roots;
- general spatial joints and `SE(3)` solving.
