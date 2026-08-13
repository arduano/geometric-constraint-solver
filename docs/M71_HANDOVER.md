<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 fresh-session handover

Status: the clean M71 candidate is qualified and published for supervising-human UAT. M71 is not
closed; M71-U1 through M71-U5 and explicit approval remain pending.

This document is the canonical short restart contract for the interrupted M71 implementation
session. Read the repository-required project documents first, then this file, ADR 0035,
`docs/M71_GOALS.md`, `docs/M71_IMPLEMENTATION.md` and `docs/M71_UAT.md`. Do not reconstruct M71
from chat history.

## Repository history at consolidation

- Sole worktree: `/home/arduano/programming/geometric-constraint-solver`.
- Branch: `main`.
- Pre-consolidation base: `4d5bec1d395c37cfdabc8448933db19d3f94f8b8`, one commit ahead of
  live `origin/main` at `8ebe2e171ece7faf95057dc39c9ff2c6c7804c2f`.
- The complete M71 implementation was confined to that worktree. The five formerly untracked
  files were all intentional M71 relation, persistence, parity and implementation records; no
  scratch, reject, backup or dangling untracked file was found.
- Clean candidate `ad01912eac28275644dcfc867a2dc70030b5406d` is frozen at
  `/tmp/geosolve-m71-uat.yFBsnX` and served at `http://100.94.63.83:8080/` by PID `49116`. Every
  asset and `/` byte-matches the snapshot; its ordered manifest aggregate is
  `43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`. The historical M70B
  snapshot remains on disk but is no longer served.

The current qualified state supersedes those historical branch bullets: candidate source
`ad01912eac28275644dcfc867a2dc70030b5406d` has qualification/publication documentation prepared
for the final record commit and push. Recheck exact divergence after that commit rather than using
the pre-consolidation hashes below as current state.

After this handover is committed, use `git log -5 --oneline --decorate`, `git status --short
--branch`, `git worktree list --porcelain` and `git rev-list --left-right --count
origin/main...main` to establish the exact new checkpoint. Do not assume it has been pushed unless
the latter command and `git ls-remote origin refs/heads/main` agree.

## Implemented scope

M71 promotes exactly four already-existing mathematical relations into the ordinary retained
document/editor lifecycle:

- stored-point `HorizontalPoints` and `VerticalPoints`;
- semantic-center `Concentric`; and
- directed native-support `Collinear`.

The sketch domain owns validation, lowering, audit grouping, suppression, deletion, dependency
closure, retained solve/history and persistence. Canonical sketch v4 is isolated behind a private
frozen wire DTO and rejects M71 state with `UnsupportedM71State`; unsupported draft v5 carries the
new records in an omitted-when-empty side section and transactionally merges them into the complete
source order.

The headless editor owns variable-arity contextual authoring, semantic inference, candidate
ranking, bounds, prospective same-transaction operands, atomic commit plans and presentation
metadata. The browser adapter renders and dispatches those public DTOs and supplies one ordinary
editable **Retained drafting relations** sample. It owns no equations or applicability policy.

M71 adds no residual, Jacobian, solver priority or implicit branch rule. It lowers to existing
`add_horizontal_points`, `add_vertical_points`, center `add_coincident` and `add_collinear`
operations, followed by the existing independent finite hard-residual validation.

## Implicit-correctness law

The supervising principle is **implicit correctness**: prefer strong composable semantics over a
tool-specific edge-case table.

The implemented center rule is expressed through one operand capability:

- `CenteredPointOperand` means a stored construction point that will also be the semantic center
  of a prospective curve;
- for that operand only, an exact accepted semantic-center/Concentric candidate outranks incidental
  structural reuse of the stored center point;
- ordinary `PointOperand` retains M70 point-identity precedence;
- midpoint and PointOnCurve remain available;
- an explicit candidate preference remains authoritative; and
- disabling Concentric falls back to ordinary point identity.

This rule covers Circle, counter-clockwise Circular Arc, Ellipse, Elliptical Arc and Hyperbola
without ranking branches named after those tools. Distinct curves that share one stored center are
distinct retained operands and therefore produce an ambiguous choice; repeated scene occurrences
of one curve are deduplicated. Persistent IDs never silently break a semantic tie.

Scene collection is all-or-nothing and bounded before publication. Ordinary and semantic anchors
share one subject-relevant bound; suppression bypasses traversal; overflow publishes no prefix;
scope/visibility filtering, ambiguity and post-overflow reacquisition are directly tested.

## Exact checkpoint evidence

The following commands passed on the consolidated tree on 2026-08-13:

```text
cargo test --locked -p geosolve-sketch --test m71_relations --test m71_persistence
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
git diff --check
```

Observed results:

- M71 relation owner matrix: 11/11 pass;
- M71 persistence matrix: 7/7 pass;
- constraint editor: 297/297 unit tests plus every integration and doc-test suite pass;
- demo web: 102/102 library tests, 1/1 decoder test and doc tests pass;
- canonical golden: 234/234 `PASS`, SHA-256
  `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`;
- `git diff --check`: pass.

Earlier in the same implementation checkpoint, focused warnings-denied editor Clippy, native M71
parity, M71 WASM parity inside `nix-shell shell.nix`, and the demo-web WASM check passed. Treat an
ambient-shell WASM invocation that could not find `wasm-bindgen-test-runner` as a harness error,
not test evidence. The fresh session should rerun the complete gates below rather than relying on
that narrative for release nomination.

## Review before release nomination

The completed audit found no solver or mathematical blocker, but two cleanup questions remain.
They should be resolved by semantic consolidation, not by adding more examples:

1. `construction_point_stage` and `draft_inference_subject` currently classify centered stages
   from closely related exhaustive `EditorTool` matches. Consider replacing that duplicated tool
   knowledge with one construction-stage semantic descriptor. Preserve the current centered-tool,
   coordinate-only-stage and prospective-curve-slot tests.
2. The older direct `available_constraints`/`constraint_edit` path and the contextual
   `resolve_constraint` coordinator path each contain M71 applicability knowledge. Audit whether
   both public surfaces must remain; if so, derive them from one semantic predicate or retain an
   explicit parity law. Do not casually remove compatibility APIs.

The parallel semantic-center vector/latch/candidate pipeline was reviewed as an implementation
shape, but its behavior is now governed by operand capability, retained curve identity, shared
bounds and subject-aware ranking. Do not refactor it merely for visual uniformity unless the
result makes those laws smaller and clearer.

## Next-session sequence

1. Confirm the documented candidate is still served by PID `49116`; hard-refresh because the
   endpoint reuses port 8080.
2. Complete M71-U1 through M71-U5 in `docs/M71_UAT.md`, recording observations and any exact
   reproduction payloads.
3. Route any solver/headless defect through `geosolve-harden-defect`; a material correction
   requires a new clean gate and replacement immutable publication before retest.
4. Close M71 only after explicit supervising-human approval. Do not infer approval from the
   mechanical evidence.

## Deliberately deferred

Do not expand M71 into derived-point H/V operands, M37 catalog consolidation, generic
intersections, quadrant anchors, nonlinear tangent/normal inference, equality/symmetry inference,
host axes/grids/increments, persistent wake state, canonical sketch v5, computed-feature chaining,
browser E2E, mobile support or legacy UI.
