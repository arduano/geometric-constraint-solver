<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B focused UAT — Workspace reproduction handoff

Status: human UAT has opened `M70B-F003` and `M70B-F004`; the prior clean `M70B-H1` candidate is not
approved.
`M70B-F001` and `M70B-F002` retain their owning-layer corrections and complete replacement
evidence. M70B-H1 adds a 193/193 passing constraint/dimension-authoring and scene oracle; its
complete release gate and fresh byte-verified Tailscale publication now pass. M70B-H2 only
generalized that test infrastructure and added the repo-local defect workflow; its clean release
qualification passed without replacing or altering the served H1 product candidate. Test-only H3
preserves those 193 row records and adds four isolated `feature.fillet` rows: point and curve-pair
Coincident-closure authoring record F003, while lower same-cell and periodic-seam line-circle
evaluation record F004. The current 197-row golden contains 193 `PASS` and four reviewed `DEFECT`;
`--check` passes while `--require-clean` intentionally fails on exactly those four open rows. H3
changes no production behavior or served bytes, and no fix or repair plan is authorized yet. This
scorecard records no human pass or approval.

Prior `M70B-F001` candidate source: `b4ec279e221df38816b7376a6978712e21df02c2`

`M70B-F002` replacement candidate source: `2e0f6c348ea0d3d9ee0bc2fd556f402a29d7059b`

Current `M70B-H1` candidate source: `dd645d99e705e56c80ab2a4a136f7a4d03baafbf`

Qualified `M70B-H2` test-infrastructure source: `47584bdb607c722df508eae56584726954a03205`

Historical `M70B-H1/H2` golden SHA-256:
`803c443d12a7362993fd557bd96d9db496ce162579d0ae08e2feff57b009e19b`

Current `M70B-H3` golden SHA-256:
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`

Prior `M70B-F001` Tailscale endpoint: `http://100.94.63.83:8080/`

Prior `M70B-F001` release distribution manifest aggregate:
`b91f25a600e09f99c67f7b8a77d2bc6a38d7a1517fead2b70942ed5681337c28`

Current `M70B-H1` Tailscale endpoint: `http://100.94.63.83:8080/`

Current `M70B-H1` read-only snapshot: `/tmp/geosolve-m70b-h1-uat.viSB9G`

Current `M70B-H1` release distribution manifest aggregate:
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`

H1-H3 are test-only, so the served release bytes and aggregate intentionally remain the prior F002
product bytes qualified and republished by H1.

## Preconditions

- [x] `docs/M70B_IMPLEMENTATION.md` records passing focused/direct `M70B-F002` qualification.
- [x] The complete integrated release gate passes on the `M70B-F002` clean nominated source.
- [x] An `M70B-F002` replacement read-only release distribution is served through Tailscale.
- [x] Every `M70B-F002` served asset and `/` matches the frozen local bytes.
- [x] M70B-H1's checked golden and clean-oracle gate pass with 193/193 classified rows.
- [x] The clean M70B-H1 source passes the complete integrated release gate.
- [x] A fresh M70B-H1 read-only distribution is served and byte-verified through Tailscale.
- [x] M70B-H3's reviewed 197-row golden `--check` passes with 193 `PASS` and four expected
  `DEFECT` rows carrying only `M70B-F003`/`M70B-F004`.
- [ ] M70B-H3's `--require-clean` gate returns zero; it intentionally fails on exactly those four
  rows while the findings remain open.
- [ ] The browser has hard-refreshed that exact candidate.

Use only the ordinary GeoSolve Sketch Workbench. The reproduction overlay is global workbench UI;
there is no scenario mode, protected fixture, alternate coordinator or restored legacy page. Direct
Native Rust tests remain authoritative for exact bytes, bounds, workspace/high-water fidelity and
atomicity; the same codec path must also compile for WASM. Human review assesses discoverability,
text handoff, visible restoration and failure recovery.

## M70B-H3 — Reviewed test-only Fillet golden

The current aggregate driver executes the original 193 H1/H2 rows plus four isolated
`feature.fillet` rows:

- `feature.fillet.authoring.coincident-closure.point` — `M70B-F003`;
- `feature.fillet.authoring.coincident-closure.curve-pair` — `M70B-F003`;
- `feature.fillet.evaluation.line-circle.same-cell-lower` — `M70B-F004`, winding zero; and
- `feature.fillet.evaluation.line-circle.same-cell-seam` — `M70B-F004`, winding one.

The checked golden SHA-256 is
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`. The reviewed check is
green: all 197 rows match their expected bytes, with 193 `PASS` and four `DEFECT`. The clean gate is
deliberately red: `./scripts/golden-authoring-scene-oracle.sh --require-clean` exits nonzero and
names exactly those four rows. That red result is the expected open-defect gate, not a panic,
timeout, harness error or production regression introduced by H3.

