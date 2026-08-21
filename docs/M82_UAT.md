<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 focused UAT — certified computed all-family Curve Offset

Status: **replacement mechanically qualified and frozen; human UAT pending and executable**.
M82-F006 withdrew the post-F005 candidate after the supplied periodic-NURBS payload and fresh
Bezier previews exposed blank-scene composition. M82-F007 adds computed Offset inverse-edit proxy
handles that edit the constrained owning source. One unchanged replacement now passes the expanded
all-family golden matrix, complete clean gate, no-rebuild freeze and exact Tailscale verification.
M82 remains active until this scorecard, explicit approval, Pages publication and closure pass.

## Nominated replacement authority

- Product source: `06e3cce249834959808149441661dd8fdaf47373`.
- Product tree: `9c754edb0bf28d15252c849123d7a7923f26f10c`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m82-uat.G4pmMH` (directory `0555`; seven regular
  non-symlink files at `0444`).
- Ordered file-manifest aggregate:
  `70885cec204ffc586fe84f1faf94b89e3998c23fca0035bb8909f1e57957b535`.
- Current Tailscale endpoint: `http://100.94.63.83:8080/` under
  `geosolve-m82-uat.service`, PID `3024723`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, direct
  systemd process exit 0 from 2026-08-21 11:30:53 to 11:54:30 AEST; 275,822-byte, 3,586-line log
  `/tmp/geosolve-m82-clean-gate.06e3cce.attempt3.nix.log`, SHA-256
  `ca390a3a137b10b07a6534a72b182bb35a67a314bc05441084376e5cb93f861d`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m82-temp-verify.pLz16X/results.tsv` and retained
  `/tmp/geosolve-m82-final-verify.HK7BJ3/results.tsv`, each SHA-256
  `65018fd399cb3e2f13f7d8b6694b349a96c1e1c278861449edf5552095fa71bf`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. The temporary listener is retired; the retained listener serves
only the immutable replacement and remains live through UAT. This evidence-only documentation is
a descendant of the nominated source and does not replace or rebuild it.

## Withdrawn post-F005 authority

- Product source: `d52104595ee11f9e460e98ea5e26200bb34a5d94`.
- Product tree: `0a3bcb066a6a2d5d5d2d99591441035be23d20fe`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m82-uat.iOg5Do` (directory `0555`; seven regular
  non-symlink files at `0444`).
- Ordered file-manifest aggregate:
  `3e6d15dc04fd190c904559dc540936c4f31921d0e8bb257266dff40a2ed8327e`.
- Historical Tailscale service run: `geosolve-m82-uat.service`, retired PID `1272147`, formerly at
  `http://100.94.63.83:8080/`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 from
  2026-08-20 23:17:08 to 23:28:33 AEST; 269,138-byte, 3,530-line log at
  `/tmp/geosolve-m82-clean-gate.d521045.nix.log`, SHA-256
  `b66c277a00861854865440911e769aa5f9e94dbd55114e723b43b3bf46743472`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m82-temp-verify.76WBWb/results.tsv` and retained
  `/tmp/geosolve-m82-final-verify.wU6t8i/results.tsv`, each SHA-256
  `35fa0bb1109d96e97f7107f81ac76292ccd6fbb5cbc10da418d836bb05e6a3dd`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. Both listener runs are retired. M82-F006/F007 withdrew this
snapshot before human UAT; it is historical evidence only and cannot qualify the replacement.

## Withdrawn pre-F005 authority

- Product source: `7fd31c0137f6979f945e5ab4d320e7adb552c03d`.
- Product tree: `c6b6c89cecde30b2b3a7cf057ec61317a38a5634`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m82-uat.I58j21` (directory `0555`; seven regular
  non-symlink files at `0444`).
- Ordered file-manifest aggregate:
  `cb07c77de43544be251f97321bba8f978a018078a7b332d3752b39b55dff1a8e`.
- Historical Tailscale service run: `geosolve-m82-uat.service`, retired PID `1188633`, formerly at
  `http://100.94.63.83:8080/`.
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 at
  2026-08-20 22:57:08 AEST; log SHA-256
  `7f4ef08a66851c1a117bf091af6bfb49a83abf33face7807e451e9b75ae064cf`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m82-temp-verify.QnWhyi/results.tsv` and retained
  `/tmp/geosolve-m82-final-verify.Jlu7xZ/results.tsv`, each SHA-256
  `1355605506f1a656e8ec883e57bc989727f8c24838172a82d46540d3b94748a6`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. Both listeners are retired. This snapshot was withdrawn before
human UAT because its Offset help still claimed native-only curve support; it is not a candidate.

## Prepared scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M82-U1 | Offset a native-only line/arc face and open chain, then Undo/Redo. | The familiar M80 native association, annotation, drag and editable target behavior remain unchanged. | pending |
| M82-U2 | Offset Ellipse, EllipticalArc, RationalQuadratic, Parabola, Hyperbola, Quadratic/Cubic Bezier, open/clamped and periodic B-spline, and open/clamped and periodic NURBS specimens on both sides. | One consistent Offset tool previews finite smooth output; every generated edge selects one stable feature and reveals the eligible source-owned inverse proxies without becoming a direct constraint operand. | pending |
| M82-U3 | Offset a mixed analytic/general open chain, including intrinsically adjacent spline spans; reverse traversal and Flip. | Collection order, Start/End, side and junction presentation are predictable; Flip changes only the intended side and the complete preview remains continuous. | pending |
| M82-U4 | Offset a closed general-curve face and a face with at least one hole inward/outward. | Outer/hole material semantics are visually correct, all contours appear or none do, and feature/tree/Problems attribution is clear. | pending |
| M82-U5 | Increase distance toward a tight-curvature cusp, self-intersection or contour-touch limit, then reduce it. | The last complete valid preview remains visible; Apply is unavailable with a useful local reason; reducing distance recovers without stale errors or partial fragments. Any computed-presentation failure still leaves the complete native accepted scene visible. | pending |
| M82-U6 | Select computed Offsets from several families and drag their point/rational-middle proxy grips in X/Y; include reverse Line/CircularArc/Circle portions in mixed/general operands, constrained source controls, then Undo/Redo. | The proxy is painted on the traversal-correct parallel; connector-only miter edges invent no proxy. The ordinary source control moves through its constraints, one history/replay edit commits, output regenerates Current with fresh generated IDs, and unrelated/native accepted geometry never blanks or jumps. | pending |
| M82-U7 | Compare a computed Fillet, a native-published Fillet and an Offset over nearby source geometry. | Active computed Fillet output is unavailable as an Offset operand with clear feedback; native-published line–arc–line topology offsets normally; Fillet corner selection remains distinct from Offset feature selection. | pending |
| M82-U8 | Save/reload and copy/load a reproduction containing computed Offset; repeat at `1440x900` and about `1024x720`. | Feature intent, side, distance, source provenance and output appearance regenerate correctly; proxy grips rebuild from source authority, no generated ID/certificate/proxy leaks into persistence and the panel remains polished at both sizes. | pending |
| M82-U9 | Load the exact M82-F006 periodic-NURBS fixture, create its representative Offset, then repeat with fresh quadratic and cubic Beziers. | Preview, Apply, feature selection and a proxy edit retain the complete finite native scene throughout; no family makes all geometry disappear. | pending |

## Acceptance rule

Any blank/partial scene, stale output, silent distance reduction, topology repair, unexpected route
away from M80, proxy edit that bypasses source constraints, computed-on-computed consumption,
selection ambiguity, history mismatch or failed recovery opens a focused M82 finding and withdraws
the nominated replacement. Human approval is required before M82 closure and public Pages
publication.
