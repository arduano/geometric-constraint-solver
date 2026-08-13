<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 focused UAT — Retained drafting relations

Status: M71-U2 exposed M71-F003. The corrected clean replacement is mechanically qualified,
published and byte-verified. The withdrawn U2 result remains discovery evidence;
corrected-candidate M71-U1 through M71-U5 results remain pending.

Replacement source: `83bd2b575784c44b618fb3ad144f24e84702d764`

Current endpoint: `http://100.94.63.83:8080/`

Current immutable snapshot: `/tmp/geosolve-m71-f003-uat.hybK8W`

Current release distribution manifest aggregate:
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`

Withdrawn pre-F003 source: `ad01912eac28275644dcfc867a2dc70030b5406d`

Historical pre-F003 endpoint (now serving the replacement instead): `http://100.94.63.83:8080/`

Withdrawn immutable snapshot: `/tmp/geosolve-m71-uat.yFBsnX`

Withdrawn release distribution manifest aggregate:
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

Use only the ordinary GeoSolve Sketch Workbench and one editable **Retained drafting relations**
playground. Direct Rust/native-WASM tests are authoritative for equations, residuals, lifecycle,
persistence, ranking and publication. Human review assesses discoverability, predictability,
annotation clarity and recovery.

## Withdrawn candidate evidence

The pre-F003 source passed exactly:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate completed formatting and diff hygiene, warnings-denied workspace Clippy, all locked
all-feature workspace tests, the 234/234 clean golden oracle at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`, native/WASM M70 and M71
transition parity, the demo-web WASM check, warnings-denied rustdoc, benchmark compilation, M14
and M32 performance budgets, the 144.08-second 256-moving-body sparse crossover, licence and
package checks, and Trunk 0.21.14 release assembly. It exited successfully. Cargo emitted only the
repository's longstanding non-failing `license` plus `license-file` advisories.

Exactly seven release files were copied without rebuilding, byte-compared with the clean-gate
`dist`, and frozen with directory mode `0555` and file mode `0444`:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `d97357c908774e51f39724d25d1f5fdabacd30cf13f7371a6df0d8957209a98b` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-49ba2a1c36571a17.js` | `a51b4caefd9c38224a75820a44d5a3b49e3bd6d4eeeb7aba3930ecd9a558e31d` |
| `geosolve-demo-web-49ba2a1c36571a17_bg.wasm` | `c51cb77d38ab682e21b940eb5f26a4e73ff92a5ac007c5fc3de7e70323290fc2` |
| `index.html` | `3d87c4b54efb42c8fdcb62c841140db29b9bb7b832733b197a9b4ac50cfee128` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

At that historical checkpoint, PID `49116` served only that snapshot and listened only on
`100.94.63.83:8080`; its log was outside the snapshot at
`/tmp/geosolve-m71-uat.yFBsnX.server.log`. Proxy-disabled, cache-bypassed HTTP requests
byte-matched every listed asset. A separate request for `/` byte-matched `index.html`. The
fetched-file aggregate and post-fetch snapshot aggregate both reproduced the recorded ordered
manifest aggregate exactly. Do not continue UAT against these withdrawn bytes. The historical
M70B and pre-F003 M71 snapshots remain on disk but are no longer served.

M71-F003 was independently reproduced at the public scene/editor/coordinator boundary: remembered
midpoints entered tracking, but only persistent-point references could become durable H/V. The
root cause was in `DraftInferenceEngine::point_tracking_candidates`: midpoint anchors could
originate guides, but only persistent-point anchors entered the durable relation branch. The
corrected contract adds explicit one-row `HorizontalPointToMidpoint` and
`VerticalPointToMidpoint` relations for accepted native line/polyline spans. The focused owner
regression is `crates/geosolve-constraint-editor/tests/m71_f003_midpoint_axis.rs`; it exercises the
ordinary scene/editor/coordinator transition, atomic point-plus-relation publication, Horizontal
constraining Y, Vertical constraining X, independent accepted residual evidence and later endpoint
edits updating the live midpoint average. Post-F003 owner and full development-gate outcomes pass;
the initial run used `GEOSOLVE_ALLOW_DIRTY=1`, so it was not clean candidate qualification. The
clean replacement gate and publication below supply that evidence.

## Post-F003 provisional mechanical evidence

The current correction passes 17/17 M71 relation tests, 7/7 persistence tests, the exact
AxisMidpointResidual finite-difference check, the 2/2 public F003 coordinator regression, 302/302
constraint-editor unit tests plus integration/doc tests, and 104/104 demo-web unit tests plus its
decoder/doc tests. The canonical golden remains 234/234 `PASS` at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`.

Native/WASM M70 and M71 transition parity, demo-web WASM, formatting, warnings-denied workspace
Clippy, locked all-feature workspace tests, warnings-denied rustdoc, benchmark compilation,
M14/M32 budgets, the 152.53-second sparse crossover, licensing/package validation and Trunk 0.21.14
release assembly all pass through:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

Cargo emitted only the longstanding non-failing `license` plus `license-file` advisories. This
provisional evidence was followed by the clean replacement gate and publication below.

## Current replacement evidence

Clean source `83bd2b575784c44b618fb3ad144f24e84702d764` passed exactly:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate passed the complete sequence described above, with a 145.13-second sparse crossover and
successful Trunk 0.21.14 release assembly. Its exact seven-file `dist` was copied without
rebuilding, byte-compared, and frozen with directory mode `0555` and file mode `0444`:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `bf7bb1b88a7a6ae55701d10af9b58e2dddbcfaa0f899931d9937c3272f50f239` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-f3ecc0dffeb9ce14.js` | `ae66dbea0ce8581e4b0ae2a63a83db2e18a4489f7bfa245627e2c16b757ef22b` |
| `geosolve-demo-web-f3ecc0dffeb9ce14_bg.wasm` | `53bd9bfdc0cec56f9f3520af328c45c8a5dcda3e836c43017d2b1409b48c1a9e` |
| `index.html` | `946d66a5e03e56b22efd3ee99fc157ba9668c10ae4393695b6200274f57aace4` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

