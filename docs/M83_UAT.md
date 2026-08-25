<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Status: **accepted by the supervising user on 2026-08-25 for milestone-level closure; F010 clean
qualification and immutable Tailscale replacement nomination are complete; final GitHub Pages
publication remains pending**. Automation owns exact identities, equations, residuals,
persistence and deterministic reconstruction; human review owns clarity and interaction feel.

## Candidate authority

Initial nomination commit `232b83a` and post-F005 source `a621cdd`/tree `f6d77b4` are withdrawn by
M83-F001 through M83-F007 and remain historical evidence only. The post-F007 candidate below is
also superseded by the architecture-hardening pass and is historical evidence only.

- Superseded post-F007 product source: `fafea4ebeddc295ca898258ac604858bcdd4db5f`.
- Superseded post-F007 product tree: `ff75c36aacdabe601f34bb59baaba01f6c91687a`.
- Historical frozen no-rebuild snapshot: `/tmp/geosolve-m83-f007-uat.52r7H7` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `bc04955f52ac14f3eba96637b23210772ab339e1a2f3ac60be59558bcb4c5973`.
- Retired superseded Tailscale service: `geosolve-m83-uat.service`, PID `4006665`; its immutable
  snapshot remains preserved.
- Clean gate:
  `env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`,
  exit 0 at 2026-08-24 20:30:47 AEST after 967 seconds; 373,070-byte, 5,530-line log
  `/tmp/geosolve-m83-f007-gate.aGkBUu/release-gate.log`, SHA-256
  `20807697a2df8941d59c9033c1604063940284e7b89f10ad19b1aeddcf25a7f4`.
- Existing frozen browser verification: 3/3 deterministic drag/capture/Undo, before/after Outline
  drop and curve-property/Undo checks passed on the post-F007 artifact.
- Focused F006/F007 frozen-browser verification: 2/2 passed on both temporary and retained
  endpoints—six repeated rectangle/shared-diagonal drags commit with exact Undo, and New clears
  durable/transient state, selects Select and persists the empty workspace. Final log SHA-256:
  `a97160b3b4e57f0c822454f318210798dadb41c0e804e216acba1dbb5f64e9b7`.
  This thin artifact/adaptor parity supplements rather than replaces the Rust owner regressions;
  no repository browser E2E harness is restored.
- Temporary exact served-byte verification:
  `/tmp/geosolve-m83-f007-temp-verify.3CjjqM/results.tsv`, SHA-256
  `9573901313adf09b23e93e27857639fa7bf96eb969121a6f22277cadb275b9ec`.
- Retained exact served-byte verification:
  `/tmp/geosolve-m83-f007-final-verify.nHhejs/results.tsv`, with the same SHA-256.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body and root equality with `index.html`.
Temporary byte/browser listeners were retired only after their passes; the retained service then
passed independent byte and focused browser verification. Complete freeze, browser and serving
evidence is in `/tmp/geosolve-m83-f007-freeze-evidence.y5GbCJ`.

Superseded post-hardening mechanical authority:

- Product source: `1e70f3f4dc6778881ce180b2922235a6cc103cf7`.
- Product tree: `77251dbe393cd57b9d036e9611f5a8aaa192f5ee`.
- Clean gate: exit 0 on 2026-08-25 at 05:14:03 AEST after 1,102 seconds; 386,013-byte,
  5,667-line log `/tmp/geosolve-m83-post-hardening-gate.fCN9ie7B/release-gate.log`, SHA-256
  `53e9da5f91978d90899c2a54a6a01c9b16f0616c9b76ea93dde8d57b78416136`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-post-hardening-uat.R821Vpjj` (directory `0555`;
  seven regular non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `63730632c228e39f5243dde0d2f906eb493e5e61bd916faece6615f61adc9aef`.
- Frozen browser verification: the existing 3/3 suite and focused F006/F007 2/2 suite pass locally;
  the focused suite also passes on temporary and retained Tailscale endpoints.
- Temporary exact served-byte verification:
  `/tmp/geosolve-m83-post-hardening-temp-verify.iED2TLx3/results.tsv`, SHA-256
  `5fe8485fc0c0c16ea00b745c69add9303e2665eb830bb88a890b53ddc4a77259`; the temporary service is
  retired after its byte and browser passes.
- Retained exact served-byte verification:
  `/tmp/geosolve-m83-post-hardening-final-verify.456o9Uey/results.tsv`, with the same SHA-256.
- Historical retained endpoint: `http://100.94.63.83:8080/` (`geosolve-m83-uat.service`, retired
  PID `2404961`); that URL now serves the current authority below.
