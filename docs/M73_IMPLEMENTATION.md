<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 implementation — Retained authoring semantic consolidation

Status: **M73-F001 through M73-F004, the clean replacement release gate and byte-verified immutable
publication are complete; M73 remains open only for focused human UAT and explicit approval**. The
supervising caller accepted the corrected scope on 2026-08-15.

Activation baseline source: `daea43de51c9a1a720da1a245747e67735448f7d`

Activation baseline tree: `b2eb479d396b3c9a0075be9117787f2c75ecd15f`

F001-F003 implementation source: `b1b2162eb20fa5bd088c5ddf80c3bfb97fd11223`

F001-F003 implementation tree: `1890ab4330bd78f26c187ebed5fadea97370101e`

M73-F004 initial implementation source: `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88`

M73-F004 early-suppression hardening: `0153fc045cac541cb0cbd2348ad1d51d5768da8c`

M73-F004 final focused source: `89e409a6ebe12c640ae2f313f95de67430dfa8d0`

M73-F004 regression-hardening source: `f41e398d00b7a7ca1e12a12a285408a0b7bd3566`

Clean replacement product source: `4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`

Clean replacement product tree: `fe9897153baa974b3c5c06e7a3bf5eee76e920f2`

Architecture decision: no new ADR. M73 remains within ADR 0034's drafting-authority boundary and
ADR 0035's retained-relation lifecycle.

## Implemented behavior and finding history

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

### M73-F004 — live world-axis span precedence correction

Finding reproduced against the nominated F001-F003 product source
`efde645345577f44e0d6b691f7ca27eb587c4b53`. A live world Horizontal or Vertical span direction
whose inference behavior both adjusts coordinates and persists the constraint could coexist with
same-axis remembered point/native-midpoint tracking even though both own the same endpoint
coordinate. That exposed redundant candidate/guide alternatives before the retained solver had an
opportunity to reject a redundant commit.

Implementation commit `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88` gives eligible live world
Horizontal precedence over durable `HorizontalPoints` and `HorizontalPointToMidpoint` tracking,
with the symmetric Vertical rule for durable `VerticalPoints` and `VerticalPointToMidpoint`.
Hardening follow-up `0153fc0` moves that suppression into tracker collection, before guide
publication, candidate-budget accounting, latch acquisition or cross-axis pair construction.
Final boundary follow-up `89e409a` limits suppression to trackers that can actually persist a
durable point/midpoint relation; generic tracking-only cues retain their guide and wake behavior
without contributing a competing relation. Orthogonal durable point/native-midpoint plus world-axis
bundles remain. Remembered Parallel, Perpendicular and Collinear directions retain their current
behavior, including Cartesian supports, and solver redundancy rejection is not weakened or
bypassed.

This narrowly supersedes M71-F004's same-axis-alternative rule only for eligible live world-axis
span constraints; the M71 record remains unchanged historical evidence. Public regression target
`m73_f004_span_axis_precedence`, focused/proportional editor qualification and the clean replacement
release/publication gates pass. Focused human UAT and explicit approval remain pending.

## Compatibility result

- No released `0.2.0` API was removed. The retired editor compatibility surface was introduced
  after `0.2.0`, had no non-test direct caller and duplicated only part of contextual authoring.
- `ConstraintIntent`, `ResolvedConstraintKind`, `AuthoringState` and
  `RetainedEditorCoordinator::{resolved_constraint, apply_authoring}` remain the public headless
  authoring route.
- `SketchConstraintKind`, `DocumentConstraintDefinition`, direct sketch builders, persistent
  relations and canonical sketch v1-v4 bytes are unchanged.
- The F001-F004 production implementation diff changes only `geosolve-constraint-editor`; no
  browser, solver, residual, Jacobian, priority, branch, persistence or golden-oracle
  implementation changed.

## Pre-F004 mechanical qualification

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

Because `docs/API_COMPATIBILITY.md` is copied into the seven-file web distribution, the historical
F001-F003 candidate nomination deliberately waited for one more clean gate on committed
current-status source.

## M73-F004 focused and proportional qualification

Final behavior source: `89e409a6ebe12c640ae2f313f95de67430dfa8d0`

Regression-hardening source: `f41e398d00b7a7ca1e12a12a285408a0b7bd3566`

Passed evidence:

- public integration target `m73_f004_span_axis_precedence`: **3/3 pass**, including one atomic
  retained commit/history step, finite accepted geometry and independently bounded accepted hard
  residual;
- inference-owner tests cover Horizontal and Vertical durable point/native-midpoint precedence,
  suppression before guide publication, candidate-budget accounting, latch acquisition and
  cross-axis pair construction, retained orthogonal bundles, preserved generic tracking-only cues
  and remembered Parallel/Perpendicular/Collinear controls;
- the focused `same_axis_span` owner run includes the complete four-way durable point/midpoint by
  axis matrix and passes **5/5** with exact top-level guide and latch checks; the public midpoint
  row proves its native reference wake before suppression;
- complete `geosolve-constraint-editor` suite: **325 unit tests plus every integration suite pass**;
- M71 F003, F004 and F005 public regressions plus native/WASM transition parity pass;
- warnings-denied Clippy passes; and
- the golden survey, check and `--require-clean` pass unchanged at **234/234**.

This is focused interaction-owner hardening, not a new systemic golden dimension. No clean
replacement release gate or publication is claimed from these development checks; the separate
clean candidate gate below owns that evidence.

## Historical F001-F003 candidate nomination

