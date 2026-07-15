# ADR 0007: Persistent SolveSession, bounds and active sets

Status: accepted

## Context

The baseline recompiles domain problems and `Problem::solve_decomposed` trusts callers to list edited variable IDs. Bounded curve contacts and drivers are often rejected after an unconstrained solve. Production editing needs retained compiled structure, automatic dirty tracking, mathematical bounds and atomic commit semantics.

## Decision

M10 introduces a persistent `SolveSession` with three separated layers:

- immutable compiled topology: variable/residual/source identity, incidence, elimination declarations, structural signatures and audit descriptors;
- mutable accepted state: finite ambient values, explicit branch/domain/active-set state and revision;
- mutable source parameters: dimensions, drivers, targets and bounds with their own revisions.

Every edit is a typed transaction. The session derives dirty variables, residuals and components from changed revisions; callers cannot provide an incomplete dirty-ID hint. Structural edits rebuild affected topology and symbolic data. Parameter/state edits retain compatible runtime IDs, component layouts, factorization structure and accepted caches.

Solving operates on a candidate patch or clone. The session commits continuous values, active bounds and discrete domain state together only after independent hard and domain/branch validation. Failure leaves the entire previous accepted revision unchanged.

Scalar variables and named tangent coordinates may have finite lower/upper box bounds. Bounds are validated at construction and participate in a deterministic projected/active-set LM policy. A trial step is limited to its first bound event; the active set is updated from bound location and directional/KKT evidence, not from an unconditional post-solve clamp.

Each bound has stable source/domain identity and reports `Inactive`, `ActiveLower`, `ActiveUpper` or `Fixed`. The report retains equality rank before bounds, computes bidirectional mobility from the equality Jacobian augmented by active-bound normals, and separately reports whether a nonzero one-sided direction exists in the feasible tangent cone. Endpoint-active contact, a radius at its positive lower domain and a bounded driver are therefore visible mathematical states.

Secondary objectives never override hard rows or bounds. Branch, span, winding, contact-neighborhood and assembly-mode state remains outside AD and changes only through explicit validated domain transitions.

Diagnostic budgets are session configuration. Redundancy/conflict output includes configured budget, consumed work, `Complete`/`Truncated`/`Skipped` and an incomplete reason under the M8 contract.

The implementation is pure safe Rust with no `unsafe` code or native solver FFI. Session/compiler traits remain internal until both domain migrations prove them.

## Transition

Baseline `Problem` and `solve_decomposed` remain supported while M10 is implemented, but their caller-supplied edit hints are not the target persistence contract. `geosolve-sketch` adopts the session through M13 and `geosolve-linkage` through M14/M16. No compatibility adapter may silently discard bound or branch state.

## Consequences

- One-component edit/re-solve has a stable benchmark boundary and cannot accidentally reuse a dirty component.
- Candidate state and accepted state are explicit, enabling reliable rollback and audit snapshots.
- Bound-aware rank/mobility is part of reports, not a domain-side guess.
- General inequalities, unilateral physical contact and complementarity physics are not introduced.
