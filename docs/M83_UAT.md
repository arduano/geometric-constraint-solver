<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Projectional sketch design intent

Status: **not yet nominated**. This scorecard becomes executable only after a clean-gate,
no-rebuild Tailscale candidate is recorded here. Automation owns exact identities, equations,
residuals, persistence and deterministic reconstruction; human review owns clarity and interaction
feel.

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create several geometry variants, constraints, a computed Fillet and a native Profile Offset while watching Outline. | Each user action appears as one understandable declaration or property change; no low-level generated clutter is presented as separate authored history. | pending |
| M83-U2 | Select declarations in Outline and edit supported fields in the Inspector. Move declarations and cells by buttons and drag/drop. | Typed edits update the accepted sketch; organization reorder changes only presentation and never geometry, IDs, constraints or DOF. | pending |
| M83-U3 | Open Structured source, rename/reorder content and edit recognized numeric/branch tokens. | Formatting/order stays non-semantic; typed edits match canvas/Inspector behavior; invalid explicit intent remains visible with the prior accepted canvas. | pending |
| M83-U4 | Drag free points and curve controls repeatedly in large samples, including reversal and one rejected sample. | Preview remains smooth, last-valid geometry stays visible, release commits the newest sample once and Undo restores the complete pre-drag state. Driving/fixed properties are never silently rewritten. | pending |
| M83-U5 | Change Fillet radius and Offset distance through their dedicated gestures, then delete their declarations from Outline. | Property gestures remain intuitive; deletion removes the complete owned feature/association in one action and one Undo restores it. | pending |
| M83-U6 | Undo/Redo a mixture of canvas, Inspector, source and organization changes, then inspect History. | One coherent history is observed; History is read-only and no action is duplicated or skipped. | pending |
| M83-U7 | Save/reload the v8 workspace, then load representative v1-v6 workspaces/samples and continue editing. | v8 returns exactly; older flat scenes restore honestly and stay editable without invented recipe history or blank geometry. | pending |
| M83-U8 | Exercise an AI/RPC or packaged TypeScript patch example and then continue editing its output in the GUI. | Code-authored declarations use the same stable references and typed Inspector/canvas behavior; no separate JS solver or opaque uneditable result appears. | pending |

Any blank/withheld accepted scene, semantic change from display reordering, duplicate history,
silent driver rewrite, stale pointer-up publication, invalid reconstruction success, or
uneditable code-authored output withdraws the candidate and opens an owning-layer regression.
