# ADR 0011: Adaptive and pseudo-arclength continuation

Status: accepted

## Context

The planar linkage baseline changes one hard driver through deterministic fixed
target subdivisions. That preserves accepted-prefix rollback and explicit branch
checks, but it has no predictor, adaptive retry or way to continue through a
physical driver turning point. Treating a failed natural step as permission to
switch branches or silently replace the driver with a soft objective would make
the path depend on solver accidents and would violate hard-priority semantics.

Continuation must also follow ADR 0006. Pose predictors and control equations use
right/body-local tangent coordinates, retraction and manifold local difference,
not ambient pose subtraction.

## Decision

`geosolve-core` owns domain-independent continuation math:

- validated adaptive step policy and deterministic retry state;
- the oriented unit right-null tangent of `[J_q J_lambda]` using the accepted
  component rank threshold;
- an exact requirement that the augmented numerical right nullity is one;
- independent validation of the augmented tangent equation before publication;
- a hard pseudo-arclength row over normalized manifold local differences, with
  structured audit metadata and finite-difference Jacobian coverage. Its public
  `Problem` addition API resolves coefficients from the referenced variables'
  authoritative step scales; callers cannot supply independent row scales.

The first tangent has an explicit increasing/decreasing parameter orientation.
Later tangents are oriented by positive dot product with the previous accepted
tangent. A zero parameter component at a fold is valid only when the previous
tangent supplies an unambiguous orientation.

`geosolve-linkage` exposes natural-target and explicit pseudo-arclength modes over
one selected hard driver. Both use body-local predictors and adaptive retry
without mutating accepted geometry. Natural mode verifies the post-corrector
tangent before commit and stops at parameter reversal; it never switches to
pseudo mode.

Every request, including a zero-distance natural request, starts with a fresh
ordinary fixed-driver physical solve and branch validation. A rejected entry
returns `InitialRejected`, never `Completed`. The private `SolveSession` used to
form a tangent must retain every accepted body pose bit-for-bit; any silent
private correction rejects tangent construction rather than detaching the
tangent from the linkage state.

Corrector locality is an acceptance condition, not only a next-step heuristic.
For normalized path step `ds` and normalized manifold correction norm `c`, the
candidate is local only when

```text
c <= minimum(maximum_correction,
             maximum_correction_step_ratio * ds)
```

Both limits are positive finite policy fields. A nonlocal candidate is discarded
and retried at a smaller step; exhausting the minimum returns
`CorrectionNotLocal`. Natural mode also evaluates every explicit branch monitor
and prismatic branch at the uncorrected predictor endpoint. An endpoint violation
is shrunk and retried before correction. This is not deterministic interval
tracing and does not claim to detect every boundary crossing whose two endpoints
retain the same branch state; the endpoint check plus locality policy is covered
for the documented built-in fixtures.

Pseudo mode temporarily replaces the selected fixed driver target with a scalar
parameter, adds the core pseudo-arclength row, and solves the augmented corrector.
That problem is ephemeral. A successful augmented candidate seeds a separate
ordinary fixed-driver physical solve, which is freshly validated against core
hard rows, linkage equations and explicit branch state before commit. Only this
ordinary physical report, source mapping, rank, audit and diagnostics are
published. A sample separately retains only the corrector's linear backend and
typed sparse-fallback summary so backend parity is auditable; it never exposes
the ephemeral rows, numerical rank/nullity values, spectrum, residuals or
report. The typed summary may state `RankAmbiguous` only as a routing reason.
The parameter/control row can never affect published physical rank or audit.

Every rejected trial retains the previous accepted prefix. An ordinary physical
solve accepted by hard/branch validation but rejected by continuation locality or
post-corrector tangent policy is published in `rejected_attempts`. Existing
`drive_to` behavior remains unchanged, including rollback when a displacement
target beyond the L3 fold has no physical root.

Predictors, augmented correctors and ordinary physical correctors run on cloned
`Linkage` values. Manifold differences, correction normalization, branch checks,
post-corrector tangents, sample construction and controller acceptance all finish
before the candidate is atomically assigned to retained state. No fallible
post-commit calculation is permitted.

## Consequences

- The displacement-driven L3 fixture can cross its `x = 4.75 * model_scale` fold
  only through an explicit pseudo-arclength request.
- Natural continuation remains a truthful parameterization and stops before
  committing a tangent reversal with the explicit `PseudoArclengthRequired`
  status.
- Scale-normalized behavior is covered at `1e-6`, `1` and `1e6`, including
  manifold finite differences. Forced dense/sparse correctors run without
  fallback at all three scales, and physical endpoint parity covers geometry,
  rank/mobility, diagnostics, audit structure and branch state.
- Common-left `SE(2)` continuation equivariance is covered at the accepted
  physical endpoint.
- M16 implements the planar single-driver slice. Spatial assemblies and
  multi-driver continuation remain allocated to later milestones.
