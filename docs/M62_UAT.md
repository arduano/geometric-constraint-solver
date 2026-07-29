<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M62 human UAT — CAD constraint authoring

Status: approved by the supervising human on 2026-07-29.

This review uses the ordinary workspace. Do not load a scenario: M62 adds no scenario definitions
or UAT-only geometry. Existing scenarios remain read-only.

Temporary Tailscale endpoint:

```text
http://100.94.63.83:8080/
```

The release watcher is serving the current M62 palette build. The endpoint is temporary and is not
a production deployment.

## Review scorecard

1. Create simple point, line and circle geometry. Confirm the wider two-column palette keeps
   geometry, constraint and dimension tools easy to scan.
2. Preselect compatible operands and click Coincident, Perpendicular / Normal, Tangent and Equal.
   Confirm each applies once, preserves selection and leaves Select active.
3. Preselect incompatible operands and click a relation. Confirm a specific warning appears and no
   design/history mutation occurs.
4. Clear selection, enter Coincident, then repeatedly click point pairs. Confirm each second click
   applies and the tool remains active with a fresh operand set. Also click a point and the exact
   endpoint of a bounded line; confirm point-on-curve applies with the endpoint branch rather than
   rejecting it as an interior contact.
5. Exercise Lock, Horizontal or Vertical in repeated single-pick mode; then Symmetric in
   point/point/axis order. Confirm one canvas click contributes exactly one operand and immediately
   re-arms each single-pick tool.
6. Enter Normal with no selection and click two different lines once each. Confirm the first line
   is pending, the second applies Perpendicular, and the active tool immediately requests a fresh
   first line rather than remaining stuck at two operands.
7. While one operand is pending, press Escape twice. Confirm the first clears operands and the
   second exits authoring. Before exiting, deliberately create a retained-rejected pair, Undo it
   and confirm a valid retry works while the same authoring tool remains active.
8. Use a canvas curve pick and a tree pick in the same constraint. Confirm both follow the same
   guidance and pending highlighting.
9. Open Tangent, Equal and Continuity option flyouts. Confirm explicit orientation, curvature and
   continuity choices remain understandable and are remembered during this session only. For
   Continuity, pick the End of one bounded curve and the Start of another; confirm the constraint
   applies rather than reporting inconsistent contact metadata.
10. Create all five dimension kinds at their current accepted values. For oriented angle, draw two
   lines at roughly 45 degrees with either endpoint order, enter angle authoring and pick them one
   at a time. Confirm creation does not move either line and the annotation/editor reports the
   acute angle in degrees rather than raw directed radians.
11. Select the angle dimension and change its acute-degree target to 60, then Undo and Redo.
    Confirm the visible acute angle, retained directed branch, history and selected-dimension
    metadata remain coherent. Values above 90 degrees must reject without mutation.
12. Load any existing scenario and confirm ordinary authoring is disabled and the scenario remains
   unchanged. Exit it and resume ordinary authoring.

## Finding ledger

- `M62-F001` — resolved and accepted: angle creation measured retained
  design seeds instead of the accepted canvas, exposed raw directed radians and did not distinguish
  retained rejection from accepted publication. The corrected candidate measures accepted
  geometry, presents acute supporting-line degrees, preserves the directed solver branch during
  edits and reports rejected publication explicitly.
- `M62-F002` — resolved and accepted: one canvas click was routed as both a
  pointer-down pick and a bubbled generic click, duplicating operands and leaving failed terminal
  candidates at full arity. Canvas pointer-down now owns the canvas pick exactly once, tree clicks
  retain their separate one-event route and every terminal attempt re-arms repeated authoring.
- `M62-F003` — resolved and accepted: the headless request adapter attached
  contact branch choices to simple curve relations that explicitly accept no contact state, so
  Horizontal and line-line Normal reached full arity but were rejected before document creation.
  Contact choices are now limited to contact-owning definitions; direct skew-line applications
  publish accepted Horizontal and Perpendicular constraints.
- `M62-F004` — resolved and accepted: the closed-path audit found that two
  picks on the same curve span both recovered the first pick parameter, and an End pick for
  continuity was paired with the Start neighborhood. Contact operands now preserve occurrence
  order and endpoint neighborhoods follow the actual parameter. Request-level and accepted
  transaction matrices cover all sixteen resolved relation families; a separate accepted
  transaction matrix covers all five dimension paths, which do not translate contact metadata.
- `M62-F005` — resolved and accepted: pre-closure headless hardening found
  that an ordinary bounded point-on-curve pick at a line endpoint retained parameter `0` or `1`
  but defaulted to the invalid Interior neighborhood. Bounded contacts now default to the matching
  Start/End neighborhood. Direct tests also cover repeated mode for every relation/dimension
  family, representative line/circle/Bezier/NURBS point-on-curve authoring, both continuity
  endpoint orders, rejection Undo/retry, dimension Undo/Redo and process-local option memory.

## Approval

- Candidate implementation: `53e7867` (including headless base `0ec560b`), with UAT follow-ups
  `10f95d5`, `435e898`, `3a59767`, `e49d124` and `b0d3913`.
- Mechanical qualification: Pass on 2026-07-29; exact commands are recorded in
  `docs/M62_IMPLEMENTATION.md`.
- Human rating: Pass for the recorded M62 scope.
- Approval: explicitly approved by the supervising human on 2026-07-29.
