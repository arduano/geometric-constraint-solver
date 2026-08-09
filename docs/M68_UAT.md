<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M68 focused UAT — Fillet direct manipulation

Status: closed with explicit supervising-human approval on 2026-08-09. Implementation, focused
direct qualification and complete release qualification pass.

Candidate source: `edffb8a`

Historical Tailscale endpoint: `http://100.94.63.83:8080/`

Release distribution manifest:
`77d071d711255c2c2385cee04d3b6820e5a0ed2dc4d8ffa501abcbab97657c79`

Delivery check: all seven served HTTP responses matched the frozen local distribution by SHA-256
before handoff. The endpoint is not a continuing post-close requirement.

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

Result: Accepted under the explicit M68 close decision.

Notes:

## M68-U2 — fold and invalid-sample behavior

1. On the near-fold line-circle specimen, confirm the exact radius-`0.5` fold reports a typed limit
   and deliberately exposes no fabricated radius rail.
2. Use the compact numeric radius control to continue the same branch to a nearby regular value;
   then drag its restored rail back toward radius `0.5` with slow and fast pointer sampling.
3. Move beyond the valid range, release while invalid, then try again and return to valid samples.
4. Cancel an active drag with Escape and by beginning a camera change.
5. While dragging, deliberately enter and leave an invalid state as the global problem card
   appears; watch the canvas framing and geometry under the stationary pointer.
6. Undo/Redo and reload after both rejected and accepted attempts.

Expected: the exact fold is truthful and rail-less; after numeric continuation reaches regular
same-branch geometry, its rail appears. Solid geometry then stops at the last valid result and a
concise typed limit/reason appears. No sample silently crosses to another root. Releasing without
a current preview, cancelling or changing the camera publishes nothing and adds no history entry.
Valid recovery resumes from the same absolute branch. One accepted gesture is exactly one Undo
step and survives reload. The problem appears as a bottom-left canvas overlay: it neither resizes
the canvas nor changes pointer-to-model mapping, and it does not intercept the active gesture.

Result: Accepted under the explicit M68 close decision.

Notes:

## M68-U3 — retained-direction and branch editing

1. Select a published line-circle Fillet and inspect its retained-direction arrows.
2. Hover/focus a retained-direction arrow, inspect the preview and commit it.
3. Exercise the same retained-direction and available branch actions through the compact panel.
4. Repeat on reversed line-line and line-Bezier examples, including a tied/ambiguous choice.
5. Put Fillets on both corners of a three-segment polyline, select the shared FilletSet and inspect
   the middle segment's direction controls.
6. Fillet a line against a full circle or ellipse, then compare it with a line against an arc.

Expected: current source geometry stays normally selectable, and preview/commit preserve every
branch field not explicitly changed. A tied choice reports ambiguity rather than guessing. The
canvas has no endpoint contact circles; retention actions first preview and then commit through the
same action in the canvas and accessible panel. Each arrow is the sole symbol for that action and
becomes visibly brighter, thicker and glowing when its headless preview is active. Direct contact
manipulation is not part of the M68 canvas or panel surface; its typed metadata and internal
continuation seam remain headless. A shared segment already consumed at both ends has no arrow for
an impossible retained-direction change, while valid outer-segment controls remain available. A
full circle or ellipse remains visually complete and exposes no meaningless trim-direction arrow;
an arc or other open curve retains its ordinary trimming behavior.

Result: Accepted under the explicit M68 close decision.

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

Result: Accepted under the explicit M68 close decision.

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

Result: Accepted under the explicit M68 close decision.

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

Result: Accepted under the explicit M68 close decision.

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

Retest: Accepted under the explicit M68 close decision. The close decision records the resolved
finding without claiming a separate exhaustive replay of every scripted step.

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

Retest: Accepted under the explicit M68 close decision. The accepted contract has one central
radius handle, no endpoint dots or circular branch controls, and no hidden canvas contact-drag
behavior. Branch choices remain lightweight icons/arrows.

### M68-F003 — branch arrows fell through to Fillet radius dragging

Observation: hovering or dragging a visible branch control could resolve to the Fillet radius
surface instead. Removing its circular backplate made the control look lighter, but did not fix
the interaction mismatch. Some retained-direction arrows also still had a redundant glyph beside
the arrow, and their hover response was not visually obvious.

Root cause: the headless resolver originally gave every radius hit priority over a branch action.
After reversing that semantic priority, overlapping 24-pixel SVG action corridors exposed a
second adapter defect: DOM paint order could report a different stamped arrow from the uniquely
nearest headless action, causing safe rejection and a fall-through to ordinary Fillet dragging.

