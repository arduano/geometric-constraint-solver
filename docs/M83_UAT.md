<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Status: **clean-qualified immutable candidate nominated; focused human UAT pending**. Automation
owns exact identities, equations, residuals, persistence and deterministic reconstruction; human
review owns clarity and interaction feel.

## Candidate authority

- Product source: `1b4f4558688e1bd32be075793e11885f892a3245`.
- Product tree: `ce09e010dc74f2e97b19d52d15433eef8f2f78d3`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-uat.DFamHN` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `4bb4bf4f22caefae429b514cfffda1d92ac3704e6f3474c5022413ec0f242989`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`
  (`geosolve-m83-uat.service`, PID `2747514`).
- Clean gate:
  `env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`,
  exit 0 at 2026-08-24 14:52:37 AEST; log SHA-256
  `b6547c1bbbb99175d108c5a2a506f6146a8b01b133a3953c472c9705dd5caeae`.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m83-temp-verify.2ogZzJ/results.tsv` and retained
  `/tmp/geosolve-m83-final-verify.GG1MfU/results.tsv`, each SHA-256
  `6d57de9beadd0114afb2d1f101b0f9a542c876ea45aa9f9efda11724b351ce97`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. Temporary `:18083` verification passed before the retained
`:8080` service started, then the temporary listener was retired. The documentation-only
descendant recording this evidence neither replaces the product source/tree nor rebuilds the
candidate bytes.

GitHub Pages deliberately remains on accepted M81 bytes. M83-U1 through M83-U8 are pending and no
automated result below is presented as human evidence.

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create several geometry variants, constraints, a computed Fillet and a native Profile Offset while watching Outline. | Each user action appears as one understandable declaration or property change; no low-level generated clutter is presented as separate authored history. | pending |
| M83-U2 | Select declarations in Outline and edit supported fields in the Inspector. Move declarations and cells by buttons and drag/drop. | Typed edits update the accepted sketch; organization reorder changes only presentation and never geometry, IDs, constraints or DOF. | pending |
| M83-U3 | Open Structured source, rename/reorder content and edit recognized numeric/branch tokens. | Formatting/order stays non-semantic; typed edits match canvas/Inspector behavior; invalid explicit intent remains visible with the prior accepted canvas. | pending |
| M83-U4 | Drag free points and curve controls repeatedly in large samples, including reversal and one rejected sample. | Preview remains smooth, last-valid geometry stays visible, release commits the newest sample once and Undo restores the complete pre-drag state. Driving/fixed properties are never silently rewritten. | pending |
| M83-U5 | Change Fillet radius and Offset distance through their dedicated gestures, then delete their declarations from canvas/tree and Outline/source selection. | Property gestures remain intuitive; every surface names one visible declaration owner, deletion removes the complete owned feature/association in one action and one Undo restores it. Private Offset helpers never become invisible mutation targets. | pending |
| M83-U6 | Undo/Redo a mixture of canvas, Inspector, source and organization changes, then inspect History. | One coherent history is observed; History is read-only and no action is duplicated or skipped. | pending |
| M83-U7 | Save/reload the v8 workspace, then load representative v1-v6 workspaces/samples and continue editing. | v8 returns exactly; older flat scenes restore honestly and stay editable without invented recipe history or blank geometry. | pending |
| M83-U8 | Exercise an AI/RPC or packaged TypeScript patch example and then continue editing its output in the GUI. | Code-authored declarations use the same stable references and typed Inspector/canvas behavior; no separate JS solver or opaque uneditable result appears. | pending |

Any blank/withheld accepted scene, semantic change from display reordering, duplicate history,
silent driver rewrite, stale pointer-up publication, invalid reconstruction success, or
uneditable code-authored output withdraws the candidate and opens an owning-layer regression.

Moved annotation positions are intentionally presentation-only cache state. Workspace reload may
retain compatible positions, but source/Outline/History, solver materialization and Undo/Redo must
not treat those positions as design declarations; dropping or corrupting the cache must simply
recompute deterministic placement.
