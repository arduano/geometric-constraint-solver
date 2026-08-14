<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 focused UAT — Retained drafting relations

Status: the clean M71-F004 UAT candidate is mechanically qualified, frozen and byte-verified at
the endpoint below. All M71-U1 through M71-U5 results remain pending; M71 is not complete until
the supervising human records an explicit approval decision.

Qualified F004 product source: `a2e51efba7d79f684d264094ffd7dd0e37a4d089`

Qualified F004 product tree: `8b73be00a384fe4a36ebe13fa0c06f32a6694a14`

Current F004 endpoint: `http://100.94.63.83:8080/`

Current F004 immutable snapshot: `/tmp/geosolve-m71-f004-uat.SaXMVY`

Current F004 release distribution manifest aggregate:
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`

Withdrawn F003 source: `83bd2b575784c44b618fb3ad144f24e84702d764`

Former F003 endpoint — the shared address now serves F004; **do not use the preserved F003
snapshot for UAT**: `http://100.94.63.83:8080/`

Preserved F003 immutable snapshot: `/tmp/geosolve-m71-f003-uat.hybK8W`

Preserved F003 release distribution manifest aggregate:
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`

Withdrawn pre-F003 source: `ad01912eac28275644dcfc867a2dc70030b5406d`

Shared historical endpoint — the shared address now serves F004; **do not use the withdrawn
snapshot for UAT**: `http://100.94.63.83:8080/`

Withdrawn immutable snapshot: `/tmp/geosolve-m71-uat.yFBsnX`

Withdrawn release distribution manifest aggregate:
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

Use only the ordinary GeoSolve Sketch Workbench and one editable **Retained drafting relations**
playground. Direct Rust/native-WASM tests are authoritative for equations, residuals, lifecycle,
persistence, ranking and publication. Human review assesses discoverability, predictability,
annotation clarity and recovery.

## M71-F004 discovery and corrected contract

While drawing a vertical line, the top endpoint could not simultaneously remain vertical to the
line start and horizontally aligned with a remembered point to the side. At the exact intersection,
the headless engine returned an ambiguity between singleton `Vertical` and `HorizontalPoints`;
with pointer bias it selected only one. The symmetric horizontal-line plus vertical-point case had
the same defect. Clean base `603194947a642917b9e44359326708de37f1a1d2` independently reproduces
the failure.

The corrected contract publishes one candidate at the exact Cartesian intersection and one atomic
plan whose relation order is endpoint axis first and line/polyline direction second. The focused
owner regression `crates/geosolve-constraint-editor/tests/m71_f004_axis_bundle.rs` covers
`HorizontalPoints + Vertical` for a line and `VerticalPoints + Horizontal` for a polyline, including
finite accepted geometry, independent endpoint equations, accepted hard residual `<= 1e-9`, two
constraints, one history step and later compatible edits.

Composition is limited to complementary exact Cartesian directions. Exact axis-aligned remembered
Parallel/Perpendicular/Collinear evidence may compose; oblique and same-axis directions remain
alternatives. Distinct remembered operands remain ambiguous. Candidate IDs, both hysteresis
latches, conservative worst-component angular evidence and streaming fail-closed candidate limits
have direct owner coverage. No solver equation, Jacobian, priority, branch or persistence format
changes.

The F003 snapshot is preserved unchanged so release evidence is not destroyed, but its former
server has exited. Those bytes lack F004 and are withdrawn from continued UAT. Do not perform any
scorecard step against those historical bytes; use only the clean, byte-verified F004 candidate
published above.

## Clean F004 candidate qualification and publication

The qualified product source was HEAD `a2e51efba7d79f684d264094ffd7dd0e37a4d089` with tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14` on `main`. Before and after qualification, `git
status --short` was empty, origin divergence was `0 3`, and exactly one worktree existed. The clean
gate ran from `2026-08-14T13:04:17+10:00` through `2026-08-14T13:11:13+10:00` using exactly:

```text
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

It passed completely; the retained log is
`/tmp/geosolve-m71-f004-clean-gate.ZGQEKU.log`. The canonical golden remained unchanged at 234
rows, the 256-moving-body sparse crossover took 125.55 seconds, and Trunk 0.21.14 completed the
release assembly. Cargo emitted only the repository's longstanding non-failing `license` plus
`license-file` advisories. HEAD, tree and empty status were unchanged after the gate.

