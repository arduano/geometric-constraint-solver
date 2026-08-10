<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70 focused UAT — Auto-constraint drafting

Status: focused supervising-human review is paused on open finding `M70-F001`. Implementation,
focused direct qualification, the complete integrated release gate, frozen-candidate publication
and served-byte verification pass for the initial candidate below. A replacement candidate,
targeted recheck, remaining human results and approval are Pending.

Candidate source: `4b16db3a885f5e28f508189b8817797375f05807` on `main`

Tailscale endpoint: `http://100.94.63.83:8080/`

Release distribution manifest aggregate:
`e0cf0a44184ae1a3e5308e77adb478cb41db1fa529d42f3c8cb9969160325044`

```text
29725af79af0ecb8198fe2c4fd5bfb80b69f1e9f81ec418e7bc1f056ba2480d7  dist/API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  dist/LICENSE
665e4df98334f5efea3efa83d18ea71198a182825c2d40f96dbf141e43a2a418  dist/THIRD_PARTY_LICENSES.md
ff0797fa408bc3be7ad572af8541bb31ccc9767914d8c4629c77cd298925cefd  dist/geosolve-demo-web-2f7c0aa7bbcd31d0.js
9632e099cb42a6c9e29487018260be6d8d1c2fdc948fdc82ff831556b2b8f242  dist/geosolve-demo-web-2f7c0aa7bbcd31d0_bg.wasm
7546200a552bf530f3464cab2406b54eb1bf9d8dc423663b3662a0c632b07e03  dist/index.html
cee6aac04d97f80072827c8b29a86f79071d01fa0cc523736c0c5f20e27b0e1b  dist/styles-aafdbbd399fb8c99.css
```

All seven served assets and `/` were fetched through the Tailscale address with proxy/cache bypass
and compared byte-for-byte with the read-only frozen release distribution. `/` matched
`index.html`, and the local aggregate remained unchanged. Because the endpoint reuses port 8080
from earlier milestones, perform one hard refresh before starting this scorecard.

Use only the ordinary GeoSolve Sketch Workbench and the editable **Samples → Constraints &
dimensions → Auto-constraint drafting playground**. Direct Rust tests are authoritative for
candidate ranking, constraint metadata, atomicity and solver validity. This scorecard assesses
discoverability, predictability, suppression and recovery.

This frozen source has the passing direct matrix and clean release gate recorded in
`docs/M70_IMPLEMENTATION.md`. The placement click is the explicit confirmation: there is no second
Apply step for inferred relations.

## M70-U1 — existing points, circle-through-point and horizontal/vertical spans

1. Activate Point and click the centre of an existing persistent-point specimen.
2. Draw lines whose free endpoint approaches an existing persistent endpoint from several sides.
3. Place one line endpoint while the existing-point proposal is active, then Undo and Redo.
4. Draw separate nearly horizontal, nearly vertical and clearly diagonal lines.
5. Repeat for successive spans of one polyline and at several zoom levels.
6. Draw circles whose circumference click approaches an existing standalone point and a persistent
   line endpoint.
7. Repeat the circumference click over an arbitrary line interior away from either endpoint.

Expected: the adjusted preview, guide and glyph agree before placement. Existing-point placement
reuses the point identity rather than leaving a coincident duplicate. Clicking that point with the
standalone Point tool is a history-neutral no-op because there is no new durable construction.
Reusing it as a line/polyline operand commits that complete construction in one history step. H/V
activates and releases with stable hysteresis; clearly diagonal input remains unadjusted and
behavior remains consistent under zoom. A circle circumference may pass through an existing
persistent point, including a line endpoint, by atomically committing PointOnCurve with that
existing point and the newly created circle. The radius click creates no hidden rim point, and an
arbitrary line interior creates no implicit contact or tangency.

Result: Finding open — `M70-F001`; M70-U1 is not approved.

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

## Finding ledger

### M70-F001 — circle circumference did not express circle-through-point intent

Status: **Open**.

The supervising-human UAT of frozen candidate
`4b16db3a885f5e28f508189b8817797375f05807` identified a missing semantic distinction at the second
Circle click. That click supplies a radius sample; it is not an ordinary authored point operand.
When it enters the point tolerance of an existing persistent point or line endpoint, the preview
must say **Circle through point** and confirmation must atomically create the circle plus
PointOnCurve(existing point, created circle). It must allocate no hidden circumference point.
Semantic midpoints and arbitrary line interiors are not eligible for this reverse-incidence snap,
and the gesture must not infer line contact or tangency.

Closure requires direct headless inference/commit regressions, thin presentation coverage, a fresh
clean release candidate and publication, and a targeted supervising-human recheck of the two U1
circle steps. Until then the prior release evidence remains historical candidate evidence only;
M70 remains open and M71 remains inactive.

## Approval

Pending. M70 closes only after the supervising human explicitly approves M70-U1 through M70-U5,
`M70-F001` is resolved on a freshly qualified and published candidate, and all objective findings
have direct owning-layer regressions plus the necessary targeted recheck.