- Complete freeze, browser, service and byte evidence:
  `/tmp/geosolve-m83-post-hardening-freeze-evidence.BlN1Fo2q`.

Both superseded byte-verification passes cover `/` plus all seven assets: HTTP 200, zero redirects,
no `Location` or `Content-Encoding`, exact media type/length/body and root equality with
`index.html`. F008/F009 retired PID `2404961` only after their replacement passed; the immutable
snapshot and complete evidence remain historical.

Superseded F008/F009 mechanical UAT authority:

- Product source: `b0de5af55a8c9fe3550137cda91dae63c87666b1`.
- Product tree: `ff0b29dee074bc67a136c23feb5ee56c99deeba1`.
- Clean gate: exit 0 on 2026-08-25 at 09:33:51 AEST after 975 seconds; 381,514-byte,
  5,645-line log `/tmp/geosolve-m83-f008-f009-release-gate.log`, SHA-256
  `fc07730eed2c158c234828700ea9fbed1b6c7f0e396c5464baab20063e967ed1`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-f008-f009-uat.zLfB22EK` (directory `0555`; seven
  regular non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `f2092e54b1b014618dcdded21e3bc0907a280fc15aa93b0c18913cf87d9b30d6`.
- Frozen browser verification: existing 3/3, focused F006/F007 2/2 and focused F008/F009 2/2
  suites pass locally (7/7 total); both focused suites pass on temporary and retained Tailscale
  endpoints (4/4 each).
- Temporary exact served-byte verification:
  `/tmp/geosolve-m83-f008-f009-temp-verify.uNTvzMdz/results.tsv`, SHA-256
  `b5bef9cc6274258c217f5edf44c7a6ed06b7299c524f5f0ed3ea4aa64d8866b4`; temporary PID `3349639`
  is retired after its byte and browser passes.
- Retained exact served-byte verification:
  `/tmp/geosolve-m83-f008-f009-final-verify.na1TWg0E/results.tsv`, with the same SHA-256.
- Historical retained service: `geosolve-m83-uat.service`, PID `3376452`; F010 retired it only
  after replacement verification, and its immutable snapshot remains preserved.
- Complete freeze, browser, service and byte evidence:
  `/tmp/geosolve-m83-f008-f009-freeze-evidence.GZ1Vp2es`.

Both historical byte-verification passes cover `/` plus all seven assets: HTTP 200, zero redirects,
exact media type/length/body and root equality with `index.html`. Temporary listeners were retired
only after passing; the retained service then passed independent byte and focused-browser
verification.

Current F010 mechanical UAT authority:

- Product source: `ee18dbda89b6973ac54baea3ac0e0dbbd126ca59`.
- Product tree: `889f730e033ce5c728fddbac345263d8c26b8b93`.
- Clean gate: exit 0 on 2026-08-25 at 13:52:11 AEST after 1,105 seconds; 385,323-byte,
  5,683-line log, SHA-256
  `71495557ca5638000cfec265d9b97c0b0e71c72d3cbdfbbd09dbea8518484f9a`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-f010-uat.Qmrz2R36` (directory `0555`; seven
  regular non-symlink files `0444`). Source, pre-freeze and frozen manifests are byte-identical.
- Ordered file-manifest aggregate:
  `e01d642438ae8337e9abe1ddeadb7b176375ae40f8b411785edae717e30b5d54`.
- Frozen browser verification: all 9/9 checks pass locally, on the temporary Tailscale listener
  and on the retained Tailscale listener. The set carries F001-F009 checks and adds F010
  Segment/NURBS Inspector/source parity plus multi-corner Fillet arrays and closed enums.