Without rebuilding, exactly seven regular, non-symlink files were copied directly from that
gate's `crates/geosolve-demo-web/dist`, byte-compared by manifest and frozen at
`/tmp/geosolve-m71-f004-uat.SaXMVY` with directory mode `0555` and file modes `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 14165 | `bf7bb1b88a7a6ae55701d10af9b58e2dddbcfaa0f899931d9937c3272f50f239` |
| `LICENSE` | 35148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-4c3212f5ba819fe0.js` | 33327 | `ae66dbea0ce8581e4b0ae2a63a83db2e18a4489f7bfa245627e2c16b757ef22b` |
| `geosolve-demo-web-4c3212f5ba819fe0_bg.wasm` | 6014468 | `f5dfccd077120d4ed0876f318c4cd6a86bfc672a74c40e496a01bd232923a911` |
| `index.html` | 22977 | `98c30dd76cb6f9cd5c33d86b41b3769e5fabbf25fe7f87b612acfbd2d865104c` |
| `styles-36c74d05d21a90c9.css` | 29304 | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

The C-locale ordered `sha256sum *` manifest aggregate is
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`.

PID `2848202` serves only that immutable snapshot at `http://100.94.63.83:8080/` with argv
`/run/current-system/sw/bin/python3 -u -m http.server 8080 --bind 100.94.63.83 --directory
/tmp/geosolve-m71-f004-uat.SaXMVY`. Its resolved executable is
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`; its log is
`/tmp/geosolve-m71-f004-uat.SaXMVY.server.log`. The candidate server listens only on
`100.94.63.83:8080`; the unrelated VS Code listener on `127.0.0.1:8080` is not part of this
publication.

At `2026-08-14T13:13:48+10:00`, proxy-disabled and cache-bypassed requests with
`Accept-Encoding: identity` fetched all seven assets and `/`. Every request returned HTTP 200 from
remote IP `100.94.63.83`, each named asset had the exact recorded size and compared byte-for-byte
equal, and `/` equalled `index.html`. The fetched aggregate, post-fetch snapshot aggregate and
recorded aggregate all matched. Fetch evidence is retained at
`/tmp/geosolve-m71-f004-fetch.aDfzU7`.

This publication documents the already-qualified product source. The later documentation commit
that records the evidence is not part of, and must not replace, the product source identity above.

## Historical post-F004 provisional mechanical evidence

The dirty F004 development tree based on HEAD `603194947a642917b9e44359326708de37f1a1d2`
passed exactly:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate passed formatting/diff hygiene, warnings-denied workspace Clippy, all locked all-feature
workspace tests, the unchanged 234/234 golden at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`, native/WASM M70 and M71
transition parity, with the updated M71 fixture at SHA-256
`98df37349faab89e7ca7da763d898b84d4f04588a4923539cd790ca673a53442`, demo-web WASM,
warnings-denied rustdoc, benchmark compilation, M14/M32 budgets, the 151.18-second 256-moving-body
sparse crossover, licence/package validation and Trunk 0.21.14 release assembly. Current focused
and owner results include the 2/2 F004 public regression,
311/311 constraint-editor unit tests plus every integration/doc test, 104/104 demo-web unit tests
plus decoder/doc tests, 17/17 M71 sketch relation tests and 7/7 persistence tests. Cargo emitted
only the longstanding non-failing `license` plus `license-file` advisories.

This was complete development evidence but not clean nomination evidence. The implementation/test
repair is committed as `1f542555d7fcaf98ecf92c69a10b951fbfcc3dff`, and the supervising human
has granted ordinary reviewable-commit authority. At that checkpoint the complete source still had
to be clean and the gate repeated without `GEOSOLVE_ALLOW_DIRTY`; the clean qualification and
publication above subsequently satisfied that requirement.

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
edits updating the live midpoint average. Post-F003 owner and full development-gate outcomes
passed; the initial run used `GEOSOLVE_ALLOW_DIRTY=1`, so it was not clean candidate qualification.
The later F003 clean gate and now-withdrawn publication below supplied that historical evidence.

## Historical post-F003 provisional mechanical evidence

