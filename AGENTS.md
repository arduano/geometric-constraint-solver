# Agent instructions

Read `START_HERE.md`, `ARCHITECTURE.md`, `PLAN.md`, `ACCEPTANCE.md`, and `docs/SCENARIOS.md` before changing implementation code.

## Non-negotiable requirements

- Pure Rust. Do not add C/C++/Fortran solver FFI or SuiteSparse bindings.
- GPL-3.0-or-later project licence; preserve SPDX/licence metadata.
- Keep `geosolve-sketch` and `geosolve-linkage` as separate domain models over `geosolve-core`.
- Keep `geosolve-demo-web` as a separate WASM crate. It must consume public domain/audit APIs rather than duplicating solver equations.
- No `unsafe` code without an explicit ADR and caller approval; workspace currently forbids it.
- The solver may only return a success-like status after independent residual validation.
- Never turn NaN/Inf or invalid geometry into convergence.
- Do not use weighted least squares as an undocumented substitute for hard-vs-soft priority semantics.
- Branch/orientation choices must be explicit state, not accidental consequences of initial coordinates.

## Work style

- Implement milestones in `PLAN.md` in order.
- Use `$geosolve-harden-defect` for every reported solver, sketch/linkage, or
  headless-interaction defect and for any golden-oracle expansion.
- Do not broaden the primitive/constraint set until the previous milestone's acceptance tests pass.
- Keep public APIs small. Prefer an internal prototype over premature generic traits.
- Every residual implementation needs a finite-difference Jacobian test and a structured human-readable audit descriptor.
- Every bug involving convergence, rank, scaling, or a branch flip gets a regression scenario.
- Run format, clippy, tests, and the relevant WASM build before claiming a milestone complete.
- Update `PLAN.md` checkboxes and add short notes under the completed milestone.
- Make small, reviewable commits if the supervising caller permits commits; do not rewrite history.

## Completion report format

For each milestone report:

1. files/API added;
2. mathematical behavior implemented;
3. exact commands run and outcomes;
4. acceptance criteria passed;
5. known limitations or next blocker.