- Temporary exact served-byte verification:
  `/tmp/geosolve-m83-f010-temp-verify.fa7xynxe/results.tsv`, SHA-256
  `9914a99483df9dee2059a1e0eabf173c8055af6a9173c2a7e5288e31ffeef10b`; temporary PID `224467`
  is retired after its byte and browser passes.
- Retained exact served-byte verification:
  `/tmp/geosolve-m83-f010-final-verify.jKSWm8xi/results.tsv`, with the same SHA-256.
- Current retained endpoint: `http://100.94.63.83:8080/` (`geosolve-m83-uat.service`, PID
  `276377`).
- Complete freeze, browser, service and byte evidence:
  `/tmp/geosolve-m83-f010-freeze-evidence.nIFsx9ww`.

Both current byte-verification passes cover `/` plus all seven assets: HTTP 200, zero redirects,
exact media type/length/body and root equality with `index.html`. The temporary listener was
retired only after passing; historical retained PID `3376452` was then retired and preserved, and
the current retained service passed independent byte and 9/9 browser verification.

| Mechanical nomination | Status |
| --- | --- |
| M83-F010 clean qualification, no-rebuild freeze and immutable Tailscale replacement | complete |

GitHub Pages deliberately remains on accepted M81 bytes until the standard publication closeout.
No automated result below is presented as human evidence.

## Supervising-user acceptance

On 2026-08-25 the supervising user explicitly approved the implementation plan that closes M83
from the existing F010 qualification, frozen-artifact and review evidence, and instructed that
plan to be implemented. That milestone-level decision accepts M83-U1 through M83-U10 and the
F001-F010 replacement disposition for closure. It does **not** claim a separate row-by-row
hands-on replay or invent observations that were not logged. The statuses below therefore say
`accepted by milestone-level approval`, not `manually passed`. Exact GitHub Pages publication,
hosted-byte verification and retained-service retirement remain the only standard closeout work.

