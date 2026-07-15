# Architecture

## 1. Product boundary

GeoSolve is a pure-Rust library for two products built over one numerical kernel:

1. production-capable 2D CAD sketches with editable analytic and parametric curves, dimensions, contact, tangency, continuity, persistence and truthful diagnostics;
2. position- and velocity-level 2D/3D rigid-body kinematics for linkages and CAD assemblies, including explicit assembly modes, continuation, persistence and truthful mobility.

The products share numerical machinery, not a domain object model. `geosolve-sketch` and `geosolve-linkage` remain separate frontends over `geosolve-core`.

This is not a solid modeller, B-rep kernel, mesher, renderer, collision engine, statics solver, dynamics engine or global polynomial-root enumerator. Mass, inertia, force, reaction, friction, impact and time integration are outside M8-M22.

## 2. Status of this document

M1-M7 are the frozen implemented baseline. M8 accepts the target contracts in this document without changing production solver behavior. Statements are therefore marked as:

- **Baseline:** behavior implemented before M8 and protected by regression tests.
- **Target:** behavior required by the named M9-M22 milestone.

A target statement must not be exposed as an implemented capability before its milestone gate passes.

## 3. Crate responsibilities

### `geosolve-geometry`

Owns pure immutable numerical geometry:

- 2D and 3D points, vectors and validated frames;
- planar curve evaluation and regularity/domain metadata;
- `Pose2`, `Pose3`, `SE(2)` and `SE(3)` operations under ADR 0006;
- angle wrapping/unwrapping, normalization and degeneracy-safe helpers.

It does not know about variable IDs, constraints, iterations, design entities or rigid-body topology.

### `geosolve-core`

Owns domain-independent numerical infrastructure:

- stable runtime IDs for variable, residual and source blocks;
- packed ambient state and normalized tangent coordinates;
- residual incidence, category, scaling and structured audit metadata;
- canonical component-local linearization and analytic/local-AD adapters;
- fixed/alias elimination, decomposition, dense and sparse assembly;
- strict hard, temporary and preference hierarchy;
- nonlinear iteration, factorization, rank and diagnostic policy;
- persistent solve sessions, bounds, active sets and validated transactions;
- continuation primitives and complete solve reports.

It does not contain CAD entities, curve-definition variants, rigid bodies, joints, mates, branch labels or persistence schemas from either domain.

### `geosolve-sketch`

Owns the 2D design graph:

- persistent design points and typed design scalars;
- closed, versioned built-in curve definitions;
- semantic features, dimensions, contacts and constraints;
- explicit branch, span, winding, tangent-orientation and contact-neighborhood state;
- compilation into core residuals, validators and commit mappings;
- source-level audit and persistence mappings.

The frozen baseline includes points, segments, circles, oriented arcs and the M5/M7 constraint corpus. M13 migrates those entities to the generic design graph; M15-M20 add the remaining product curve and continuity families.

### `geosolve-linkage`

Owns planar and spatial kinematic domain models:

- rigid bodies and body-local point, axis, plane and frame features;
- physical grounding, joints, mates, drivers and assembly modes;
- branch-preserving continuation and velocity-level queries;
- domain validation, source mapping and persistence.

The frozen baseline is planar `Pose2` linkage kinematics. M14 migrates it to the shared session/gauge architecture; M16, M18 and M21 add and complete spatial kinematics. No linkage API implies physics.

### `geosolve-demo-web`

Is a separate WASM compatibility and audit consumer:

- it uses public sketch, linkage and audit APIs;
- it contains no duplicate residual equations or domain model;
- it retains automated build/browser smoke coverage;
- UI feature work is not an M8-M22 product deliverable.

## 4. Numerical representation and linearization

A problem contains variable blocks `x` and residual blocks `r_i(x_incident)`. Every variable has an ambient representation, a tangent dimension, a local retraction and positive finite characteristic step scales. Every residual declares its source, priority category, ordered local incidence, output dimension, positive finite residual scales, evaluator, Jacobian path and audit rows.

Residual values and Jacobian columns are normalized before convergence or rank decisions:

```text
r_normalized[row] = r_raw[row] / residual_scale[row]
J_normalized[row, col] = d(r_normalized[row]) / d(delta_normalized[col])
delta_local[col] = step_scale[col] * delta_normalized[col]
```

Baseline variable blocks are scalar, `Vec2` and additive-coordinate `Pose2`. Baseline assembly can materialize global dense columns, while reduced components are solved independently.

