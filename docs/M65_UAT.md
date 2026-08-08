# M65 focused UAT

Historical record: the endpoint and candidate language below describe the approved M65 review
session. The endpoint is not expected to remain live.

Status: complete and explicitly approved by the supervising human on 2026-08-01.

Candidate code source: `b6433d1`.

Historical Tailscale endpoint: `http://100.94.63.83:8080/`

The review used only the ordinary workbench candidate recorded above. This scorecard is a
focused human behavior check after direct native, WASM and release qualification; it does not
replace those gates.

Mechanical qualification (2026-08-01): formatting, warnings-denied locked workspace Clippy,
locked all-feature workspace tests, the all-feature `wasm32-unknown-unknown` check, the optimized
Trunk 0.21.14 release bundle and `git diff --check` pass. A verified GET from the Tailscale
endpoint returns the ordinary GeoSolve Sketch Workbench built from the code source recorded above.

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

Result: Pass (2026-08-01 closure review; unaffected by the replacement candidate).

Notes: No continuity, branch-jump or responsiveness blocker remains in this scorecard area.

### M65-U2 — pantograph responsiveness

1. Open **Mechanisms → Pantograph linkage · 2 DOF**.
2. Drag **Pantograph input A** through short horizontal, vertical and diagonal paths with
   reversals.
3. Exercise the guide, output and center controls in turn.
4. Alternate between independently movable controls.

Expected: all intended freedoms remain usable, the selected control moves predictably, and
independent passive motion is not introduced merely to satisfy a cursor sample. Reversal does not
switch assembly root, and no interaction synchronously locks the main thread.

Result: Pass (2026-08-01 focused retest approval against `b6433d1`).

Notes: Holding the opposite arm stationary while dragging either one-DOF side control is the
accepted M65 locality policy. `M65-F005` fixes the separate numerical rejection that made natural
off-manifold upper-guide drags appear mostly immovable. The supervising human approved this
focused result.

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

Result: Pass (2026-08-01 focused retest approval against `b6433d1`).

Notes: `M65-F004` restores direct access to the left roller where its driving-radius annotation
overlaps the center or canonical circumference handle. The supervising human approved this
focused result.

### M65-U4 — ordinary lifecycle, authoring and persistence

1. On a movable mechanism, drag through several accepted samples and release.
2. Undo and Redo the released move.
3. Start another drag and cancel it; confirm history and accepted geometry are unchanged.
4. Apply and delete an ordinary constraint using the normal authoring flow.
5. Save/reload the workspace and verify the accepted geometry, constraint and editability.

Expected: one release produces one ordinary history edit, Cancel publishes nothing, and Undo/Redo
remain coherent. Constraint authoring and save/reload behave as before M65. No alternate-branch
control, branch-only sample or modal branch workflow is present.

Result: Pass (2026-08-01 closure review; unaffected by the replacement candidate).

Notes: No lifecycle, authoring or persistence blocker was reported.

## Finding ledger

Finding IDs `M65-F001` through `M65-F003` remain reserved by the withdrawn earlier candidates and
are not reused below.

### M65-F004 — left twin roller was hidden behind its radius annotation

- Reproduction: open **Twin-roller cam · 2 DOF** and press the left roller at its center or the
  positive-X circumference point used by the visible radius leader. The dimension was selected
  and no drag gesture began, while the right roller remained directly draggable.
- Root cause: Select pointer-down tested visible annotations before accepted geometry, and radial
  annotation hit-testing includes the complete center-to-edge leader. The left driving dimension
  is always visible; the right reference dimension is contextual, creating asymmetric access to
  otherwise symmetric solver freedoms.
- Disposition: a directly draggable point or semantic curve handle now wins only its exact overlap
  with an annotation. Offset labels and annotations over non-draggable geometry retain the
  existing annotation-first behavior.
- Direct regression: the real `MotionCam` scene starts the correct semantic-center gesture from
  both roller centers and both positive-X circumferences, while the left radius label remains
  selectable without starting a gesture.
- Human retest: Pass under M65-U3 on 2026-08-01.

### M65-F005 — pantograph guide rejected ordinary off-manifold cursor targets

- Reproduction: from the initial pantograph pose, project upper guide B toward `[1.2, 3.0]`.
  Before remediation, one bounded attempt rejected after 6 nonlinear iterations and 11
  factorizations even with unlimited control. Hard geometry remained valid with maximum residual
  `5.55e-17`; the Temporary solve reported numerical/evaluation failure.
- Root cause: a two-coordinate cursor projected through B's one instantaneous freedom creates an
  almost rank-one `2 x 2` least-squares system. The dynamic bidiagonal SVD result could lose enough
  stationarity for the strict KKT check to reject it depending on matrix orientation.
- Disposition: the rank-aware solve uses a fixed-size analytic `2 x 2` SVD for this exact shape,
  retains the authoritative unsquared rank cutoff, and publishes a step only after stationarity
  and retained-row-space minimum-norm certification. No tolerance, hierarchy or work limit is
  relaxed.
- Direct regression: the exact reduced matrix is finite, bounded, minimum norm and KKT-certified;
  three natural guide targets project to the nearest radius-`sqrt(10)` position in one bounded
  attempt while input A remains within `1e-8` of its gesture-start position.
- Human retest: Pass under M65-U2 on 2026-08-01.

## Approval

The supervising human explicitly approved the two outstanding focused tests and asked to proceed
to M66 on 2026-08-01. M65-U1 through M65-U4 are therefore all Pass against mechanically qualified
source `b6433d1`; `M65-F004` and `M65-F005` have direct regressions and accepted human retests.
M65 is closed without broadening its recorded scope or treating any inherent alternate-assembly
search as implemented behavior.
