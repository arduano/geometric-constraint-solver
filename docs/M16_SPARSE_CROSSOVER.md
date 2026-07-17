# M16 sparse and hierarchy evidence

This note records the completed M16 sparse numeric and cross-hard-component
hierarchy evidence. Continuation is covered separately by ADR 0011 and the
core/linkage M16 regressions. ADR 0012 records the caller-approved division of
responsibility between sparse damped steps and dense rank authority.

## Policy

`LinearSolveBackendPolicy::Auto` selects sparse QR only when the current LM
free-column system satisfies all of:

- at least 256 source Jacobian rows;
- at least 256 ordered free columns;
- at least 256 canonical source Jacobian structural entries;
- source Jacobian density at most `1/128` (0.0078125).

Auto additionally requires the current free-column Jacobian to be full rank and
outside the reported near-singular band. Rank-deficient, near-singular, or
otherwise invalid current rank evidence selects dense LM with typed
`RankAmbiguous` evidence. `SparsePreferred` still attempts sparse QR for parity
tests. Faer sparse QR is not used as a numerical-rank oracle: all published
rank, nullity, conditioning and near-singular fields remain dense-SVD results.
An end-to-end 256-row full-rank regression also requires `Auto` to report an
actual sparse-QR step, so the standalone crossover predicate cannot pass while
real routing remains permanently dense.

The damping identity is excluded from the density calculation. The density
guard is deliberately the measured 256-column chain envelope: that fixture has
511 structural entries in 65,536 positions, immediately below `1/128`. No 5%
density boundary was benchmarked, so Auto no longer presents 5% as
benchmark-derived. These constants keep the preserved M8 components, whose
tangent dimensions are at most 33, on the dense path. Rank, nullity and spectrum
remain dense-SVD results at every size, so the crossover changes only the
damped LM step backend.

## Release Probe

Recorded 2026-07-16 on Linux x86-64, Intel Core i5-14400F (10 cores, 16
threads), Rust release profile, faer 0.24.4 with `std` and `sparse-linalg` only.
The connected scalar chain has one component, equal row/column counts and
`2 * size - 1` canonical structural entries. Diagnostics unrelated to backend
selection were disabled, while returned-state validation and dense SVD rank
reporting remained enabled.

The structural count is the canonical declared block envelope. The public
evaluator interface declares incidence by variable block, so explicit zero
slots remain structural to keep matching, signatures and symbolic reuse
independent of the current numerical state.

Command:

```bash
cargo bench --locked -p geosolve-core --bench connected_sparse
```

Criterion 95% timing intervals:

| columns | dense solve | sparse cold symbolic + numeric/solve | sparse reused numeric/solve |
| ---: | ---: | ---: | ---: |
| 64 | 1.3015-1.3386 ms | 1.2868-1.3745 ms | 1.3200-1.4269 ms |
| 128 | 9.3677-9.6351 ms | 8.6944-8.8648 ms | 8.8278-9.4832 ms |
| 256 | 68.024-69.686 ms | 62.617-63.719 ms | 62.783-66.612 ms |

The 64-column intervals overlap and symbolic reuse does not help the complete
solve/report timing there. The 128-column cold result is faster, but the reused
interval overlaps dense. At 256, both sparse intervals are below the dense
interval. Auto therefore waits until 256 rather than selecting the first
observed win. Future benchmark descendants should revisit the constants after
sparse-compatible rank diagnostics exist; current timings include the common
dense SVD cost required by the M9 rank contract.

The benchmark performs central finite-difference checks for both benchmark-only
residual implementations at every represented size before entering any timed
boundary.

## Symbolic Cache

Each `Problem` holds at most eight exact sparse symbolic entries. Entries are
immutable and insertion-ordered; inserting a ninth distinct exact
pattern/free-column key evicts the oldest entry. Cache hits do not refresh age,
so clone/reuse and eviction are deterministic.
Public reuse counts include only successful sparse numeric solves that reused a
symbolic entry; a failed factorization with reused symbolic analysis is only an
attempt and is not counted as a successful reuse.

## Cross-Component Hierarchy

Temporary residual incidence is treated as a deterministic hypergraph over the
unchanged hard components. Preference connectivity is then seeded with those
Temporary groups, so a Preference pass includes every component needed to
protect each independently attained Temporary cost. Fixed-only residuals remain
acceptable without a step only after both values and derivatives validate at
the returned state; only incidence that cannot map through the accepted
elimination plan is an evaluation failure.

Singleton groups retain the M5/M10 dense active-set and curvature path. Coupled
groups materialize hard-nullspace bases independently per component and apply
one composite step without constructing a full-group `n x nullity` basis. Small
groups use a dense reduced working set. Groups with at least 128 reduced
coordinates use projected CGLS for both bounded and unbounded objectives. A
component at or above that threshold represents its hard rows directly in the
projector instead of materializing a component-wide nullspace basis.

The large bounded path maps coordinate bounds to block-local reduced normals.
Its deterministic active set starts at the feasible zero step, installs fixed
and independent currently-active normals, advances to the first finite bound
event, and releases lower/upper normals whose KKT multiplier has the wrong
sign. Fixed normals are never released. Protected Temporary rows and active
bound normals are combined by a row-space projector; this stores no global
`n x nullity` matrix. Duplicate alias bounds merge into the authoritative root
coordinate interval before normals are formed.

Before a projected bounded step can enter nonlinear line search, the solver
independently checks projector equations, active affine equations, full-coordinate
bound feasibility, finite values, the projected/KKT gradient residual and
predicted objective decrease. Every nonlinear trial then restores and validates
hard rows and protected Temporary levels. Incomplete operator evidence is
`Stalled`, not `Optimal` or `NumericalFailure`; positive-cost one-sided curvature
remains conservative. A globally zero-cost large group still returns `Optimal`
before selecting a basis backend.
Every trial then reprojects and validates each hard component separately and
reoptimizes every protected Temporary group before Preference is measured.

`PrioritySolveReport` records deterministic group/component identity, scope,
backend, largest explicit local nullspace block, and each protected Temporary
cost and preservation result. `ComponentSolveReport` separately records hard
cache reuse, hierarchy participation, and hierarchy-induced state changes.
Session variable, residual, source, and bound edits invalidate affected
hierarchy groups while preserving hard reuse for otherwise clean components.
Optimization reuse never reuses returned-state evidence: every residual value,
Jacobian/derivative status, incident-variable audit snapshot and priority final
cost is freshly evaluated at the published state. Hierarchy-only residual and
source edits advance dependency stamps for every participating component even
when the optimum does not move.

Regression evidence is in `crates/geosolve-core/tests/m5_priority.rs` and
`crates/geosolve-core/tests/m16.rs`. It covers coupled dense movement,
positive-cost KKT/curvature certification, stationary-maximum escape, bounds,
two independently protected Temporary groups, hierarchy-only session edits,
and a 128-component projected Preference group protecting 128 Temporary
groups. Large bounded regressions cover interior and lower/upper endpoint
solutions, active-bound release, duplicate alias normals, Temporary protection,
small dense oracle parity, evaluator-domain containment, and implicit hard-row
projection without a global basis. Curvature search uses three decreasing
stencil scales. It may establish
descent, but absence of a detected negative sample does not prove arbitrary
evaluator optimality: positive-cost first-order stationary results are
`Acceptable`, while zero-cost least squares remains `Optimal`. Dense curvature
reconstruction is deliberately limited to 16 reduced coordinates; inconsistent
stencils, larger positive-cost stationary operator groups and unsupported
multidimensional one-sided curvature remain conservatively `Stalled`.
