<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 focused UAT — Authoritative sketch lineage

Status: **candidate not yet nominated; human UAT pending**. This scorecard will be bound to the
exact clean-gate source, immutable no-rebuild snapshot and byte-verified Tailscale endpoint after
mechanical qualification. M81 remains accepted public product authority meanwhile.

M83 deliberately preserves existing sketch mathematics and visible authoring behavior while
replacing persistent workbench authority. Human review should therefore focus on end-to-end
editing/history/reload behavior and failure recovery rather than inspecting internal lineage JSON.

## Candidate authority

- Product source/tree: pending clean nomination.
- Clean release-gate log and digest: pending.
- Immutable no-rebuild snapshot and ordered file aggregate: pending.
- Tailscale endpoint: pending.
- Exact `/` plus asset byte/media/length verification: pending.

## Focused scorecard

| ID | Check | Expected result | Status |
| --- | --- | --- | --- |
| M83-U1 | Create and edit representative Point, Line/Polyline, Rectangle, Circle/Arc, Ellipse, Bezier/conic and open/periodic NURBS variants, including Shift variants and snapped point reuse. | Recipe behavior, intrinsic constraints, branches and stable selection remain indistinguishable from the accepted workbench; no blank/partial scene appears. | pending |
| M83-U2 | Add representative point, direction, symmetry, contact/tangency/continuity constraints and Driving/Reference distance, radius and angle dimensions; edit, suppress, delete, Undo and Redo them. | One ordinary history controls the complete scene; annotations, target values, source identity and explicit branches survive history. | pending |
| M83-U3 | Move simple and advanced curve handles, including a drag that changes several owning points/parameters; edit radius/trim/rational/NURBS properties and Profile/Construction role. | The accepted projection persists after release and reload; all affected owners change atomically, with no partial motion or hidden generic Move action. | pending |
| M83-U4 | Create, edit and remove a computed multi-corner Fillet; exercise radius manipulation, an explicit branch action and an invalid/recovery path. | Stable feature/corner intent survives while generated edges remain current-only; failure retains the last complete scene and later valid input recovers. | pending |
| M83-U5 | Apply native-profile Fillet, then create a face or open-chain Profile Offset and drag/edit its distance; Undo/Redo and delete the operation. | Native topology and Offset association rebuild exactly, operation-owned objects are removed/restored as one history action, and unrelated identities remain stable. | pending |
| M83-U6 | Deliberately create a structurally valid but unsolved/conflicting edit, then Undo, Redo, repair it and continue authoring. | The retained failure is visible while the prior complete accepted scene remains authoritative; history never becomes stuck or publishes a partial scene. | pending |
| M83-U7 | Save/reload the workspace with moved annotations, branches, features and history; repeat once after a failed retained edit and continue Undo/Redo. | Workspace v7 restores retained-versus-accepted authority, exact history and annotation placement, then continues editing without ID reuse or stale Problems. | pending |
| M83-U8 | Import one older reproduction/workspace payload from the existing v1-v6 corpus; edit and delete one imported entity, Undo/Redo that deletion, save as v7 and reload. | The imported scene remains one truthful baseline with no invented history; deletion behaves as a later retirement action, Undo restores the exact identity, Redo retires it again, and neither history nor reload reuses or silently removes that baseline identity. | pending |

## Acceptance rule

Any lost object/feature, branch flip, changed authoring behavior, partial multi-owner drag, stale or
nested history, accepted-scene substitution, failed recovery, reload mismatch or blank scene opens
an exact owner regression under the GeoSolve defect-hardening workflow and withdraws the
candidate. Cosmetic requests unrelated to M83 authority are deferred rather than folded into the
milestone.

M83 closes only after the exact candidate above passes this scorecard or receives an explicit
scoped disposition, the supervising caller approves it, and the accepted bytes are subsequently
published and exactly verified on GitHub Pages.