The M9 target is one canonical component-local linearization under ADR 0005. It evaluates only incident blocks, writes into caller-provided storage, never allocates global columns for a component, and feeds the dense component solve. The additive caller-storage method is public and unstable before 1.0 because it extends the existing public residual evaluator trait; the local AD formula trait/adapter and normalized-coordinate storage marker remain private. M12 adds indexed block coordinates and materializes triplet/COO and sparse storage from that IR. Analytic Jacobians remain valid and central finite differences remain an independent oracle. Branch, span, winding, active-bound and assembly-mode state are fixed discrete inputs outside AD.

Public and best-effort audit evaluate fresh raw/normalized values at one state and independently require successful canonical Jacobian/fused validation before marking a row `Evaluated`. A structured derivative failure marks the row `Failed` while retaining any fresh finite displayed values and its category/message. Successful numeric IR blocks are `Evaluated`; any failure aborts before partial IR consumption.

## 5. Solve pipeline and persistent state

The logical target pipeline is:

1. validate domain topology, geometry, scales and discrete state;
2. compile or incrementally update immutable topology and source parameters;
3. eliminate trusted fixed and alias relationships;
4. split the reduced incidence graph into deterministic components;
5. determine dirty components and active bounds;
6. linearize each dirty component in normalized local coordinates;
7. solve the strict hard/temporary/preference hierarchy;
8. independently re-evaluate all hard rows and domain/branch validators;
9. compute rank, mobility and bounded diagnostics at the returned state;
10. atomically commit only a finite, independently valid accepted patch;
11. retain prior accepted state and discrete state on rejection.

Baseline `Problem::solve_decomposed` has component caching but relies on caller-supplied edited variable IDs. M10 replaces that hint-based lifecycle with the persistent `SolveSession` and revision/dirty tracking in ADR 0007. M12 adds sparse storage, structural matching and symbolic cache reuse. No benchmark or performance policy may bypass independent validation.

## 6. Hard validity and secondary optimum status

Hard validity is independent from nonlinear termination, rank classification and secondary-objective completion.

Starting with M9, the report has these orthogonal facts:

- `HardValidity::Valid`: every hard row was freshly evaluated, finite and within the configured normalized tolerance, and all domain/branch validators accepted the same returned state;
- `HardValidity::Invalid`: evaluation completed but at least one hard row or domain/branch validator failed;
- `HardValidity::NotEvaluated`: complete independent validation could not be performed;
- hard nonlinear termination: why hard iteration stopped;
- one secondary result per requested temporary/preference level: `NotRequested`, `Optimal`, `Acceptable`, `Stalled`, `IterationLimit` or `EvaluationFailure`;
- rank, structural class, singularity and diagnostic completeness as separate fields.

A state is hard-valid only for `HardValidity::Valid`. A domain may commit a hard-valid state even when a secondary objective is not optimal, but it must report that secondary status and the domain interaction policy may reject it. No secondary success can turn invalid or unevaluated hard geometry into a success-like result.

Baseline transition: through M8, `SolveReport` exposes `hard_residuals_validated` and hard norms, but top-level `SolveTermination::Converged` also requires every priority pass to terminate as `Converged`. That frozen behavior remains accepted and is not a failure of the M1-M8 baseline. M9 introduces the orthogonal fields above and makes them mandatory for all new reports. M10 `SolveSession` commits consume the M9 hard-valid field as authoritative. Compatibility wording must not call a secondary stall a hard-constraint failure.

## 7. Rank and mobility contract

### 7.1 Numerical rank

Starting with M9, numerical rank is computed independently for each reduced connected component from its finite, normalized, component-local hard Jacobian `J_c`. Let:

- `m_c` be active hard scalar rows;
- `n_c` be active tangent coordinates after trusted fixed/alias elimination;
- `sigma_max` be the largest singular value, or zero for an all-zero matrix;
- `tau_rel` be the configured relative tolerance;
- `d_c = max(m_c, n_c, 1)`;
- `tau_machine = EPSILON * d_c * max(sigma_max, 1)`;
- `tau_c = max(tau_rel * sigma_max, tau_machine)`.

The numerical rank is the count of singular values strictly greater than `tau_c`. The report includes `tau_rel`, `tau_machine`, `tau_c`, `sigma_max`, the smallest retained singular value and enough spectrum/estimator information to reproduce the classification. Rank is invalid if any required value or decomposition result is non-finite.

For valid rank `r_c`:

```text
right_nullity = n_c - r_c   // equality mobility in tangent coordinates
left_nullity  = m_c - r_c   // dependent hard-row space
```

Whole-problem rank and nullities are sums of component-local values. A global largest singular value never sets another component's threshold.

This M9 contract governs core equality/position reports and every sketch/linkage position solve built from them. The compatibility linkage velocity solver retains its existing dense reduced policy until M14 moves velocity solving onto the shared accepted hard linearization and rank policy. Linkage position conditioning summaries use within-component spectra and M9 `near_singular`; they never compare concatenated extrema from disconnected components.

