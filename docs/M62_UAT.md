<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M62 human UAT — CAD constraint authoring

Status: pending objective qualification and supervising-human review.

This review uses the ordinary workspace. Do not load a scenario: M62 adds no scenario definitions
or UAT-only geometry. Existing scenarios remain read-only.

## Review scorecard

1. Create simple point, line and circle geometry. Confirm the wider two-column palette keeps
   geometry, constraint and dimension tools easy to scan.
2. Preselect compatible operands and click Coincident, Perpendicular / Normal, Tangent and Equal.
   Confirm each applies once, preserves selection and leaves Select active.
3. Preselect incompatible operands and click a relation. Confirm a specific warning appears and no
   design/history mutation occurs.
4. Clear selection, enter Coincident, then repeatedly click point pairs. Confirm each second click
   applies and the tool remains active with a fresh operand set.
5. Exercise Lock, Horizontal or Vertical in repeated single-pick mode; then Symmetric in
   point/point/axis order.
6. While one operand is pending, press Escape twice. Confirm the first clears operands and the
   second exits authoring.
7. Use a canvas curve pick and a tree pick in the same constraint. Confirm both follow the same
   guidance and pending highlighting.
8. Open Tangent, Equal and Continuity option flyouts. Confirm explicit orientation, curvature and
   continuity choices remain understandable and are remembered during this session only.
9. Create all five dimension kinds at their current accepted values. Verify Driving/Reference and
   oriented-angle direction options.
10. Select a dimension, change its numeric target in the inspector, then Undo and Redo. Confirm
    retained geometry, history and selected-dimension metadata remain coherent.
11. Load any existing scenario and confirm ordinary authoring is disabled and the scenario remains
    unchanged. Exit it and resume ordinary authoring.

## Approval

- Candidate commit: pending.
- Mechanical qualification: pending.
- Human rating: pending.
- Approval: pending explicit supervising-human decision.
