<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M74 focused UAT — Production-style sketch reference UX

Status: **M74 is complete under the supervising caller's scoped close decision on 2026-08-16**.
The exact clean gate, independent review, immutable snapshot, served-byte verification and exact
final GitHub Pages publication are accepted as closing evidence. U1-U8 remain intentionally
deferred to the next bug-fixing/UAT follow-up milestone and are not claimed as manually passed;
that milestone remains unstarted.

Candidate source: `55693372bea4759c9a67eee14f1af3d6a9e0690c`

Candidate tree: `866fbf8b58ec19e72cbe6936e06f3615dba2f692`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID: `2599593`

Immutable snapshot: `/tmp/geosolve-m74-uat.jFfAm4` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`1e5d00474c383102f4f6189a534e5acb395d92e94a7c0853b72d9c25b0f4fe13`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 16,702 | `a3b8ca5a5d5999d09a05c7910eab952929e2dc3f07eeb27ccc36b7fe3a992701` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-b110169860de7f0f.js` | 33,221 | `980c38ffa22901ee90bebec8b705f92b07b651ec92001fffd4a62ac03055b74b` |
| `geosolve-demo-web-b110169860de7f0f_bg.wasm` | 6,102,644 | `d2932cf18e67a0e0c087ab4ccacf2ac3be086d2da74b10ac9026c53e4e64ccf4` |
| `index.html` | 27,478 | `9968011bc0524e30d03a4c299098e047957af96336ec6289842d4ceb724a6fb5` |
| `styles-711a681b653e6d49.css` | 30,861 | `d75f830c2e0af21399fd94f31dda74888a4ce82bbe7527521c7d5f5a1c948532` |

The candidate's exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 on the candidate
source. It includes the 270/270 golden check, native/WASM M74 5/5 parity, workspace format, Clippy,
locked all-feature tests, Rustdoc, benchmark compilation, M14/M32 performance,
licensing/package checks, the 256-moving-body sparse crossover in 86.79 seconds and Trunk 0.21.14
release assembly.

The gate distribution was copied without rebuilding. Proxy/cache-bypassed identity requests for
all seven files and `/` return HTTP 200 from the Tailscale address with exact media types, lengths
and bytes; `/` equals `index.html`, and the fetched aggregate matches. The M72 compatibility and
M74 browser scripts both pass at `1440x900` and `1024x720` with no console/page errors. The HTTP
evidence directory is `/tmp/geosolve-m74-http-verify.85lR5D`. Historical initial M74 snapshot
`/tmp/geosolve-m74-uat.MpvYrl` remains read-only but is no longer served or UAT authority.

Final public authority is documentation-only approval descendant
`b6b1d62b49466ea06522dbdd3f5444a324d36584`, successful Pages run `31923806117`, deployment
`5927348343` and artifact `9257602997` at
`https://arduano.github.io/geometric-constraint-solver/`. The public root and all seven paths
return HTTP 200 and byte-match the downloaded artifact; `/` equals `index.html`, application URLs
are repository-prefixed, media types are correct, and both public browser scripts pass at the two
desktop sizes. The hosted artifact's C-locale manifest aggregate is
`df421cc0050c31008e5cb5620092c4d05e91191fd1eccaaf020ca437ce97e725`; the complete file
manifest and archive hashes are recorded in `docs/M74_IMPLEMENTATION.md`.

Direct Rust/WASM tests are authoritative for exact residuals, Jacobians, persistence rejection,
pixel boundaries, action atomicity and history. The scorecard below remains the future hands-on
check for visual hierarchy, discoverability, predictable capture and desktop interaction feel; it
is deferred intact rather than converted into synthetic M74 evidence.

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
7. Draw two free points, select both points plus X axis, and apply Symmetric. Confirm the points
   have equal X coordinates and opposite Y coordinates. Repeat with Y axis: equal Y coordinates
   and opposite X coordinates.
8. Repeat with the axis preselected first and with the two point selections reversed. Then enter
   active Symmetric mode and pick point → point → axis by clicking the painted canvas axis itself,
   not only its tree entry. All complete routes should create one ordinary relation; active mode
   should continue to ask for a line or reference axis after the two points.
9. In active Symmetric mode, pick the same point twice. Confirm the second pick is rejected without
   losing the valid first point. With two distinct points pending, pick Origin; it must reject and
   retain both points so selecting X or Y axis can immediately complete the relation.
10. Select/hover the symmetry annotation and referenced axis. Confirm the usual Symmetry glyph is
    anchored midway between the paired points and becomes related through the axis. Suppress,
    reactivate, delete, Undo/Redo and save/reload one axis-symmetry relation.
11. Recheck ordinary two-point-plus-drawn-line Symmetric. Its established behavior must remain
    unchanged; axis support must not create or expose a hidden construction line.

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
desktop sizes. This hands-on pass is now deferred to the next bug-fixing/UAT follow-up milestone;
M74's final publication has completed from the accepted automated, review and frozen-artifact
evidence.

## Approval record

- M74-U1: **Deferred; not manually executed or marked passed**.
- M74-U2: **Deferred; not manually executed or marked passed**.
- M74-U3: **Deferred; not manually executed or marked passed**.
- M74-U4: **Deferred; not manually executed or marked passed**.
- M74-U5: **Deferred; not manually executed or marked passed**.
- M74-U6: **Deferred; not manually executed or marked passed**.
- M74-U7: **Deferred; not manually executed or marked passed**.
- M74-U8: **Deferred; not manually executed or marked passed**.
- Final M74 approval: **Pass for scoped closure** — explicitly approved by the supervising caller
  on 2026-08-16 from the automated, review and frozen-artifact evidence.

This approval intentionally does not reinterpret automation as hands-on UAT. The complete U1-U8
scorecard and any future findings transfer to the next bug-fixing/UAT follow-up milestone. That
milestone is reserved by this handoff but is not activated, planned or started here. M74's final
public artifact is successful Pages run `31923806117`, deployment `5927348343` and artifact
`9257602997`; its hosted bytes are exact-verified above.
