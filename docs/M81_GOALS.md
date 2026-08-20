<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 — Core architecture consolidation

Status: **complete and closed 2026-08-20**. Implementation, rotating review, final clean
qualification, frozen Tailscale nomination, supervising-human acceptance, Pages publication and
exact hosted-byte verification pass. M81 is a behavior-preserving cleanup after the M66–M80
feature sequence. It changes private ownership and file boundaries only, except for the exact
transactional-authority repair recorded as M81-F001. It adds no geometry, relation, solver policy,
persistence language, public API or workbench UX.

## Goal

Make the solver, sketch compilation/document, equation-free operation and retained-editor paths
easier to review without replacing their established domain boundaries. Large mixed-responsibility
files are split along existing semantic seams; orchestration facades keep their current public
paths, ordering, diagnostics and failure behavior.

## Frozen activation contract

The activation baseline is M80 closeout source `e6df1a26610a2e4ee811fa122867d2b86518e516`.
Before implementation, M81 froze:

- locked no-dependency Cargo metadata, SHA-256
  `691591282713d87fb2ae63ff12873577c91daadaa2af0c817c0543b9bfd4d66e`;
- the ordered leading crate-root public declaration/export lines selected for `geosolve-core`,
  `geosolve-sketch`, `geosolve-linkage` and `geosolve-constraint-editor`, SHA-256
  `0b46f015459f16f20de98373c452c8cb203aebe4cd4cdc54dbfbc78f4d01e9cd`; and
- the reviewed milestone-neutral golden authoring/scene authority at 271 rows, with `--check` and
  `--require-clean` both passing.

Canonical sketch v1–v4 bytes, the explicitly unsupported draft-v5 boundary, workspace v6,
reproduction v1, structured audit text, persistent/runtime ID order, trace order, work charging,
first fallback reasons and exact branch/orientation state are compatibility inputs, not cleanup
opportunities.

## Accepted work

### M81-C1 — solver responsibility boundaries

- Keep public solve DTOs, configuration and `Problem` orchestration in the existing `solver`
  facade.
- Isolate hard-component iteration and linear algebra, lexicographic priority optimization, and
  independent returned-state/rank/diagnostic validation behind private modules.
- Preserve dense/sparse selection, fallback order, cache compatibility, controller checkpoints,
  hard-versus-temporary priority semantics and independent residual validation exactly.
- Do not introduce a generic solver trait, shared residual/validator implementation or a new
  crate.

### M81-C2 — sketch document and compilation boundaries

- Extract cohesive private curve-query/Profile Offset validation and compiler registration/
  lowering/validation helpers where a mechanical move can be qualified independently. Frozen
  private wire DTO ownership is unchanged; M81 does not perform a wire-layer extraction.
- Keep `SketchDocument`, session and compiler public paths unchanged and preserve arena insertion,
  source/residual mapping, audit-row and iteration order.
- Independent candidate validation remains deliberately separate from residual evaluation; no
  equation or derivative is deduplicated across that trust boundary.

### M81-C3 — equation-free operation planning

- Move Profile Offset operand, path and junction planning out of the `geosolve-sketch-ops` public
  orchestration facade.
- Preserve exact topology provenance, selected-set branch policy, deterministic traversal/order,
  typed incomplete/unsupported outcomes and proposal bytes.
- Keep the crate graph and the rule that operations consume public domain/topology APIs without
  owning solver equations.

### M81-C4 — retained editor/coordinator boundaries

- Extract feature-specific or transaction-specific coordinator helpers only where candidate
  staging, exact-input authentication, publication, transient clearing and history effects remain
  visibly ordered.
- Audit rejected computed-feature mutations for allocator, history, transcript, accepted
  identity/JSON, scene-input authority and feature-document neutrality through the narrow public
  coordinator boundary. A reproduced defect receives an M81 finding ID, exact owner regression
  and smallest transactional repair before any surrounding refactor proceeds.
- Do not normalize differing preview/durable allocator policies without evidence, merge browser
  behavior into the headless editor or create a generic transaction framework.

### M81-C5 — computed-feature composition boundaries

- Move the existing source-claim, interval-composition, discarded-fragment, combined-role and
  revision-local output-ID responsibilities behind one private evaluator module.
- Preserve exact conflict attribution, source ordering, roots, explicit branches, continuation,
  tolerances, work charging, public DTOs and independent output validation.
- Do not introduce a generic feature framework, new computed feature or public composition API.

### M81-C6 — rotating independent audit and qualification

- Use multiple independent agents in rotating implementation/review roles across core/linkage,
  sketch/compiler/persistence, features/operations and editor/coordinator/demo seams.
- Every implementation batch receives a fresh read-only diff review for behavior drift, privacy
  expansion, order changes, validator coupling, accidental API changes and missing collateral
  tests.
- Record accepted, corrected and intentionally deferred audit findings in the implementation
  ledger. An unconfirmed concern is not assigned a defect ID.

## Acceptance

- Locked package metadata and the originally frozen four-root selected public declaration/export
  lines are byte-identical to the activation freeze. A supplemental selected `pub use`/`pub
  struct`/`pub enum` leading-line comparison across all nine workspace library roots must also be
  byte-identical; these text snapshots are declaration/export coverage, not a substitute for Rust
  type checking or a claim that multiline bodies were hashed.
- Canonical persistence fixtures, reproduction/workspace round trips and the 271-row golden
  inventory remain byte-identical; no golden output is re-blessed.
- Focused core, linkage, sketch, operations, features and editor/coordinator suites pass after
  their owning extraction, followed by native/WASM parity and demo adapter tests.
- Formatting, diff hygiene, warnings-denied workspace Clippy and Rustdoc, locked all-feature
  workspace tests, benchmark compilation, performance/licence/package checks, release Trunk and
  the complete clean release gate pass from committed source.
- The exact no-rebuild release output is frozen and byte-verified over the retained Tailscale UAT
  endpoint before nomination.
- The worktree is clean and commits remain subsystem-sized and reviewable.

Final qualification satisfies this contract at exact product source
`e4eca327fc69c92f95b1722142289302ba4f67bc`, tree
`f3ed1bf50b793daae328adf04c0924655dc13d74`. The original four-root selected-line hash remains
`0b46f015459f16f20de98373c452c8cb203aebe4cd4cdc54dbfbc78f4d01e9cd`; the supplemental all-nine-
root selected-line hash is
`5cd55480a3d0f8a1d7175ef9359c94cc4dcd14cbf6b5d865abf1697667d1af90`. The immutable seven-file
candidate and HTTP evidence are bound in `docs/M81_IMPLEMENTATION.md` and `docs/M81_UAT.md`.
The supervising caller accepted the qualified candidate and requested closure on 2026-08-20
without opening a new finding. Documentation-only approval descendant `b582b82` passes Pages run
`32328472125`, artifact `9392295853` and exact hosted-byte verification. M81 is closed.

## Non-goals

M81 does not add primitives, constraints, dimensions, authoring variants, inferred relations,
computed features, topology behavior, schema versions, host expressions, browser layouts or
mobile behavior. It does not change residuals, Jacobians, tolerances, rank policy, priority policy,
branch selection, public DTOs or published error text. Any improvement that cannot be shown to be
mechanical or required by a reproduced transactional defect is deferred to a later milestone.

## ADR decision

No ADR is expected. M81 preserves the existing crate graph, public authority boundaries,
independent-validation rule and persistence architecture. Discovery that requires changing one of
those decisions stops that item for a separately approved milestone or ADR.
