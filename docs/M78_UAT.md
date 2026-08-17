<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 focused UAT — CAD geometry tool families and authoring variants

Status: **open; implementation, complete clean release qualification and immutable Tailscale
nomination pass**. No U1-U8 scorecard item or final supervising approval is accepted. GitHub Pages
publication must not start before that explicit approval.

Candidate source: `1b2ce0f9d843c036e3a7023674cbf219c9f593b7`

Candidate tree: `321ca280a5f581ee9755d615733617c98c0e21d7`

Tailscale endpoint: `http://100.94.63.83:8080/`

Server PID/session: `1753616` / retained command-runner session `76097`

Immutable snapshot: `/tmp/geosolve-m78-uat.SNgu3D` (directory `0555`, seven regular non-symlink
files `0444`)

Ordered-manifest aggregate:
`803b539588fa2d462f154feded4a71b4c4b94a6fe2f6480b25af584b109ceba4`

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 23,484 | `bdbd0eaf11d96425b98d52f546417e3e4f7dbe50568568aca30d8fe34f01a30f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-59998f0c1a23e0f9.js` | 33,333 | `99dc56d063d0397708890b9805612f2c22dc22445a899d105a848eaa3c3a5e73` |
| `geosolve-demo-web-59998f0c1a23e0f9_bg.wasm` | 6,535,148 | `0441c6fc9e931d0fe75358ac24d6f78b465008a04792475547840f9003699ae1` |
| `index.html` | 29,143 | `1598ad7ce70d892496a55a3ea86b45ceb23fbbf9763278993f1e79f4cb5974d5` |
| `styles-a83e80383c7972df.css` | 35,731 | `cc0f03992191c1952bc4242fc951eac0e4c1d3a6bce0965a2290f2892cbe6572` |

The exact clean command
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from 02:14:05 through 02:26:24 AEST on 2026-08-18 and exited successfully in 12m19s without
changing candidate HEAD, tree or worktree. Its retained 250,089-byte log is
`/tmp/geosolve-m78-clean-gate.8n2Fik.log`, SHA-256
`da48367b41084007637b08290e56fadd889dd1200f7918b83550237bf76d5fe3`. The gate includes
formatting/diff hygiene, warnings-denied workspace Clippy/Rustdoc, 1,730 passing locked all-feature
workspace tests with zero failures and three intentional ignores, editor 362/362, M78 geometry
variants 32/32, editor extreme-finite 7/7, sketch endpoint/extreme-finite 3/3 and 1/1, demo-web
143/143, six carried WASM parity binaries 28/28, unchanged 270/270 clean golden authority,
benchmark/performance budgets, the 150.29-second release sparse crossover, licence/package checks
and Trunk 0.21.14 release assembly.

The gate-produced `dist` was copied without rebuilding, compared byte-for-byte and frozen above.
Freeze evidence is `/tmp/geosolve-m78-freeze-evidence.IRltTB`. Proxy-disabled, cache-bypassed,
identity-encoded requests for `/` and all seven files return HTTP 200 with zero redirects, no
content encoding, exact lengths, expected media types and snapshot-identical bodies. `/` equals
`index.html`, and the fetched manifest has the same aggregate. HTTP evidence is
`/tmp/geosolve-m78-http-verify.wpLUFR`. Previous M77 PID `284248` stayed live until the new freeze
was complete and is now retired. This mechanically nominates the candidate; it does not accept any
human scorecard row.

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

- U1 — family palette, variant memory and stage language: pending
- U2 — points, segments, polylines and midpoint lines: pending
- U3 — four rectangle recipes and Shift squares: pending
- U4 — circles and arcs: pending
- U5 — ellipses, Béziers and conics: pending
- U6 — open and periodic control NURBS: pending
- U7 — modifiers, inference cycling and recovery: pending
- U8 — role, persistence and desktop polish: pending
- Final supervising approval: pending
- GitHub Pages publication and hosted-byte verification: pending
