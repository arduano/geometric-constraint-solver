<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 focused UAT — hover and primary pointer-owner parity

Status: **complete (2026-08-16); the post-F002 replacement and focused F001/F002 hover recheck are
accepted for scoped closure and exact-verified on GitHub Pages**.
The caller also accepted U1-U12 for scoped closure, but no separate step-by-step transcript was
logged and this record does not invent one. The exact M75 public artifact below is final authority.

Withdrawn initial candidate source: `f3affff1b62b1cb484a59647c4072c94c3b12ada`

Withdrawn initial candidate tree: `7662abc8b7c71130f54fbf2745afa60f0d286431`

Historical initial snapshot endpoint: `http://100.94.63.83:8080/` (no longer serving these bytes)

Historical initial server PID: `3801058` (exited; retained command-runner session `47845`)

Withdrawn immutable snapshot: `/tmp/geosolve-m75-uat.hUSaG7` (directory `0555`, files `0444`)

Withdrawn ordered-manifest aggregate:
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

The initial candidate's exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 at the candidate
source. It includes formatting, warnings-denied workspace Clippy, locked all-feature tests,
unchanged 270/270 golden `--require-clean`, native/WASM M75 9/9 parity, Rustdoc, benchmark
compilation, M14/M32 performance, the 138.09-second 256-moving-body crossover,
licensing/package checks and Trunk 0.21.14 release assembly. The gate output was copied without
rebuilding and contains exactly seven regular non-symlink files.

Proxy/cache-bypassed identity requests for all seven files and `/` return HTTP 200 with exact media
types, lengths and bytes, no redirect or content encoding; `/` equals `index.html`, and the fetched
aggregate matches. Evidence is retained at `/tmp/geosolve-m75-http-verify.sQGN1B`. The unchanged
M72 compatibility and M74 Chromium scripts pass at `1440x900` and `1024x720`; their SHA-256 values
are `4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`. These mechanical checks do
not dispose any human scorecard item.

M75-F001 invalidates this initial candidate for hands-on review: native relation/dimension and
Fillet clicks work, but uncaptured authoring movement publishes no target. Direct Rust/WASM tests
own the corrected exact precedence, distance comparisons, boundary equality, mutation freedom and
invalidation.

### Superseded M75-F001 replacement candidate

Withdrawn replacement source: `57f407ada2eb8a16f8162d1db4126d5c5024f1b4`

Withdrawn replacement tree: `7bff59c5d4d36d1acb687a93d78707b32e323d65`

Served historical endpoint: `http://100.94.63.83:8080/`

Historical server PID: `4026985` (retired)

Server log: `/tmp/geosolve-m75-f001-uat.2Ju7gq.server.log`

Withdrawn immutable snapshot: `/tmp/geosolve-m75-f001-uat.2Ju7gq` (directory `0555`, files `0444`)

Withdrawn ordered-manifest aggregate:
`9ecf1dde82ca777ae8de6dc380606512008b3bf088808e995fd0c4b2b8896967`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 18,270 | `b2c503a0ca2ad33c0fcc137666a349a773630fb712a4cdd50f8fea64454614d0` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-41f4150de02af486.js` | 33,221 | `39eebb2d778b7470d0b2bd552ab7716cb12e38fe072bca05905e1f936fc81f09` |
| `geosolve-demo-web-41f4150de02af486_bg.wasm` | 6,117,357 | `cc194398055211d420a82b058fb83cf3d3e2e54bcded5c6c5116cca086be3d7d` |
| `index.html` | 27,478 | `fa50308533c8a98f2c8f37b63a72414ddba2f33d9a2f4339157779a7a2e875bc` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 on the source
above. M75 parity passes 11/11 natively and under WASM, demo-web passes 116/116, the reviewed golden
remains unchanged at 270/270, and the sparse crossover completes in 143.27 seconds. The exact
gate-produced seven files were frozen without rebuilding.

Proxy/cache-bypassed identity requests for `/` and every file return HTTP 200 with exact media
types, lengths and bytes, no redirect or content encoding; `/` equals `index.html` and the fetched
aggregate matches. Evidence is retained at `/tmp/geosolve-m75-f001-http-verify.kXc5g5`. The
unchanged M72 and M74 Chromium scripts pass at both desktop sizes; their SHA-256 values are
`4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`.

M75-F002 withdraws this replacement from hands-on review. In `fillet-workshop`, collecting point
`6600000000000000000000000000004f` and curve `66000000000000000000000000000038`
creates a computed-radius grip overlap where a native point paints above the correct
`FeatureCorner`. The top-target-only adapter supplied native curve
`66000000000000000000000000000052`; pressing without moving destroyed the preview and captured no
radius gesture. The corrected provisional build enumerates the complete paint stack only for
uncaptured Fillet authoring, reconciles the exact headless `SceneFilletHit::Radius` owner through
one move/down helper, otherwise retains the top painted item without promoting an owner, and leaves
the coordinator as final authority. Demo-web 117/117, native/WASM M75 11/11, focused Clippy, WASM,
formatting, diff and unchanged-golden checks pass. `/tmp/m75_f001_browser_check.mjs`, SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`, passes 6/6 on that
provisional local build.

