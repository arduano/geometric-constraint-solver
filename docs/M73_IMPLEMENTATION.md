<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 implementation — Retained authoring semantic consolidation

Status: **implemented and mechanically qualified; final immutable candidate handoff and focused
human UAT remain pending**. The supervising caller accepted the corrected scope on 2026-08-15.

Activation baseline source: `daea43de51c9a1a720da1a245747e67735448f7d`

Activation baseline tree: `b2eb479d396b3c9a0075be9117787f2c75ecd15f`

Implementation source: `b1b2162eb20fa5bd088c5ddf80c3bfb97fd11223`

Implementation tree: `1890ab4330bd78f26c187ebed5fadea97370101e`

Architecture decision: no new ADR. M73 remains within ADR 0034's drafting-authority boundary and
ADR 0035's retained-relation lifecycle.

## Implemented behavior

### M73-F001 — one construction-stage semantic owner

- `ConstructionStageSemantics` now describes each valid tool/stage as a point operand, a centered
  point plus prospective curve, a circle circumference or a coordinate-only stage, with an
  optional created line/polyline span.
- Inference subject, point-slot identity, directional eligibility, created-span lowering and
  remembered line/polyline reference handoff derive from that one private description.
- Invalid completed stages remain descriptor-free. Proposal geometry, completion policy and
  renderer-facing previews remain separate.

Implementation commit: `fe356c2` (`refactor(editor): unify construction stage semantics`).

### M73-F002 — one contextual retained-relation route

- Removed unreleased `ConstraintKind`,
  `ConstraintEditor::{available_constraints, constraint_edit}` and
  `EditorError::IncompatibleConstraint`, including their duplicate compatibility lowerer and
  tests.
- All 20 `ResolvedConstraintKind` families now execute through `AuthoringState` and
  `RetainedEditorCoordinator::apply_authoring`. Fourteen simple families share one exhaustive
  internal definition lowerer; the six contact-bearing families retain their specialized branch
  and contact construction.
- Empty selection retains repeated-mode entry while action availability reports
  `DisabledReason::EmptySelection`. Missing objects, invalid spans, semantic tautologies, stale
  resolution and accepted-state retention remain typed and fail closed.

Implementation commits: `a973b73`, `585cf65` and `6fa28a4`.

### M73-F003 — authenticated terminal candidate provenance

- Private `ConfirmedDraftInference` retains the winning `DraftInferenceCandidateId`.
- Confirmation authenticates the exact selected candidate rather than accepting a different or
  modified candidate with superficially compatible guides.
- Guides, relations, references and lowering remain one candidate-owned bundle through commit
  construction. Candidate IDs remain runtime-only and absent from persistence and public commit
  DTOs.

Implementation commits: `fcaad55` and follow-up authentication repair `693ed17`.

Rustdoc qualification follow-up: `b1b2162` fixes the final construction-edit intra-doc link.

## Compatibility result

- No released `0.2.0` API was removed. The retired editor compatibility surface was introduced
  after `0.2.0`, had no non-test direct caller and duplicated only part of contextual authoring.
- `ConstraintIntent`, `ResolvedConstraintKind`, `AuthoringState` and
  `RetainedEditorCoordinator::{resolved_constraint, apply_authoring}` remain the public headless
  authoring route.
- `SketchConstraintKind`, `DocumentConstraintDefinition`, direct sketch builders, persistent
  relations and canonical sketch v1-v4 bytes are unchanged.
- The production implementation diff changes only `geosolve-constraint-editor`; no browser,
  solver, residual, Jacobian, priority, branch, persistence or golden-oracle implementation
  changed.

## Mechanical qualification

The exact clean command passed on implementation source `b1b2162`:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That run passed:

- all 321 editor library tests and all 17 `tests/m55.rs` tests;
- the unchanged 234/234 authoring/scene golden inventory, survey, check and clean gate;
- formatting, diff hygiene, warnings-denied workspace Clippy and locked all-feature workspace
  tests;
- native and WASM M70/M71 transition parity plus the demo-web WASM check;
- warnings-denied Rustdoc, all-target benchmark compilation, M14/M32 performance, licensing and
  packaging;
- the 256-moving-body sparse crossover in 141.76 seconds; and
- the Trunk 0.21.14 release build.

Because `docs/API_COMPATIBILITY.md` is copied into the seven-file web distribution, candidate
nomination deliberately waits for one more clean gate on the committed current-status
documentation. That rerun, immutable snapshot hashes and served-byte verification will be added
below without changing the nominated distribution.

## Qualification ledger

- [x] Correct and activate the scope; record the no-ADR decision and unreleased API disposition.
- [x] Implement M73-F001 through M73-F003 in reviewable commits.
- [x] Pass focused owner tests and all relevant collateral suites.
- [x] Preserve the canonical authoring/scene oracle at 234/234 byte-for-byte.
- [x] Pass formatting, diff hygiene, warnings-denied workspace Clippy, locked all-feature tests,
  native/WASM parity and the complete clean release gate.
- [ ] Rerun the clean gate on the committed current-status source, freeze the exact seven release
  files and verify their Tailscale publication byte-for-byte.
- [ ] Complete `docs/M73_UAT.md` with explicit human approval.

## Current blocker

None. Immutable candidate nomination and focused human UAT are the only remaining ordered work.
