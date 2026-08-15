<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M74 focused UAT — Production-style sketch reference UX

Status: **prepared but not started as of 2026-08-16**. No M74 candidate is yet nominated. Candidate
source/tree, immutable snapshot, Tailscale endpoint, manifest and qualification results remain
pending. GitHub Pages must continue serving the accepted M73 build until M74 passes this scorecard
and receives explicit approval.

Direct Rust/WASM tests are authoritative for exact residuals, Jacobians, persistence rejection,
pixel boundaries, action atomicity and history. Human UAT should judge visual hierarchy,
discoverability, predictable capture and desktop interaction feel.

## M74-U1 — intrinsic datums look permanent and selectable

1. Open an empty ordinary sketch. Confirm Origin, X axis and Y axis are visible without creating
   geometry. Pan and zoom: each axis continues across the full mapped sketch plane, labels remain
   readable, and Origin stays at model `[0, 0]`.
2. Select each datum from both canvas and the **Reference geometry** tree group. Confirm hover and
   selected styling agree, the inspector identifies an intrinsic protected reference, and no
   editable coordinate, role, suppression, delete or lock control is offered.
3. Put a native point or curve directly over a datum and click the overlap. Confirm native geometry
   wins. Select the datum explicitly from the tree to prove it remains available.
4. Attempt drag, Delete, suppress/unsuppress, Unconstrain, Lock and Profile/Construction conversion
   with one datum selected, then with a mixed datum/native selection. Every attempt must leave the
   whole scene and history unchanged and show the protected-datum reason where actions expose one.
5. Confirm the status count still reports only native points/curves and that Undo/Redo acquired no
   datum-only checkpoint.

Pass when datums feel ever-present and useful but unmistakably different from editable geometry.

## M74-U2 — relations use datums without owning them

1. Select a free point and Origin in both operand orders and apply Coincident. The point moves to
   `[0, 0]`; the relation annotation/tree entry is selectable while Origin remains intrinsic.
2. Repeat point + X axis and point + Y axis in both orders. Confirm only the normal coordinate is
   constrained and tangential drag remains available when the rest of the sketch permits it.
3. Apply Collinear to an ordinary line and each axis in both orders. Confirm both endpoints lie on
   that axis. Try an incompatible non-line curve and confirm mutation-free typed rejection.
4. Apply Parallel and Perpendicular between a line and each axis. Inspect the result: it should be
   the ordinary Horizontal or Vertical relation implied by the axis, not a second persistent datum
   family.
5. Suppress, reactivate and delete each datum-backed relation, then Undo/Redo representative edits.
   The constrained geometry follows normal relation lifecycle, but all three datums remain.
6. Save/reload the ordinary browser workspace and confirm the relations return. This exercises the
   existing draft-v5 workspace side records; it is not evidence that canonical sketch v5 is
   supported.

Pass when relations behave like ordinary removable design intent and the referenced datum never
behaves like a removable document object.

## M74-U3 — datum inference priority, tolerance and composition

Run at two visibly different zoom levels so the capture envelope can be judged in screen pixels.
Exact boundary equality is owned by direct tests; human review should confirm the same apparent
pixel feel at both zooms.

1. With a point-bearing Line or Polyline stage, approach Origin. It enters at **6 px Euclidean
   distance**, remains latched through **9 px**, and releases beyond 9 px. The preview must show one
   Origin coincidence, not separate X-axis and Y-axis relations.
2. Approach each axis away from Origin. It enters at **4 px perpendicular distance**, remains
   latched through **7 px**, and releases beyond 7 px. The preview projection and retained relation
   must name the same axis.
3. Place a native point on an axis and repeat the approach. Native point reuse/tracking must win and
   the datum must not steal the candidate. Near the shared axis intersection, Origin must win over
   either axis.
4. Draw a live Horizontal line toward the X axis. Horizontal already owns Y, so no Point-on-X-axis
   candidate, guide or relation may compete. Repeat live Vertical toward the Y axis.
5. Check both orthogonal combinations: live Horizontal plus Y-axis inference must produce the
   expected two-coordinate/two-relation bundle, and live Vertical plus X-axis inference must do the
   same. Commit, Undo and Redo each representative bundle as one construction history step.
6. Hold Shift while inside an Origin/axis envelope. The raw point must be used and the datum latch
   cleared. Release Shift and re-enter to reacquire normally.
7. Hide **References** and repeat. No canvas datum guide, canvas pick or inference may occur.
   The always-present Reference tree may still explicitly select the hidden protected datum; it is
   the discoverability/inspection route, not a canvas hit surface. Restore References and confirm
   capture returns. Toggling Grid alone must not affect datum inference.
8. Exercise Circle circumference/radius placement over Origin and both axes. It must remain a
   radius sample and never add a hidden point or datum relation. Circle centre and other genuine
   point-bearing stages remain eligible.
9. Cancel and retry, then pan or zoom during an unconfirmed candidate. No stale guide, latch or
   retained relation may survive the stage/camera change.

