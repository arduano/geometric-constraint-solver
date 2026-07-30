<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M63 UAT — canvas constraints

Status: candidate awaiting supervising-human review.

## Candidate

- Branch: `main`
- Implementation commits: `9727b60`, with `M63-F001` remediation in `8fcf270` and `M63-F002`
  remediation in `e020477`. The insufficient `M63-F003` remediation is preserved in `e160116`;
  `M63-F004` supersedes it in `ea5dabd`.
- Temporary Tailscale endpoint: `http://100.94.63.83:8080/`.
- Scenario group: **Scenarios → M63 Canvas constraints**
- Mechanical gate: format, warnings-denied workspace Clippy, all-feature workspace tests,
  all-feature WASM check and release Trunk build passed on 2026-07-30.
- Delivery check: all seven served files matched the local release distribution byte-for-byte on
  2026-07-30. The endpoint is temporary and is not a production deployment.

## Scorecard

| Area | Scenario | What to verify | Result |
| --- | --- | --- | --- |
| Dimensions | Canvas angle & dimension presentation | Angle arcs/values are always visible; driving dimensions remain visible; other reference dimensions are contextual and editing keeps accepted branch behavior. | Pending |
| Discovery | Contextual constraint symbols | Geometry hover/selection reveals direct relations only; symbols are selectable and emphasize all direct operands. | Pending |
| Density | Crowded relation fan-out | Shared anchors fan out predictably with leaders and remain selectable through zoom/pan/reset. | Pending |
| Input ownership | Contextual constraint symbols | Constraint authoring continues to collect geometry once per physical click and visible symbols do not obstruct it. | Pending |
| Accessibility/errors | All three | Keyboard focus/activation works and targeted problems keep their relevant annotation visible. | Pending |

## Review sequence

1. Open each M63 leaf and press **Fit** when instructed.
2. Follow the scenario sidebar steps, including hover, click, keyboard, dimension edit and camera
   checks.
3. Record any finding with the stable scenario ID, selected persistent item and visible accepted
   diagnostics.
4. Explicitly approve or reject M63 for the scope above.

## Finding ledger

- `M63-F001` — resolved; human retest pending. In `canvas-relation-glyphs`, moving the tangent
  line left the circle radius numerically correct but made its leader jump unpredictably around
  the circumference. The presentation layer was choosing the farthest adaptively tessellated
  sample from an inferred center; all samples on a circle are mathematically tied, so tiny
  accepted-state differences changed the winner. Radial dimensions now use accepted persistent
  curve semantics and public curve evaluation: full circles use canonical parameter zero and
  circular arcs use their semantic midpoint. A headless regression perturbs the accepted radius
  and requires the anchor to remain on that branch. Recheck the new stability step in
  `canvas-relation-glyphs`.
- `M63-F002` — resolved; human retest pending. In `canvas-crowded-annotations`, the nominal
  eight-direction fan-out still placed adjacent symbol centers closer than the rendered glyph and
  hit-target footprints, and offsets around nearby but non-identical origins could collide again.
  The headless layout now searches deterministic concentric candidates and accepts one only when
  its final center is at least 22 px from every already placed glyph. Displaced glyphs retain their
  semantic leader. A regression builds the actual rotating-square fixture and checks every pair of
  final anchors plus leader exercise. Recheck all crowded corners after Fit, zoom and pan.
- `M63-F003` — remediation insufficient and superseded by `M63-F004`. The first correction made
  only the rendered leader a hover corridor. It still failed whenever the user left a different
  location on the related geometry and took a natural direct path toward the revealed symbol.
- `M63-F004` — resolved; human retest pending. The headless editor now retains the last direct
  geometry-hover position and builds bounded corridors from that actual position to every directly
  related revealed annotation. When corridors overlap, the nearest path wins deterministically.
  Hover transfers to the persistent constraint inside a corridor and clears immediately outside
  all contextual corridors. The regression point is deliberately outside both geometry and
  rendered-leader hit tolerances, then verifies transfer and unrelated-canvas clearing. Confirm the
  on-screen instruction begins **“Move directly from any hovered location…”** before retesting.

## Approval

Pending supervising-human UAT.
