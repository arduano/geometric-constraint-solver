<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 focused UAT — Core architecture consolidation

Status: **candidate not yet nominated**. This scorecard will bind to the exact clean-gate release
snapshot served at the shared Tailscale endpoint. Do not score another working tree or a fresh
rebuild under the same URL.

M81 intentionally adds no visible feature. Human UAT is a compact behavior-preservation smoke test;
mathematical, persistence, authority and ordering compatibility are owned by the automated and
frozen-artifact gates in `docs/M81_IMPLEMENTATION.md`.

## Candidate authority

- Product source: pending.
- Product tree: pending.
- Frozen no-rebuild snapshot: pending.
- Ordered file-manifest aggregate: pending.
- Tailscale endpoint: pending.
- Exact served-byte verification: pending.

## Focused scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M81-U1 | Open several editable samples, then create a small line/arc sketch and Fit/zoom/pan it. | Accepted geometry and ordinary canvas interaction look unchanged; no blank or withheld scene appears. | pending |
| M81-U2 | Add representative point, direction and dimensional constraints; drag a remaining free point; Undo and Redo. | Constraint placement, dragging, accepted geometry and history behave exactly as before M81. | pending |
| M81-U3 | Create and modify a computed Fillet, including one invalid/exhausted-style correction or cancel path if convenient. | Successful previews/publications still work; rejection or cancel retains the prior scene and a later valid action recovers normally. | pending |
| M81-U4 | Create a native-profile Fillet and use its line–arc–line result in a Profile Offset chain or face; drag the Offset distance. | Native publication, Offset preview/drag/Apply and one-step Undo/Redo remain unchanged. | pending |
| M81-U5 | Save, reload and continue editing the workspace, including any annotation previously moved. | Geometry, branches, feature intent, Offset state and disposable annotation placement round-trip as before. | pending |

## Acceptance rule

Any visible regression, stale problem, history mismatch, failed recovery, lost branch/feature state
or persistence change opens an exact owning-layer finding under the repository defect workflow and
withdraws the candidate. Cosmetic or feature requests unrelated to behavior preservation are
deferred rather than folded into M81.

M81 closes only after the clean gate, frozen Tailscale verification and explicit supervising-human
decision. GitHub Pages publication remains a closeout step after acceptance, not nomination
authority.
