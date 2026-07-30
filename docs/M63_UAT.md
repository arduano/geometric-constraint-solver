<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M63 UAT — canvas constraints

Status: candidate awaiting supervising-human review.

## Candidate

- Branch: `main`
- Implementation commits: `9727b60`, with `M63-F001` remediation in `8fcf270` and `M63-F002`
  remediation in `e020477`. The insufficient `M63-F003` remediation is preserved in `e160116`;
  the insufficient `M63-F004` remediation is preserved in `ea5dabd`, and `M63-F005` supersedes it
  in `88295f5`. The icon-language refinement `M63-F006` is implemented in `1978e33`;
  `M63-F007` moves line-relation markers to line interiors in `e75bb1b`; `M63-F008`
  specializes visible line-line perpendicularity as right-angle geometry in `22f52b3`; and
  `M63-F009` completes the adjacent workbench icon audit in `38f79f3`.
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
| Icon language | Palette, sketch tree and all three leaves | Geometry/constraint/dimension icons are representative, shared concepts match between palette and canvas, advanced curves remain distinguishable, and tree/problem symbols communicate their category without placeholder text. | Pending |
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
- `M63-F004` — remediation insufficient and superseded by `M63-F005`. The headless editor retained the last direct
  geometry-hover position and builds bounded corridors from that actual position to every directly
  related revealed annotation. When corridors overlap, the nearest path wins deterministically.
  However, it transferred the persistent constraint into the geometry-hover state, so crossing one
  icon could hide siblings; leaders also counted as icon hits and all occurrences of one
  multi-marker constraint highlighted together.
- `M63-F005` — resolved; human retest pending. Headless `EditorHoverState` now keeps the geometry
  context owner separate from exact `EditorHoverTarget` proximity. Glyph targets carry a stable
  marker index. Leaders and bounded links between related icons preserve the complete revealed set
  as transit without hovering any icon; only the proximate occurrence highlights, and clicking it
  selects the persistent constraint once. Direct regressions cover the full geometry → transit →
  first icon → inter-icon transit → second icon → blank sequence and renderer child-level hover.
  Confirm the on-screen instruction begins **“Move from related geometry through the revealed
  set…”** before retesting.
- `M63-F006` — implemented; human review pending. The palette previously used Unicode characters
  and letters unrelated to the separately hand-drawn canvas glyphs, while contact, direction,
  normal, curvature and continuity were especially ambiguous. One text-free CAD vector catalog
  now owns all eleven constraint intents, five dimension actions and nineteen accepted canvas
  glyphs. Shared concepts reuse exactly the same fragment; specialized persistent relations keep
  distinct geometric symbols. The contextual button now says **Perp / normal**. Review the whole
  left palette, then compare the contextual and crowded M63 leaves.
- `M63-F007` — implemented; human review pending. Line relations used the nominal middle display
  sample, but a two-sample line made that integer index its first endpoint. Horizontal, vertical,
  parallel, perpendicular, collinear and equal-length occurrences now use the geometric midpoint
  of each related line. The direct parallel-relation regression requires both symbols to remain
  inside their respective lines. Recheck the line relations in the contextual and crowded leaves.
- `M63-F008` — implemented; human review pending. Line-line perpendicularity is an angular
  relationship, but it still appeared as two detached compact symbols after `M63-F007`. A
  perpendicular constraint now exposes one selectable square corner at the finite visible
  supporting-line intersection. Endpoint-adjacent spans place the square between their interiors,
  dense fan-out reserves its corner, and an off-screen intersection falls back to the compact
  midpoint symbols rather than inventing false geometry. Curve-contact Normal remains a distinct
  contact-local symbol. Recheck every corner in `canvas-crowded-annotations`, including hover,
  click, zoom and pan.
- `M63-F009` — implemented; human review pending. The remaining geometry palette still used
  letters and punctuation, sketch-tree rows used the same generic diamond for every object, and
  canvas problem markers drew an exclamation as SVG text. Fifteen distinct geometry vectors now
  cover Select through NURBS, five tree vectors identify point/curve/constraint/dimension/external
  categories, and the alert mark is path-based. Constraint/dimension vectors also use true icon
  hosts instead of keyboard elements. Review the complete left palette at normal size, then open a
  populated scenario to compare tree categories and use an error scenario to inspect targeted and
  global problem marks. Enter/Esc hints and camera controls intentionally remain textual.

## Approval

Pending supervising-human UAT.