PID `1202735` serves only `/tmp/geosolve-m71-f003-uat.hybK8W` and listens only on
`100.94.63.83:8080`. Proxy-disabled, cache-bypassed requests matched every listed file
byte-for-byte. A separate request for `/` matched `index.html`. The fetched and post-fetch local
ordered aggregates both reproduced
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`.

## Preconditions

- [x] The complete post-F003 pre-UAT automated M71 acceptance matrix passes at its owning Rust
  layers, including finite-difference Jacobian, structured audit and independent validation
  coverage for both midpoint-axis rows.
- [x] Frozen v1-v4 compatibility and corrected draft-v5/workspace/reproduction round trips pass.
- [x] The canonical golden authoring/scene oracle passes clean after review confirms that F003
  remains a focused owner regression rather than a new systemic golden dimension.
- [x] Native/WASM parity, formatting, warnings-denied Clippy and locked workspace tests pass.
- [x] One clean post-F003 nominated source passes `./scripts/release-gate.sh`.
- [x] Its immutable release distribution is published through Tailscale and every served byte is
  verified against the local candidate.

## M71-U1 — manual authoring and canvas presentation

1. Apply Horizontal and Vertical to one line, then to two stored points in both selection orders.
2. Apply Concentric to representative circle/arc/ellipse center-bearing pairs in both orders.
3. Apply Collinear to line and polyline supports in both orders and directions.
4. Hover/select each accepted relation and inspect its canvas annotation and constraint entry.

Expected: applicability and labels describe the actual relation; operand order does not change
meaning; every accepted relation remains editable and attributable to one ordinary source. Invalid
selections show a precise disabled reason and create nothing.

Result: pending

Notes:

## M71-U2 — durable point and native-midpoint axis alignment

1. Wake a stored persistent point, then author another point or suitable construction near its
   horizontal and vertical guide.
2. Confirm the displayed constraint-backed candidate and place it; move either point afterward.
3. Repeat from native line and polyline midpoints, first one axis at a time and then both axes on
   the same point; move and resize the source span afterward.
4. Repeat with a fillet-discarded midpoint occurrence and an unsupported nonlinear derived anchor.
5. Exercise suppression, leave/re-enter hysteresis and an exact ambiguous tie.

Expected: stored-point alignment may atomically create HorizontalPoints/VerticalPoints and remains
durable during later edits. A native line/polyline midpoint may create
HorizontalPointToMidpoint and/or VerticalPointToMidpoint: Horizontal constrains Y and Vertical X,
and both axes keep the point at the live endpoint average as the span moves or resizes.
`FilletDiscarded` and nonlinear curve-parameter midpoint occurrences remain visibly tracking-only.
No case creates a fixed coordinate, zero dimension or hidden midpoint point. Suppression and
ambiguity never commit a stale or arbitrary candidate.

Result: pending

Notes:

## M71-U3 — concentric inference and same-construction operands

1. Author a centered primitive near the accepted center of another eligible primitive.
2. Inspect the Concentric preview and place it, then drag/edit either parent.
3. Repeat with reversed construction/selection order and Profile/Construction geometry.
4. Try non-center-bearing/unsupported curves and close-but-not-center targets.

Expected: one atomic placement creates the new geometry plus Concentric against exact semantic
centers. At a curve's stored center, centered-construction intent wins over incidental point reuse;
ordinary point authoring still reuses that point identity. It never invents a shared point or
coordinate snap. Unsupported and ambiguous centers fail closed without losing the draft.

Result: pending

Notes:

## M71-U4 — certified collinear extension inference

1. Wake a native line/polyline support, then author a compatible affine span along its supporting
   line beyond the finite endpoint.
2. Approach from both directions and repeat with reversed support direction.
3. Compare near-parallel, sampled-crossing, overlapping/identical and degenerate cases.
4. Edit the source support after placement.

Expected: only exact certified line-support evidence proposes Collinear. A finite-span extension is
explicitly a supporting-line relationship, not a hidden contact outside the bounded span. Generic
intersections and uncertified cases remain unavailable or ambiguous, and the accepted relation
tracks later edits.

Result: pending

Notes:

## M71-U5 — retained lifecycle and recovery

1. Suppress/reactivate and delete each relation, then Undo and Redo.
2. Reload the workspace and load a copied reproduction payload containing all six relations.
3. Create one redundant and one conflicting proposal and recover the still-active draft.
4. Change accepted scene/input state between preview and placement to exercise stale rejection.

Expected: source IDs/order, annotations, diagnostics and accepted geometry round-trip exactly.
Suppression, deletion and history use ordinary retained behavior. Rejected, stale, cancelled or
resource-exhausted work changes no live document/history and never publishes partial geometry or a
different relation.

Result: pending

Notes:

## Approval

Pending explicit supervising-human approval after M71-U1 through M71-U5. M71 is not complete until
that decision is recorded; mechanical qualification alone is insufficient.
