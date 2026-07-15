# Overnight report: Human checkpoint A

> Historical record only. This report describes the completed M1-M4 bootstrap block and is not current project status. See `START_HERE.md` and `PLAN.md` for active guidance.

Date: 2026-07-14

At the time of this historical checkpoint, the autonomous M1-M4 block was complete and M5 had not begun.

## Milestones and commits

| Milestone | Commit | Result |
| --- | --- | --- |
| M1 problem representation | `793171d` | Complete |
| M2 dense nonlinear solver | `86b0e19` | Complete |
| M3 adversarial verification | `7ac73f8` | Complete |
| M4 decomposition and diagnostics | `67d87db` | Complete |

## M1: problem representation

### Files and APIs

- Added `crates/geosolve-core/src/{error,problem,residual,variable}.rs`.
- Added stable `VariableId`, `ResidualId`, and `SourceConstraintId` stores.
- Added scalar, `Vec2`, and `Pose2` variable blocks, packed ambient/tangent layouts,
  local increments, residual categories, executable residual blocks, audit metadata,
  dense assembly, and central finite-difference Jacobian reports.
- Added `crates/geosolve-core/tests/m1.rs`.

### Mathematical behavior

- Residual rows are normalized by their characteristic residual scales.
- Jacobian columns differentiate with respect to normalized tangent coordinates:
  `raw_delta = step_scale * normalized_delta`.
- Scalar, vector, and unwrapped planar-pose increments are applied per block.
- Residual/Jacobian dimensions, scales, IDs, geometry errors, and all finite values
  are validated before dense matrices are returned.

### Acceptance

- Stable generational IDs and deterministic packing pass.
- Heterogeneous residual incidence and dense matrix ranges pass.
- Scalar quadratic, two-point distance, and transformed `Pose2` fixtures pass.
- Analytic Jacobians satisfy central finite-difference relative error `<= 1e-6`.
- Invalid scales, NaN/Inf values, stale IDs, and invalid geometry are rejected.

### Limitations

- M1 intentionally contains no nonlinear iteration or rank diagnosis.

## M2: dense solver, rank, and validation

### Files and APIs

- Added `crates/geosolve-core/src/solver.rs`.
- Added `Problem::solve`, `SolverConfig`, `SolveReport`, `SolveTrace`, accepted-state
  audit snapshots, independent hard validation, rank/DOF, and source diagnostics.
- Added `crates/geosolve-core/tests/m2.rs`.

### Mathematical behavior

- Levenberg-Marquardt steps solve the augmented dense least-squares system
  `[J; sqrt(lambda) I] delta = [-r; 0]`.
- Dense QR is attempted first and SVD is the rank-deficient fallback.
- Damping adapts from actual/predicted reduction, with deterministic accept/reject
  records and block-local normalized step limits.
- Only finite accepted trial states are committed. The returned state is the last
  accepted state.
- Numerical rank uses component-local singular values and a reported relative
  tolerance, defaulting to `1e-10`.
- `Converged` requires a fresh hard-residual evaluation at `<= 1e-9` normalized
  maximum residual.
- Hard rows alone define the solve objective. Temporary and preference rows are
  evaluated and audited but are not silently approximated with weights.

### Acceptance

- Exact linear and nonlinear circle systems pass.
- One-equation/two-variable systems report one local DOF.
- Duplicate rows converge and produce deterministic redundancy candidates.
- Contradictions never converge and retain a finite accepted state.
- Configuration-dependent rank loss is separate from nonlinear termination.
- Iteration-limit and stagnation paths retain the last finite accepted state.
- Required classifications and normalized accuracy pass at `1e-6`, `1`, and `1e6`.
- Audit values and incident variable values match the returned accepted state.

### Limitations

- Hierarchical temporary/preference optimization is deferred for checkpoint review;
  those rows are currently validation/audit-only.

## M3: adversarial verification

### Files and APIs

- Added `crates/geosolve-core/tests/m3.rs` and test support helpers.
- Added `crates/geosolve-core/benches/small_dense.rs` using Criterion 0.8.
- Kept production API changes limited to deterministic source grouping and a tested
  private QR/SVD least-squares seam.

### Mathematical behavior and tests

- Rank-by-construction matrices use independent pivot rows followed by exact linear
  combinations, then deterministic row/column permutations.
- Exact, underdetermined, and overdetermined shapes run 32 generated cases each.
- Explicit rank-zero, rank-one, and full-rank cases are also verified.
- Construct-valid circle intersections recover from five documented perturbations.
- Translation, rotation, and uniform scaling preserve normalized residuals,
  rank/DOF, diagnosis, and the selected local branch.
- Variable/residual permutations compare semantic geometry against the independent
  known solution and preserve deterministic source diagnostics.
- Failure tests cover non-finite values/Jacobians, invalid scales, singular dense
  fallback, rejected trials, accepted-then-invalid trials, stagnation, and iteration
  limit.
- Trace tests verify component-local accepted costs do not increase beyond
  `128 * EPSILON` bookkeeping tolerance and rejected states are never committed.

### Reproduction

Fixed ChaCha base seed:

```text
4d33a7419c2e5b7088d4f1036ac952ef117b8d60c4aa39e275018bc6de42f90a
```

Shape tags `0x11`, `0x22`, and `0x33` are XORed into the final byte for exact,
underdetermined, and overdetermined cases. Failure persistence is disabled and
failures print the effective seed and minimized case.

```bash
cargo test -p geosolve-core --test m3 property_linear_systems_have_constructed_rank_nullity_and_solution -- --exact --nocapture
```

### Benchmark

```bash
cargo bench -p geosolve-core --bench small_dense
```

