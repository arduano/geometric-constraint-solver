<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 active handover

Status: **M86 is active and unaccepted**. M86-F001 is human-approved. M86-F002 is implemented,
clean-qualified and immutably nominated; only its focused human UAT and the eventual approved
closeout/publication remain open. Do not reconstruct this state from chat history.

## Resume point

Read `AGENTS.md` and its required project documents, then `docs/M86_GOALS.md`,
`docs/M86_IMPLEMENTATION.md` and `docs/M86_UAT.md`. The F002 implementation and nomination records
are committed at `dbe94da` and `67f385e`; expect a clean worktree after this handover commit.
Accepted M85 Pages remains public-byte authority until explicit M86 approval.

## Delivered work

- **M86-F001 — managed dimension Inspector edits:** commit `9050424` routes authenticated direct
  code-owned curve-length/diameter target edits through managed TypeScript source rewriting and
  atomic rematerialization. Invalid edits retain the last accepted scene and ordinary Undo/recovery.
  The supervising user's “Looks good” assessment accepts this finding.
- **M86-F002 — Fillet source-corner hit priority:** commit `dbe94da` makes an exact persistent
  line/polyline endpoint shared by both Fillet parents win over the Fillet radius surface in Select
  hover, click and drag. Active `Coincident` topology also qualifies; coordinate overlap alone,
  unrelated points, Fillet authoring and ordinary radius/arc interaction do not. Hover and down use
  the same resolver. Nomination documentation is commit `67f385e`.

Neither finding changes solver equations, residuals, Jacobians, branch semantics, persistence
formats or public APIs. Focused native/WASM parity, the full editor suite, warnings-denied Clippy,
formatting and the unchanged 271-case golden corpus pass.

## Current immutable candidate

- Product source/tree: `dbe94daf152515169b78a310cf2286f9ea04c80b` /
  `77f86c0a198af12e10537dc4d6d7d90066ba48e8`.
- Complete clean release gate: exit `0`; log
  `/tmp/geosolve-m86-f002-gate.wSztT0Bb/release-gate.log`, SHA-256
  `34ac3e398953398495d22d480d9a11d88b03020c0235c07db61c443534fa4278`.
- Frozen seven-file snapshot: `/tmp/geosolve-m86-f002-uat.CPfe9QD8`, aggregate
  `e3f9581a05a8cbf5731b33625fa63f2b35e62f4ebcfdacb6d75a4486f80fc850`.
- Local: `http://127.0.0.1:18101/`, PID `597410`.
- Tailscale: `http://100.94.63.83:8080/`, PID `597412`.
- Both endpoints serve the frozen snapshot; their exact eight-path ledger SHA-256 is
  `e5513ab3e36262f2ccedf175006f1283d5504180c8d0be46e1e90dded999a3df`.

## Remaining work

1. Ask the supervising user to hard-refresh the Tailscale candidate and perform M86-U6: select a
   Fillet, then hover/click/drag its original native line/polyline corner. The point must win.
2. Perform M86-U7: radius dragging and Fillet arc selection must still work away from that corner;
   an unrelated overlapping point must not steal the Fillet surface.
3. If UAT fails, use `geosolve-harden-defect`, preserve this candidate, reproduce at the headless
   editor boundary and qualify a replacement. Current F002 scope does not cover point-backed
   Bezier/rational-conic endpoints or a dense stack of multiple overlapping Fillet surfaces.
4. If UAT passes, record explicit approval consistently, perform the normal clean closeout, deploy
   the approved frozen M86 candidate to GitHub Pages, exact-verify it, retire the local/Tailscale
   candidate services and close M86. Do not publish or close before that approval.
