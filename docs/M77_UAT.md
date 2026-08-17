<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 focused UAT — CAD curve handles and implicit parameters

Status: **replacement accepted for closeout (2026-08-17)**. UAT findings M77-F012 and M77-F013
supersede the initial candidate. Their corrections and review follow-ups pass the complete clean
gate, immutable freeze and served-byte verification below. The supervising caller explicitly
approved the current replacement and requested milestone closure; GitHub Pages publication and
hosted-byte verification remain pending.

## Current replacement candidate

Product source: `cc99b11071dc62732e02b630ba7a1381d754b04c`

Candidate tree: `3315a2bdd0137f59657ea2500962ef971a23ea15`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID/session: `284248` / retained command-runner session `5213`

Immutable snapshot: `/tmp/geosolve-m77-uat.ARrQFw` (directory `0555`, seven regular non-symlink
files `0444`)

Ordered-manifest aggregate:
`abfa7ef6b75f127fa6d93ff6ad6960c7f5df7d4c799a578c785e1192c2b7ee94`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 23,216 | `841936f73b5d21fbee999ec2bc4140ae0869cd2821429816e3766bd026ad771b` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-74621b73a35eab86.js` | 33,221 | `9f28eed1331a570a1fa894f16834a40be0593ef9bd673ca80db8fbea4017eef1` |
| `geosolve-demo-web-74621b73a35eab86_bg.wasm` | 6,426,513 | `1c4701e10d4ca672b0aa2511ff3fc4067be5c03965274de4925a711b5414e3f1` |
| `index.html` | 28,940 | `f3740f54742d6895e204cc41c08e031d0f2b639e6dd30df30c3e08b1b878527d` |
| `styles-d7435a6d60dc3430.css` | 34,689 | `870bde7d758fe95f4323bedc6588ff2cffaf3c826549e684718ebfd818eebcd6` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` ran from 18:21:03 through
18:33:22 AEST and exited successfully without changing HEAD, tree or worktree. Its retained
243,128-byte log is `/tmp/geosolve-m77-replacement-clean-gate.cc99b11.log`, SHA-256
`0da2456b69951a50ba41cfe939f37764fcc211f2af45f4b9526cd5c974829301`. The gate includes formatting
and diff hygiene, warnings-denied workspace Clippy/Rustdoc, every locked all-feature workspace test,
unchanged 270/270 clean golden, native/WASM M70/M71/M74/M75/M76/M77 parity, demo WASM, benchmark
compilation, M14/M32 workloads, the 138.34-second 256-body sparse crossover, licence/package checks
and Trunk 0.21.14 release assembly.

The gate-produced `dist` was copied without rebuilding, compared byte-for-byte and frozen above.
Freeze evidence is `/tmp/geosolve-m77-replacement-freeze-evidence.2kfhjk`. Proxy-disabled,
cache-bypassed, identity-encoded requests for `/` and all seven files return HTTP 200 with zero
redirects, no content encoding, exact expected media types/lengths and snapshot-identical bytes.
`/` equals `index.html`, and the fetched manifest has the same aggregate. HTTP evidence is
`/tmp/geosolve-m77-replacement-http-verify.yxgjkL`. Withdrawn PID `3912158` stayed live until the
replacement snapshot was ready and is now retired. The evidence-ledger commit is a documentation
descendant; exact source `cc99b11` remains the mechanically qualified product authority.

## Superseded initial candidate

Historical source: `51a3b95d04f27216c164febf0808a180b6775537`

Candidate tree: `8d154a147a08c7d6bc79008f19b74311cd60905a`

These bytes are no longer served. Do not use them for current UAT.

Historical Tailscale endpoint: `http://100.94.63.83:8080/`

Former server PID/session: `3912158` / command-runner session `12828` (retired)

Immutable snapshot: `/tmp/geosolve-m77-uat.1mDjQv` (directory `0555`, seven regular non-symlink
files `0444`)

