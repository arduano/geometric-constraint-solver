# M63 UAT — canvas constraints

Status: candidate awaiting supervising-human review.

## Candidate

- Branch: `main`
- Entry point: sole desktop workbench served through the usual temporary UAT endpoint.
- Scenario group: **Scenarios → M63 Canvas constraints**
- Mechanical gate: format, warnings-denied workspace Clippy, all-feature workspace tests,
  all-feature WASM check and release Trunk build passed on 2026-07-30.

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

## Approval

Pending supervising-human UAT.