Pass when native intent outranks references, Origin outranks axes, same-coordinate constraints do
not fight, orthogonal constraints compose, and the `6/9 px` versus `4/7 px` feel is restrained and
consistent across zoom.

## M74-U4 — grid and camera controls remain visual

1. Toggle Grid and References independently in several combinations. Grid visibility must not hide
   datums; References visibility must not hide the Grid.
2. Pan so Origin is off-centre and zoom repeatedly. Grid lines must remain aligned to model Origin,
   change density smoothly through a `1–2–5 × 10^n` major-spacing sequence and avoid becoming an
   unreadable fixed-pixel wallpaper.
3. Draw points and lines near grid intersections with References both visible and hidden. Unless
   some ordinary geometry/datum inference applies, coordinates must remain raw: the Grid never
   snaps, selects, creates a guide or adds history.
4. Press **Origin** from a panned view. The camera recentres `[0, 0]` without changing zoom or sketch
   state. Undo/Redo must remain unchanged.
5. Create native geometry far from Origin and press Fit. Fit must frame native accepted geometry,
   not the infinite axes. Delete all native geometry, pan/zoom away and press Fit again; the empty
   sketch resets to the canonical camera.

Pass when the grid improves spatial orientation without acquiring CAD semantics and camera actions
never mutate the sketch.

## M74-U5 — coordinate HUD and contextual cursors

1. Move over empty canvas. The HUD reports model X/Y coordinates and updates smoothly; values near
   display zero should read as zero rather than negative-zero noise.
2. Enter a native or datum inference candidate. The HUD switches to the exact adjusted inference
   coordinate while its explanatory text preserves the raw pointer coordinate. Move outside the
   candidate and confirm raw display returns immediately.
3. Compare the HUD coordinate with the committed point and guide. They must agree; the browser must
   not appear to calculate a second snap.
4. Switch among Select, a drawing tool, a constraint/relation tool and Fillet, then middle-drag pan.
   Confirm the cursor distinguishes ordinary selection, crosshair drawing/Fillet, relation
   placement and active grabbing, and returns correctly when the operation ends or is cancelled.

Pass when coordinates and cursor shape communicate current intent without obscuring the scene or
suggesting a snap that will not commit.

## M74-U6 — Undo/Redo shortcuts respect editing ownership

1. Make two ordinary geometry edits. Use `Ctrl+Z` on Linux/Windows or `Cmd+Z` on macOS to Undo once,
   then `Ctrl+Shift+Z`/`Cmd+Shift+Z` to Redo once. On Linux/Windows also confirm `Ctrl+Y` Redo.
2. Hold both Ctrl and Command, or add Alt, and press the same letters. No history action should
   occur.
3. Focus each ordinary text/number input, a select control, any content-editable surface and an
   open dialog/overlay field. The editing control owns its keystroke; sketch history must not move.
4. Return focus to the canvas and repeat Undo/Redo. Confirm exactly one history step per shortcut,
   correct selection/accepted-scene recovery and no stale error message.

Pass when standard desktop shortcuts are discoverable and reliable but never steal text-editing or
dialog input.

## M74-U7 — SVG letterbox bands are inert

At a desktop window/aspect ratio that produces unused bands around the mapped SVG view box:

1. Start from Select and click, double-click and move in each band. Selection, hover, camera and
   geometry must remain unchanged.
2. Activate Line/Polyline and click or double-click in a band. No point, provisional segment,
   completion or history entry may be created.
3. Wheel over a band. Sketch zoom must not change and the band must not become a zoom anchor. Move
   to the mapped sketch plane and confirm wheel zoom still anchors normally.
4. Start a pan or captured edit inside the valid plane and finish through its existing supported
   path. The new band guard must not regress valid in-plane interaction or the established captured
   gesture terminal behavior.

Pass when only the actual mapped sketch plane starts semantic input and the unused SVG area is
reliably inert.

## M74-U8 — compact-desktop polish and acceptance

Repeat representative U1, U3, U4 and U6 paths at `1440x900` and approximately `1024x720`. Check
tree labels, inspector text, axis labels, HUD and camera controls for overlap or clipping. Mobile and
tablet behavior remain outside scope.

Pass when the complete treatment feels like one coherent CAD demonstration at both supported
desktop sizes. After explicit approval, deploy the exact accepted source to GitHub Pages and repeat
one datum relation, one inference bundle, grid/camera/HUD checks and asset/media verification there.

## Approval record

- M74-U1: **Pending**.
- M74-U2: **Pending**.
- M74-U3: **Pending**.
- M74-U4: **Pending**.
- M74-U5: **Pending**.
- M74-U6: **Pending**.
- M74-U7: **Pending**.
- M74-U8: **Pending**.
- Final M74 approval: **Pending**.

Do not mark any item accepted from automated evidence alone. Record the nominated source/tree,
immutable Tailscale manifest, exact human findings and final public artifact only after those events
occur.
