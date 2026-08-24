<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Status: **post-F005 clean-qualified immutable replacement candidate nominated; focused human UAT
pending**. Automation owns exact identities, equations, residuals, persistence and deterministic
reconstruction; human review owns clarity and interaction feel.

## Candidate authority

Initial nomination commit `232b83a` and its advertised source `1b4f455`/tree `ce09e01` are
withdrawn by M83-F001 through M83-F005 and remain historical evidence only.

- Product source: `a621cddc0a3b8687d6b7686bc850619332c73779`.
- Product tree: `f6d77b447552d0d120c48be4bfdeb96bc5f2da59`.
- Frozen no-rebuild snapshot: `/tmp/geosolve-m83-f005-uat.ge07gw` (directory `0555`; seven regular
  non-symlink files `0444`).
- Ordered file-manifest aggregate:
  `720ea687a7002a9f1dbc818147263d9b003cc03bd79cefb2cf8e6aca2597b89b`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`
  (`geosolve-m83-uat.service`, PID `3331431`).
- Clean gate:
  `env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`,
  exit 0 at 2026-08-24 17:24:39 AEST; log SHA-256
  `1043356cb2480944314566eab7fb92e5d560f48ee606ce3ca15ea4753f16af1d`.
- Frozen browser verification: 3/3 deterministic drag/capture/Undo, before/after Outline drop and
  curve-property/Undo checks passed.
- Exact served-byte verification: temporary
  `/tmp/geosolve-m83-f005-temp-verify.YYwD99/results.tsv` and retained
  `/tmp/geosolve-m83-f005-final-verify.EGsGQD/results.tsv`, each SHA-256
  `585aa1571a1bc4440fcce609afa17435382ad1ee8d8f9d60e128229f18313a87`.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body, the frozen aggregate above and
root equality with `index.html`. Temporary `:18085` verification passed before the retained
`:8080` service started, then the temporary listener was retired. The documentation-only
descendant recording this evidence neither replaces the product source/tree nor rebuilds the
candidate bytes.

GitHub Pages deliberately remains on accepted M81 bytes. M83-U1 through M83-U8 are pending and no
automated result below is presented as human evidence.

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create several geometry variants, constraints, a computed Fillet and a native Profile Offset while watching Outline. | Each user action appears as one understandable declaration or property change; no low-level generated clutter is presented as separate authored history. | pending |
| M83-U2 | Select declarations in Outline and edit supported fields in the Inspector. Move declarations and cells by buttons and by upper/lower-half drag/drop, including adjacent/end moves. | Typed edits update the accepted sketch; before/after organization reorder changes only presentation and never geometry, IDs, constraints or DOF; Undo restores exact order. | pending |
| M83-U3 | Open Structured Source, inspect stable input bindings, rebind through code/RPC, then reorder and attempt an edit from a stale token before editing a current numeric/branch token. | Source and Inspector show the same updated stable binding, Inspector inputs are read-only, stale tokens reject without retargeting, and current typed edits match canvas behavior while invalid intent retains the prior accepted canvas. | pending |
| M83-U4 | Drag free points and curve controls repeatedly in large samples, including reversal, one accepted preview followed by a rejected sample, and delayed capture loss. | Preview remains smooth; release commits the newest visible accepted preview exactly once, delayed/duplicate terminals are inert, and Undo restores the complete pre-drag state. Driving/fixed properties are never silently rewritten. | pending |
| M83-U5 | Change Fillet radius and Offset distance through their dedicated gestures, then delete their declarations from canvas/tree and Outline/source selection. | Property gestures remain intuitive; every surface names one visible declaration owner, deletion removes the complete owned feature/association in one action and one Undo restores it. Private Offset helpers never become invisible mutation targets. | pending |
| M83-U6 | Undo/Redo a mixture of canvas, Inspector, source and organization changes, then inspect History. | One coherent history is observed; History is read-only and no action is duplicated or skipped. | pending |
| M83-U7 | Save/reload ordinary and retained-invalid v8 workspaces, then load representative v1-v6 workspaces/samples and continue editing. | v8 returns exactly; retained-invalid migrated/bootstrap intent preserves the prior accepted canvas, current failure and Undo; older flat scenes restore honestly without invented recipe history or blank geometry. | pending |
| M83-U8 | Exercise an AI/RPC or packaged TypeScript patch example and then continue editing its output in the GUI. | Code-authored declarations use the same stable references and typed Inspector/canvas behavior; no separate JS solver or opaque uneditable result appears. | pending |

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

Moved annotation positions are intentionally presentation-only cache state. Workspace reload may
retain compatible positions, but source/Outline/History, solver materialization and Undo/Redo must
not treat those positions as design declarations; dropping or corrupting the cache must simply
recompute deterministic placement.
