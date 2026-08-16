<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 focused UAT — Select hover and primary pointer-owner parity

Status: **clean-qualified immutable candidate nominated; human scorecard not started and every
human disposition remains pending as of 2026-08-16**. GitHub Pages currently serves the accepted
M74 artifact. Do not use it as M75 authority.

Candidate source: `f3affff1b62b1cb484a59647c4072c94c3b12ada`

Candidate tree: `7662abc8b7c71130f54fbf2745afa60f0d286431`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID: `3757674` (retained command-runner session `23697`)

Immutable snapshot: `/tmp/geosolve-m75-uat.hUSaG7` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`69425a504453eda6645c96b6163b5b899ab455f40828f3cdecc73b90ff3c41d9`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 17,616 | `be4769bf0f57d1f27d7068e6e1e47a41305a320d08948fa306a38ca620db92b3` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-fc3fd24fd70a16aa.js` | 33,221 | `1e24182d7c61f3681b5fd62591a2f33b4ada6e3a1d3fd2fe884ad3484a2060bc` |
| `geosolve-demo-web-fc3fd24fd70a16aa_bg.wasm` | 6,109,194 | `76944eddca4ca6c95ad967c0b5b8dc215d292ca07515740fe3914588c1f4f70b` |
| `index.html` | 27,478 | `e00a829f0f954422fd9c5454110fd67d979b5fde42934ac230fbf34822c18430` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 at the candidate
source. It includes formatting, warnings-denied workspace Clippy, locked all-feature tests,
unchanged 270/270 golden `--require-clean`, native/WASM M75 9/9 parity, Rustdoc, benchmark
compilation, M14/M32 performance, the 138.09-second 256-moving-body crossover,
licensing/package checks and Trunk 0.21.14 release assembly. The gate output was copied without
rebuilding and contains exactly seven regular non-symlink files.

Proxy/cache-bypassed identity requests for all seven files and `/` return HTTP 200 with exact media
types, lengths and bytes, no redirect or content encoding; `/` equals `index.html`, and the fetched
aggregate matches. Evidence is retained at `/tmp/geosolve-m75-http-verify.iY4VKV`. The unchanged
M72 compatibility and M74 Chromium scripts pass at `1440x900` and `1024x720`; their SHA-256 values
are `4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`. These mechanical checks do
not dispose any human scorecard item.

Direct Rust/WASM tests will own exact precedence, distance comparisons, boundary equality,
mutation freedom and invalidation. Human UAT judges whether hover truthfully predicts a click,
whether related context is understandable, and whether the complete desktop interaction remains
polished and accessible.

## Review matrix

Run every section at `1440x900` and approximately `1024x720`. For M75-U9 through M75-U12, repeat
representative paths at a coarse and a fine sketch zoom. Approach each point, semantic center,
annotation, curve, Fillet radius affordance and datum from outside its established hit envelope;
sample visibly just outside and just inside the fringe. Exact equality belongs to direct tests.

Use Select unless a step explicitly says otherwise. After each hover prediction, press and release
without moving first; then repeat with a small valid drag where the predicted owner is draggable.
The item that reacts on pointer-down must be the one hover predicted.

For M75-U1 through M75-U8, execute every numbered step in the corresponding `docs/M74_UAT.md`
section. The routing summaries below make the carryover visible in M75; they do not replace, narrow
or retire any original M74 check.

## M75-U1 — carried M74-U1: intrinsic datums look permanent and selectable

This section carries every M74-U1 requirement forward; none was accepted at M74 close.

1. Open an empty ordinary sketch. Confirm Origin, X axis and Y axis are present without creating
   geometry. Pan/zoom: axes cross the mapped plane, labels remain readable and Origin remains model
   `[0, 0]`.
2. Select each datum from canvas and the **Reference geometry** tree. Hover, selected and inspector
   presentation must agree; no editable coordinate, role, suppression, Delete or Lock control is
   offered.
3. Put native geometry over a datum. Native geometry wins the canvas overlap while the tree can
   still select the datum explicitly.
4. Attempt drag, Delete, suppress/reactivate, Unconstrain, Lock and Profile/Construction conversion
   with a datum alone and in a mixed selection. The whole scene/history remains unchanged and the
   protected-datum reason appears where applicable.
5. Counts include only native geometry, and no datum-only operation adds Undo/Redo history.

Pass when datums feel permanent, selectable and useful without behaving like document objects.

## M75-U2 — carried M74-U2: relations use datums without owning them

This section carries every M74-U2 requirement forward.

1. Apply Coincident to point + Origin in both orders, then point + X/Y axis in both orders. Origin
   fixes both coordinates; an axis fixes only its normal coordinate.
2. Apply Collinear to an ordinary line and each axis in both orders. An incompatible non-line curve
   rejects without mutation.
3. Apply Parallel/Perpendicular between a line and each axis. The result is the implied ordinary
   Horizontal/Vertical relation, not a second datum family.
4. Suppress, reactivate, delete and Undo/Redo representative datum relations. Save/reload the
   ordinary workspace; relations return while all intrinsic datums remain.
5. Apply Symmetric to two distinct points plus X axis, then Y axis, in reversed point order and
   with the axis preselected. X gives equal X/opposite Y; Y gives equal Y/opposite X.
6. In active Symmetric mode, use point → point → axis. Repeated-point and Origin third picks reject
   without discarding the valid prefix; choosing an axis afterward completes one relation.
7. Hover/select the symmetry glyph and datum. Related highlighting, suppression, lifecycle,
   Undo/Redo and reload behave like an ordinary relation. Recheck two-point + drawn-line Symmetric;
   no hidden construction line appears.

Pass when removable design intent can reference a datum without making the datum removable.

## M75-U3 — carried M74-U3: datum inference priority and composition

This section carries every M74-U3 requirement forward and must be repeated at two sketch zooms.

1. In a point-bearing Line/Polyline stage, approach Origin: `6 px` entry, through-`9 px` latch and
   one Origin relation rather than two axis relations must feel consistent.
2. Away from Origin, approach each axis: `4 px` perpendicular entry and through-`7 px` latch. Guide,
   adjusted point and retained relation name the same axis.
3. Native points/curves beat datums; Origin beats either axis at the shared intersection.
4. Live Horizontal suppresses same-coordinate X-axis inference and live Vertical suppresses
   same-coordinate Y-axis inference. Horizontal + Y axis and Vertical + X axis remain valid atomic
   two-relation bundles with one-step Undo/Redo.
5. Shift suppression, hidden References, cancellation and camera/stage changes clear datum
   candidates. Grid visibility alone does not affect them.
6. Circle circumference/radius placement over Origin/axes remains a radius sample; genuine
   point-bearing stages remain eligible.

Pass when reference capture is restrained, zoom-independent and never fights an already-owned
coordinate.

## M75-U4 — carried M74-U4: grid and camera controls remain visual

1. Toggle Grid and References independently. Hiding one does not hide the other.
2. Pan/zoom repeatedly. Grid lines stay Origin-aligned, change density through the `1–2–5` sequence
   and never snap, select, guide or add history.
3. **Origin** recentres without changing zoom or sketch state. Fit frames native accepted geometry,
   excludes infinite datums and restores the canonical camera on an empty sketch.

Pass when the grid/camera improve orientation without acquiring sketch semantics.

## M75-U5 — carried M74-U5: coordinate HUD and contextual cursors

1. Move over empty canvas. HUD coordinates update smoothly and avoid negative-zero noise.
2. Enter/leave native and datum inference. The HUD shows the exact adjusted headless coordinate
   while retaining raw input as explanatory text; committed point and guide agree.
3. Switch among Select, drawing, relation and Fillet tools, then middle-drag pan. Selection,
   crosshair/relation and grabbing cursors appear and clear at the correct lifecycle points.

Pass when HUD and cursor communicate actual intent without inventing a second snap.

## M75-U6 — carried M74-U6: Undo/Redo respects editing ownership

1. Exercise `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` and Linux/Windows `Ctrl+Y` over canvas. Each performs
   exactly one appropriate history action.
2. Ctrl+Command and Alt-modified variants do nothing.
3. Inputs, selects, content-editable surfaces and open dialog/overlay fields retain their own
   keystrokes. Return focus to canvas and confirm history shortcuts recover without stale error or
   hover state.

Pass when standard shortcuts work without stealing editing input.

## M75-U7 — carried M74-U7: SVG letterbox bands are inert

At an aspect ratio with unused SVG bands, move, click, double-click and wheel in each band under
Select and active Line/Polyline. Hover, selection, camera, draft and history remain unchanged.
Valid mapped-plane and captured-gesture completion still work.

For this carried wording, "hover unchanged" means that a band never manufactures a new target.
M75-U11's stricter lifecycle rule applies when entering a band from an existing mapped-plane
hover: that stale Select target clears, while selection, camera, draft and history remain intact.

Pass when only the mapped sketch plane starts semantic input.

## M75-U8 — carried M74-U8: compact-desktop polish

At both required desktop sizes, repeat representative U1, U3, U4 and U6 paths. Check tree,
inspector, axis labels, HUD, camera controls, Problems and tool popouts for clipping or overlap.
Mobile/tablet remains outside scope.

Pass when the complete M74 treatment reads as one coherent desktop CAD demonstration.

## M75-U9 — hover predicts the shared primary owner

Build or load ordinary editable geometry that exposes the following overlaps. Test each side of the
overlap and the center of the shared envelope.

1. Put a current, applicable Fillet radius surface/grip over another eligible item. Hover must
   show the Fillet radius owner; pointer-down starts the same radius interaction. Moving outside
   its real hit surface allows the next eligible class to win.
2. Put a stored point and a visible semantic center over an annotation/curve/datum in separate
   cases. The draggable point/center receives hover and pointer-down before those lower classes.
3. Put a visible constraint and a visible dimension occurrence over native/computed geometry.
   The nearest annotation occurrence wins before the underlying non-draggable geometry, and click
   selects that exact relation/dimension.
4. Put native and computed geometry over an intrinsic axis. Geometry wins; move away from it while
   staying on the axis and the datum becomes the predicted/clicked owner.
5. Move to empty mapped canvas. No item highlights as primary and a plain click owns no semantic
   item. Related context may still appear only under U10's corridor rule.
6. Repeat representative cases with Shift/Ctrl/Command. Membership changes may differ, but the
   primary item under the pointer must not reorder.

Pass when hover is a reliable promise of the very next pointer-down owner throughout the exact
Fillet → draggable geometry → annotation → other geometry → datum → none order.

## M75-U10 — problem annotations, crowded ties and context-only corridors

1. Create an ordinary retained problem that forces a constraint/dimension annotation visible.
   Hover and click that occurrence: both identify the same problem-owned semantic item. Repair or
   remove the problem and confirm the no-longer-visible occurrence cannot retain an invisible hit.
2. Use a crowded relation/dimension cluster with multiple occurrences. Approach from several
   directions and repeat after pan/zoom away-and-back. Nearest occurrence choice remains stable;
   an apparent tie does not flicker between items or occurrences.
3. Enter a contextual geometry/annotation corridor while staying outside every visible glyph,
   value, curve, point and datum hit envelope. Related annotations/operands may reveal, but no
   primary hover target appears and clicking blank corridor does not select the revealed item.
4. Move from the corridor onto a real visible annotation occurrence. It becomes primary while the
   relevant context remains coherent; moving back returns to context with target none.

Pass when visibility and clickability agree, crowded choice is stable and contextual reveal never
masquerades as a target.

## M75-U11 — stale hover clears with ownership and browser paint

Acquire a clear primary hover before each step.

1. Switch away from Select and back. The old highlight clears immediately and does not return until
   a new valid canvas move.
2. Pan, zoom, Fit and use Origin. Camera motion clears the prior target/context; stationary old
   screen coordinates do not retain it against moved geometry.
3. Change selection, then Edit, Delete, Undo/Redo, load a sample/workspace and toggle the relevant
   visibility/problem state. The old prediction clears before changed annotation eligibility or a
   replacement scene paints.
4. Open a tool popout, dialog or other canvas overlay and move into it. Canvas hover clears while
   the overlay owns input. Closing it or returning focus does not resurrect hover without a new
   canvas move.
5. Leave/re-enter the mapped plane and letterbox bands. No stale browser-only CSS/SVG highlight
   survives when headless state reports none.
6. Activate Fillet authoring and move across points, annotations, geometry and datums before
   pressing. No Select hover appears while feature picking owns the canvas. Start a Fillet-radius
   drag from a prepared preview and confirm its captured movement still tracks until release.

Pass when every painted hover is visibly tied to the current headless tool/camera/scene/input
context.

## M75-U12 — zoom fringes and accessibility

1. At coarse and fine zoom, approach each owner class slowly from outside and cross both sides of
   its existing screen-space tolerance fringe. Capture feel remains stable in pixels; there is no
   new aggressive or unreachable zone. Repeat where two classes overlap and confirm precedence
   does not flip merely because zoom changed.
2. Tab through the tree, inspector, tool palette/popouts, Problems controls, dialogs and accessible
   Fillet controls. Existing accessible names and visible focus indicators remain; keyboard focus
   does not manufacture pointer hover or steal canvas selection.
3. With keyboard focus on a non-canvas control, confirm canvas hover is cleared. Return to canvas
   and move to reacquire. Escape/close behavior preserves the established Select/focus contract.
4. Inspect normal, hover, selected, related and problem states. Line weight/shape/focus treatment
   supplies a non-colour distinction; the new primary-hover truth is not communicated by colour
   alone.

Pass when the behavior stays predictable across scale and remains usable without conflating
pointer hover, keyboard focus, selection or problem state.

## Approval record

Carried deferred items:

- M74-U1 / M75-U1: **Pending**.
- M74-U2 / M75-U2: **Pending**.
- M74-U3 / M75-U3: **Pending**.
- M74-U4 / M75-U4: **Pending**.
- M74-U5 / M75-U5: **Pending**.
- M74-U6 / M75-U6: **Pending**.
- M74-U7 / M75-U7: **Pending**.
- M74-U8 / M75-U8: **Pending**.

New M75 items:

- M75-U9: **Pending**.
- M75-U10: **Pending**.
- M75-U11: **Pending**.
- M75-U12: **Pending**.
- Final M75 approval: **Pending**.

Do not mark any item passed from automated evidence alone. The exact nominated source/tree,
immutable manifest and endpoint are recorded above; add findings, any replacement
fix/requalification, supervising-human disposition and final public artifact only after those
events occur.
