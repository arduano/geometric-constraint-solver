<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M68 focused UAT — Fillet direct manipulation

Status: not started. Implementation and focused direct qualification are complete. Do not run or
approve this scorecard until the complete clean release gate passes and a frozen candidate is
nominated.

Candidate source: pending

Tailscale endpoint: pending

Release distribution manifest: pending

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

## M68-U3 — contact and retained-direction editing

1. Select a published line-circle Fillet and inspect its two named contact handles and retained-
   direction arrows.
2. Drag each contact along its own native parent, including near but not through a tied/ambiguous
   choice.
3. Hover/focus a retained-direction arrow, inspect the preview and commit it.
4. Repeat on reversed line-line and line-Bezier examples.

Expected: the manipulated parent is unambiguous, current source geometry stays normally
selectable, and preview/commit preserve every branch field not explicitly changed. A tied choice
reports ambiguity rather than guessing. Retention actions first preview and then commit through the
same action in the canvas and accessible panel.

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

1. Press a contact handle where it overlaps a generated arc or native support.
2. Press the explicit radius grip/arc where it overlaps a native support.
3. During a drag, move outside the SVG and release; repeat with pointer cancellation and a second
   pointer/button attempt.
4. Compare hover/focus feedback with the action that clicking actually performs.
5. Pan and wheel-zoom while only collecting or inspecting a Fillet, then begin a Fillet drag and
   attempt a camera change.

Expected: contact beats radius, and explicit radius beats native support. Painted identity alone
cannot select a stale or foreign owner. The initiating pointer remains captured until clean release
or cancellation, no gesture is stranded, and another pointer cannot steal or publish it. Hover and
click resolve the same action. Camera navigation remains available outside live manipulation; a
live Fillet gesture cancels/restores before camera state changes.

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

No M68 human finding has been recorded yet. Add each finding here with a stable ID, reproduction,
root cause, disposition, mechanical regression and explicit retest status; do not silently edit an
Expected statement to hide a failure.

## Approval

Pending explicit supervising-human approval. Mechanical qualification, a reachable Tailscale
candidate and completion of this scorecard do not by themselves close M68.
