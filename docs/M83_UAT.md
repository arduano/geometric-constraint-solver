<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

Status: **accepted and closed 2026-08-25; F010 clean qualification, immutable preview
nomination, maintainer acceptance, GitHub Pages publication and exact hosted-byte
verification pass**. Automation owns exact identities, equations, residuals, persistence and
deterministic reconstruction; human review owns clarity and interaction feel.

## Candidate authority

Initial nomination commit `232b83a` and post-F005 source `a621cdd`/tree `f6d77b4` are withdrawn by
M83-F001 through M83-F007 and remain historical evidence only. The post-F007 candidate below is
also superseded by the architecture-hardening pass and is historical evidence only.

Both verification passes covered `/` plus all seven assets: HTTP 200, zero redirects, no
`Location` or `Content-Encoding`, exact media type/length/body and root equality with `index.html`.
Temporary byte/browser listeners were retired only after their passes; the retained service then
passed independent byte and focused browser verification. Complete freeze, browser and serving
evidence is in `geosolve-m83-f007-freeze-evidence.y5GbCJ`.

Superseded post-hardening mechanical authority:

Superseded F008/F009 mechanical UAT authority:

Both historical byte-verification passes cover `/` plus all seven assets: HTTP 200, zero redirects,
exact media type/length/body and root equality with `index.html`. Temporary listeners were retired
only after passing; the retained service then passed independent byte and focused-browser
verification.

Accepted F010 mechanical UAT authority:

| Mechanical nomination | Status |
| --- | --- |
| M83-F010 clean qualification, no-rebuild freeze and immutable preview replacement | complete |
| maintainer milestone-level acceptance | complete |
| GitHub Pages publication, exact hosted-byte verification and service retirement | complete |

No automated result below is presented as human evidence. Pages is final M83 public-byte
authority.

## Maintainer acceptance

On 2026-08-25 the maintainer explicitly approved the implementation plan that closes M83
from the existing F010 qualification, frozen-artifact and review evidence, and instructed that
plan to be implemented. That milestone-level decision accepts M83-U1 through M83-U10 and the
F001-F010 replacement disposition for closure. It does **not** claim a separate row-by-row
hands-on replay or invent observations that were not logged. The statuses below therefore say
`accepted by milestone-level approval`, not `manually passed`.

## Final GitHub Pages publication

Documentation-only approval descendant `2006c86b936c3522cc48fbf26cf78664d5e31e90`, tree
`c4a59d252ec94cd9344acf1646efd3fcc62d39df`, passes Pages run `32817232564`, build job
`97707877103`, deploy job `97709242120` and artifact `9551973351` (Actions API size 3,960,865
bytes). Downloaded artifact
`geosolve-m83-pages-artifact.NgAszX6o/artifact.tar` is 12,288,000 bytes with SHA-256
`06bce15ddea6d21048a25e3630a368ebe0ba883be98ee296869f77c47b86218b`. It extracts to exactly
seven regular files, no symlinks, with ordered-manifest aggregate
`75234fd6ff4349e4b75c858b90e90630002a7dd9b9171e47dfe28bc253cf23fc`.

The accepted F010 candidate mechanically preserves the automation-only architecture contract:
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
