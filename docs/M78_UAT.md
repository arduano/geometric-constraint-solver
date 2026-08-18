<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 focused UAT — CAD geometry tool families and authoring variants

Status: **approved on the clean-qualified, immutable M78-F011 replacement candidate; final GitHub
Pages publication remains pending**. On 2026-08-18 the supervising caller reported that the
candidate works correctly and requested milestone closure. U1-U8 and the focused F011 centre-drag
recheck are accepted under that explicit milestone-level decision.

The publication source will be the forthcoming documentation-only approval descendant that records
this decision. It does not replace or requalify exact gate-qualified product source
`793e9de39d78bdabfded15d8c8e79f86df0f52bc`.

Current replacement source: `793e9de39d78bdabfded15d8c8e79f86df0f52bc`

Current replacement tree: `9f74ec9b63955bfffdf2338fd1ab95ac8092856a`

Product fix: `e43aa8537f8d45533c2d445ea310f340aac5a530`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID/session: `3120501` / retained command-runner session `40375`

Immutable snapshot: `/tmp/geosolve-m78-f011-uat.MOsOFy` (directory `0555`, seven regular non-
symlink files `0444`)

Ordered-manifest aggregate:
`a51e76c2567d7e6c0352503cb3abeed23bddb7ecbd04e5c3d7acd1dd1d45fd97`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 23,484 | `bdbd0eaf11d96425b98d52f546417e3e4f7dbe50568568aca30d8fe34f01a30f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-5e889f68dd26a44a.js` | 33,333 | `99dc56d063d0397708890b9805612f2c22dc22445a899d105a848eaa3c3a5e73` |
| `geosolve-demo-web-5e889f68dd26a44a_bg.wasm` | 6,535,152 | `8dab4bb97047798e92bfc906694aa69d447e8ebf600d6cd83e3024ab3d770460` |
| `index.html` | 29,143 | `5ce14e955e0ac798a61b0f06a6cccdbd44f0b2308b2aed67674d30e8e3c7b76d` |
| `styles-a83e80383c7972df.css` | 35,731 | `cc0f03992191c1952bc4242fc951eac0e4c1d3a6bce0965a2290f2892cbe6572` |

The exact clean command
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from 11:07:08 through 11:19:06 AEST on 2026-08-18 and exited successfully without changing
candidate HEAD, tree or worktree. Its retained 251,148-byte, 3,280-line log is
`/tmp/geosolve-m78-f011-clean-gate.xNKJwu.log`, SHA-256
`d8ae7648a5c1426d5d275b0c2178df49a1793130d16532c6b36214ce0fb73fc6`. The gate includes
formatting/diff hygiene, warnings-denied workspace Clippy/Rustdoc, 1,734 passing locked all-feature
workspace tests with zero failures and three intentional ignores, core M16 47/47, core M10 34/34,
sketch lifecycle 26/26, sketch locality 5/5, M78 geometry variants 33/33, demo-web 143/143, carried
native/WASM parity, unchanged 270/270 clean golden authority, benchmark/performance budgets, the
149.39-second release sparse crossover, licence/package checks and Trunk 0.21.14 release assembly.

The gate-produced `dist` was copied without rebuilding, compared byte-for-byte and frozen above.
Freeze evidence is `/tmp/geosolve-m78-f011-freeze-evidence.gS2PTc`. The snapshot first passed
proxy-disabled, cache-bypassed, identity-encoded verification on temporary port `18081`. Only then
was withdrawn PID `1753616` retired and the current listener started on `8080`; temporary PID
`3116484` is also retired. Final requests for `/` and all seven files return HTTP 200 with zero
redirects, no content encoding, exact lengths, expected media types and snapshot-identical bodies.
`/` equals `index.html`. Final evidence is
`/tmp/geosolve-m78-f011-final-verify.yHlzj1/results.tsv`, SHA-256
`8e9ed63257499b6073d381bd02962d9c46d05cc52e84fa86917c4829347e86da`. This mechanically
nominates the replacement candidate; it does not accept any human scorecard row.

The withdrawn initial source `1b2ce0f9d843c036e3a7023674cbf219c9f593b7`, tree
`321ca280a5f581ee9755d615733617c98c0e21d7`, snapshot `/tmp/geosolve-m78-uat.SNgu3D` and aggregate
`803b539588fa2d462f154feded4a71b4c4b94a6fe2f6480b25af584b109ceba4` remain historical evidence
only and are no longer served.

