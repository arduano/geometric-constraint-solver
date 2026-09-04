<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 UAT: cohesive code-driven authoring

Status: **Complete and publicly closed on 2026-09-04 after explicit supervising-user approval.**

The supervising user's explicit approval of every part of this UAT accepts M91-U1 through M91-U14
as Pass at milestone level. This is blanket/composite approval of the clean-qualified immutable M91
candidate, not a claim that a separate row-by-row replay was logged. M91-U1 through M91-U10 retain
their origin in the transferred M90-U1 through M90-U10 assertions; this disposition does not
retroactively record those historical M90 rows as executed or passed in M90.

## Candidate

The nominated implementation is commit `6d0155151133ba2540fd1dc4b2b071f141b86064`, tree
`972ad507c2cdfad2c9cd664e49feaf79ae381c81`. `M91-F001` was resolved by `d2170c4` and the
packaging-verifier finding `M91-F002` by `6d01551`.

- clean release command:
  `env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run 'TMPDIR=/home/arduano/t ./scripts/release-gate.sh'`;
  result: exit `0`;
- release log: `/home/arduano/m91-gate.t8TTq0Bj/release-gate.log`, `706,478` bytes,
  SHA-256 `cc4f4580a0637cfddad5d96d7510d4a5f4dc707d99010b300f1a43451a7cc8cd`;
- reviewed golden: all 271 rows pass and `--survey`, `--check`, `--require-clean` pass; fixture
  SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`;
- dual-backend parity: every applicable row passes. The four reviewed non-pass exclusions remain
  `constraint.external-line-collinear.*`, `constraint.external-point-coincident.*`,
  `dimension.profile-offset.*` and `spline.noncanonical-knot-topology.*`; computed Fillet is not
  excluded;
- release WASM: `/tmp/geosolve-m91-uat.17Q5LnSg/assets/geosolve_demo_web_bg-tvc8MGYX.wasm`,
  `18,368,160` bytes, SHA-256
  `6832d1b6fd984076a47440ccac82ece0dfd205a9e93346dfb3cbd6783240e961`;
- distribution: 10 files and `27,158,025` bytes; `validate:dist` and the stricter frozen inventory
  pass;
- immutable snapshot/manifest: `/tmp/geosolve-m91-uat.17Q5LnSg`,
  `/tmp/geosolve-m91-uat.17Q5LnSg.sha256`, manifest SHA-256
  `b1e95b608b465a545791e55cc762052704f2d7139b8e4c3a9f8a68b0411a009b`;
- identical staging/live HTTP ledger SHA-256:
  `35531210b63479565e4350b44593ebe62d228e756e67378f829c99399e86bab4`;
  system-Chrome qualification passes 20/20 on both endpoints;
- Tailscale service/PID/invocation/URL: `geosolve-m91-uat-18091.service`, `2142854`,
  `bf93a3f5dab84809a24fdc2db6f23f4f`, `http://100.94.63.83:18091/`.

At nomination, the protected M90 service and served bytes remained identical and were not restarted.
Automated nomination evidence accepted no scorecard row. After the human acceptance recorded above,
GitHub Actions run `33878060784` (build `101039695274`, deploy `101042386558`) published artifact
`9938976843` as deployment `6265455733` from descendant
`177227941af97f24307fe4229797bbd84857e458`. Downloaded archive SHA-256
`7dd107d94b22c1f364fbcd824980170cf9ce33f80b6cbefd6118967101b3783d` has exact ten-file aggregate
`a8133280c286771ece4a2069880f417ea05f72980fbfa034cb774cb7a2156bad`. Eleven public routes
byte-match with correct HTTP metadata and public Chromium passes 20/20. Both accepted M90/M91
listeners were stopped only after verification. M91 is closed.

## Transferred M90 scorecard

1. **M91-U1 (transferred M90-U1) — Pass.** Start an empty coded sketch and draw one Center-Radius Circle with two
   clicks. Confirm one compact declaration, the exact needed SDK helper import, immediate continued
   interaction and exact Undo. In Compass Rose draw a three-vertex Polyline with an inferred
   constraint, then exercise repeated point drags, reload, Undo/Redo and the M90-F005 supplied
   workspace without compile-blocking, quota errors or release snapback.
2. **M91-U2 (transferred M90-U2) — Pass.** Draw Quadratic and Cubic Beziers. Confirm readable named control arguments
   and source navigation.
3. **M91-U3 (transferred M90-U3) — Pass.** Draw two circles and a Segment snapped to both circumferences. Confirm one
   source transaction owns the Segment, two Point-on-Curve constraints and inferred relation; then
   author representative point, curve, datum, contact and curvature constraints and exercise
   reload/Undo/Redo.
4. **M91-U4 (transferred M90-U4) — Pass.** Author and edit a dimension in the Inspector; confirm the exact source
   value changes and selection stays on the same declaration.
5. **M91-U5 (transferred M90-U5) — Pass.** Create a Fillet, edit radius, suppress, restore, delete and Undo; confirm
   one direct `computed.filletSet` declaration retains complete explicit branch state.
6. **M91-U6 (transferred M90-U6) — Pass.** Reorder declarations and confirm source order changes; a dependency-invalid
   move must retain exact accepted scene and history.
7. **M91-U7 (transferred M90-U7) — Pass.** Edit a shared typed-panel radius and confirm every consumer changes with no
   edit-lens metadata.
8. **M91-U8 (transferred M90-U8) — Pass.** Open every bundled sample and confirm finite accepted geometry, usable
   selection and no upgrade prompt or unsolicited source rewrite.
9. **M91-U9 (transferred M90-U9) — Pass.** Exercise browser-free inspect/render and pinned-Deno prepare/resolve edit;
   confirm output project and image reflect exactly one value change.
10. **M91-U10 (transferred M90-U10) — Pass.** Enter unsupported source and an invalid numeric edit; confirm positioned
    diagnostics and exact retention of prior canvas, source and history authority.

The detailed original wording remains frozen in `docs/M90_UAT.md`; this compact copy does not narrow
any transferred assertion.

## M91 scorecard

11. **M91-U11 — Pass.** Open a code sample with a Point-on-Curve contact initially beyond `0.5`.
    Change only its authored range to `{ lower: 0, upper: 0.5 }`. Confirm the edit publishes at the
    exact upper bound, reports an active upper bound, preserves unrelated geometry and remains
    draggable within the new range. Remove the range and confirm intrinsic curve topology remains.
12. **M91-U12 — Pass.** In `sketch.ts`, create a syntax error, an unknown builder/property and a
    wrong argument type. Confirm precise underlines/messages, completion, hover and signature help;
    correct them and confirm feedback clears without ever changing the accepted canvas until Apply.
13. **M91-U13 — Pass.** Open all 37 catalog entries and confirm each has editable typed source,
    no native/code category split, finite accepted geometry and responsive selection. Exercise the
    intended motion in Scotch Yoke, Scissor Jack and Five-stage Scissor Tower and confirm release
    retains the terminal pose with useful declared mobility.
14. **M91-U14 — Pass.** Toggle individual and grouped Explorer eyes, observe mixed/inherited
    states, isolate a group and restore it. Confirm hidden objects cannot paint, pick or expose
    controls/annotations; source, solver state, history and suppression do not change. Toggle the
    pinned Construction filter, reload, and confirm presentation choices restore and recompose.

Automated dual-backend oracle parity remains nomination evidence rather than the source of these
human dispositions. M91-U1 through M91-U14 are Pass by the explicit composite approval above; no
separately logged row-by-row replay is claimed.
