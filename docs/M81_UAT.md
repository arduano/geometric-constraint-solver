<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 focused UAT — Core architecture consolidation

Status: **accepted 2026-08-20; GitHub Pages publication and final closeout pending**. The
supervising caller reviewed the qualified consolidation/finding summary, explicitly approved M81
and requested closure. This scorecard remains bound to the exact clean-gate release snapshot
served at the shared Tailscale endpoint.

M81 intentionally adds no visible feature. Human UAT is a compact behavior-preservation smoke test;
mathematical, persistence, authority and ordering compatibility are owned by the automated and
frozen-artifact gates in `docs/M81_IMPLEMENTATION.md`.

## Candidate authority

- Product source: `e4eca327fc69c92f95b1722142289302ba4f67bc`.
- Product tree: `f3ed1bf50b793daae328adf04c0924655dc13d74`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m81-uat.QqItRd` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `df24deb988a31a373b3f973432081078c15e157382134f62c99aaabe96b8e49e`.
- Tailscale endpoint: `http://100.94.63.83:8080/` (`geosolve-m81-uat.service`, nominated PID
  `2850776`).
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 at
  2026-08-20 11:58:14 AEST; log SHA-256
  `43abb1e262293d607e6e37d636b90979d9be7c0807020c0d7bbc49800716797e`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m81-temp-verify.baolJt/results.tsv` and retained
  `/tmp/geosolve-m81-final-verify.Z4aCP5/results.tsv`, each SHA-256
  `7e981a47c3d02957c55e81eddb747e749e21f32d42464cdff4f6b1065e94a855`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. The temporary `:18080` service is retired; `:8080` remains live.
This evidence-only documentation is a descendant of the nominated source and does not alter the
served product bytes.

## Focused scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M81-U1 | Open several editable samples, then create a small line/arc sketch and Fit/zoom/pan it. | Accepted geometry and ordinary canvas interaction look unchanged; no blank or withheld scene appears. | pass |
| M81-U2 | Add representative point, direction and dimensional constraints; drag a remaining free point; Undo and Redo. | Constraint placement, dragging, accepted geometry and history behave exactly as before M81. | pass |
| M81-U3 | Create and modify a computed Fillet, including one invalid/exhausted-style correction or cancel path if convenient. | Successful previews/publications still work; rejection or cancel retains the prior scene and a later valid action recovers normally. | pass |
| M81-U4 | Create a native-profile Fillet and use its line–arc–line result in a Profile Offset chain or face; drag the Offset distance. | Native publication, Offset preview/drag/Apply and one-step Undo/Redo remain unchanged. | pass |
| M81-U5 | Save, reload and continue editing the workspace, including any annotation previously moved. | Geometry, branches, feature intent, Offset state and disposable annotation placement round-trip as before. | pass |

The approval is a scoped milestone decision over the complete qualified candidate and the five
behavior-preservation rows above. It opens no new finding and does not claim a separate exhaustive
manual replay of every historical M80 scenario; exact mathematical, persistence and authority
coverage remains owned by the recorded automated and immutable-artifact evidence.

## Acceptance rule

Any visible regression, stale problem, history mismatch, failed recovery, lost branch/feature state
or persistence change opens an exact owning-layer finding under the repository defect workflow and
withdraws the candidate. Cosmetic or feature requests unrelated to behavior preservation are
deferred rather than folded into M81.

The clean gate, frozen Tailscale verification and explicit supervising-human decision now pass.
GitHub Pages publication and exact hosted-byte verification remain the final closeout step; the
Tailscale listener stays live until that public authority is established.
