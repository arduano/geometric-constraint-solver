# Acceptance criteria

These are behavioral gates, not implementation suggestions. A milestone is incomplete if its applicable criteria do not pass.

## Global quality gates

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p geosolve-demo-web --target wasm32-unknown-unknown
(cd crates/geosolve-demo-web && trunk build --release)
```

Additional requirements:

- no non-finite values in accepted state, residuals, Jacobians or reports;
- deterministic result and diagnostic ordering for identical input and initial state;
- no `unsafe` code;
- no native solver FFI;
- all public crates carry `GPL-3.0-or-later` metadata;
- every claimed success independently re-evaluates and validates hard constraints.

## Numerical tolerance policy

Unless a scenario explicitly overrides it:

- residuals are dimensionless before convergence testing;
- accepted maximum hard residual: `<= 1e-9`;
- finite-difference Jacobian relative error away from singular/nondifferentiable points: `<= 1e-6`;
- rank tolerance is relative to the largest singular/QR pivot and included in the report/config;
- tests must cover model characteristic scales `1e-6`, `1`, and `1e6` without changing topology, DOF, branch label or source diagnostics.

A test may use a looser documented tolerance only when it explains the conditioning reason and still independently validates geometry.

## M1 — Problem representation

- Stable variable/residual/source IDs survive unrelated insertions and removals.
- Packed ordering is deterministic.
- Scalar, `Vec2`, and `Pose2` blocks apply local increments correctly.
- A residual can touch multiple heterogeneous blocks and assemble into the correct matrix ranges.
- Characteristic scales must be positive and finite; invalid scales are rejected.
- Analytic Jacobian tests meet the finite-difference threshold.
- Every residual row has an audit descriptor with source ID, readable template, named bindings, category and finite positive scale.
- Invalid geometry, NaN and Inf are reported before factorization.

## M2 — Dense solver and diagnostics

Required cases:

1. Exactly determined linear system converges to the known solution.
2. Nonlinear distance/circle system converges from at least three documented nearby initial guesses.
3. One-equation/two-variable system converges and reports local DOF `1`.
4. Duplicate rows converge geometrically and report redundant source candidates.
5. Contradictory equations do not report `Converged`; they report nonzero validated residual and conflict candidates once M4 exists.
6. A configuration-dependent rank drop sets `is_singular` without conflating it with nonlinear termination.
7. Iteration-limit and stagnation paths retain the last finite state and return truthful termination.
8. Scaling tests retain the same solution classification and normalized accuracy.

## M3 — Adversarial verification harness

- Property-generated linear systems report the independently known rank and nullity and recover a valid known solution when consistent.
- Property failures print a deterministic seed/case that reproduces the failure.
- Construct-valid nonlinear fixtures recover from documented local perturbation ranges without relaxing hard-residual validation.
- Translation and rotation metamorphic transforms preserve normalized residuals, rank, DOF and diagnosis.
- Uniform scale factors `1e-6`, `1`, and `1e6` preserve normalized accuracy and classification.
- Permuting insertion order changes neither accepted geometry beyond tolerance nor deterministic source ordering.
- Injected non-finite residual/Jacobian and invalid-scale cases terminate without committing a non-finite state.
- Rejected iteration steps never become the returned accepted state.
- Stagnation and iteration-limit outcomes retain the last finite independently validated state.
- Solve-trace invariant tests verify accepted/rejected accounting and non-increasing accepted objective within documented numerical tolerance.
- Benchmarks are recorded separately and do not alter correctness thresholds.

## Sketch MVP

### S1 underconstrained triangle

- Construction and initial state match `docs/SCENARIOS.md`.
- Solve returns `Converged` termination with local DOF `1`.
- All hard residuals validate to `<= 1e-9`.
- Moving point C with a temporary target changes only its permitted one-dimensional motion after hard projection.
- Drag target error is minimized without hard residual exceeding tolerance.
- Removing the drag target does not cause a branch jump or large unrelated motion.

### Constraint behavior

For each supported constraint:

- at least one exact valid fixture;
- at least one perturbed recovery fixture;
- analytic/local-AD Jacobian comparison;
- translation and rotation metamorphic test;
- uniform-scale metamorphic test where applicable;
- explicit invalid-geometry behavior.

### Driving/reference dimensions

- driving dimensions add equations and affect DOF;
- reference dimensions add no equation and report the solved value;
- toggling driving/reference produces deterministic source IDs and reports.

## Diagnostics and decomposition

- Two disconnected components solve independently.
- Editing one component leaves the other component unchanged within `1e-12` and does not iterate it.
- Exact duplicated source constraint is marked redundant rather than conflicting.
- `S2 conflicting rectangle` fails validation and names the two incompatible high-level dimensions as conflict candidates.
- If one source constraint emits multiple rows, diagnostics name the source once.
- Underconstraint, redundancy and singularity may coexist in the report; none is represented only as a mutually exclusive termination enum.

## Linkage MVP

### L1/L2 four-bar

- Closure residual at every accepted driver sample is `<= 1e-9` normalized.
- The ground body remains unchanged.
- A sweep uses the previous accepted pose as the next initial state.
- The open scenario retains its documented orientation sign; the crossed scenario retains the opposite sign.
- No step silently changes assembly mode.
- A near-singular sample raises the singularity indicator or documented conditioning warning.

### L3 slider-crank

- Revolute anchors coincide within tolerance.
- Slider transverse displacement and relative orientation residuals validate within tolerance.
- The slider remains on its guide throughout the sweep.
- Position and velocity solves both validate their respective equations.

## WASM/SVG demonstration crate

The separate `geosolve-demo-web` crate must:

- build for `wasm32-unknown-unknown` with no backend service;
- load from the Trunk-generated static output;
- offer hardcoded selectors for S1, S2, L1, L2 and L3 by the end of M6;
- add the S3 selector with its curve/tangency implementation in M7;
- construct scenarios through `geosolve-sketch`/`geosolve-linkage` public APIs;
- contain no duplicate constraint equations;
- render solved geometry in SVG (Canvas may be added later but is not required);
- support pointer dragging in S1;
- support a driver slider for L1 and L3;
- show termination, validated maximum residual, rank, DOF, iterations, branch label and singularity/conflict/redundancy notices;
- show constraints grouped by high-level source with readable expanded residual rows, current named bindings, target/unit, scale, raw residual and normalized residual;
- update the equation panel from the same accepted state as the geometry after every drag or driver step;
- distinguish hard, temporary/driver and preference rows visually;
- contain no handwritten duplicate equation implementation: audit rows must come from core/domain audit snapshots by M5/M6;
- visibly preserve the previous valid geometry when a requested edit fails rather than drawing NaNs;
- have at least one automated WASM/browser smoke test by M9;
- use plain text/Unicode or simple HTML successfully; MathML/LaTeX and full symbolic simplification are explicitly not required.

## Regression and oracle policy

- Every solver bug gets a minimal regression test before or with the fix.
- Differential tests compare geometric validity, DOF/status and branch continuity—not identical internal coordinates or iteration counts.
- SolveSpace and PlaneGCS are references/oracles, not dependencies.
- A convergence flag from an external solver is never accepted without local residual validation.