Independent adapter review also found the visible radius rail and spoke lacked an extractable
`FeatureCorner` identity. The corrected radius-affordance group now supplies that same owner to the
grip, rail and spoke; focused presentation coverage freezes all three surfaces, and the provisional
browser run exercises visible spoke and rail hover/capture/release.

### Accepted post-F002 candidate

Accepted product source: `553fd912730b1de3b39736c49b669e94cabdd2c3`

Accepted product tree: `83df4efb99ca66cf0cebc0caec4515b61afd33cf`

Historical accepted endpoint: `http://100.94.63.83:8080/` (now serving M76 bytes)

Historical server PID: `37152` (retired before the M76 nomination)

Server log: `/tmp/geosolve-m75-f002-uat.hlSQYT.server.log`

Immutable snapshot: `/tmp/geosolve-m75-f002-uat.hlSQYT` (directory `0555`, files `0444`)

Ordered-manifest aggregate:
`eae64913c29d760f6eb64d7681212facca0c6d8869dee9631aeb9d77b059a139`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 18,564 | `b99a56b9c1aa8679538726c95b1ed29729174ff2945a44be1ea07b08d6f22cf2` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-35551745a5e20011.js` | 33,221 | `ade1f75e65ca2636f29259c7b3716d375e0b3886a6ba1bdf61817686b2dad2d2` |
| `geosolve-demo-web-35551745a5e20011_bg.wasm` | 6,117,030 | `9d01af2fee2d7ce3884020579187037eb617fe73ede243e491842ba044adf9dc` |
| `index.html` | 27,478 | `9bff14da5388601e8d48a175e65c033141f383736fcd9da4065350eb9baebf33` |
| `styles-5ae33f7d5d5aaecf.css` | 30,672 | `54e768998dbc7ba1bac4da87b5b48feac14abe214448790afade36fa42990fb4` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` exited 0 in 480.94 seconds
on the source above. Demo-web passes 117/117, M75 parity passes 11/11 natively and under WASM, the
reviewed golden remains unchanged at 270/270, the sparse crossover completes in 141.82 seconds and
Trunk release assembly passes. The seven files above are the gate output frozen without rebuilding.

PID `37152` served the exact argv before retirement:

```text
/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m75-f002-uat.hlSQYT
```

Old PID `4026985` is retired. Proxy/cache-bypassed identity requests for `/` and all seven files
return HTTP 200 with exact media types, lengths and bytes, no redirect or content encoding; `/`
equals `index.html`, and the fetched aggregate matches. Evidence is retained at
`/tmp/geosolve-m75-f002-http-verify.1nRxtz`.