A component is numerically singular when `r_c < min(m_c, n_c)`. A distinct near-singular warning is raised without changing rank when the smallest retained singular value is at most `near_singular_factor * tau_c`; the configured factor and ratio are reported. The initial target factor is `100`. A warning is not convergence and a rank drop is not nonlinear failure.

Baseline transition: M1-M8 use normalized component-local Jacobians and default `tau_rel = 1e-10`, report right nullity as local DOF, and flag a rank drop. They use only `tau_rel * sigma_max`, do not report the machine floor or left nullity, and have no separate near-singular band. That behavior remains the accepted frozen baseline. M9 atomically adopts the machine-floor threshold, numerical left/right nullity and near-singular reporting above; existing rank fixtures remain regression oracles.

### 7.2 Structural classification

Numerical rank and graph structure answer different questions. The M12 target computes maximum structural matching on the reduced hard incidence graph before numerical values are considered. For structural rank `s_c`:

```text
structural_right_nullity = n_c - s_c
structural_left_nullity  = m_c - s_c
```

Classification is:

- `Under`: right nullity is positive and left nullity is zero;
- `Well`: both nullities are zero;
- `Over`: left nullity is positive and right nullity is zero;
- `Mixed`: both are positive; the report includes Dulmage-Mendelsohn under, well and over partitions rather than hiding them in one label.

Baseline structural summaries report reduced counts and deterministic signatures only. Count comparisons may be displayed as count heuristics, never as structural matching or numerical rank. M12 implements matching and partitions.

### 7.3 Active bounds

M10 reports every bound as inactive, active-lower, active-upper or fixed. Equality rank is retained before adding bounds. For bidirectional mobility, append independent active-bound coordinate normals to the equality Jacobian; the nullity of this augmented matrix is the lineality dimension of the feasible tangent cone. The report includes:

- equality right nullity before active bounds;
- bidirectional DOF after the active set;
- active bound IDs and sides;
- whether a nonzero one-sided feasible tangent direction exists.

An active lower bound permits inward positive motion and an active upper bound permits inward negative motion, so subtracting one DOF per active bound is not a sufficient mobility analysis. Bound activation is explicit state and cannot be inferred from a post-solve clamp.

### 7.4 Gauge versus internal mobility

A domain-certified free world action contributes gauge DOF: three for a floating planar component and six for a floating spatial component. Reports split numerical right nullity into `gauge_dof` and `internal_mobility`; they do not blindly subtract three or six unless the domain certifies the corresponding invariant action. Physical grounding removes physical gauge freedom. A numerical gauge only chooses coordinates and must not remove reported physical mobility. ADR 0009 governs this split; M14 applies it to planar linkage and M16 to spatial linkage.

## 8. Diagnostic completeness and budgets

Redundancy and conflict candidates are bounded explanatory diagnostics, not proofs of a globally minimal dependent set or unsatisfiable core. Every bounded diagnostic section carries:

- `status`: `Complete`, `Truncated` or `Skipped`;
- the configured budget, including applicable maximum component tangent dimensions, scalar rows, candidate sources and deletion/rank trials;
- actual work consumed;
- a machine-readable reason for `Truncated` or `Skipped`;
- deterministic candidate IDs in source order.

`Complete` means every candidate in the documented algorithmic scope was examined. `Truncated` means at least one eligible candidate was examined but the budget stopped remaining work. `Skipped` means no eligible analysis was performed, for example because diagnostics were disabled, rank/evaluation was invalid, hard constraints were valid for conflict analysis, or the first component already exceeded budget.

An empty candidate list is meaningful only together with its status. In particular, an empty list with `Skipped` or `Truncated` must never be presented as “no conflict” or “no redundancy”. A `Complete` result still claims completeness only for the documented bounded deletion/rank algorithm, not global minimality.

Baseline transition: conflict deletion currently has fixed limits of 12 candidate sources and 24 active tangent dimensions and silently omits over-budget components; redundancy runs only after valid hard evaluation/rank. The baseline report has no completeness or budget fields, so empty baseline candidate vectors are ambiguous. M10 makes budgets configurable/reportable in the session report; M12 extends the same contract to structural and sparse diagnostics.

## 9. Priority semantics

Hard, temporary and preference rows are different categories, not weights in one undocumented least-squares objective. The implemented baseline uses a lexicographic hierarchy and reprojects secondary steps onto hard validity. The target retains this ordering through component-local and sparse paths:

1. attain and validate hard constraints;
2. optimize temporary objectives in the valid hard tangent/null space;
3. optimize previous-state preferences without worsening the attained temporary level beyond documented numerical resolution;
4. independently validate hard rows and report each secondary outcome.

