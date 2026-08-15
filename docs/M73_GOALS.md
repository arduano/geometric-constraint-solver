<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 — Retained authoring semantic consolidation

Status: **complete and explicitly approved by the supervising caller on 2026-08-15**. M73-F001
through M73-F004, the clean replacement release gate, byte-verified immutable UAT snapshot,
focused human UAT and exact final GitHub Pages publication all pass. The supervising caller
accepted the original scope, focused F004 correction and final candidate. No new ADR is required
because M73 changes no solver or persistence architecture and retires only an editor API introduced
after the published `0.2.0` baseline.

## Goal

Consolidate the remaining construction-stage and retained-relation dispatch seams before any
broader M37 catalog expansion. M73 keeps the contextual authoring API and all M70/M71 behavior
outside F004's narrow live world-axis precedence correction, removes one unreleased redundant
direct API and makes inference-candidate provenance explicit through confirmation and commit
lowering.

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
4. The nominated F001-F003 product exposed a narrower inference conflict: a live world Horizontal
   or Vertical span and same-axis remembered point/native-midpoint tracking could both survive even
   though they adjust the same endpoint coordinate. M73-F004 gives the eligible live world-axis
   span direction precedence over competing durable trackers without changing generic
   tracking-only cues, orthogonal bundles or remembered direction semantics.

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

### M73-F004 — live world-axis span precedence

- When a live world Horizontal span direction both adjusts coordinates and persists its relation,
  suppress durable same-axis `HorizontalPoints` and `HorizontalPointToMidpoint` tracking before it
  can publish a guide, consume candidate capacity, acquire a latch or enter cross-axis pairing.
  Apply the symmetric rule to live world Vertical versus durable `VerticalPoints` and
  `VerticalPointToMidpoint`.
- Preserve generic tracking-only cues and wake state: they may coexist visually with the live
  world-axis candidate but contribute no competing retained relation. Preserve durable
  point/native-midpoint tracking on the orthogonal axis so it can still compose with the live
  world-axis direction into the existing two-relation line/polyline bundle.
- Do not apply this suppression to remembered Parallel, Perpendicular or Collinear directions,
  even when their source support is Cartesian. Do not weaken solver-owned redundancy rejection.
- Treat this as a narrow supersession of M71-F004's same-axis-alternative rule only for eligible
  live world-axis span constraints. Preserve the M71 wording and qualification as historical
  evidence.

## Acceptance

- Focused stage-table tests cover every tool/stage role, coordinate-only and invalid stages,
  prospective point/curve indices and line/polyline segment slots.
- Focused contextual-authoring tests cover all 20 resolved families, accepted reversals,
  intentionally ordered operands, empty/wrong/foreign/missing operands, invalid spans and stale
  design/application resolution.
- F001-F003 candidate-trace tests cover ordinary line/polyline handoff, the M71-F004/F005
  compound-axis cases, circle-through-point, centered Concentric, ambiguity and stale/rejected
  commits without mutation.
- Public regression `m73_f004_span_axis_precedence` and focused inference-owner tests cover
  Horizontal/Vertical durable point and native-midpoint suppression before guide publication,
  candidate accounting, latch acquisition and cross-axis pairing; generic tracking-only cue
  preservation; orthogonal bundle retention; remembered Parallel/Perpendicular/Collinear
  non-regression; exact guides/relations; atomic retained commit; and independent finite accepted
  geometry/residual checks.
- Existing M70/M71 behavior, public contextual DTOs, browser dispatch and the reviewed 234-row
  golden remain unchanged except for F004's explicitly superseding precedence rule.
- No residual, Jacobian, solver priority, branch state, canonical sketch v1-v4 byte, unsupported
  draft-v5 meaning or browser-owned geometric policy changes.
- Formatting, warnings-denied workspace Clippy, locked all-feature tests, relevant native/WASM
  parity and the complete clean release gate pass before candidate nomination. Focused human UAT
  then receives explicit approval.

M73-F001 through M73-F004 and their focused/proportional qualification are complete. F004 initial
implementation `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88`, hardening `0153fc0` and final
durable-tracker boundary follow-up `89e409a` produce final focused source
`89e409a6ebe12c640ae2f313f95de67430dfa8d0`. It passes the public 3/3 regression,
inference-owner matrix, full editor suite at 325 unit tests plus every integration, M71
F003/F004/F005 and transition parity, warnings-denied Clippy, and the unchanged 234/234 golden
survey/check/clean gate.

Regression-hardening follow-up `f41e398d00b7a7ca1e12a12a285408a0b7bd3566` makes the full
point/native-midpoint by Horizontal/Vertical matrix part of the focused `same_axis_span` run,
asserts exact top-level guide and latch state, and proves the public native-midpoint wake before
suppression. The focused owner run passes 5/5 and the public target passes 3/3.

Exact clean replacement source `4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`, tree
`fe9897153baa974b3c5c06e7a3bf5eee76e920f2`, passes the complete release gate, including editor
325/325 plus every integration, unchanged golden 234/234, native/WASM parity and the complete
workspace, documentation, benchmark, performance, licence, packaging and Trunk matrix. Its exact
gate distribution is frozen read-only at `/tmp/geosolve-m73-uat.JKAWtJ`, aggregate
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`, and byte-verified at
`http://100.94.63.83:8080/`. This replacement is the accepted closing candidate. The supervising
caller confirmed that the focused behavior works and explicitly requested closure on 2026-08-15;
M73-U1 through M73-U4 pass under that scoped decision.

Documentation-only approval descendant `ef7b90feb17bfba62c45f9463ceb934fc34e6f4d`, tree
`f9debcdf268d52a8959166fadf5505b67c7fbaa7`, passes final GitHub Pages run `31878139709` and
deploys artifact `9245585021`. Its downloaded ZIP and inner tar SHA-256 values are
`fcfdb7f573bbfde86f70bc56126fe5c800428bc58991eb445eba33f122bf2222` and
`d6c210b50aa9bb7e257555f931016551402fb7a8faa5d4ccfe267c68c44ceb56`; the C-locale seven-file
manifest aggregate is `4e562280bc0656f9bd7358057d62739ba02e74a5f76b0328c5e45bf18640031c`.
The public root and all seven files return HTTP 200 and match the artifact byte-for-byte, `/`
equals `index.html`, asset URLs are repository-prefixed and JavaScript/WASM/CSS media types are
correct. Every M73 goal is complete.

## ADR decision

No ADR is required. M73 follows ADR 0034's drafting-authority boundary and ADR 0035's retained
relation lifecycle, changes no released architecture and introduces no new public or wire contract.
F004 adjusts private inference precedence while leaving solver redundancy authority intact.
Discovery that requires one of those architectural changes stops M73 for a separate decision.

## Non-goals

M73 does not promote remaining M37 relations, broaden point-reference operands, add generic
intersections or quadrant anchors, infer nonlinear tangent/normal or equality/symmetry, add host
axes/grids/increments, persist wake state or candidate IDs, freeze canonical sketch v5, chain
computed features, change workbench behavior, restore browser E2E, or add mobile behavior.
