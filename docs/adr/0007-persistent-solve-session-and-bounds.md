# ADR 0007: Persistent SolveSession, bounds and active sets

Status: accepted

## Context

The baseline recompiles domain problems and `Problem::solve_decomposed` trusts callers to list edited variable IDs. Bounded curve contacts and drivers are often rejected after an unconstrained solve. Production editing needs retained compiled structure, automatic dirty tracking, mathematical bounds and atomic commit semantics. The user-approved next cut requires this lifecycle to be proven first through an embeddable sketch editing consumer rather than as unused core infrastructure.

## Decision

M10 introduces a persistent `SolveSession` with three separated layers and a reusable public `SketchSession` domain consumer:

- immutable compiled topology: variable/residual/source identity, equation and row shape, incidence, elimination declarations and structural signatures;
- mutable accepted state: finite ambient values, explicit branch/domain/active-set state and revision;
- mutable source parameters: dimensions, drivers, targets, bounds and accepted audit labels/bindings with their own revisions. Audit-only refresh cannot replace evaluators, scales or equations.

Every edit is a typed transaction. The session derives dirty variables, residuals and components from changed revisions; callers cannot provide an incomplete dirty-ID hint. Structural edits rebuild affected topology and symbolic data. Parameter/state edits retain compatible runtime IDs, component layouts, factorization structure and accepted caches. Domain consumers rebuild only changed source payloads against retained mappings; a non-structural edit does not perform a throwaway full topology compile.

`SketchSession` owns sketch-to-core compilation, accepted sketch geometry, source mappings and transaction results while delegating numerical lifecycle to `SolveSession`. It exposes no web selection, hit-testing, tool, rendering or storage state. M11 commands target persistent sketch IDs and use this transaction boundary; browser code never edits packed core values directly.

Solving operates on a candidate patch or clone. Residual evaluators are behavior-pure; shared interior mutability may record telemetry but cannot affect equations. The session commits continuous values, active bounds and discrete domain state together only after independent hard and domain/branch validation. Reused components skip nonlinear iterations but still receive fresh hard-row/Jacobian validation and diagnostics. Failure leaves the entire previous accepted revision unchanged.

Scalar variables and named tangent coordinates may have finite lower/upper box bounds. Bounds are validated at construction and participate in a deterministic projected/active-set LM policy. A finite outside initial guess is projected before evaluation; subsequent trial steps are limited to their first bound event. The active set uses an independent projected-normal basis and multiplier/directional KKT evidence, not an unconditional post-solve clamp. Weak one-sided critical-cone curvature is reported conservatively as stalled whenever finite sampling cannot certify optimality.

Each bound has stable source/domain identity and reports `Inactive`, `ActiveLower`, `ActiveUpper` or `Fixed`. The report retains equality rank before bounds, computes bidirectional mobility from the equality Jacobian augmented by active-bound normals, and separately reports whether a nonzero one-sided direction exists in the feasible tangent cone. Endpoint-active contact, a radius at its positive lower domain and a bounded driver are therefore visible mathematical states.

Secondary objectives never override hard rows or bounds. Branch, span, winding, contact-neighborhood and assembly-mode state remains outside AD and changes only through explicit validated domain transitions.

Diagnostic budgets are session configuration. Redundancy/conflict output includes configured budget, consumed work, `Complete`/`Truncated`/`Skipped` and an incomplete reason under the M8 contract.

The implementation is pure safe Rust with no `unsafe` code or native solver FFI. Generic compiler extension traits remain internal; the concrete sketch session/document workflow becomes public only at its milestone gate.

## Transition

Baseline `Problem` and `solve_decomposed` remain supported, but their caller-supplied edit hints are not the persistent-session contract. `geosolve-sketch` adopted the session in M10 and layers `SketchDocument` commands/history over it in M11. `geosolve-linkage` adopts the architecture in M17/M18. No compatibility adapter may silently discard bound or branch state.

## Consequences

- One-component edit/re-solve has a stable benchmark boundary and cannot accidentally reuse a dirty component.
- Candidate state and accepted state are explicit, enabling reliable rollback and audit snapshots.
- Bound-aware rank/mobility is part of reports, not a domain-side guess.
- The M13-M14 playground can project drag and report failures through public sketch APIs without owning equations or accepted state.
- General inequalities, unilateral physical contact and complementarity physics are not introduced.
