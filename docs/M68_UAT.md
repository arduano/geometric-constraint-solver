<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M68 focused UAT — Fillet direct manipulation

Status: ready for supervising-human UAT. Implementation, focused direct qualification and complete
release qualification pass. M68 remains open until this scorecard receives
explicit human approval.

Candidate source: `25211e5`

Tailscale endpoint: `http://100.94.63.83:8080/`

Release distribution manifest:
`24438f7019d58628ca3c34814be890c6a7a6687f233545d7b6ef03ee84664e05`

Delivery check: all seven served HTTP responses match the frozen local distribution by SHA-256.

Use only the ordinary GeoSolve Sketch Workbench. Direct Rust tests are the correctness authority;
this scorecard assesses discoverability, continuity and interaction feel. It does not qualify an
Offset/Mirror tool, two-non-affine-parent Fillet, computed chaining, Bake/Explode, profile/topology
consumption, cross-revision topological naming, computed arcs as sketch-constraint operands,
persistence/schema changes, global root enumeration, browser E2E, mobile behavior or a legacy UI.

Recommended starting point after the candidate is published: open **Samples → Curves &
constructions → 2D Fillet playground**. Use the friendly line-circle specimen for ordinary
contact/branch exploration and the separately labelled near-fold specimen only for limit behavior.
Both must remain normal editable save-like geometry.

## M68-U1 — stable radius rail

1. Author a line-line Fillet and drag its visible radius grip slowly along the displayed rail.
2. Repeat by pressing the generated arc body, then use the compact radius control.
3. Move mostly perpendicular to the rail, reverse direction several times and cross multiple zoom
   levels.
4. Repeat on a multi-corner FilletSet and on the friendly line-circle and line-Bezier specimens.

Expected: radius follows one stable local axis without chasing the moving centre, inverting or
jumping roots. Perpendicular motion is effectively a no-op. Arc-body, grip and numeric edits share
the same behavior. Every arc in a shared-radius set is visibly identified and updates atomically;
native points, constraints, rank and DOF do not change.

Result: Pending

Notes:

## M68-U2 — fold and invalid-sample behavior

1. On the near-fold line-circle specimen, confirm the exact radius-`0.5` fold reports a typed limit
   and deliberately exposes no fabricated radius rail.
2. Use the compact numeric radius control to continue the same branch to a nearby regular value;
   then drag its restored rail back toward radius `0.5` with slow and fast pointer sampling.
3. Move beyond the valid range, release while invalid, then try again and return to valid samples.
4. Cancel an active drag with Escape and by beginning a camera change.
5. Undo/Redo and reload after both rejected and accepted attempts.

Expected: the exact fold is truthful and rail-less; after numeric continuation reaches regular
same-branch geometry, its rail appears. Solid geometry then stops at the last valid result and a
concise typed limit/reason appears. No sample silently crosses to another root. Releasing without
a current preview, cancelling or changing the camera publishes nothing and adds no history entry.
Valid recovery resumes from the same absolute branch. One accepted gesture is exactly one Undo
step and survives reload.

Result: Pending

Notes:

## M68-U3 — retained-direction and branch editing

1. Select a published line-circle Fillet and inspect its retained-direction arrows.
2. Hover/focus a retained-direction arrow, inspect the preview and commit it.
3. Exercise the same retained-direction and available branch actions through the compact panel.
4. Repeat on reversed line-line and line-Bezier examples, including a tied/ambiguous choice.

Expected: current source geometry stays normally selectable, and preview/commit preserve every
branch field not explicitly changed. A tied choice reports ambiguity rather than guessing. The
canvas has no endpoint contact circles; retention actions first preview and then commit through the
same action in the canvas and accessible panel. Each arrow is the sole symbol for that action and
becomes visibly brighter, thicker and glowing when its headless preview is active. Direct contact
manipulation is not part of the M68 canvas or panel surface; its typed metadata and internal
continuation seam remain headless.

Result: Pending

Notes:

## M68-U4 — explicit local alternatives

1. Select a Fillet with more than one bounded local solution.
2. Hover/focus each outlined alternative and dashed complementary/local arc preview.
3. Move from one preview to another without clicking, then commit one through the canvas.
4. Undo and commit the same action through the compact accessible panel.

Expected: the solid current branch never changes on hover/focus. Alternatives are local to the
same two native parents/neighbourhoods and are visually distinct from the current branch. Canvas
and panel use the same labels, disabled reasons and action identities. Click commits only the
explicitly previewed alternative; Undo restores the exact prior branch.

Result: Pending

Notes:

## M68-U5 — crowded hit priority and pointer capture

1. Confirm each selected Fillet corner shows one central radius handle and no endpoint circles.
2. Press the explicit radius grip/arc, including near an endpoint or overlapping native support.
3. During a drag, move outside the SVG and release; repeat with pointer cancellation and a second
   pointer/button attempt.
4. Compare hover/focus feedback with the action that clicking actually performs.
5. Pan and wheel-zoom while only collecting or inspecting a Fillet, then begin a Fillet drag and
   attempt a camera change.

Expected: explicit radius beats native support, while a visible branch arrow beats an overlapping
Fillet radius surface unless the central radius grip visibly covers it; an endpoint has no
invisible contact-drag hit zone. Overlapping arrow corridors resolve to the unique headless-nearest
action rather than SVG paint order, and painted identity alone cannot select a stale or foreign
owner. The initiating pointer remains captured until clean release or cancellation, no gesture is
stranded, and another pointer cannot steal or publish it. Hover and click resolve the same action.
Camera navigation remains available outside live manipulation; a live Fillet gesture
cancels/restores before camera state changes.