Result: **REVIEWED CHECK PASS — CLEAN GATE BLOCKED BY EXACTLY FOUR OPEN DEFECT ROWS**

## M70B-U1 — Discover and copy a self-contained payload

1. Open **Samples → Curves & constructions → 2D Fillet playground** or another ordinary editable
   workspace.
2. Make recognizable accepted edits: create or adjust a computed Fillet, add one constraint or
   dimension and create or convert one Construction curve.
3. Activate **Copy repro**, which also attempts to place the payload on the clipboard.
4. Inspect the visible overlay and its payload/status, then paste into a plain text editor to verify
   the automatic copy.
5. If practical, deny clipboard permission and repeat; otherwise choose **Select text** and press
   Ctrl/Cmd+C to exercise the manual fallback.

Expected: the action is discoverable and produces one complete single-line value beginning
`GEOSOLVE_REPRO_V1:zlib-base64url:`. The overlay reports success or leaves the entire payload
available for manual copy; denial does not lose or truncate it. Opening/closing the overlay causes
no canvas resize, geometry move, solve or accepted-state change.

Result: **PENDING**

Notes:

## M70B-U2 — Restore the complete persisted workspace

1. Keep the copied text outside the workbench, such as in a plain text editor.
2. Record the visible geometry, Construction roles, constraints/dimensions and computed Fillet.
3. Replace the current scene with another sample or delete/move several recognizable objects.
4. Activate **Load repro**, paste the saved text and choose **Load payload**.
5. Inspect the tree, canvas, constraints/dimensions and computed feature, then make one ordinary edit.

Expected: one action restores the copied persisted design/accepted workspace coherently, including
the computed Fillet and Construction semantics. No intermediate or mixed scene is painted. The old
camera, selection, hover, active tool, sample label and pre-copy Undo stack are not restored; those
are intentional non-persisted state. The restored workspace remains normally editable.

Result: **PENDING**

Notes:

## M70B-U3 — Handoff is independent of browser storage

1. Copy a payload from the edited first tab/profile.
2. Open the nominated candidate in a separate fresh tab or browser profile whose ordinary
   workspace differs.
3. Activate **Load repro**, paste the payload and choose **Load payload** without copying any
   `localStorage` key or file.
4. Compare the visible sketch, roles, constraints/dimensions and computed feature with the source.

Expected: the text alone carries the complete persisted workspace needed for reproduction. The
destination's prior browser storage neither supplies missing scene data nor overrides the capsule.
Transient camera/tool/selection state may differ by design.

Result: **PENDING**

Notes:

## M70B-U4 — Corruption and invalid workspace are atomic

1. With a valid unrelated scene visible, change one character in the copied payload body and choose
   **Load payload**.
2. Load the original text successfully, return to the unrelated scene, then use **Load payload**
   with a truncated value and an unsupported version prefix.
3. After each rejection, close/reopen the overlay and manipulate the existing scene normally.

Expected: each bad input produces a specific visible error in the overlay. The current canvas,
tree, accepted geometry and persisted workspace remain unchanged; no partially loaded scene,
layout shift or frozen interaction remains. Returning to the untouched valid payload succeeds.

Result: **PENDING**

Notes:

## M70B-U5 — Ordinary workflow and text ergonomics remain coherent

1. Pan/zoom before opening the overlay and verify the canvas does not move under the pointer when
   it appears.
2. Scroll/select within a representative long payload and close it with the close button or Escape
   without choosing **Load payload**.
3. Reopen, load a valid payload and perform drawing, selection, drag, constraint authoring, camera
   and one new Undo/Redo operation.
4. Refresh after the accepted post-load edit.

Expected: the overlay contains its own overflow and never shifts the canvas. Closing without
loading is mutation-free. After load, ordinary authoring and camera controls work; new post-load history and
normal workspace persistence behave normally without claiming restoration of the old history
cursor.

Result: **PENDING**

Notes:

## Approval

### Finding `M70B-F001` — Local contact branch blocks a free endpoint drag

The payload with identity `8446:ea81c82137d5b13c` restored successfully but its otherwise-free
line endpoint moved only in small increments or appeared immobile. Headless reduction found a
healthy ten-DOF accepted graph and healthy locality plan; a Local ellipse-point-on-line parameter
was instead settling exactly on a semantically open branch edge and failing independent
validation. The Local-only effective-bound correction and exact payload regression are recorded
in `docs/SCENARIOS.md` and `docs/M70B_IMPLEMENTATION.md`.

Targeted recheck after the replacement candidate is published:

1. Load the original supplied payload through **Load repro**.
2. Drag the free line endpoint—the endpoint not incident to the circle—in sizeable horizontal,
   vertical and diagonal motions, including reversing direction during one gesture.
3. Confirm the endpoint follows the pointer normally rather than advancing only in tiny steps,
   and that the circle/line/ellipse contacts remain valid without a global error.

