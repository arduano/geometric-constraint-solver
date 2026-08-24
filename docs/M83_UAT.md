<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Status: **post-F007 clean-qualified immutable replacement candidate nominated; focused human UAT
pending**. Automation owns exact identities, equations, residuals, persistence and deterministic
reconstruction; human review owns clarity and interaction feel.

## Candidate authority

Initial nomination commit `232b83a` and post-F005 source `a621cdd`/tree `f6d77b4` are withdrawn by
M83-F001 through M83-F007 and remain historical evidence only.

- Product source: `fafea4ebeddc295ca898258ac604858bcdd4db5f`.
- Product tree: `ff75c36aacdabe601f34bb59baaba01f6c91687a`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-f007-uat.52r7H7` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `bc04955f52ac14f3eba96637b23210772ab339e1a2f3ac60be59558bcb4c5973`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`
  (`geosolve-m83-uat.service`, PID `4006665`).
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

GitHub Pages deliberately remains on accepted M81 bytes. M83-U1 through M83-U9 are pending and no
automated result below is presented as human evidence.

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create several geometry variants, constraints, a computed Fillet and a native Profile Offset while watching Outline. | Each user action appears as one understandable declaration or property change; no low-level generated clutter is presented as separate authored history. | pending |
| M83-U2 | Select declarations in Outline and edit supported fields in the Inspector. Move declarations and cells by buttons and by upper/lower-half drag/drop, including adjacent/end moves. | Typed edits update the accepted sketch; before/after organization reorder changes only presentation and never geometry, IDs, constraints or DOF; Undo restores exact order. | pending |
| M83-U3 | Open Structured Source, inspect stable input bindings, rebind through code/RPC, then reorder and attempt an edit from a stale token before editing a current numeric/branch token. | Source and Inspector show the same updated stable binding, Inspector inputs are read-only, stale tokens reject without retargeting, and current typed edits match canvas behavior while invalid intent retains the prior accepted canvas. | pending |
| M83-U4 | Drag free points and curve controls repeatedly in large samples, including reversal, one accepted preview followed by a rejected sample, delayed capture loss, and six successive drags of a rectangle corner shared with a diagonal. | Preview remains smooth; release commits the newest visible accepted preview exactly once, derived rectangle/polyline branch round-off never causes snap-back, explicit Segment/Midpoint Line branches remain stable, delayed/duplicate terminals are inert, and Undo restores the complete pre-drag state. Driving/fixed properties are never silently rewritten. | pending |
| M83-U5 | Change Fillet radius and Offset distance through their dedicated gestures, then delete their declarations from canvas/tree and Outline/source selection. | Property gestures remain intuitive; every surface names one visible declaration owner, deletion removes the complete owned feature/association in one action and one Undo restores it. Private Offset helpers never become invisible mutation targets. | pending |
| M83-U6 | Undo/Redo a mixture of canvas, Inspector, source and organization changes, then inspect History. | One coherent history is observed; History is read-only and no action is duplicated or skipped. | pending |
| M83-U7 | Save/reload ordinary and retained-invalid v8 workspaces, then load representative v1-v6 workspaces/samples and continue editing. | v8 returns exactly; retained-invalid migrated/bootstrap intent preserves the prior accepted canvas, current failure and Undo; older flat scenes restore honestly without invented recipe history or blank geometry. | pending |
| M83-U8 | Exercise an AI/RPC or packaged TypeScript patch example and then continue editing its output in the GUI. | Code-authored declarations use the same stable references and typed Inspector/canvas behavior; no separate JS solver or opaque uneditable result appears. | pending |
| M83-U9 | Populate a sketch and its History, enter an authoring tool, then press New. | New is enabled; geometry and design history clear, Select becomes active, transient authoring/problems/camera state reset, and reload returns the same empty workspace v8. | pending |

Any blank/withheld accepted scene, semantic change from display reordering, duplicate history,
silent driver rewrite, stale pointer-up publication, invalid reconstruction success, or
uneditable code-authored output withdraws the candidate and opens an owning-layer regression.

## Targeted replacement rechecks

| ID | Human recheck | Status |
| --- | --- | --- |
| M83-F001 | U4 deterministic last-accepted release and exact-once capture lifecycle | pending |
| M83-F002 | U2 adjacent/end before/after Outline and cell movement | pending |
| M83-F003 | U7 retained-invalid migrated/bootstrap reload and Undo | pending |
| M83-F004 | U3 stale Structured Source token cannot retarget after reorder | pending |
| M83-F005 | U3 stable input binding visibility and read-only Inspector presentation | pending |
| M83-F006 | U4 repeated rectangle/shared-diagonal drags commit without snap-back while explicit branches and Undo remain exact | pending |
| M83-F007 | U9 enabled New clears the projectional workspace and persists the empty Select state | pending |

Moved annotation positions are intentionally presentation-only cache state. Workspace reload may
retain compatible positions, but source/Outline/History, solver materialization and Undo/Redo must
not treat those positions as design declarations; dropping or corrupting the cache must simply
recompute deterministic placement.
