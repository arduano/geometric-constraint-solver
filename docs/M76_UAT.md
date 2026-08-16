<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 focused UAT — production-quality annotations

Status: prepared, not executed. Candidate source, tree, manifest and Tailscale URL are TBD. GitHub
Pages remains on accepted M75 until explicit M76 approval.

Run at `1440x900` and approximately `1024x720`, at coarse and fine zoom.

## U1 — dimension readability

Open the dimension sampler and inspect point distance, affine line/polyline-span length, radius,
diameter, angle, supporting offset and exact translated offset. Values must be compact, unambiguous and
visually attached to truthful geometry. Reference values must remain distinguishable without
colour, while tooltip/inspector/accessibility text stays descriptive.

## U2 — constraint readability and density

Open the constraint sampler. Hover/select operands and annotations, then enable Display “show all”.
Check all twenty symbol families, paired marks, local rotation, leaders and the fixed right-angle
square. Dense scenes should remain deterministic and legible without obvious collisions.

## U3 — move, cancel and reset

Move examples from every dimension family and several compact glyphs. Confirm the 3 px threshold,
that line/leader clicks select without unexpectedly moving, and that Escape, tool/camera change and
capture loss restore the original position. Test selected reset and reset-all.

## U4 — persistence and editing neutrality

Reload after moving annotations, then Delete/Undo/Redo nearby sketch content. Surviving placement
should persist and sketch history should contain no annotation-only step. Load a new sample and
confirm old offsets do not transfer. A deliberately incompatible cache must still restore the valid
sketch with deterministic automatic layout.

## Acceptance record

- U1: Pending.
- U2: Pending.
- U3: Pending.
- U4: Pending.
- Final supervising approval: Pending.
- GitHub Pages publication: Deferred until approval.
