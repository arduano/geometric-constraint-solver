<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 — Retained authoring semantic consolidation

Status: **implemented and mechanically qualified; focused human UAT remains pending**. The
supervising caller accepted this scope on 2026-08-15. No new ADR is required because M73 preserves
accepted behavior, changes no solver or persistence architecture and retires only an editor API
introduced after the published `0.2.0` baseline.

## Goal

Consolidate the remaining construction-stage and retained-relation dispatch seams before any
broader M37 catalog expansion. M73 keeps the M70/M71 user behavior and contextual authoring API,
removes one unreleased redundant direct API and makes inference-candidate provenance explicit
through confirmation and commit lowering.

## Corrected baseline

The earlier proposal overstated the construction-stage duplication. M71 commit `57a0bf5` already
made `construction_point_stage` a projection of `draft_inference_subject`; those functions no
longer contain separate exhaustive `EditorTool` matches. That stale premise is not M73 work.

The remaining seams at activation were narrower and directly observable:

1. `directional_span_stage`, `draft_span_slot` and line/polyline reference handoff independently
   encoded which construction stage owned the prospective segment.
2. The unreleased `ConstraintKind` plus
   `ConstraintEditor::{available_constraints, constraint_edit}` and
   `EditorError::IncompatibleConstraint` compatibility surface duplicated part of the contextual
   `ConstraintIntent`/`ResolvedConstraintKind` authoring path and advertises three contact-bearing
   families it cannot directly lower. Its public methods have no non-test caller; the coordinator's
   remaining `ConstraintKind` use was the duplicate simple-lowering seam M73 removed.
3. `DraftInferenceCandidate` owned a stable candidate ID, guides, relations and references, but
   `ConfirmedDraftInference` dropped the winning candidate ID before commit lowering.

## Accepted work

### M73-F001 — one private construction-stage description

- Introduce one private stage description for persistent-point, centered-point with prospective
  curve, circle-circumference and coordinate-only roles, plus the optional created line/polyline
  span slot.
- Derive inference subject, point-slot ownership/indexing, directional-span eligibility, created
  span lowering and line/polyline reference handoff from that description.
- Return no description for invalid completed stages and preserve every current stage result,
  prospective curve/segment index and point-identity precedence rule.
- Do not fold proposal geometry, preview rendering or tool completion policy into this descriptor.

### M73-F002 — retire the unreleased direct constraint API

- Remove `ConstraintKind` and
  `ConstraintEditor::{available_constraints, constraint_edit}` plus the now-orphaned
  `EditorError::IncompatibleConstraint` variant, private compatibility-only lowering and tests.
- Keep `ConstraintIntent`, `ResolvedConstraintKind`, `AuthoringState` and
  `RetainedEditorCoordinator::{resolved_constraint, apply_authoring}` as the one presentation-
  independent authoring route. One contextual resolver owns applicability and typed disabled
  reasons; one internal lowerer owns the simple retained definitions.
- Preserve all 20 contextual resolved families, exact operand ordering, contact/branch choices and
  current `DisabledReason` classifications.
- This is a pre-release source cleanup, not removal of a supported `0.2.0` API. It does not remove
  `SketchConstraintKind`, `DocumentConstraintDefinition`, direct sketch builders or any persisted
  relation.

### M73-F003 — retain terminal candidate provenance

- Carry the winning `DraftInferenceCandidateId` into the private confirmation record.
- Require candidate-owned guides, confirmed relations/references and the lowered commit plan to
  describe the same terminal candidate.
- Preserve ambiguity, preferred-candidate staleness, bounded enumeration and mutation-free
  rejection. Candidate identity remains runtime-only and is not added to persistence or public
  commit-plan DTOs.

## Acceptance

- Focused stage-table tests cover every tool/stage role, coordinate-only and invalid stages,
  prospective point/curve indices and line/polyline segment slots.
- Focused contextual-authoring tests cover all 20 resolved families, accepted reversals,
  intentionally ordered operands, empty/wrong/foreign/missing operands, invalid spans and stale
  design/application resolution.
- Candidate-trace tests cover ordinary line/polyline handoff, the M71-F004/F005 compound-axis
  cases, circle-through-point, centered Concentric, ambiguity and stale/rejected commits without
  mutation.
- Existing M70/M71 behavior, public contextual DTOs, browser dispatch and the reviewed 234-row
  golden remain unchanged.
- No residual, Jacobian, solver priority, branch state, canonical sketch v1-v4 byte, unsupported
  draft-v5 meaning or browser-owned geometric policy changes.
- Formatting, warnings-denied workspace Clippy, locked all-feature tests, relevant native/WASM
  parity and the complete clean release gate pass before candidate nomination. Focused human UAT
  then receives explicit approval.

M73-F001 through M73-F003 and their focused qualification are complete. The exact implementation
and gate evidence is recorded in `docs/M73_IMPLEMENTATION.md`; `docs/M73_UAT.md` remains the only
open acceptance step.

## ADR decision

No ADR is required. M73 follows ADR 0034's drafting-authority boundary and ADR 0035's retained
relation lifecycle, changes no released architecture and introduces no new public semantic or wire
contract. Discovery that requires one of those changes stops M73 for a separate decision.

## Non-goals

M73 does not promote remaining M37 relations, broaden point-reference operands, add generic
intersections or quadrant anchors, infer nonlinear tangent/normal or equality/symmetry, add host
axes/grids/increments, persist wake state or candidate IDs, freeze canonical sketch v5, chain
computed features, change workbench behavior, restore browser E2E, or add mobile behavior.