Bounds participate through the M10 active-set policy. Secondary objectives spanning hard components are implemented in M12 without merging hard components or weakening hard tolerance.

## 10. Manifold and frame conventions

ADR 0006 defines body-to-world transforms, right/body-local retraction, tangent ordering, local difference, quaternion ordering and sign canonicalization. Baseline `Pose2` stores `[x, y, unwrapped_angle]` and applies additive increments. M11 performs the tested transition to manifold `Pose2` and quaternion-backed `Pose3`; finite differences then perturb tangent coordinates through the same retraction.

Planar geometry is evaluated in local 2D coordinates. A workplane maps it into world coordinates as:

```text
p_world = origin_world + u_world * x + v_world * y
```

Same-plane constraints remain 2D. A planar body pose composes with the workplane frame; redundant `z = 0` rows are not added per point.

## 11. Sketch design and curve architecture

ADR 0008 defines persistent external IDs, runtime generational keys and a closed versioned `CurveDefinition`. “Closed” means an exhaustive built-in serializable enum, not that every represented curve is periodic. Evaluation uses internal traits/adapters until built-in line, circle, arc, Bezier, conic, B-spline and NURBS families prove the seam.

Generic curve constraints use latent contact coordinates and explicit discrete state:

```text
point on curve:       P - C(t) = 0
curve/curve contact:  C1(t1) - C2(t2) = 0
tangent alignment:    cross(unit(C1'(t1)), unit(C2'(t2))) = 0
```

Design controls, weights and contact parameters that are active variables must all appear in residual incidence and derivatives. Parameter domains, spans, winding, contact neighborhoods and tangent orientation remain outside AD. Bounded endpoints use M10 bounds/active sets. Cusps, zero-speed jets, invalid knots, rational poles and ambiguous neighborhoods are explicit evaluation/domain outcomes and cannot converge through normalization.

M13 migrates baseline entities and persistence topology. M15 proves generic editable-curve differentiation with Bezier curves. M17 adds conics, M19 B-splines and M20 NURBS plus curvature/G2 and separately named parametric C2 continuity.

## 12. Kinematic architecture

Rigid bodies own local features; joints and mates relate features rather than reconstructing rigidity with sketch distance webs. Branch/assembly state is persistent domain state. Physical ground and numerical gauge are distinct under ADR 0009. Position and velocity queries use the same accepted-state reduced hard linearization and rank policy.

M14 migrates planar linkage to shared sessions and gauges. M16 adds a spatial vertical slice. M18 completes common spatial joints/mates. M21 completes continuation, multi-driver velocity queries and planar/spatial consistency. These milestones do not add forces, reactions or dynamics.

## 13. Equation audit and persistence

Every executable residual row has structured audit metadata generated with the equation, never duplicated in a UI. An accepted-state audit groups rows by persistent domain source and reports:

- runtime and persistent source identity;
- readable source label, equation template and named feature bindings;
- hard/temporary/preference category;
- target, units and characteristic scale;
- raw and normalized finite values or an explicit evaluation failure;
- elimination, active-bound, redundancy, conflict and singularity annotations;
- diagnostic completeness links where candidate analysis is bounded.

Persistence stores domain topology, continuous accepted state and every discrete branch/span/winding/gauge/assembly choice in a versioned envelope. Runtime slot-map keys are remapped deterministically and are never serialized as persistent identity. M13 completes sketch foundations; M20 and M21 complete each product schema; M22 finalizes migration and public compatibility policy.

## 14. Linear algebra policy

- Dense QR/SVD remains the correctness and diagnostic path for small components.
- Successful Cholesky never proves rank.
- M12 introduces pure-Rust `faer` sparse storage and rank-revealing least squares after canonical component-local assembly exists.
- Dense and sparse paths must agree on independently validated geometry, rank/nullity, mobility, diagnostics and branch state.
- Sparse crossover values are benchmark-derived and reported; they never alter correctness tolerances.
- The workspace remains `unsafe_code = "forbid"`; native solver FFI is not permitted.

## 15. Roadmap allocation

- M8: accept contracts, ADRs and deterministic representative baselines.
- M9: canonical component-local linearization, internal local AD, orthogonal solve status and the complete numerical-rank contract.
- M10: persistent sessions, bounds, active sets and diagnostic budgets.
- M11: manifold `Pose2`/`Pose3` and sensitivity APIs.
- M12: sparse structure, matching, hierarchy and robust continuation.
- M13-M14: add persistent domain identity and migrate sketch and planar linkage architectures.
- M15-M20: complete the 2D CAD sketch product.
- M16, M18 and M21: complete planar/spatial kinematic product behavior.
- M22: stabilize public APIs, persistence, documentation and release gates.
