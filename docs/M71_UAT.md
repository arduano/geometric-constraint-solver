<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 focused UAT — Retained drafting relations

Status: implementation, clean integrated release qualification and immutable Tailscale
publication complete on 2026-08-13. Supervising-human approval is pending.

Candidate source: `ad01912eac28275644dcfc867a2dc70030b5406d`, nominated from `main`

Tailscale endpoint: `http://100.94.63.83:8080/`

Immutable snapshot: `/tmp/geosolve-m71-uat.yFBsnX`

Release distribution manifest aggregate:
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

Use only the ordinary GeoSolve Sketch Workbench and one editable **Retained drafting relations**
playground. Direct Rust/native-WASM tests are authoritative for equations, residuals, lifecycle,
persistence, ranking and publication. Human review assesses discoverability, predictability,
annotation clarity and recovery.

## Qualified candidate evidence

The clean nominated source passed exactly:

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

PID `49116` serves only that snapshot and listens only on `100.94.63.83:8080`; its log is outside
the snapshot at `/tmp/geosolve-m71-uat.yFBsnX.server.log`. Proxy-disabled, cache-bypassed HTTP
requests byte-matched every listed asset. A separate request for `/` byte-matched `index.html`.
The fetched-file aggregate and the post-fetch snapshot aggregate both reproduced the recorded
ordered manifest aggregate exactly. Because this endpoint reuses the historical M70B port, hard
refresh before review. The historical M70B snapshot remains on disk but is no longer served.

## Preconditions

- [x] The complete pre-UAT automated M71 acceptance matrix passes at its owning Rust layers.
- [x] Frozen v1-v4 compatibility and draft-v5/workspace/reproduction round trips pass.
- [x] The canonical golden authoring/scene oracle passes clean with reviewed M71 systemic rows.
- [x] Native/WASM parity, formatting, warnings-denied Clippy and locked workspace tests pass.
- [x] One clean nominated source passes `./scripts/release-gate.sh`.
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

## M71-U2 — durable point alignment versus tracking-only anchors

1. Wake a stored persistent point, then author another point or suitable construction near its
   horizontal and vertical guide.
2. Confirm the displayed constraint-backed candidate and place it; move either point afterward.
3. Repeat from a line midpoint or another derived semantic anchor.
4. Exercise suppression, leave/re-enter hysteresis and an exact ambiguous tie.

Expected: stored-point alignment may atomically create HorizontalPoints/VerticalPoints and remains
durable during later edits. Midpoint/derived alignment stays visibly tracking-only and creates no
fixed coordinate, zero dimension, hidden geometry or retained relation. Suppression and ambiguity
never commit a stale or arbitrary candidate.

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
2. Reload the workspace and load a copied reproduction payload containing all four relations.
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