Run the scorecard in the ordinary editable workbench at `1440x900` and approximately `1024x720`,
at coarse and fine zoom, using both Profile and Construction roles. Direct tests, not visual
judgment, own exact threshold equality, residual validation, persistent IDs and branch bytes.

## U1 — family palette, variant memory and stage language

Open each of the nine family buttons and verify the exact 25 variants in `docs/M78_GOALS.md`.
Choose a non-default variant, blur the overlay, pan/zoom and click the canvas; it should stay open
and remember the variant. Switch families and return; session memory should restore the last valid
variant/options. The main icon should remain centred and should not expose a separate chevron.

Start every variant and compare its prompt, stage markers and live readout with the gesture being
performed. Width/height, `R`/`Ø`, sweep and control-count language should be concise and truthful.
Closing the overlay should activate/focus Select. A successful shape should leave its exact variant
active for repeated creation.

## U2 — points, segments, polylines and midpoint lines

Place one free Sketch Point, then confirm the tool over an existing persistent point. The second
action should reuse that identity as a history-neutral no-op rather than allocate a duplicate.
Create a Segment with one reused endpoint and ordinary directional inference. Create an open
Polyline finished by Enter and another by double-click; create a closed Polyline by clicking its
first vertex. Closure must not show or persist a doubled first point or zero-length final edge.

During a Polyline, use Backspace and then Undo to remove only the latest unfinished vertex. Confirm
that accepted history is unchanged until the draft is empty. Press Escape once to cancel the
current chain while staying in Polyline, then Escape again to return to Select.

Create a Midpoint Line from a free centre and from an existing point. Move an endpoint afterward:
the retained centre should remain the segment midpoint through an ordinary visible relation, not a
hidden lock or dimension.

## U3 — four rectangle recipes and Shift squares

Create all four rectangle variants in ordinary mode, then repeat each while holding Shift. Inspect
the tree and Problems/constraint presentation:

- every result has four explicit shared-corner line edges;
- aligned variants remain aligned and three-point variants retain their orientation;
- centre variants show one ordinary Construction helper diagonal and a centre Midpoint relation;
- no rectangle creates a lock, driving/reference dimension or target scalar; and
- every Shift result retains one EqualLength square intent after release, drag, Undo/Redo and
  reload.

Repeat one Shift rectangle while holding Ctrl/Cmd. Ambient snapping should be suppressed, but the
intrinsic square and rectangle relations must remain. Approach conflicting ambient H/V guidance;
the recipe's own alignment/shape must win without a failed placement or stale global problem. On an
oriented rectangle, deliberately accept a compatible horizontal/vertical baseline guide and confirm
that this useful ambient orientation remains alongside the recipe's perpendicular/parallel intent.

## U4 — circles and arcs

Create Center–Radius, 2-Point Diameter and 3-Point Circle examples. For diameter and three-point
recipes, snap some rim samples to existing points and leave others free. Existing points should
receive visible curve incidence while free samples should not create synthetic tree points.
Three coincident or nearly collinear samples should keep a correction-ready draft and a local
message; moving the last sample to a valid position should recover without Escape or reload.

Create Center Arc and use `F` before release to compare complementary sweeps. Create a 3-Point Arc
and confirm it passes through the ordered Through sample with the intended Start/End span. Existing
snapped trim/rim points should remain associative without new synthetic endpoint objects.

Create Tangent Arcs from eligible endpoints of several native open families and from both endpoint
directions. The preview and committed arc should leave the source smoothly with a visible ordinary
tangency relation. Try an interior point, periodic curve, zero-length chord and near-straight
infinite-radius case; each should be unavailable or locally recoverable, never accepted as
non-finite geometry or a stale global failure. Move an eligible source endpoint between attempts and
confirm the next preview follows its current position/tangent rather than a cached jet; deleting an
attempted source should leave the draft recoverable through Backspace/Escape rather than blanking the
scene.

For the F011 targeted recheck, create a source circular arc and place a Tangent Arc from its End so
the document contains exactly the intended generic tangency and no lock/dimension. In Select, drag
the source-arc centre diagonally, then Undo and drag the created Tangent-Arc centre diagonally. Both
gestures must show and commit a live finite preview rather than appearing locked; the other centre
may move as required by tangency. The join must retain its endpoint contact, aligned tangent
orientation and both arc sweeps, with no extra constraint or dimension appearing. This check does
not require the tangency-owned source-End or created-Start trim grip itself to move.

## U5 — ellipses, Béziers and conics

