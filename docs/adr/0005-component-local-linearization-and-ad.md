# ADR 0005: Canonical component-local linearization and local AD

Status: accepted

## Context

The M1-M7 baseline lets every residual return raw values and analytic local Jacobian blocks, then assembles those blocks through a dense layout that can include every problem column. Sparse solving, editable curves and spatial poses need one allocation-conscious linearization representation without replacing proven analytic residuals or their finite-difference oracle.

## Decision

M9 introduces one internal canonical linearization for a reduced connected component. It contains:

- deterministic component-local active variable-block ranges;
- normalized residual rows and source/row identity;
- normalized dense local Jacobian blocks in declared incidence order;
- `Evaluated` status on every successfully constructed numeric block.

Residual evaluators may write values and raw-tangent Jacobian blocks into caller-provided storage through an additive public capability on the existing public residual trait. Returning `None` selects the legacy value/Jacobian methods; returning `Some` is authoritative success or failure and is never a fallback sentinel. This caller-storage capability is public but unstable before 1.0. A component linearization never allocates columns for variables outside that component. M9 owns only this component-local block IR and its dense consumer. M12 assigns indexed block coordinates and materializes COO/triplet and sparse storage from the same evaluated blocks rather than evaluating equations independently.

Analytic evaluators remain supported. The local forward-AD trait and object-safe adapter remain crate-private; they are not a public formula/plugin interface. Their dual width is the sum of the incident tangent dimensions for one residual block, not the whole problem dimension. AD seeds normalized tangent coordinates through each variable's retraction and passes those finite normalized-coordinate derivatives directly into canonical residual normalization. It never divides by a tiny step scale merely for core to multiply by that scale again.

Branch, span, winding, contact-neighborhood, tangent-orientation, active-bound and assembly-mode choices are immutable discrete inputs to an evaluation. They are never dual values and AD never selects or changes them.

Central finite differences through the same variable retraction remain the independent derivative oracle. Every analytic or AD residual must agree with that oracle to the acceptance tolerance away from explicitly reported nonsmooth states.

The implementation is pure safe Rust. It adds no native solver FFI and no `unsafe` code. The AD formula trait and adapter remain private or crate-private until built-in CAD and pose families prove a stable seam. Public fused storage is an unstable pre-1.0 extension of the already-public residual evaluator trait, documented in the crate API rather than presented as a stable third-party plugin commitment.

## Error contract

Linearization distinguishes at least:

- invalid or out-of-domain geometry;
- degenerate geometry;
- nondifferentiable state;
- ambiguous discrete neighborhood/branch;
- non-finite evaluator output;
- dimension or incidence mismatch.

No error is converted into zero residual, zero derivative or convergence.

Successful IR blocks are `Evaluated`. Any value, Jacobian, fused, shape, category or finiteness failure aborts canonical construction with a structured error before a partial numeric IR can be consumed; failed numeric blocks are not retained in the IR. Best-effort audit is separate and can retain fresh displayed values with a failed evaluation status.

## Consequences

- M9 changes internal assembly and adopts the accepted M9 status/numerical-rank contract, but does not change accepted geometry, source ordering, audit equations or hard/secondary priority semantics.
- M12 owns indexed triplet/COO materialization and sparse storage as conversions from this IR, not as a second residual implementation.
- Public third-party curve or AD-formula plugins are not implied. Before 1.0, source compatibility can change at an explicit milestone review; evolving public enums are non-exhaustive and changes are documented with the owning milestone.
- Benchmark-local component sharding remains an M8 baseline workaround. M9 has an internal same-workload shape regression proving component matrix storage remains constant as disconnected global columns grow, but no timed performance threshold and no public exposure of the private IR solely for Criterion.
- Existing benchmark/core fused evaluators are sufficient production-path evidence for M9; migrating every sketch/linkage residual is not an M9 gate.