The final post-M4 run completed without correctness thresholds. Observed ranges on
this machine were approximately `7.07-7.19 us` for 2x2, `16.13-16.55 us` for 4x4,
and `54.49-55.90 us` for 8x8. M4's additional component construction, validation,
and diagnostics increased these timings relative to the pre-M4 run; no performance
gate was weakened.

## M4: decomposition, elimination, and diagnostics

### Files and APIs

- Added `crates/geosolve-core/src/analysis.rs`.
- Added original incidence analysis and reduced structural component summaries.
- Added trusted `ResidualBlock::fixed_variable` and `ResidualBlock::exact_alias`
  constructors plus explicit validated elimination declarations.
- Added `Problem::analyze_incidence`, `Problem::structural_summary`, and
  `Problem::solve_decomposed`.
- Extended solve reports with component reports/traces, structural counts,
  redundant rows, fully redundant sources, sources containing redundant rows,
  singular rows, bounded conflict candidates, and audit annotations.
- Added `crates/geosolve-core/tests/m4.rs`.

### Mathematical behavior

- Original variable/residual incidence remains available for audit. Fixed coordinates
  and trusted equality rows are removed, and alias roots are coalesced, before solve
  components are formed.
- Alias-member Jacobian columns are summed into their shared active coordinate.
- Eliminated equality/fixed rows remain part of independent returned-state hard
  validation and can prevent convergence.
- Each reduced component is assembled, iterated, validated, and rank-tested
  independently. Global rank and DOF are sums of component-local results and
  component-local relative thresholds avoid cross-scale rank loss.
- Cached components are reused only when stable IDs, structural signatures, and
  current-tolerance independent validation match. Reused components have zero
  iterations and an empty trace.
- Redundancy is diagnosed per scalar row. `redundant_sources` means every active row
  from that source is redundant; partial groups appear only in
  `sources_containing_redundant_rows` and row annotations.
- Conflict candidates are deterministic restoring candidates from non-recursive
  source deletion/re-solve. Bounds are 12 candidate sources and 24 active dimensions
  per failed component.
- Deleting a trusted fixed/alias source also removes that elimination semantic in the
  trial plan.
- Every returned-state hard, temporary, and preference residual value/Jacobian is
  checked. Failed audit rows remain present with finite placeholders and an explicit
  evaluation error status.

### Acceptance

- Disconnected and isolated graph components pass.
- Editing one component reuses an unaffected component bitwise with zero iterations.
- Fixed scalar/`Vec2` blocks and exact alias chains reduce active dimensions and
  independently validate eliminated rows.
- Fixed-variable graph splitting and alias-member Jacobian summation pass.
- Same-source, separate-source, and partial-row redundancy attribution pass.
- Contradictory scalar/vector sources and disconnected conflicts produce deterministic
  source candidates once per high-level source.
- Underconstraint, redundancy, and singularity coexist independently in reports.
- Zero-gradient and dependent-nonzero singular configurations are annotated.
- Structural signatures remain stable across value/configuration-dependent rank loss.
- Elimination, redundancy, and conflict behavior pass at scales `1e-6`, `1`, and
  `1e6`.
- Invalid auxiliary residuals/Jacobians cannot coexist with `Converged`.

### Limitations

- Numerical solving remains dense within each reduced component.
- Conflict output is a bounded deterministic candidate set, not a globally minimal
  unsatisfiable core.
- `solve_decomposed` requires callers to accurately identify externally edited
  variables.
- Exact elimination is deliberately restricted to trusted core-owned residual
  constructors; broader domain ergonomics need checkpoint review.

## Verification commands

The following milestone gate was run after every milestone and passed after M4:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p geosolve-demo-web --target wasm32-unknown-unknown
nix-shell ../../shell.nix --run 'trunk build --release'  # from crates/geosolve-demo-web
```

Final outcomes:

- format: passed;
- Clippy with warnings denied: passed;
- workspace tests: passed;
- `geosolve-core`: 62 tests passed (`3` unit, `11` M1, `11` M2, `12` M3,
  `25` M4);
- WASM target check: passed;
- Trunk 0.21.14 release build: passed through the repository Nix shell;
- Criterion benchmark: completed;
- `git diff --check`: passed;
- Rust source search found no `unsafe` block or native `extern "C"` FFI.

Direct `trunk build --release` was unavailable in the invoking shell because `trunk`
was not on `PATH`; the declared `shell.nix` toolchain was used successfully instead.

## Decisions deferred to Human checkpoint A

- Public ownership and mutability of `Problem`, residual evaluators, and packed state.
- Whether temporary/preference rows should use a strict null-space hierarchy and the
  exact public interaction API for that policy.
- Damping defaults, step-limit policy, and public trace detail.
- Rank/DOF/singularity wording for reduced coordinates and source diagnostics.
- Audit snapshot field naming, failed-row representation, and browser presentation.
- Domain ergonomics for trusted fixed/equality elimination declarations.
- Conflict/redundancy candidate wording and how aggressively bounded deletion should
  run in interactive use.
- Performance work after profiling; the current implementation prioritizes dense
  correctness and diagnosis.

No sketch/linkage model, browser interaction, sparse solver, curve primitive,
continuation, or M5 behavior was added.

## Diff and Git state

Diff from pre-M1 handoff commit `5758589` through M4 commit `67d87db`:

```text
16 files changed, 10598 insertions(+), 104 deletions(-)
```

At that historical checkpoint the branch was clean immediately after `67d87db`.
Creating the report added only uncommitted `OVERNIGHT_REPORT.md`; no implementation
changes were pending at that time.