Final qualified product source: `efde645345577f44e0d6b691f7ca27eb587c4b53`

Final qualified product tree: `ae1ddaebd75e740c48eafc0b9ef2ad07cd99378b`

The same exact clean command passed again on that source. The 256-moving-body sparse crossover
completed in 124.36 seconds; the unchanged 234/234 golden, complete workspace matrix, native/WASM
parity, licensing, packaging and Trunk 0.21.14 release assembly all passed.

The gate-produced distribution was copied without rebuilding to read-only snapshot
`/tmp/geosolve-m73-uat.5EhWNL` (directory mode `0555`, files `0444`):

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `c3ef0cedd4de5968e36d2917daaf463c450fbe2266a06bc45b0cfae2dc20b935` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-e4b791bbd99777b2.js` | `5647aeac2f7852f1bf4015722528386b67c7c31208f9f5ce2cccbbb7171f2988` |
| `geosolve-demo-web-e4b791bbd99777b2_bg.wasm` | `77f232d4b41c5bbe6a5e4db0982d987e20d1ca88c89335f4155565b496e2a34c` |
| `index.html` | `067d186896a12889f35b11f99331088eb04d8f4ce05149e2663b223cfd40d5c7` |
| `styles-437727272832bc26.css` | `9e4b1c6985f119cff35366119fbeef8abb2096b386a8db78a4cd730915316344` |

The C-locale ordered-manifest aggregate is
`371596d68a75ce4415970d3237f0511426958918b55b1376fc44700735ba2095`.
PID `3403533` served only that snapshot on Tailscale and has since exited. The immutable snapshot
remains as historical evidence.

M73-F004 withdraws this snapshot and endpoint from current UAT authority. The bytes, hashes and
clean-gate result remain historical F001-F003 evidence; they are not a replacement candidate for
the implemented F004 correction. The qualified replacement below now owns current UAT authority.

## Current F004 replacement qualification and nomination

Final qualified product source: `4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`

Final qualified product tree: `fe9897153baa974b3c5c06e7a3bf5eee76e920f2`

The exact clean command passed completely:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That run passed:

- editor **325/325** plus every integration suite and public M73 regression **3/3**;
- the unchanged **234/234** authoring/scene golden inventory, survey, check and clean gate;
- native/WASM transition parity, workspace formatting, warnings-denied Clippy, locked all-feature
  tests and warnings-denied Rustdoc;
- all-target benchmark compilation, M14/M32 performance, licensing and packaging;
- the 256-moving-body sparse crossover in **135.18 seconds**; and
- the Trunk **0.21.14** release build.

The exact gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m73-uat.JKAWtJ` (directory mode `0555`, files `0444`):

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 15,490 | `c3ef0cedd4de5968e36d2917daaf463c450fbe2266a06bc45b0cfae2dc20b935` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-92f14bb278c26c6b.js` | 33,093 | `5647aeac2f7852f1bf4015722528386b67c7c31208f9f5ce2cccbbb7171f2988` |
| `geosolve-demo-web-92f14bb278c26c6b_bg.wasm` | 6,021,403 | `bc1a23dd0f7917152c69a1f94e9858ceaf0d912a955db4bd68d77bca5a268342` |
| `index.html` | 26,345 | `a2cf744c5daea9cea42c5dbd7dd58c6a27d9e508841f54e5589a4256ef7b3f40` |
| `styles-437727272832bc26.css` | 27,010 | `9e4b1c6985f119cff35366119fbeef8abb2096b386a8db78a4cd730915316344` |

The C-locale `sha256sum *` aggregate is
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`.

PID `3870531` serves only this snapshot at `http://100.94.63.83:8080/` with exact argv:

```text
python3 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m73-uat.JKAWtJ
```

Its executable is
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`.
Proxy/cache-bypassed, identity-encoded requests for all seven files and `/` return HTTP 200 with
the expected media types and compare byte-for-byte. `/` equals `index.html`, and the fetched
aggregate equals the frozen aggregate. This replacement is current UAT authority.

## Qualification ledger

- [x] Correct and activate the scope; record the no-ADR decision and unreleased API disposition.
- [x] Implement M73-F001 through M73-F003 in reviewable commits.
- [x] Pass F001-F003 focused owner tests and all relevant collateral suites.
- [x] Preserve the canonical authoring/scene oracle at 234/234 byte-for-byte.
- [x] Pass formatting, diff hygiene, warnings-denied workspace Clippy, locked all-feature tests,
  native/WASM parity and the complete clean release gate.
- [x] Rerun the clean gate on the committed current-status source, freeze the exact seven release
  files and verify their Tailscale publication byte-for-byte; retain that withdrawn candidate as
  historical F001-F003 evidence.
- [x] Freeze public regression `m73_f004_span_axis_precedence` and complete the focused
  Horizontal/Vertical durable point/midpoint precedence, early-suppression, budget/latch,
  generic-tracking, orthogonal-bundle and remembered-direction owner matrix.
- [x] Implement M73-F004 without changing solver redundancy authority, persistence, browser policy
  or the 234-row golden.
- [x] Pass focused/proportional editor, M71 transition, Clippy and unchanged golden checks.
- [x] Repeat the complete clean replacement release gate on committed product/documentation source.
- [x] Freeze, serve and byte-verify a replacement immutable UAT candidate.
- [ ] Complete `docs/M73_UAT.md` with explicit human approval.

## Current blocker

No external blocker. Focused human UAT and explicit approval are the only remaining ordered work;
M73 remains open and the replacement candidate above is current authority.
