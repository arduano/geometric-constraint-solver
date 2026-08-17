<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 focused UAT — CAD curve handles and implicit parameters

Status: **planned and open**. No candidate is nominated and no scorecard item is accepted. Record
the exact clean source, tree, immutable snapshot, manifest, endpoint and browser-check evidence
here only after the complete release gate passes.

Candidate source: pending

Candidate tree: pending

Tailscale endpoint: pending

Immutable snapshot and ordered-manifest aggregate: pending

Run the scorecard in the ordinary editable workbench at `1440x900` and approximately `1024x720`,
at coarse and fine zoom. Use a mouse or equivalent precise pointer. Repeat crowded targets on both
sides of their hit fringe; direct tests, not human judgment, own exact boundary equality.

## U1 — visibility, ownership and visual language

Select, deselect and reselect examples of circle, circular arc, ellipse, elliptical arc, rational
quadratic conic, parabola, hyperbola, Bezier, B-spline and NURBS.

- Only selected editable curves should show their applicable handles/cage; deselection, another
  tool and a different sample/document should remove them immediately.
- The cage should be quiet but legible: endpoint roles, stored controls, middle control and scalar
  size rails should be visually distinguishable without relying only on colour.
- Hover each visible handle and then click at the same location. Highlight, tooltip/cursor and
  pointer action must name the same role and owning curve. Underlying curve or annotation paint
  must not steal the click; stored control points must retain their ordinary point ownership.
- Selecting a derived endpoint or size handle must select its curve and must not create a point in
  the tree, persistence payload or constraint operand list.
- An active Fillet-owned output arc must keep its Fillet affordance and expose no competing generic
  radius/endpoint handles.

## U2 — trim endpoints

For a circular arc, elliptical arc, parabola segment and hyperbola segment, drag Start and End
independently. Begin off-centre inside each handle to check that it preserves the grab offset and
does not jump. Make a sub-3 px movement and release; it should select without editing or adding an
Undo step. Then cross the threshold and commit a visible change.

The endpoint must remain on its exact support curve while the opposite endpoint and support
controls behave consistently. Start and End must not silently exchange. Circular/elliptical sweep
and hyperbola branch must not flip when an endpoint passes near a wrap, axis or invalid crossing.
Inspect coarse and fine zoom for stable hover/click parity and no screen-space drift.

## U3 — rational and stored control cages

Select rational quadratic conics with positive and negative nonzero weights. Drag `P1` and confirm
that the control cage and curve update continuously while both stored endpoints and the numeric
middle weight remain unchanged. Undo and Redo once each. The middle handle is a control point, not
a promise that the curve passes through it; tooltip and inspector language should make that clear.

Create a non-unit-weight rational curve and confirm its construction click and later handle mean
the same `P1`. Load or enter a zero-weight curve and confirm it exposes an explicitly labelled
projective `Qh` vector instead of an ordinary point; entering and leaving that mode must be
deliberate, finite and free of division-by-zero jumps.

Select quadratic/cubic Bezier, B-spline and NURBS examples. Their stored controls should remain
ordinary draggable points, with a selected control polygon that makes influence understandable.
NURBS and rational weights should remain available as precise numeric inspector controls without a
second ambiguous spatial weight rail. A NURBS gauge row is read-only, and “Make gauge” must change
the numeric normalization without moving the curve.

## U4 — size handles and domains

Exercise circle and circular-arc radius, ellipse and elliptical-arc minor axis, and hyperbola
semi-conjugate size. The handle should follow a clear deterministic rail, preserve the initial grab
offset and update the intended scalar without moving unrelated stored controls.

Approach and cross each domain boundary: zero radius, zero semi-conjugate size, zero minor ratio and
a minor ratio greater than one. No non-finite, negative or family-swapped geometry may appear. An
invalid sample should retain the last valid finite preview; returning to a valid pointer location
should resume normally. Releasing while invalid should publish only the exact last valid candidate,
or no edit if no changed valid candidate ever existed. Existing driving/locked ownership should be
reported honestly rather than accepting a contradictory handle move.

## U5 — cancellation, stale work and history

For endpoint, middle and size gestures, cancel independently with Escape, pointer-capture loss,
tool change and camera change. Each must restore the exact pre-gesture accepted scene and add no
history entry. Start another gesture, then trigger an accepted-scene replacement or Undo from the
owning workbench path; an old preview/result must not reappear or commit afterward.

Commit one valid gesture. It must add exactly one Undo step regardless of preview sample count.
Undo must restore the complete pre-drag curve and Redo the exact final candidate, including trim,
sweep/branch and control values. A rejected or unchanged gesture must add none. Problems text,
selection and hover must describe the current scene rather than a stale preview.

## U6 — persistence and desktop polish

Commit representative endpoint, rational-middle and size edits, save/reload the workspace and use
the reproduction copy/restore path. The curve geometry and existing scalar/weighted-middle values
must round-trip; transient handles should be recomputed only after selection and no handle cache or
synthetic point should persist.

At both desktop sizes and zoom ranges, check that handles remain easy to acquire without becoming
visually aggressive, control cages do not obscure annotations, tool popouts remain usable, and
keyboard focus/accessibility names stay meaningful. Tab focus must not synthesize pointer hover,
and canvas hover must not steal focus.

## Acceptance record

- U1 — visibility, ownership and visual language: pending
- U2 — trim endpoints: pending
- U3 — rational and stored control cages: pending
- U4 — size handles and domains: pending
- U5 — cancellation, stale work and history: pending
- U6 — persistence and desktop polish: pending
- Final supervising approval: pending
- GitHub Pages publication and hosted-byte verification: pending