The F003 correction passed 17/17 M71 relation tests, 7/7 persistence tests, the exact
AxisMidpointResidual finite-difference check, the 2/2 public F003 coordinator regression, 302/302
constraint-editor unit tests plus integration/doc tests, and 104/104 demo-web unit tests plus its
decoder/doc tests. The canonical golden remained 234/234 `PASS` at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`.

Native/WASM M70 and M71 transition parity, demo-web WASM, formatting, warnings-denied workspace
Clippy, locked all-feature workspace tests, warnings-denied rustdoc, benchmark compilation,
M14/M32 budgets, the 152.53-second sparse crossover, licensing/package validation and Trunk 0.21.14
release assembly all passed through:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

Cargo emitted only the longstanding non-failing `license` plus `license-file` advisories. This
provisional evidence was followed by the historical F003 clean gate and withdrawn publication
below.

## Withdrawn F003 replacement evidence

Clean F003 source `83bd2b575784c44b618fb3ad144f24e84702d764` historically passed exactly:

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

At the F003 checkpoint PID `1202735` served only `/tmp/geosolve-m71-f003-uat.hybK8W` and listened
only on `100.94.63.83:8080`. Proxy-disabled, cache-bypassed requests matched every listed file
byte-for-byte. A separate request for `/` matched `index.html`. The fetched and post-fetch local
ordered aggregates both reproduced
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`.
These bytes are now withdrawn because they predate M71-F004. PID `1202735` has since exited and
that F003 server is offline; the shared endpoint now serves only the F004 snapshot recorded above.

## Preconditions

- [x] The focused F004 owner regression and inference composition matrix pass, including finite
  accepted geometry, independently recomputed endpoint equations, ambiguity, identity,
  hysteresis, exact-axis provenance and resource limits.
- [x] The complete post-F004 dirty-tree development gate passes, including the owning Rust
  acceptance matrix, finite-difference Jacobian and audit coverage, frozen compatibility,
  persistence, clean golden, native/WASM parity, formatting, warnings-denied Clippy and locked
  workspace tests. This historical dirty-tree result remains provisional evidence only.
- [x] The complete matrix passes on unchanged clean nominated F004 product source
  `a2e51efba7d79f684d264094ffd7dd0e37a4d089`.
- [x] That clean post-F004 nominated source passes `./scripts/release-gate.sh` without a dirty-tree
  override.
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
2. While drawing a vertical **line**, place its endpoint level with a remembered stored point to
   the side and require one preview and atomic placement carrying exactly
   `HorizontalPoints + Vertical`. Repeat the same bundle through the **polyline** path.
3. While drawing a horizontal **line**, place its endpoint vertically aligned with a remembered
   stored point and require exactly `VerticalPoints + Horizontal`. Repeat the same bundle through
   the **polyline** path.
4. For every stored-point bundle above, confirm both constraint-backed guides end at the same exact
   intersection. After placement, edit the remembered point and then the new span start; require
   both retained relations to remain present and jointly satisfied after each edit.
5. Wake native midpoints from both a line and a polyline span. For each source kind, exercise both
   the line and polyline authoring paths: a vertical new span must place
   `HorizontalPointToMidpoint + Vertical`, and a horizontal new span must place
   `VerticalPointToMidpoint + Horizontal`.
6. For every native-midpoint bundle, confirm both guides and the exact intersection, then move and
   resize the remembered midpoint's source span and edit the new span start. Require both retained
   relations to survive and remain jointly satisfied after every edit.
7. Also exercise native midpoint alignment one axis at a time, then repeat with a fillet-discarded
   midpoint occurrence and an unsupported nonlinear derived anchor.
8. Exercise suppression, shared leave/re-enter hysteresis and an exact ambiguous tie.

Expected: stored-point alignment may atomically create HorizontalPoints/VerticalPoints and remains
durable during later edits. A complementary exact Cartesian line/polyline direction may be retained
in that same atomic placement, with both guides ending at the exact intersection. This must work
for both line and polyline authoring paths in both Cartesian orientations. A native line/polyline
midpoint may create HorizontalPointToMidpoint and/or VerticalPointToMidpoint: Horizontal constrains
Y and Vertical X. Its complementary new-span direction must be retained in the same atomic
placement for both line and polyline paths. Editing the remembered point or midpoint source, or
editing the new span start, preserves both relations; midpoint alignment continues to use the live
endpoint average as its source span moves or resizes.
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