Result: Pending

Notes:

## M68-U6 — ordinary workflow and compatibility

1. Draw/edit native geometry, author representative constraints and drag native points before and
   after Fillet interaction.
2. Create several Fillets, change branches/radii, delete one, then exercise Undo/Redo and refresh.
3. Verify computed arcs are not offered as sketch-constraint operands.
4. Confirm no Offset/Mirror helper UI, Bake/Explode, profile-consumption claim, legacy harness or
   `/#/dev/lab` application appears.

Expected: M68 direct manipulation does not lock or mutate native sketch state. Stable
feature/corner identities and explicit branch intent survive history and reload while generated
output IDs remain revision-local. M27/M28/M58 advanced compatibility remains unchanged. The
workbench exposes only the approved M68 Fillet slice.

Result: Pending

Notes:

## Finding ledger

### M68-F001 — false radius-rail branch fold after native source edits

Reproduction: draw a four-point/three-segment polyline, apply Fillets to both corners, adjust a
Fillet, move several source points and then try to adjust a Fillet again. The arcs remain visibly
valid, but the frozen `b01e583` candidate can reject the radius gesture with “the Fillet radius
rail is ill-conditioned at a branch fold.”

Root cause: valid affine/affine feature evaluation used the complete unique line-intersection
domain after source edits, while continuation reconstructed the radius rail only within ±12.5%
of stale pre-edit parent-contact parameters. A moved valid contact outside that artificial window
was mislabeled as an ill-conditioned fold.

Disposition: fixed by `c82d420`. Evaluation and continuation now share one current-branch domain
policy. Affine/affine supports search their complete certified cells; any non-affine parent keeps
the prior seed-local guard, and genuine fold/near-parallel sensitivity checks are unchanged.

Mechanical regression: feature, editor-unit, editor-integration and demo-web suites pass. Direct
tests cover two regular grouped Fillets after large source-point drags, finite rails without a
continuation status, radius preview/publication, one history step, stable IDs and unchanged native
sketch identity, coordinates, residuals, rank and DOF. The full release gate passes on `c82d420`,
and the replacement frozen distribution is byte-verified at the Tailscale endpoint.

Retest: Pending. Repeat the reproduction on the candidate and confirm both Fillets remain
adjustable after moving the polyline points.

### M68-F002 — redundant endpoint Fillet drag handles

Observation: a selected Fillet displayed a central radius handle plus two contact circles at its
ends. All three appeared to resize the Fillet, making the canvas unnecessarily busy.

Disposition: fixed by `227cc9a` and `5355162`. The first change removes the two endpoint contact
circles and their canvas hit priority. Live-browser inspection then showed that the circular
backplates behind retained-direction and branch icons still looked like extra handles; the second
change removes those backplates while preserving the icon/arrow actions. The visible generated arc
and single central grip retain radius dragging, while typed contact/branch metadata and internal
headless continuation support remain preserved.

Mechanical regression: editor tests prove endpoint hover/press resolves to the visible radius
surface rather than an invisible contact target. Web markup tests prove one central grip for the
selected corner, no `wb-fillet-contact` elements and no circular branch-action backplates. A live
browser reproduction over the frozen Tailscale bundle confirms the selected affordance group has
one circle, class `wb-fillet-radius-grip`. The full release gate passes on `5355162`.

Retest: Pending. Select a Fillet and confirm it shows one central radius handle, no endpoint dots
or circular branch controls, and no hidden contact-drag behavior at either end. Branch choices may
remain visible as lightweight icons/arrows.

### M68-F003 — branch arrows fell through to Fillet radius dragging

Observation: hovering or dragging a visible branch control could resolve to the Fillet radius
surface instead. Removing its circular backplate made the control look lighter, but did not fix
the interaction mismatch. Some retained-direction arrows also still had a redundant glyph beside
the arrow, and their hover response was not visually obvious.

Root cause: the headless resolver originally gave every radius hit priority over a branch action.
After reversing that semantic priority, overlapping 24-pixel SVG action corridors exposed a
second adapter defect: DOM paint order could report a different stamped arrow from the uniquely
nearest headless action, causing safe rejection and a fall-through to ordinary Fillet dragging.

Disposition: fixed by `8e3ee5d` and `25211e5`. A current stamped painted action must match exact
owner, action, accepted/computed input, applicability and model-space proximity before outranking
the radius surface. The adapter now submits every stamped action in the SVG stack at the pointer,
so the unique headless-nearest action wins independently of paint order. The visible central grip
keeps priority where it actually covers an arrow. Retained-direction arrows have no adjacent
glyph, and an active headless preview brightens and thickens the full arrow with a glow.

Mechanical regression: 169 editor unit tests, 46 editor integration tests and 68 web tests pass.
Focused cases reject stale, foreign, far and spoofed targets; admit an independently verified
arrow over the Fillet surface; and prove an overlapping topmost corridor cannot suppress the
headless-nearest action. A real release-browser reproduction showed a three-entry overlapping SVG
stack resolving to `reverse-first`; the arrow became `3px` bright/glowing and a drag changed
neither the `0.5` radius, central grip nor generated arc.

Retest: Pending. Select a Fillet, hover each visible arrow—including crowded arrows—and confirm
exactly one arrow highlights strongly. Click one to commit its branch action, then drag from an
arrow and confirm it does not resize the Fillet. Confirm there is no separate glyph beside a
retained-direction arrow.

## Approval

Pending explicit supervising-human approval. Mechanical qualification, a reachable Tailscale
candidate and completion of this scorecard do not by themselves close M68.
