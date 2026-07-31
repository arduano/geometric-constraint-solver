# M65 focused UAT

Status: mechanically qualified reduced-scope candidate available; supervising-human approval is
pending.

Candidate code source: `42d55b1` (with core certification prerequisite `f647318`).

Tailscale endpoint: `http://100.94.63.83:8080/`

Use only the ordinary workbench candidate recorded above once it is available. This scorecard is a
focused human behavior check after direct native, WASM and release qualification; it does not
replace those gates.

Mechanical qualification (2026-07-31): formatting, warnings-denied locked workspace Clippy,
locked all-feature workspace tests, the all-feature `wasm32-unknown-unknown` check, the optimized
Trunk 0.21.14 release bundle and `git diff --check` pass. A verified GET from the Tailscale
endpoint returns the ordinary GeoSolve Sketch Workbench built from the code source recorded
above.

## UAT scorecard

### M65-U1 — Scotch-yoke and scissor continuity

1. Open **Mechanisms → Scotch yoke · 1 DOF**.
2. Delete **Yoke slider on horizontal guide** so the lower point has two freedoms.
3. Drag that point horizontally, vertically and diagonally, including short reversals.
4. Repeat opening/closing paths and reversals on **Scissor jack** and
   **Five-stage scissor tower**.

Expected: the selected control follows the cursor on the current local configuration. Motion is
continuous and does not jump to an unrelated valid assembly. A rejected or exhausted sample keeps
the complete last valid preview, and returning to a nearby valid target can recover in the same
gesture. The tab remains responsive.

Result: Pending.

Notes:

### M65-U2 — pantograph responsiveness

1. Open **Mechanisms → Pantograph linkage · 2 DOF**.
2. Drag **Pantograph input A** through short horizontal, vertical and diagonal paths with
   reversals.
3. Exercise the guide, output and center controls in turn.
4. Alternate between independently movable controls.

Expected: all intended freedoms remain usable, the selected control moves predictably, and
independent passive motion is not introduced merely to satisfy a cursor sample. Reversal does not
switch assembly root, and no interaction synchronously locks the main thread.

Result: Pending.

Notes:

### M65-U3 — symmetric twin-roller independence and recovery

1. Open **Mechanisms → Twin-roller cam · 2 DOF**.
2. Press the first circle on its circumference, away from its center, and drag horizontally,
   vertically and diagonally with reversals.
3. Repeat symmetrically for the second circle.
4. Push either roller toward a difficult or invalid location, then return to a nearby valid
   location without ending the gesture.

Expected: the pressed circumference behaves as an offset-preserving handle for its own center.
The other roller remains stationary. Difficult work rejects within a responsive bounded interval,
keeps the full last valid preview, and permits same-gesture recovery.

Result: Pending.

Notes:

### M65-U4 — ordinary lifecycle, authoring and persistence

1. On a movable mechanism, drag through several accepted samples and release.
2. Undo and Redo the released move.
3. Start another drag and cancel it; confirm history and accepted geometry are unchanged.
4. Apply and delete an ordinary constraint using the normal authoring flow.
5. Save/reload the workspace and verify the accepted geometry, constraint and editability.

Expected: one release produces one ordinary history edit, Cancel publishes nothing, and Undo/Redo
remain coherent. Constraint authoring and save/reload behave as before M65. No alternate-branch
control, branch-only sample or modal branch workflow is present.

Result: Pending.

Notes:

## Finding ledger

No finding is recorded for this reduced-scope candidate. Add durable `M65-Fxxx` entries here for
any UAT failure, with reproduction, owning layer, direct regression and human retest disposition.

## Approval

M65 remains open. Approval requires:

1. one exact candidate commit and Tailscale endpoint recorded above;
2. fresh formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature WASM
   check, release Trunk build and `git diff --check` recorded against that source state;
3. every scorecard item marked Pass or an explicitly accepted scoped limitation;
4. every finding given a tested disposition; and
5. an explicit supervising-human approval statement.