Create both full-ellipse variants and both elliptical-arc variants. Centre-based and axis-endpoint
forms should communicate the same centre/major/minor frame with different input order. Arc Start
and End samples must land on the displayed support ellipse. Use `F` to flip the complementary
sweep without exchanging endpoint identity. No browser-side jump, axis swap or numeric Start/End
construction field should appear.

Create quadratic and cubic Béziers and confirm the stage markers distinguish endpoints from
controls. Create Rational Quadratic, Parabola and both Hyperbola branch choices. Their family
overlays should preserve the established M77 ordinary/projective middle meaning, trim/domain
options and explicit branch state; moving them into grouped menus must not change accepted geometry
or later curve-handle editing.

## U6 — open and periodic control NURBS

Create an Open Control NURBS using enough controls for the chosen degree, remove one unfinished
control with Backspace, then finish with Enter. Repeat with double-click. Create a Periodic Control
NURBS and confirm its closure is explicit periodic topology rather than a duplicated last control
or proximity guess.

Try finishing too early and enter invalid degree/knot/weight options. The active overlay should
explain why finishing is unavailable and preserve the draft/options for correction. Switching to
another family should remain possible; invalid inactive NURBS fields must not block unrelated
geometry.

## U7 — modifiers, inference cycling and recovery

On representative Segment, rectangle, circle/arc and spline stages, hold Ctrl/Cmd and confirm that
ambient guides/adjustment disappear for that sample without changing intrinsic recipe relations.
Where several compatible inference candidates are published, use Tab to cycle them and confirm the
preview, guide and eventual relation agree.

For one fixed-length and one variable-length recipe, exercise stage Undo/Backspace, first/second
Escape, tool switch, overlay close and a deliberately invalid terminal sample. No cancelled or
rejected attempt may enter accepted history, reuse a retired persistent identity, blank the scene
or leave a global error after correction/Undo. One successful complete recipe must be exactly one
Undo/Redo step regardless of its stage count.

The direct regressions own exact publication acknowledgement, proposal work counters, allocator
high-water and legacy `auto point-on-curve contact N` bytes. Human review should still confirm their
observable consequence: a rejected/exhausted attempt never reports success, clears a successful
shape only after it is visibly published, and never leaves partial geometry or a stale notice.

## U8 — role, persistence and desktop polish

Author representative variants with Profile active and with Construction active. Main curves
should follow the active role; centre-rectangle helpers should always remain Construction. Save and
reload, then use reproduction copy/restore. Geometry, roles, relations, branch state, variant
results and accepted history should survive through ordinary persisted document state, while the
session-only last-used palette variant may reset without corrupting the scene.

At both desktop sizes and zoom ranges, verify family overlays remain contained, stage prompts do
not cover the active geometry, keyboard focus/accessibility names are meaningful, and hover/click
feedback remains consistent with the exact next accepted operand. Tab focus must not synthesize
canvas hover and canvas movement must not steal overlay focus. Every visible live measurement must
remain finite and truthful; a derived readout that cannot be represented should be absent rather than
displaying NaN or infinity.

## Acceptance record

- U1 — family palette, variant memory and stage language: accepted under the supervising caller's
  milestone-level approval
- U2 — points, segments, polylines and midpoint lines: accepted under the supervising caller's
  milestone-level approval
- U3 — four rectangle recipes and Shift squares: accepted under the supervising caller's
  milestone-level approval
- U4 — circles and arcs: accepted under the supervising caller's milestone-level approval
- U5 — ellipses, Béziers and conics: accepted under the supervising caller's milestone-level
  approval
- U6 — open and periodic control NURBS: accepted under the supervising caller's milestone-level
  approval
- U7 — modifiers, inference cycling and recovery: accepted under the supervising caller's
  milestone-level approval
- U8 — role, persistence and desktop polish: accepted under the supervising caller's milestone-
  level approval
- M78-F011 — source and created Tangent-Arc centre drag targeted recheck: passed; the caller reports
  that the replacement behaves correctly
- Final supervising approval: passed on 2026-08-18; the caller accepted the current replacement and
  requested milestone closure
- Documentation-only approval-descendant Pages publication and hosted-byte verification: pending

This is explicit milestone-level acceptance of the current replacement. It does not invent a
separately logged row-by-row replay beyond the focused defect checks and interaction review
reported by the supervising caller; exact mathematical, lifecycle, authority and persistence
behavior remains owned by the qualified native/WASM regressions above.