Ordered-manifest aggregate:
`af7c2fbca1a6481c8c055142c9a64578b570fbcb297f687f09cc8ffc85bd1b8b`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 23,124 | `f72319b7b0b0364c5ebcf3921e34d0a706f459771a182ce65b16918a805ea07e` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-be28d696c867467c.js` | 33,221 | `d5fe43e24f03ebc2301c9d560520220b4d6d1f5a43ee75400c640da8dba9cb6f` |
| `geosolve-demo-web-be28d696c867467c_bg.wasm` | 6,412,353 | `943a8bb78cc6d6d0883c9628e537b143c7703c409e975c4ce8af343391823bc1` |
| `index.html` | 29,020 | `071852cb148efae46d4ace99a83ed2595e55f4677a0407765572b1685b7dd070` |
| `styles-d7435a6d60dc3430.css` | 34,689 | `870bde7d758fe95f4323bedc6588ff2cffaf3c826549e684718ebfd818eebcd6` |

The exact clean command
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` ran from 15:48:17 to
16:00:06 AEST and exited successfully without changing HEAD, tree or worktree. Its log is
`/tmp/geosolve-m77-clean-gate.51a3b95.log`, SHA-256
`e28c50101df3b9c447ccf1a392f0e3e5644068e8abc460be389aaf3cff1984ed`. The gate includes formatting
and diff hygiene, warnings-denied workspace Clippy/Rustdoc, every locked all-feature workspace
test, unchanged 270/270 clean golden, native/WASM M70/M71/M74/M75/M76/M77 parity, demo WASM,
benchmark compilation, M14/M32 workloads, the 150.55-second 256-body sparse crossover,
licence/package checks and Trunk 0.21.14 release assembly.

The gate-produced `dist` was copied without rebuilding, compared byte-for-byte and frozen above.
Freeze evidence is `/tmp/geosolve-m77-freeze-evidence.qbBmc5`. Proxy-disabled, cache-bypassed,
identity-encoded requests for `/` and all seven files return HTTP 200 with zero redirects, no
content encoding, exact expected media types/lengths and snapshot-identical bytes. `/` equals
`index.html`, and the fetched manifest has the same aggregate. HTTP evidence is
`/tmp/geosolve-m77-http-verify.eu1KMY`. Superseded M76 PID `1780608` exited before this listener
started; its immutable snapshot remains unchanged historical evidence.

M77-F012 reproduced the reported blank/no-op drag at the ordinary retained browser-composition
boundary. M77-F013 records the approved spatial elliptical-arc authoring enhancement. Review then
resolved signed/both-pole minor-axis crowding, stored-major-axis ownership, exact-shift rail loss
and stale candidate-generation authority as M77-F014 through M77-F016. None changes a solver
equation or expands the golden inventory.

Run the scorecard in the ordinary editable workbench at `1440x900` and approximately `1024x720`,
at coarse and fine zoom. Use a mouse or equivalent precise pointer. Repeat crowded targets on both
sides of their hit fringe; direct tests, not human judgment, own exact boundary equality.

## U1 — visibility, ownership and visual language

Select, deselect and reselect examples of circle, circular arc, ellipse, elliptical arc, rational
quadratic conic, parabola, hyperbola, Bezier, B-spline and NURBS.

- Only selected editable curves should show their applicable handles/cage; deselection, another
  tool and a different sample/document should remove them immediately.
- The cage should be quiet but legible: endpoint roles, stored controls, middle control and scalar
  size rails should be visually distinguishable without relying only on colour.
- Hover each visible handle and then click at the same location. Highlight, tooltip/cursor and
  pointer action must name the same role and owning curve. Underlying curve or annotation paint
  must not steal the click; stored control points must retain their ordinary point ownership.
  In particular, acquire a stored centre/start point exactly where a radius, axis or projective
  guide begins: the point must own the inner grip region, while the guide remains selectable just
  beyond that region.
- In separate elliptical-arc examples, put Start or End at the positive major pole. The stored
  major-axis point must own the physical pole, while the offset trim square remains independently
  hoverable/clickable with matching role feedback.
- Selecting a derived endpoint or size handle must select its curve and must not create a point in
  the tree, persistence payload or constraint operand list.
- An active Fillet-owned output arc must keep its Fillet affordance and expose no competing generic
  radius/endpoint handles.

## U2 — trim endpoints

Create a circular arc as Centre, Start, End. Create an elliptical arc as Centre, Major axis, Start,
End. The latter must show a support ellipse after the axis click, project both spatial trim clicks
onto it, keep the selected sweep direction explicit and finish without numeric Start/End inputs.

For a circular arc, elliptical arc, parabola segment and hyperbola segment, drag Start and End
independently. Begin off-centre inside each handle to check that it preserves the grab offset and
does not jump. Make a sub-3 px movement and release; it should select without editing or adding an
Undo step. Then cross the threshold and commit a visible change.

The endpoint must remain on its exact support curve while the opposite endpoint and support
controls behave consistently. Start and End must not silently exchange. Circular/elliptical sweep
and hyperbola branch must not flip when an endpoint passes near a wrap, axis or invalid crossing.
Inspect coarse and fine zoom for stable hover/click parity and no screen-space drift.

