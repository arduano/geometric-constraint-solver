# M65 focused UAT

Status: mechanically qualified candidate available; supervising-human approval pending.

Candidate code source: `81ba31c` (with numerical and sketch prerequisites in its ancestry).

Tailscale endpoint: `http://100.94.63.83:8080/`

Use the ordinary workbench at that endpoint. This is a focused human usability/behavior check
after direct native, WASM and release qualification. It is not a replacement for those tests.

## UAT scorecard

### M65-U1 — continuation without branch jumps

1. Open **Mechanisms → Scotch yoke · 1 DOF**.
2. Delete **Yoke slider on horizontal guide** so the lower point has two freedoms.
3. Drag the lower point through a smooth curved path, including small reversals.
4. Repeat on **Scissor jack** and **Five-stage scissor tower**.

Expected: motion follows the current local configuration. A rejected/ambiguous sample leaves the
last valid preview in place; later motion continues from it. No point jumps to an unrelated valid
assembly merely because another root exists.

Result: Pending.

### M65-U2 — pantograph interaction work

1. Open **Mechanisms → Pantograph linkage · 2 DOF**.
2. Drag **Pantograph input A** through several short arcs.
3. Drag the independent guide arm and then alternate between both controls.

Expected: the preview remains responsive and locally continuous. The prior multi-second/tab-lock
behavior is absent. Both freedoms remain usable.

Result: Pending.

### M65-U3 — independent twin rollers and rejection recovery

1. Open **Mechanisms → Twin-roller cam · 2 DOF**.
2. Move one roller while watching the other.
3. Push toward an invalid or difficult position, then return to a nearby valid position.

Expected: moving one roller does not reposition the other. Failure retains the last valid preview,
and recovery continues from it.

Result: Pending.

### M65-U4 — locked elbow explicit branch

1. Open **Mechanisms → Locked elbow · open/crossed branches**.
2. Select the elbow point.
3. In **Assembly branch**, choose **Preview alternate**.
4. Inspect the gold dashed ghost, then Cancel.
5. Preview again and Accept; exercise Undo and Redo.

Expected: Preview never mutates the authoritative sketch. Cancel restores the unchanged view.
Accept switches the representable open/crossed branch atomically, and Undo/Redo treat it as one
edit.

Result: Pending.

### M65-U5 — locked four-bar branch evidence

1. Open **Mechanisms → Locked four-bar · open/crossed branches**.
2. Select the output joint named by the sample.
3. Preview and accept the alternate branch.

Expected: the inspector reports inspected/maximum deterministic seeds. Only the gold ghost changes
before Accept. Accepted geometry is finite, satisfies the constraints and remains editable.

Result: Pending.

### M65-U6 — ordinary workflow regression

1. Return to a normal movable mechanism.
2. Drag, release, Undo and Redo.
3. Create/delete a constraint and save/reload the workspace.

Expected: M65 adds no modal branch behavior to ordinary drag, no sample read-only state and no
regression in ordinary history or persistence.

Result: Pending.

## Finding ledger

No human findings recorded yet. New findings use IDs `M65-F001`, `M65-F002`, and so on, with the
reproduction, owner, disposition, direct regression and human retest recorded here.

## Approval

M65 remains open. Approval requires all scorecard items to be marked Pass (or an explicitly
accepted scoped limitation), every finding to have a disposition, and an explicit supervising
human approval statement.
