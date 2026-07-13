# Architecture

## 1. Product boundary

GeoSolve is initially a **position-level** geometric/kinematic solver. It is not a solid modeller, collision engine, dynamics engine, or global polynomial root enumerator.

It has two domain frontends:

- CAD sketching: editable primitives, dimensions, partially constrained motion and design intent;
- mechanisms: rigid bodies, local features, joints, drivers, assembly modes and continuation.

They share one numerical kernel but do not share one entity representation.

## 2. Crate responsibilities

### `geosolve-geometry`

Pure, immutable numerical geometry:

- `Point2`, `Vector2`, future curve evaluators;
- `Pose2` and local tangent updates;
- `PlaneFrame { origin, u, v }` mapping local 2D geometry into world 3D;
- normalization, angle wrapping/unwrapping and degeneracy-safe helpers.

It must not know about variable IDs, constraints, iterations or user-facing entities.

### `geosolve-core`

Domain-independent solver infrastructure:

- stable IDs for variable blocks, residual blocks and high-level source constraints;
- packed state and tangent vectors;
- variable block kinds: scalar, `Vec2`, `Pose2`; later `Pose3`;
- residual blocks with declared variable incidence;
- analytic or local-forward-AD Jacobian blocks;
- block-to-dense and later block-to-sparse assembly;
- hard constraints, temporary objectives and previous-state preferences as distinct categories;
- nonlinear iteration, trust/damping policy and step limits;
- graph decomposition, equality elimination and cached sparsity structure;
- rank, DOF, redundancy/conflict candidates, singularity indicators;
- continuation primitives and complete solve reports.

The core must not contain `LineSegment`, `Circle`, `RigidBody`, or `RevoluteJoint` variants.

### `geosolve-sketch`

High-level 2D CAD model:

- points, line segments, circles and arcs;
- source constraints and dimensions;
- branch state such as tangent mode, angle orientation and arc sweep;
- driving/reference dimension semantics;
- drag targets and minimum-motion preferences;
- compilation into core variable/residual blocks;
- mapping diagnostics back to sketch constraint IDs.

### `geosolve-linkage`

Planar mechanism model:

- rigid bodies with `Pose2` variables;
- body-local points/axes/features;
- ground body and gauge removal;
- revolute, prismatic and weld joints;
- angular and linear drivers;
- branch/assembly mode state;
- predictor-corrector continuation and later pseudo-arclength continuation;
- position and velocity-level queries.

### `geosolve-demo-web`

A deliberately primitive browser harness:

- compiled to `wasm32-unknown-unknown`;
- uses `wasm-bindgen`/`web-sys` and SVG DOM only;
- no React/Yew/Leptos requirement;
- hardcoded scenario constructors from the sketch/linkage crates;
- selector, drag/driver controls, solved geometry rendering and diagnostic text;
- no solver equations or duplicate domain model in the web crate.

## 3. Numerical representation

A problem consists of variable blocks `x` and residual blocks `r_i(x_incident)`.

Each variable block has:

- an ambient stored representation;
- a tangent dimension;
- a local `plus(delta)` operation;
- characteristic step scales.

Initial block kinds:

- scalar: ambient/tangent 1;
- `Vec2`: ambient/tangent 2;
- `Pose2`: ambient/tangent 3, update translation and unwrapped angle.

Future `Pose3` should store a transform/quaternion but update using a six-dimensional tangent increment.

Each residual block declares:

- source constraint ID;
- hard/temporary/preference category;
- ordered incident variable IDs;
- residual dimension;
- characteristic residual scales;
- residual evaluation;
- local Jacobian evaluation.

The assembled residual vector is dimensionless. Scaling is part of the constraint definition, not an optional UI concern.

## 4. Solve pipeline

Per solve request:

1. compile the domain model into variables and residuals;
2. reject invalid primitive geometry before iteration;
3. eliminate pinned and exact-equality-aliased variables;
4. split the incidence graph into connected components;
5. preserve components unaffected by the edit;
6. evaluate residuals and Jacobian;
7. calculate a damped Gauss-Newton/DogLeg/LM step;
8. apply block-local step limits;
9. accept/reject from actual versus predicted reduction;
10. independently validate hard residuals;
11. calculate numerical rank and local nullity;
12. classify the result and map diagnostics to source IDs;
13. update continuation/branch state only on accepted solutions.

### Linear algebra policy

- Use dense QR/SVD first for correctness and diagnosis on small components.
- A damped normal-equation Cholesky path may be a fast path, never the sole path.
- Add `faer` sparse QR/Cholesky only after sparsity patterns and correctness tests are stable.
- Never infer rank solely from successful Cholesky.

## 5. Hard versus temporary objectives

Hard geometry must validate within tolerance independently of drag or preference objectives.

MVP implementation may use separated weights plus validation, but the intended model is hierarchical:

1. find a hard-constraint step;
2. optimize drag/minimum-motion objectives in the linearized hard nullspace;
3. control the combined step with a trust region.

A large drag weight must never make a geometrically invalid state report `Converged`.

## 6. Diagnostics

Required statuses:

- `Converged`;
- `Underconstrained`;
- `Redundant`;
- `Conflicting`;
- `Singular`;
- `Stalled`;
- `IterationLimit`;
- `InvalidGeometry`.

The report must include iteration count, validated residual norms, rank, local DOF, and high-level source IDs for redundancy/conflict candidates.

Structural graph rank and numerical Jacobian rank answer different questions. Keep both. Singular positions can change numerical mobility without changing graph topology.

## 7. Branch and continuation model

Explicit discrete state is required for:

- signed angle orientation;
- arc sweep;
- internal/external tangency;
- side-of-line/circle choices;
- open/crossed linkage assembly mode.

Normal editing and driving are local continuation problems:

- warm-start from the last accepted state;
- bound driver and Newton steps;
- preserve unwrapped angles and orientation signs;
- monitor rank/small singular values;
- do not silently jump branches.

Global all-root search is outside MVP.

## 8. 2D geometry embedded in 3D

Planar points are stored as `(x, y)` under a `PlaneFrame`:

```text
world = origin + u*x + v*y
```

Same-plane constraints remain 2D. Mapping to 3D is used for rendering and future cross-plane/world constraints. A planar linkage pose is composed with the workplane frame rather than constrained by redundant world-space `z = 0` rows.
