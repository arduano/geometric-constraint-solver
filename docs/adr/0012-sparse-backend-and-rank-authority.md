# ADR 0012: Sparse backend and rank authority

Status: accepted

## Context

M16 originally called for `faer` sparse storage and rank-revealing sparse least
squares. `faer` 0.24.4 provides reusable sparse symbolic QR and numerical
least-squares solves, but its sparse QR is non-pivoted and exposes neither a
numerical rank nor a singular spectrum. Treating successful factorization,
structural matching or an unpivoted diagonal as the M9 numerical-rank result
would make rank and mobility order-dependent and could turn a singular system
into a false success.

## Decision

M16 uses deterministic `faer` CSC storage and sparse QR for the positively
damped LM system:

```text
[ J              ] delta ~= [ -r ]
[ sqrt(lambda) I ]          [  0 ]
```

Positive finite damping gives this augmented step system full column rank.
Every sparse result is checked for finite dimensions and values, model decrease,
normal/KKT residual and bound feasibility before it can produce a trial. Sparse
construction, factorization or validation failure falls back to the dense path
with a typed report reason.

Dense SVD remains the authoritative rank-revealing path for the undamped hard
Jacobian, including numerical rank, left/right nullity, singular values,
near-singular classification, mobility and accepted sensitivity. Auto routing
keeps small, rank-deficient and near-singular systems dense. Structural matching
and Dulmage-Mendelsohn partitions remain separate symbolic facts and never prove
numerical rank.

The caller approved this amendment to the literal M16 sparse-rank wording on
2026-07-17 rather than starting a project-owned pivoted sparse factorization.
Any future sparse rank authority requires its own numerical contract and dense
differential oracles before it may replace SVD in a report.

## Consequences

- M16 scales damped hard steps and hierarchy operations without weakening the
  M9 rank contract.
- Backend and rank evidence are explicit: sparse step execution never implies
  sparse rank certification.
- The benchmark-derived crossover affects performance only, never residual,
  rank, branch or acceptance tolerances.
- The implementation remains pure safe Rust with no native solver FFI.
