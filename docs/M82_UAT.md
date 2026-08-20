<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 focused UAT — certified computed all-family Curve Offset

Status: **prepared, not executed**. Implementation and development qualification pass, but no
clean-gate candidate source, frozen artifact or human acceptance exists yet. Automated owner tests
certify mathematics, topology and persistence; this scorecard is for discoverability, interaction
truthfulness and visual continuity on the eventual frozen candidate.

## Candidate authority

Pending clean-gate nomination. Record exact product source/tree, no-rebuild snapshot, ordered
manifest, Tailscale endpoint and byte-verification evidence before beginning UAT.

## Prepared scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M82-U1 | Offset a native-only line/arc face and open chain, then Undo/Redo. | The familiar M80 native association, annotation, drag and editable target behavior remain unchanged. | pending |
| M82-U2 | Offset one Ellipse/EllipticalArc, one conic, one Bezier and one B-spline/NURBS specimen on both sides. | One consistent Offset tool previews smooth, visually parallel output; every generated edge selects one stable Offset feature and is not directly draggable or constrainable. | pending |
| M82-U3 | Offset a mixed analytic/general open chain, including intrinsically adjacent spline spans; reverse traversal and Flip. | Collection order, Start/End, side and junction presentation are predictable; Flip changes only the intended side and the complete preview remains continuous. | pending |
| M82-U4 | Offset a closed general-curve face and a face with at least one hole inward/outward. | Outer/hole material semantics are visually correct, all contours appear or none do, and feature/tree/Problems attribution is clear. | pending |
| M82-U5 | Increase distance toward a tight-curvature cusp, self-intersection or contour-touch limit, then reduce it. | The last complete valid preview remains visible; Apply is unavailable with a useful local reason; reducing distance recovers without stale errors, partial fragments or a blank scene. | pending |
| M82-U6 | Edit several source controls and the Offset distance/direction; suppress, unsuppress and delete the feature; Undo/Redo each path. | Output reevaluates associatively, failure never blocks source editing, identities/history stay coherent and native source geometry is never replaced by stale output. | pending |
| M82-U7 | Compare a computed Fillet, a native-published Fillet and an Offset over nearby source geometry. | Active computed Fillet output is unavailable as an Offset operand with clear feedback; native-published line–arc–line topology offsets normally; Fillet corner selection remains distinct from Offset feature selection. | pending |
| M82-U8 | Save/reload and copy/load a reproduction containing computed Offset; repeat at `1440x900` and about `1024x720`. | Feature intent, side, distance, source provenance and output appearance regenerate correctly; no generated ID/certificate leaks into visible persistence and the panel remains polished at both sizes. | pending |

## Acceptance rule

Any blank/partial scene, stale output, silent distance reduction, topology repair, unexpected route
away from M80, computed-on-computed consumption, selection ambiguity, history mismatch or failed
recovery opens a focused M82 finding and withdraws the candidate. Human approval is required
before M82 closure and public Pages publication.