Disposition: fixed by `8e3ee5d`, `25211e5` and `f5a17b9`. A current stamped painted action must
match exact owner, action, accepted/computed input, applicability and model-space proximity before
outranking
the radius surface. The adapter now submits every stamped action in the SVG stack at the pointer,
so the unique headless-nearest action wins independently of paint order. The visible central grip
keeps priority where it actually covers an arrow. Retained-direction arrows have no adjacent
glyph, and an active headless preview brightens and thickens the full arrow with a glow.
The canvas-only SVG action suppresses Chrome's pointer-focus outline; accessible-panel buttons
retain the workbench keyboard-focus ring.

Mechanical regression: 170 editor unit tests, 46 editor integration tests and 68 web tests pass.
Focused cases reject stale, foreign, far and spoofed targets; admit an independently verified
arrow over the Fillet surface; and prove an overlapping topmost corridor cannot suppress the
headless-nearest action. A real release-browser reproduction showed a three-entry overlapping SVG
stack resolving to `reverse-first`; the arrow became `3px` bright/glowing and a drag changed
neither the `0.5` radius, central grip nor generated arc.

Retest: Accepted under the explicit M68 close decision. The accepted contract retains one strongly
highlighted headless-nearest arrow, no radius fall-through or pressed outline, no adjacent duplicate
glyph and normal keyboard focus indication for Inspector buttons.

### M68-F004 — impossible retained arrows and closed-loop trimming

Observation: a straight segment already trimmed by Fillets at both ends could still display a
retained-direction arrow even though committing it was rejected. Full circles and ellipses used by
a Fillet were also rendered as trimmed open fragments.

Root cause: local action enumeration solved the edited corner in isolation, while source-claim
conflicts are defined only by complete feature-document composition. Separately, every Fillet
contact emitted a visual trim claim even when its visible parent domain was one complete period.

Disposition: fixed by `a1ed6ff`. The headless coordinator evaluates each exact replacement in a
cloned complete feature document and publishes only actions whose owning feature remains
`Current`. Full-period parents still supply contact and branch-continuation geometry but no longer
publish source-fragment claims or retained-direction actions. Bounded/open curves and explicitly
open views of periodic supports keep their existing trimming behavior.

Mechanical regression: 37 feature tests, 170 editor unit tests, all 46 editor integration tests
and 68 web tests pass. Focused cases cover two adjacent Fillets sharing a middle segment, a
line-circle Fillet that retains the complete circle, full circle/ellipse topology, a directed arc
and an explicitly open periodic view. Strict Clippy, warnings-denied WASM checking, release Trunk
and seven-asset Tailscale byte verification pass.

Retest: Accepted under the explicit M68 close decision. The accepted contract omits an
uncommittable middle-segment arrow, retains valid outer actions, preserves a full circle/ellipse and
keeps arcs and other open parents trim-capable.

### M68-F005 — global error panel resized the canvas during invalid gestures

Observation: entering an invalid solver state automatically inserted the Problems panel below the
canvas. The resulting layout shift resized the viewport relative to the held pointer and could
make an already invalid gesture diverge further.

Root cause: `.wb-problems` owned an `auto` row in the workbench grid. Toggling its `hidden` state
therefore changed the height of the central canvas row even though problem presentation is not
geometry state.

Disposition: fixed by `edffb8a`. The same accessible assertive live region now lives inside the
position-stable canvas panel as a bounded bottom-left overlay. It is outside grid flow and does not
accept pointer events, so appearing, disappearing or manually toggling Problems cannot resize the
canvas or steal a gesture.

Mechanical regression: all 69 web tests pass. A focused presentation test owns canvas DOM
containment, absolute overlay positioning, absence of grid-flow sizing and pointer transparency.
Formatting, strict web Clippy, warnings-denied WASM checking, release Trunk and byte verification
of all seven Tailscale assets pass.

Retest: Accepted under the explicit M68 close decision. The accepted contract keeps the canvas and
pointer mapping stable while the non-intercepting global card appears or disappears.

## Approval

On 2026-08-09, the supervising human explicitly accepted the focused M68 UAT and requested
milestone closure. M68-U1 through M68-U6 and resolved findings `M68-F001` through `M68-F005` are
accepted under that close decision with no new blocker recorded. This approval does not replace
the direct qualification above or invent a separate exhaustive replay of every scripted step.