The unchanged M72 and M74 browser scripts pass over Tailscale at both desktop sizes; their SHA-256
values are `4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`. M75 script
`/tmp/m75_f001_browser_check.mjs`, SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`, passes 6/6 on the same
frozen bytes, including native authoring and grip/spoke/rail hover/capture/release.

This evidence made the snapshot ready for human UAT; mechanical evidence alone did not pass a
scorecard item. The supervising caller subsequently reported the candidate looking good and
authorized closure. The scoped approval below accepts the current candidate, focused F001/F002
hover recheck and U1-U12 without claiming a separately recorded observation for every prepared
step. At that approval checkpoint, GitHub Pages remained accepted M74 authority until M75
publication and exact verification.

### Final public authority

Documentation-only approval descendant `f80235978fbcdccd58c45a08bccf3969a20110c9`, tree
`eb05b6496aa5c761e005a40da78d8fb96e84c16a`, deploys accepted product source
`553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`, through successful Pages run `31939764951`, artifact
`9261974799` and deployment `5929879555`. The hosted complete gate, repository-prefixed build and
deploy pass. ZIP/tar SHA-256 values are
`8c031953dec4975c9b701a5ba30f060a95d5e0772286396f3c03ac74fb665fc0` and
`8ac419fbea39c306e6ee529309f2d3965c93d4ff0459fd2e21179714e9b89c1d`; the exact seven-file
manifest and `4c2da7d7860ac0bcadc64722007b5accb01aa999aa79f3046ba9d2868e86ef3b` aggregate are recorded
in `docs/M75_IMPLEMENTATION.md`.

The public root and every artifact path return HTTP 200 with zero redirects and match the artifact
byte-for-byte; `/` equals `index.html`, asset URLs are repository-prefixed and media types are
correct. M72/M74 checks pass at both desktop sizes and M75 passes 6/6 on the public URL. Evidence is
retained at `/tmp/geosolve-m75-pages-verify.NkQwem`. GitHub Pages is final public-byte authority;
the frozen Tailscale snapshot remains accepted candidate evidence but is no longer served at the
shared endpoint.

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
   pressing. Only the exact applicable native point/curve operand highlights; unrelated Select
   annotation/datum ownership does not leak into authoring. Start a Fillet-radius drag from a
   prepared preview and confirm its computed owner highlights before the press and its captured
   movement still tracks until release.

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

## M75-F001/F002 targeted recheck — active authoring hover predicts click

Run this first on the replacement candidate. At minimum repeat steps 1–4 at both required desktop
sizes; use coarse and fine zoom for the overlap case.

1. Open **2D Fillet workshop**, activate Fillet, and hover an applicable native corner point. The
   point gains the primary hover treatment before clicking; clicking without moving consumes that
   same corner and advances/previews the Fillet collector.
2. Restart Fillet and hover the middle of an applicable native line. The line highlights before
   the press, and clicking without moving makes that same line `authoring-pending`.
3. Use an overlap where the visually nearer item is not valid for the current stage but an
   underlying line is valid. Hover and unchanged click both choose the applicable line. Empty or
   wholly inapplicable geometry shows no primary highlight.
4. Repeat representative ordinary authoring tools: Coincident over a point, Horizontal/Vertical
   over a line, and one dimension tool over its valid operand. Each hover predicts the operand;
   wrong-kind geometry does not highlight and cannot suppress an applicable fallback.
5. In **2D Fillet workshop**, collect point `6600000000000000000000000000004f` and curve
   `66000000000000000000000000000038`, then hover the interactive computed-radius grip where native
   paint overlaps it. The exact computed corner highlights even when a native point is painted
   above it; native curve `66000000000000000000000000000052` must not become the promised owner.
   Press without moving: the same radius gesture captures, the preview is retained and subsequent
   movement changes the radius. Repeat on the visible rail, spoke and an isolated computed
   arc/radius surface; every pointer-active radius surface must preview/capture the same corner, and
   native geometry beneath or above a current painted preview cannot steal the authenticated owner.
6. Move between point, line and empty canvas, then switch tools or leave the canvas. Highlighting
   updates or clears immediately. Once a radius drag captures the pointer, movement continues to
   the matching release without being rerouted to native authoring hover.

Pass when active authoring hover is a truthful, restrained preview of the exact next click target
and never looks like an unrelated Select-mode highlight.

## Approval record

Carried deferred items:

- M74-U1 / M75-U1: **Pass for scoped closure under supervising approval**.
- M74-U2 / M75-U2: **Pass for scoped closure under supervising approval**.
- M74-U3 / M75-U3: **Pass for scoped closure under supervising approval**.
- M74-U4 / M75-U4: **Pass for scoped closure under supervising approval**.
- M74-U5 / M75-U5: **Pass for scoped closure under supervising approval**.
- M74-U6 / M75-U6: **Pass for scoped closure under supervising approval**.
- M74-U7 / M75-U7: **Pass for scoped closure under supervising approval**.
- M74-U8 / M75-U8: **Pass for scoped closure under supervising approval**.

New M75 items:

- M75-U9: **Pass for scoped closure under supervising approval**.
- M75-U10: **Pass for scoped closure under supervising approval**.
- M75-U11: **Pass for scoped closure under supervising approval**.
- M75-U12: **Pass for scoped closure under supervising approval**.
- M75-F001 targeted recheck: **Pass** on the accepted post-F002 candidate.
- M75-F002 targeted recheck: **Pass** on the accepted post-F002 candidate, including the
  grip/spoke/rail correction.
- Final M75 approval: **Pass for scoped closure** — the supervising caller reported “Looking good”
  and authorized closure on 2026-08-16.
- Exact GitHub Pages publication and hosted-byte verification: **Pass** — run `31939764951`,
  artifact `9261974799`, deployment `5929879555`, with all public bytes and browser matrices
  verified.

These dispositions come from explicit supervising-human approval, not automated evidence alone.
The detailed U1-U12 steps were not individually logged, so their scoped passes must not be read as
a step-by-step execution transcript. The exact accepted source/tree, immutable manifest and
endpoint remain recorded above. The final public artifact is recorded separately because its
repository-prefixed assembly differs from the frozen Tailscale candidate.
