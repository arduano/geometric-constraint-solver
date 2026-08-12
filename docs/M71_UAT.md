<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 focused UAT — Retained drafting relations

Status: planned. Implementation, mechanical qualification, release candidate, Tailscale
publication and supervising-human approval are all pending.

Candidate source: pending

Tailscale endpoint: pending

Release distribution manifest aggregate: pending

Use only the ordinary GeoSolve Sketch Workbench and one editable **Retained drafting relations**
playground. Direct Rust/native-WASM tests are authoritative for equations, residuals, lifecycle,
persistence, ranking and publication. Human review assesses discoverability, predictability,
annotation clarity and recovery.

## Preconditions

- [ ] The complete M71 acceptance matrix passes at its owning Rust layers.
- [ ] Frozen v1-v4 compatibility and draft-v5/workspace/reproduction round trips pass.
- [ ] The canonical golden authoring/scene oracle passes clean with reviewed M71 systemic rows.
- [ ] Native/WASM parity, formatting, warnings-denied Clippy and locked workspace tests pass.
- [ ] One clean nominated source passes `./scripts/release-gate.sh`.
- [ ] Its immutable release distribution is published through Tailscale and every served byte is
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
4. Try unsupported center-bearing and close-but-not-center targets.

Expected: one atomic placement creates the new geometry plus Concentric against exact semantic
centers; it never invents a shared point or coordinate snap. Unsupported and ambiguous centers
fail closed without losing the draft.

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
