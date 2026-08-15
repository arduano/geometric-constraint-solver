<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 implementation — Retained authoring semantic consolidation

Status: **active; implementation pending**. The supervising caller accepted the corrected scope on
2026-08-15. This initial ledger records the audited baseline, API disposition and qualification
plan; it contains no production-code completion claim.

Activation baseline source: `daea43de51c9a1a720da1a245747e67735448f7d`

Activation baseline tree: `b2eb479d396b3c9a0075be9117787f2c75ecd15f`

Architecture decision: no new ADR. M73 remains within ADR 0034's drafting-authority boundary and
ADR 0035's retained-relation lifecycle.

## Audited baseline

- M71 already made `construction_point_stage` derive from `draft_inference_subject`; the former
  claim of two exhaustive tool matches is withdrawn.
- The remaining construction duplication is limited to `directional_span_stage`,
  `draft_span_slot` and line/polyline reference handoff.
- The direct `ConstraintKind` plus
  `ConstraintEditor::{available_constraints, constraint_edit}` API and dependent
  `EditorError::IncompatibleConstraint` variant are unreleased and cover only part of contextual
  authoring. The public methods have no non-test caller; the coordinator's internal
  `ConstraintKind` use is the duplicate simple-lowering seam to replace. Retirement is recorded in
  `CHANGELOG.md` and `docs/API_COMPATIBILITY.md`.
- The winning `DraftInferenceCandidateId` is lost when a candidate becomes
  `ConfirmedDraftInference`, even though relations and references continue into commit lowering.

## Planned implementation

1. Add one private construction-stage description and derive inference role, point-slot indexing,
   directional eligibility, created span slots and reference handoff from it.
2. Remove the unreleased direct constraint compatibility API and its duplicate applicability/
   lowering tests. Keep the contextual 20-family route and one internal simple-definition lowerer.
3. Retain terminal candidate identity privately through confirmation and validate candidate-owned
   guides, relations, references and the final commit plan as one bundle.
4. Add the direct stage, contextual-family, operand/failure and candidate-trace matrix named in
   `docs/M73_GOALS.md` and `docs/SCENARIOS.md`.

No browser implementation, public replacement API, residual, Jacobian, branch rule, persistence
schema, golden row or sample is planned.

## Qualification ledger

- [x] Correct and activate the scope; record the no-ADR decision and unreleased API disposition.
- [ ] Implement M73-F001 through M73-F003 in reviewable commits.
- [ ] Pass focused owner tests and all relevant collateral suites.
- [ ] Preserve the canonical authoring/scene oracle at 234/234 byte-for-byte.
- [ ] Pass formatting, diff hygiene, warnings-denied workspace Clippy, locked all-feature tests,
  native/WASM parity and the complete clean release gate.
- [ ] Nominate an immutable candidate and complete `docs/M73_UAT.md` with explicit human approval.

## Current blocker

None. Production implementation is the next ordered work.
