<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 focused UAT — certified computed all-family Curve Offset

Status: **frozen candidate nominated; human UAT pending**. Implementation, the clean release gate
and immutable served-byte verification pass. Automated owner tests certify mathematics, topology
and persistence; this scorecard is for discoverability, interaction truthfulness and visual
continuity on the exact frozen candidate below.

## Candidate authority

- Product source: `7fd31c0137f6979f945e5ab4d320e7adb552c03d`.
- Product tree: `c6b6c89cecde30b2b3a7cf057ec61317a38a5634`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m82-uat.I58j21` (directory `0555`; seven regular
  non-symlink files at `0444`).
- Ordered file-manifest aggregate:
  `cb07c77de43544be251f97321bba8f978a018078a7b332d3752b39b55dff1a8e`.
- Live Tailscale endpoint: `http://100.94.63.83:8080/` (`geosolve-m82-uat.service`, nominated PID
  `1188633`).
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 at
  2026-08-20 22:57:08 AEST; log SHA-256
  `7f4ef08a66851c1a117bf091af6bfb49a83abf33face7807e451e9b75ae064cf`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m82-temp-verify.QnWhyi/results.tsv` and retained
  `/tmp/geosolve-m82-final-verify.Jlu7xZ/results.tsv`, each SHA-256
  `1355605506f1a656e8ec883e57bc989727f8c24838172a82d46540d3b94748a6`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. The temporary `:18080` listener is retired; the retained `:8080`
listener stays live until UAT disposition and any exact public publication are complete.

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
