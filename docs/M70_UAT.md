<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70 focused UAT — Auto-constraint drafting

Status: draft; implementation and focused direct qualification are complete. Integrated release
qualification and publication are pending. Human UAT has not begun, and every result below remains
Pending.

Candidate source: **PENDING — NOT YET FROZEN**

Tailscale endpoint: **PENDING — NOT YET PUBLISHED**

Release distribution manifest: **PENDING — NOT YET GENERATED OR BYTE-VERIFIED**

Use only the ordinary GeoSolve Sketch Workbench and the editable **Samples → Constraints &
dimensions → Auto-constraint drafting playground**. Direct Rust tests are authoritative for
candidate ranking, constraint metadata, atomicity and solver validity. This scorecard assesses
discoverability, predictability, suppression and recovery.

M70 UAT must not begin until `docs/M70_IMPLEMENTATION.md` records a passing direct matrix and clean
release gate against one frozen source. The placement click is the explicit confirmation: there is
no second Apply step for inferred relations.

## M70-U1 — existing points and horizontal/vertical spans

1. Activate Point and click the centre of an existing persistent-point specimen.
2. Draw lines whose free endpoint approaches an existing persistent endpoint from several sides.
3. Place one line endpoint while the existing-point proposal is active, then Undo and Redo.
4. Draw separate nearly horizontal, nearly vertical and clearly diagonal lines.
5. Repeat for successive spans of one polyline and at several zoom levels.

Expected: the adjusted preview, guide and glyph agree before placement. Existing-point placement
reuses the point identity rather than leaving a coincident duplicate. Clicking that point with the
standalone Point tool is a history-neutral no-op because there is no new durable construction.
Reusing it as a line/polyline operand commits that complete construction in one history step. H/V
activates and releases with stable hysteresis; clearly diagonal input remains unadjusted and
behavior remains consistent under zoom.

Result: Pending.

Notes:

## M70-U2 — native curves and semantic midpoints

1. Place points/endpoints on the prepared line, circle, Bezier and NURBS targets.
2. Compare a generic position on the prepared line with its exact midpoint.
3. Inspect the accepted constraint and move compatible native geometry after placement.
4. Repeat against Profile and explicit Construction targets using the canvas scope controls.

Expected: curve inference follows the visible native curve and persists explicit contact metadata;
the midpoint wins over generic PointOnCurve at the semantic midpoint. Moving source geometry keeps
the accepted relationship. Role/scope behavior matches ordinary M69 selection, and no computed
Fillet arc becomes an inference target. The line, circle, Bezier and NURBS objects are representative
UI specimens; direct Rust tests, not this manual sample, own conic, arc and B-spline family
completeness.

Result: Pending.

Notes:

## M70-U3 — wake, leave and remembered direction

1. Start a line, hover a prepared reference line until it wakes, then move away without clicking.
2. Approach parallel and perpendicular directions and place one of each.
3. Wake the midpoint of a line, leave it, then place a new span along the midpoint normal guide.
4. Repeat with a polyline reference and with an unrelated nearby point/curve present.

Expected: reference wake is immediate and visible but non-mutating. Leaving the original hover does
not lose the stage-local reference; the later unique direction proposal controls both preview and
accepted relation. Midpoint plus normal may commit its compatible positional/directional bundle in
one click. Ranking remains deterministic and unrelated geometry does not steal the intended
reference.

Result: Pending.

Notes:

## M70-U4 — suppression, ambiguity and honest tracking

1. Hold Shift while passing through point, midpoint, curve and H/V inference positions; click one
   raw placement while Shift remains held.
2. Release Shift at the same location and observe inference recompute from the current pointer.
3. Exercise the prepared exact-overlap/ambiguous area.
4. Wake a bare point and follow its horizontal/vertical tracking guide without creating a line
   direction that has a supported durable relation.

Expected: suppression clears guides/latches, remembers no reference and commits raw placement only.
Releasing it does not resurrect a stale candidate. Exact unresolved ties are visibly ambiguous and
do not auto-commit. Bare-point tracking is explicitly guidance-only: it creates no fixed coordinate,
zero dimension or hidden construction geometry.

Result: Pending.

Notes:

## M70-U5 — rejection, lifecycle and ordinary editing

1. Activate Line and click the centres of the **Redundant inference rejection start** and
   **Redundant inference rejection end** markers. The existing Construction line over those exact
   points already owns Horizontal; click the second centre while inferred Horizontal is visible.
2. After rejection, keep the same line draft active, move the endpoint visibly off the horizontal
   axis and place that corrected geometry-only candidate.
3. Cancel an awakened draft, switch tools, Undo/Redo, change camera, refresh the page and reopen
   the sample between separate attempts.
4. Continue ordinary constraint authoring, point dragging, role/scope editing and camera use after
   inferred placement.

Expected: invalid inferred work never falls through to a different relation and never partially
commits geometry. The exact draft remains recoverable after rejection. Wake/reference state clears
at every lifecycle boundary and is not restored from workspace persistence. The valid placement is
one atomic history step, and the rest of the workbench remains ordinary and editable.

Result: Pending.

Notes:

## Approval

Pending. M70 closes only after the supervising human explicitly approves M70-U1 through M70-U5 and
all objective findings have direct owning-layer regressions plus any necessary targeted recheck.