Expected: the requested endpoint follows each ordinary drag continuously; no
`AmbiguousContactNeighborhood` rejection, branch flip, freeze or DOF loss is exposed.

Result: **PENDING**

Notes:

### Finding `M70B-F002` — radial Normal collapses and the accepted canvas disappears

The payload with identity `6037:eecc886c0e61208f` retained a rejected radial Normal after a line
endpoint was placed on a circle. Generic contact defaults had constrained the circle centre to the
finite segment at the picked parameter even though radial Normal means centre-on-supporting-line.
The resulting attempt drove radius toward zero and stalled. The workbench then confused the older
accepted scene's intentionally missing current inference authority with missing presentation
geometry and emitted no scene. The headless supporting-line/projection correction and detached
accepted-scene regression are recorded in `docs/SCENARIOS.md` and
`docs/M70B_IMPLEMENTATION.md`.

Targeted recheck after the `M70B-F002` replacement candidate is published:

1. In a clean workspace, draw a circle and a line with one endpoint coincident with the circle
   perimeter, matching the original reproduction approximately.
2. Select the line and circle in either order and apply **Perp / normal**.
3. Confirm the relation applies in one action, the line support passes through the circle centre,
   the radius stays visibly positive and all geometry remains on canvas.
4. Create or retain any genuinely rejected constraint and confirm the prior accepted geometry
   remains visible with a problem report rather than disappearing.
5. Optionally load the original supplied post-failure payload. Confirm its historical accepted
   circle/line remains visible; delete or repair the rejected Normal, then repeat steps 1–3.

Expected: ordinary radial Normal uses the complete line support even when the centre projects
beyond an endpoint; there is no radius collapse, stalled/global disappearance or operand-order
difference. A rejected edit paints only historical accepted geometry and never attempted invalid
geometry; normal inference remains unavailable until the design is repaired.

Result: **PENDING**

Notes:

### Finding `M70B-F003` — Coincident closure corner cannot be Filleted

An open three-segment triangle polyline was closed by making its distinct first and last points
Coincident. Both ordinary corners entered the Fillet preview, but the closure corner failed whether
selected as a point or as its two incident spans. Independent headless reproduction confirms the accepted
triangle remains finite and hard-valid: the point path returns `WrongOperandKind`, and the explicit
last/first-span path returns `DuplicateSupport` with a same-curve adjacency message. The point
rejection retains the prior valid two-corner preview; the two-span rejection retains its pending
first support and no-preview state. Neither publishes a feature.

The exact current failure is encoded by
`m70b_f003_coincident_triangle_closure_is_not_filletable_by_point_or_curve_pair`. The historical
H1/H2 193-row golden remained green because it exercised constraint/dimension authoring and
precomposed scene authority, not computed-Fillet operand collection. H3 now records both public
routes as reviewed `DEFECT` rows:
`feature.fillet.authoring.coincident-closure.point` and
`feature.fillet.authoring.coincident-closure.curve-pair`. This checkpoint deliberately does not
implement or plan the production correction.

Result: **OPEN — ENCODED, NOT FIXED**

### Finding `M70B-F004` — valid line-circle Fillet branches are withheld after source edits

The supplied payloads with identities `4752:daa87c91c75abf9f` and
`4750:beda1885b15e38b5` both restore finite, hard-valid six-DOF sketches and the same explicit
radius-1 line-circle Fillet branch. The accepted horizontal line lies below the circle in one case
and crosses its upper region in the other. Both restored computed features report `NoLocalRoot`
and render no Fillet, even though public contact reseeding independently finds a valid root strictly
inside the unchanged certified circle branch cell.

The two cases are encoded as one focused `geosolve-sketch-features` current-behavior
characterization. The valid circle contacts move about `0.459` and `0.507` radians from the stored
seed, while persisted nonlinear evaluation searches only about `0.393` radians on either side.
Normal sides, retained endpoints, endpoint order and sweep do not need to change; the upper root's
normalized winding advances across the periodic parameter seam while its total parameter remains
inside the same certified cell. The historical H1/H2 193-row golden remained green because it
contained no computed-Fillet source-edit/branch-traversal row. H3 now records the missing axis as
reviewed `DEFECT` rows `feature.fillet.evaluation.line-circle.same-cell-lower` (winding zero) and
`feature.fillet.evaluation.line-circle.same-cell-seam` (winding one). Each independently proves the
viable branch rather than trusting the failed evaluation status.

This checkpoint deliberately does not implement or plan the production correction.

Result: **OPEN — ENCODED, NOT FIXED**

M70B remains active until the supervising human records explicit approval here. A scoped approval
may accept M70B-U1 through M70B-U5 after objective findings receive owning-layer regressions and a
targeted recheck; it must not invent an unrecorded exhaustive replay. M71 remains deferred until
M70B is closed. The H3 `--require-clean` gate remains intentionally red on exactly the four reviewed
F003/F004 rows until those findings are resolved or explicitly dispositioned.