The current F010 candidate mechanically preserves the automation-only architecture contract:
strict SHA-256 v2/legacy-v1 migration, independently validated accepted/current/history authority,
descriptor/body chronology, exact ownership, prepared Fillet/Offset publication, compact explicit
Snapshot reads, typed bounded receipts/responses, the 64 MiB workspace admission boundary and 4 MiB
disposable annotation-cache bound. These are mechanical trust/atomicity/resource properties, not
extra hands-on scorecard rows.

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create several geometry variants, constraints, a computed Fillet and a native Profile Offset while watching Outline. | Each user action appears as one understandable declaration or property change; no low-level generated clutter is presented as separate authored history. | accepted by milestone-level approval |
| M83-U2 | Select declarations in Outline and edit supported fields in the Inspector. Move declarations and cells by buttons and by upper/lower-half drag/drop, including adjacent/end moves. | Typed edits update the accepted sketch; before/after organization reorder changes only presentation and never geometry, IDs, constraints or DOF; Undo restores exact order. | accepted by milestone-level approval |
| M83-U3 | Open Structured Source, inspect stable input bindings, rebind through code/RPC, then reorder and attempt an edit from a stale token before editing a current numeric/branch token. | Source and Inspector show the same updated stable binding, Inspector inputs are read-only, stale tokens reject without retargeting, and current typed edits match canvas behavior while invalid intent retains the prior accepted canvas. | accepted by milestone-level approval |
| M83-U4 | Drag free points and curve controls repeatedly in large samples, including reversal, one accepted preview followed by a rejected sample, delayed capture loss, and six successive drags of a rectangle corner shared with a diagonal. | Preview remains smooth; release commits the newest visible accepted preview exactly once, derived rectangle/polyline branch round-off never causes snap-back, explicit Segment/Midpoint Line branches remain stable, delayed/duplicate terminals are inert, and Undo restores the complete pre-drag state. Driving/fixed properties are never silently rewritten. | accepted by milestone-level approval |
| M83-U5 | Change Fillet radius and Offset distance through their dedicated gestures, suppress and restore a rectangle Fillet, then delete the declarations from canvas/tree and Outline/source selection. | Property gestures remain intuitive; suppressing a Fillet removes its computed edge/affordance but restores every finite native parent without a blank canvas or stale error; Undo/Redo restores exact identity. Every surface names one visible declaration owner, deletion removes the complete owned feature/association in one action and one Undo restores it. Private Offset helpers never become invisible mutation targets. | accepted by milestone-level approval |
| M83-U6 | Undo/Redo a mixture of canvas, Inspector, source and organization changes, then inspect History. | One coherent history is observed; History is read-only and no action is duplicated or skipped. | accepted by milestone-level approval |
| M83-U7 | Save/reload ordinary and retained-invalid v8 workspaces, Copy repro, New, Load the copied repro, reject a deliberately corrupt payload, then load representative v1-v6 workspaces/samples and continue editing. | v8 and the copied complete projectional authority/history return exactly; annotation layout may recompute; corrupt repro rejects without changing the live scene. Retained-invalid migrated/bootstrap intent preserves the prior accepted canvas, current failure and Undo; older flat scenes restore honestly without invented recipe history or blank geometry. | accepted by milestone-level approval |
| M83-U8 | Exercise an AI/RPC or packaged TypeScript patch example and then continue editing its output in the GUI. | Code-authored declarations use the same stable references and typed Inspector/canvas behavior; no separate JS solver or opaque uneditable result appears. | accepted by milestone-level approval |
| M83-U9 | Populate a sketch and its History, enter an authoring tool, then press New. | New is enabled; geometry and design history clear, Select becomes active, transient authoring/problems/camera state reset, and reload returns the same empty workspace v8. | accepted by milestone-level approval |
| M83-U10 | Inspect a Segment, Polyline/NURBS and multi-corner Fillet in Structured Source and Inspector, including closed enum fields. | Fixed roles have meaningful names; repeated values are actual arrays with understandable one-based Inspector groups; no `0000`/`0001` storage selectors leak into fields, breadcrumbs or references; source and Inspector structure agree and enum selects expose the valid choices/default. | accepted by milestone-level approval |

Any blank/withheld accepted scene, semantic change from display reordering, duplicate history,
silent driver rewrite, stale pointer-up publication, invalid reconstruction success, or
uneditable code-authored output withdraws the candidate and opens an owning-layer regression.

## Targeted replacement rechecks

| ID | Human recheck | Status |
| --- | --- | --- |
| M83-F001 | U4 deterministic last-accepted release and exact-once capture lifecycle | accepted by milestone-level approval |
| M83-F002 | U2 adjacent/end before/after Outline and cell movement | accepted by milestone-level approval |
| M83-F003 | U7 retained-invalid migrated/bootstrap reload and Undo | accepted by milestone-level approval |
| M83-F004 | U3 stale Structured Source token cannot retarget after reorder | accepted by milestone-level approval |
| M83-F005 | U3 stable input binding visibility and read-only Inspector presentation | accepted by milestone-level approval |
| M83-F006 | U4 repeated rectangle/shared-diagonal drags commit without snap-back while explicit branches and Undo remain exact | accepted by milestone-level approval |
| M83-F007 | U9 enabled New clears the projectional workspace and persists the empty Select state | accepted by milestone-level approval |
| M83-F008 | U7 enabled Copy/Load repro round-trips complete projectional authority/history and corrupt input rejects atomically | accepted by milestone-level approval |
| M83-F009 | U5 suppressed Fillet reveals native parents, removes computed affordances and never leaves a hidden/stale canvas failure | accepted by milestone-level approval |
| M83-F010 | U10 named semantic fields, real arrays, grouped Inspector hierarchy and no visible padded storage selectors | accepted by milestone-level approval |

Moved annotation positions are intentionally presentation-only cache state. Workspace reload may
retain compatible positions, but source/Outline/History, solver materialization and Undo/Redo must
not treat those positions as design declarations; dropping or corrupting the cache must simply
recompute deterministic placement.
