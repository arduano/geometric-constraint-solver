<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 focused UAT — CAD geometry tool families and authoring variants

Status: **planned and open**. No candidate is nominated and no scorecard item is accepted. Record
the exact clean source, tree, immutable snapshot, manifest, endpoint and browser-check evidence
here only after the complete release gate passes.

Candidate source: pending

Candidate tree: pending

Tailscale endpoint: pending

Immutable snapshot and ordered-manifest aggregate: pending

Run the scorecard in the ordinary editable workbench at `1440x900` and approximately `1024x720`,
at coarse and fine zoom, using both Profile and Construction roles. Direct tests, not visual
judgment, own exact threshold equality, residual validation, persistent IDs and branch bytes.

## U1 — family palette, variant memory and stage language

Open each of the nine family buttons and verify the exact 25 variants in `docs/M78_GOALS.md`.
Choose a non-default variant, blur the overlay, pan/zoom and click the canvas; it should stay open
and remember the variant. Switch families and return; session memory should restore the last valid
variant/options. The main icon should remain centred and should not expose a separate chevron.

Start every variant and compare its prompt, stage markers and live readout with the gesture being
performed. Width/height, `R`/`Ø`, sweep and control-count language should be concise and truthful.
Closing the overlay should activate/focus Select. A successful shape should leave its exact variant
active for repeated creation.

## U2 — points, segments, polylines and midpoint lines

Place one free Sketch Point, then confirm the tool over an existing persistent point. The second
action should reuse that identity as a history-neutral no-op rather than allocate a duplicate.
Create a Segment with one reused endpoint and ordinary directional inference. Create an open
Polyline finished by Enter and another by double-click; create a closed Polyline by clicking its
first vertex. Closure must not show or persist a doubled first point or zero-length final edge.

During a Polyline, use Backspace and then Undo to remove only the latest unfinished vertex. Confirm
that accepted history is unchanged until the draft is empty. Press Escape once to cancel the
current chain while staying in Polyline, then Escape again to return to Select.

Create a Midpoint Line from a free centre and from an existing point. Move an endpoint afterward:
the retained centre should remain the segment midpoint through an ordinary visible relation, not a
hidden lock or dimension.

## U3 — four rectangle recipes and Shift squares

Create all four rectangle variants in ordinary mode, then repeat each while holding Shift. Inspect
the tree and Problems/constraint presentation:

- every result has four explicit shared-corner line edges;
- aligned variants remain aligned and three-point variants retain their orientation;
- centre variants show one ordinary Construction helper diagonal and a centre Midpoint relation;
- no rectangle creates a lock, driving/reference dimension or target scalar; and
- every Shift result retains one EqualLength square intent after release, drag, Undo/Redo and
  reload.

Repeat one Shift rectangle while holding Ctrl/Cmd. Ambient snapping should be suppressed, but the
intrinsic square and rectangle relations must remain. Approach conflicting ambient H/V guidance;
the recipe's own alignment/shape must win without a failed placement or stale global problem.

## U4 — circles and arcs

Create Center–Radius, 2-Point Diameter and 3-Point Circle examples. For diameter and three-point
recipes, snap some rim samples to existing points and leave others free. Existing points should
receive visible curve incidence while free samples should not create synthetic tree points.
Three coincident or nearly collinear samples should keep a correction-ready draft and a local
message; moving the last sample to a valid position should recover without Escape or reload.

Create Center Arc and use `F` before release to compare complementary sweeps. Create a 3-Point Arc
and confirm it passes through the ordered Through sample with the intended Start/End span. Existing
snapped trim/rim points should remain associative without new synthetic endpoint objects.

Create Tangent Arcs from eligible endpoints of several native open families and from both endpoint
directions. The preview and committed arc should leave the source smoothly with a visible ordinary
tangency relation. Try an interior point, periodic curve, zero-length chord and near-straight
infinite-radius case; each should be unavailable or locally recoverable, never accepted as
non-finite geometry or a stale global failure.

## U5 — ellipses, Béziers and conics

Create both full-ellipse variants and both elliptical-arc variants. Centre-based and axis-endpoint
forms should communicate the same centre/major/minor frame with different input order. Arc Start
and End samples must land on the displayed support ellipse. Use `F` to flip the complementary
sweep without exchanging endpoint identity. No browser-side jump, axis swap or numeric Start/End
construction field should appear.

Create quadratic and cubic Béziers and confirm the stage markers distinguish endpoints from
controls. Create Rational Quadratic, Parabola and both Hyperbola branch choices. Their family
overlays should preserve the established M77 ordinary/projective middle meaning, trim/domain
options and explicit branch state; moving them into grouped menus must not change accepted geometry
or later curve-handle editing.

## U6 — open and periodic control NURBS

Create an Open Control NURBS using enough controls for the chosen degree, remove one unfinished
control with Backspace, then finish with Enter. Repeat with double-click. Create a Periodic Control
NURBS and confirm its closure is explicit periodic topology rather than a duplicated last control
or proximity guess.

Try finishing too early and enter invalid degree/knot/weight options. The active overlay should
explain why finishing is unavailable and preserve the draft/options for correction. Switching to
another family should remain possible; invalid inactive NURBS fields must not block unrelated
geometry.

## U7 — modifiers, inference cycling and recovery

On representative Segment, rectangle, circle/arc and spline stages, hold Ctrl/Cmd and confirm that
ambient guides/adjustment disappear for that sample without changing intrinsic recipe relations.
Where several compatible inference candidates are published, use Tab to cycle them and confirm the
preview, guide and eventual relation agree.

For one fixed-length and one variable-length recipe, exercise stage Undo/Backspace, first/second
Escape, tool switch, overlay close and a deliberately invalid terminal sample. No cancelled or
rejected attempt may enter accepted history, reuse a retired persistent identity, blank the scene
or leave a global error after correction/Undo. One successful complete recipe must be exactly one
Undo/Redo step regardless of its stage count.

## U8 — role, persistence and desktop polish

Author representative variants with Profile active and with Construction active. Main curves
should follow the active role; centre-rectangle helpers should always remain Construction. Save and
reload, then use reproduction copy/restore. Geometry, roles, relations, branch state, variant
results and accepted history should survive through ordinary persisted document state, while the
session-only last-used palette variant may reset without corrupting the scene.

At both desktop sizes and zoom ranges, verify family overlays remain contained, stage prompts do
not cover the active geometry, keyboard focus/accessibility names are meaningful, and hover/click
feedback remains consistent with the exact next accepted operand. Tab focus must not synthesize
canvas hover and canvas movement must not steal overlay focus.

## Acceptance record

- U1 — family palette, variant memory and stage language: pending
- U2 — points, segments, polylines and midpoint lines: pending
- U3 — four rectangle recipes and Shift squares: pending
- U4 — circles and arcs: pending
- U5 — ellipses, Béziers and conics: pending
- U6 — open and periodic control NURBS: pending
- U7 — modifiers, inference cycling and recovery: pending
- U8 — role, persistence and desktop polish: pending
- Final supervising approval: pending
- GitHub Pages publication and hosted-byte verification: pending
