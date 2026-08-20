<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 focused UAT — Core architecture consolidation

Status: **accepted and closed 2026-08-20; exact GitHub Pages publication passes**. The supervising
caller reviewed the qualified consolidation/finding summary, explicitly approved M81 and requested
closure. This scorecard remains bound to the exact clean-gate release snapshot used for acceptance.

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
- Historical accepted Tailscale endpoint: `http://100.94.63.83:8080/`
  (`geosolve-m81-uat.service`, retired PID `2850776`).
- Clean gate: `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, exit 0 at
  2026-08-20 11:58:14 AEST; log SHA-256
  `43abb1e262293d607e6e37d636b90979d9be7c0807020c0d7bbc49800716797e`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m81-temp-verify.baolJt/results.tsv` and retained
  `/tmp/geosolve-m81-final-verify.Z4aCP5/results.tsv`, each SHA-256
  `7e981a47c3d02957c55e81eddb747e749e21f32d42464cdff4f6b1065e94a855`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. Both the temporary `:18080` and accepted `:8080` services are now
retired. This evidence-only documentation is a descendant of the nominated source and does not
alter its qualified product behavior.

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

The clean gate, frozen Tailscale verification and explicit supervising-human decision pass.

## Final public authority

Documentation-only approval descendant `b582b82a740d191bd754af2946746c548bf65b40`, tree
`e06710ef30f34a29a25bf27180d68722ff63fed5`, passes Pages run `32328472125`, assembly job
`96304406437`, deploy job `96305307291` and artifact `9392295853`. Its 7,925,760-byte artifact tar
has SHA-256 `8d390be7dc1b24a473ba7d616e02b0b8a1ba02d1fec55eff6ad24f1f5fddd70a`; the extracted seven-file
aggregate is `c461835ac327655fd16e9355e0b42c1971e74ed9233fcf500908b8051614de72`.

Root plus every hosted path at `https://arduano.github.io/geometric-constraint-solver/` exact-match
the artifact with HTTP 200, correct media/length, no redirect or content encoding, and root equal
to `index.html`. Pages is final public-byte authority, the Tailscale listener is retired and M81 is
closed.