## U3 — rational and stored control cages

Select rational quadratic conics with positive and negative nonzero weights. Drag `P1` and confirm
that the control cage and curve update continuously while both stored endpoints and the numeric
middle weight remain unchanged. Undo and Redo once each. The middle handle is a control point, not
a promise that the curve passes through it; tooltip and inspector language should make that clear.

Create a non-unit-weight rational curve and confirm its construction click and later handle mean
the same `P1`. Load or enter a zero-weight curve and confirm it exposes an explicitly labelled
projective `Qh` vector instead of an ordinary point; entering and leaving that mode must be
deliberate, finite and free of division-by-zero jumps.

Select quadratic/cubic Bezier, B-spline and NURBS examples. Their stored controls should remain
ordinary draggable points, with a selected control polygon that makes influence understandable.
NURBS and rational weights should remain available as precise numeric inspector controls without a
second ambiguous spatial weight rail. A NURBS gauge row is read-only, and “Make gauge” must change
the numeric normalization without moving the curve.

## U4 — size handles and domains

Exercise circle and circular-arc radius, ellipse and elliptical-arc minor axis, and hyperbola
semi-conjugate size. The handle should follow a clear deterministic rail, preserve the initial grab
offset and update the intended scalar without moving unrelated stored controls.

Approach and cross each domain boundary: zero radius, zero semi-conjugate size, zero minor ratio and
a minor ratio greater than one. No non-finite, negative or family-swapped geometry may appear. An
invalid sample should retain the last valid finite preview; returning to a valid pointer location
should resume normally. Releasing while invalid should publish only the exact last valid candidate,
or no edit if no changed valid candidate ever existed. Existing driving/locked ownership should be
reported honestly rather than accepting a contradictory handle move.

Create a half ellipse whose trims occupy both minor poles. The Start, End and shifted minor-size
controls must all remain separately hoverable/clickable, and the shifted size grip must keep its
rail at coarse, fine and approximately 16 px/model-unit zoom.

## U5 — cancellation, stale work and history

For endpoint, middle and size gestures, cancel independently with Escape, pointer-capture loss,
tool change and camera change. Each must restore the exact pre-gesture accepted scene and add no
history entry. Start another gesture, then trigger an accepted-scene replacement or Undo from the
owning workbench path; an old preview/result must not reappear or commit afterward.

Commit one valid gesture. It must add exactly one Undo step regardless of preview sample count.
Undo must restore the complete pre-drag curve and Redo the exact final candidate, including trim,
sweep/branch and control values. A rejected or unchanged gesture must add none. Problems text,
selection and hover must describe the current scene rather than a stale preview.

For circular and elliptical arcs, drag centre, stored axis, trim and size controls. The scene must
never blank during movement; every visible release must persist, then Undo and Redo exactly once.
Move through at least two accepted preview positions before release: an older rendered candidate
must never commit the newer unseen position.

## U6 — persistence and desktop polish

Commit representative endpoint, rational-middle and size edits, save/reload the workspace and use
the reproduction copy/restore path. The curve geometry and existing scalar/weighted-middle values
must round-trip; transient handles should be recomputed only after selection and no handle cache or
synthetic point should persist.

At both desktop sizes and zoom ranges, check that handles remain easy to acquire without becoming
visually aggressive, control cages do not obscure annotations, tool popouts remain usable, and
keyboard focus/accessibility names stay meaningful. Tab focus must not synthesize pointer hover,
and canvas hover must not steal focus.

## Acceptance record

- U1 — visibility, ownership and visual language: accepted under the supervising caller's scoped
  approval
- U2 — trim endpoints: accepted under the supervising caller's scoped approval
- U3 — rational and stored control cages: accepted under the supervising caller's scoped approval
- U4 — size handles and domains: accepted under the supervising caller's scoped approval
- U5 — cancellation, stale work and history: accepted under the supervising caller's scoped approval
- U6 — persistence and desktop polish: accepted under the supervising caller's scoped approval
- Final supervising approval: passed on 2026-08-17; the caller approved the current replacement and
  requested closure
- GitHub Pages publication and hosted-byte verification: pending

This is an explicit milestone-level acceptance of the current replacement. It does not claim a
separately logged row-by-row replay beyond the focused defects and interaction checks reported by
the supervising caller; exact boundary, lifecycle and stale-authority behavior remains owned by the
qualified native/WASM regressions above.
