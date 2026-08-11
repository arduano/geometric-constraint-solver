<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B focused UAT — Workspace reproduction handoff

Status: human UAT opened `M70B-F003` and `M70B-F004`; both findings now have authorized,
locally implemented owner repairs, while the prior clean `M70B-H1` candidate remains unapproved.
`M70B-F001` and `M70B-F002` retain their owning-layer corrections and complete replacement
evidence. M70B-H1 adds a 193/193 passing constraint/dimension-authoring and scene oracle; its
complete release gate and fresh byte-verified Tailscale publication now pass. M70B-H2 only
generalized that test infrastructure and added the repo-local defect workflow; its clean release
qualification passed without replacing or altering the served H1 product candidate. Test-only H3
preserves those 193 row records and adds four isolated `feature.fillet` rows: point and curve-pair
Coincident-closure authoring record F003, while lower same-cell and periodic-seam line-circle
evaluation record F004. H3 historically recorded 193 `PASS` and four reviewed `DEFECT`. The
repaired fixture retains the same 197 cases and exact four input fingerprints but now contains
197 `PASS` at SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. Focused F003 and F004 owner
suites, both aggregate golden modes, formatting, warnings-denied workspace Clippy, locked
all-feature workspace tests and the relevant WASM build pass. A clean nominated release build,
replacement publication and human approval remain pending. This scorecard records no human pass
or approval.

Prior `M70B-F001` candidate source: `b4ec279e221df38816b7376a6978712e21df02c2`

`M70B-F002` replacement candidate source: `2e0f6c348ea0d3d9ee0bc2fd556f402a29d7059b`

Current `M70B-H1` candidate source: `dd645d99e705e56c80ab2a4a136f7a4d03baafbf`

Qualified `M70B-H2` test-infrastructure source: `47584bdb607c722df508eae56584726954a03205`

Historical `M70B-H1/H2` golden SHA-256:
`803c443d12a7362993fd557bd96d9db496ce162579d0ae08e2feff57b009e19b`

Historical `M70B-H3` discovery golden SHA-256:
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`

Current repaired golden SHA-256:
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`

Prior `M70B-F001` Tailscale endpoint: `http://100.94.63.83:8080/`

Prior `M70B-F001` release distribution manifest aggregate:
`b91f25a600e09f99c67f7b8a77d2bc6a38d7a1517fead2b70942ed5681337c28`

Current `M70B-H1` Tailscale endpoint: `http://100.94.63.83:8080/`

Current `M70B-H1` read-only snapshot: `/tmp/geosolve-m70b-h1-uat.viSB9G`

Current `M70B-H1` release distribution manifest aggregate:
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`

H1-H3 were test-only, so their served release bytes and aggregate intentionally remained the prior
F002 product bytes qualified and republished by H1. The F003/F004 repairs change headless
production behavior and have not yet been release-qualified or published; H1 therefore remains the
last served candidate.

## Preconditions

- [x] `docs/M70B_IMPLEMENTATION.md` records passing focused/direct `M70B-F002` qualification.
- [x] The complete integrated release gate passes on the `M70B-F002` clean nominated source.
- [x] An `M70B-F002` replacement read-only release distribution is served through Tailscale.
- [x] Every `M70B-F002` served asset and `/` matches the frozen local bytes.
- [x] M70B-H1's checked golden and clean-oracle gate pass with 193/193 classified rows.
- [x] The clean M70B-H1 source passes the complete integrated release gate.
- [x] A fresh M70B-H1 read-only distribution is served and byte-verified through Tailscale.
- [x] Historical M70B-H3 preserved all 193 H1/H2 rows and reviewed exactly four `DEFECT` rows
  carrying only `M70B-F003`/`M70B-F004` at SHA-256
  `a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`.
- [x] The repaired fixture retains all 197 case IDs and exact input fingerprints, records 197
  `PASS` and has SHA-256
  `035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`.
- [x] The focused F003 editor integration suite and 42-test all-feature F004 owner suite pass.
- [x] Current aggregate golden `--check` and `--require-clean` runs pass.
- [x] Full workspace, Clippy, formatting and WASM repair qualification pass.
- [ ] The browser has hard-refreshed that exact candidate.

Use only the ordinary GeoSolve Sketch Workbench. The reproduction overlay is global workbench UI;
there is no scenario mode, protected fixture, alternate coordinator or restored legacy page. Direct
Native Rust tests remain authoritative for exact bytes, bounds, workspace/high-water fidelity and
atomicity; the same codec path must also compile for WASM. Human review assesses discoverability,
text handoff, visible restoration and failure recovery.

## M70B-H3 — Reviewed discovery golden and current repaired fixture

The current aggregate driver executes the original 193 H1/H2 rows plus four isolated
`feature.fillet` rows:

- `feature.fillet.authoring.coincident-closure.point` — `M70B-F003`;
- `feature.fillet.authoring.coincident-closure.curve-pair` — `M70B-F003`;
- `feature.fillet.evaluation.line-circle.same-cell-lower` — `M70B-F004`, winding zero; and
- `feature.fillet.evaluation.line-circle.same-cell-seam` — `M70B-F004`, winding one.

At the H3 discovery checkpoint, the checked golden SHA-256 was
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`: all 197 rows matched their
expected bytes, with 193 `PASS` and four `DEFECT`, while `--require-clean` named exactly those four
rows. That historical red result was the expected open-defect gate, not a panic, timeout or harness
error.

The current repaired fixture keeps the same four case IDs and input fingerprints:

- `input-4ba571059db7afff` for Coincident-closure point authoring;
- `input-d04adbf29c08b9bd` for Coincident-closure curve-pair authoring;
- `input-f9920c3cf170130d` for lower same-cell evaluation; and
- `input-2da21ef04cfb4246` for periodic-seam same-cell evaluation.

It now records 197 `PASS`, zero defects, panics, timeouts or harness errors at SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. The focused owner suites,
aggregate `--check`/`--require-clean`, formatting, warnings-denied workspace Clippy, locked
all-feature workspace tests and the relevant WASM build pass. Clean release nomination,
publication and human UAT remain pending.

Result: **AUTOMATED REPAIR QUALIFICATION PASS — HUMAN UAT PENDING**

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
selected as a point or as its two incident spans. Independent headless reproduction confirmed the
accepted triangle remained finite and hard-valid: the point path returned `WrongOperandKind`, and
the explicit last/first-span path returned `DuplicateSupport` with a same-curve adjacency message.

The common cause was identity-only topology. Point-to-corner incidence, same-polyline join
eligibility and retained-endpoint hints compared persistent point IDs directly, so distinct points
connected by an active Coincident constraint were not treated as one semantic corner. The repair
adds deterministic transitive point representatives derived only from active, explicit Coincident
constraints and uses them in all three decisions. Coordinate proximity and suppressed constraints
do not create a join.

The positive regression
`m70b_f003_coincident_triangle_closure_is_filletable_by_point_or_curve_pair` now covers either
Coincident endpoint and the first/last spans in both orders. Each route produces the exact three
span pairs, a three-corner preview/publication and one Current feature with three Fillet arcs. The
focused editor integration suite passes, and both F003 golden rows now remain the same inputs but
record `PASS`.

Targeted repair recheck after a replacement candidate is published:

1. Draw an open three-segment triangle polyline and make its distinct first and last points
   Coincident.
2. Apply Fillets to all three corners by clicking points, including the closure point; verify one
   operation previews and publishes all three.
3. Repeat in a fresh sketch, selecting the first and last spans in each order for the closure
   corner.
4. Confirm all three arcs remain Current and editable with no `WrongOperandKind`,
   `DuplicateSupport` or missing-corner result.

Expected: active explicit Coincident closure behaves as one semantic Fillet corner through both
public authoring routes; no coordinate-near but unconstrained endpoint is implicitly welded.

Result: **RESOLVED HEADLESS — TARGETED HUMAN RECHECK PENDING**

### Finding `M70B-F004` — valid line-circle Fillet branches are withheld after source edits

The supplied payloads with identities `4752:daa87c91c75abf9f` and
`4750:beda1885b15e38b5` both restore finite, hard-valid six-DOF sketches and the same explicit
radius-1 line-circle Fillet branch. The accepted horizontal line lies below the circle in one case
and crosses its upper region in the other. Before repair, both restored computed features reported
`NoLocalRoot` and rendered no Fillet, even though public contact reseeding independently found a
valid root strictly inside the unchanged certified circle branch cell.

The two cases were encoded as one focused `geosolve-sketch-features` owner characterization. The
valid circle contacts move about `0.459` and `0.507` radians from the stored seed, while the former
persisted nonlinear policy searched only about `0.393` radians on either side.
Normal sides, retained endpoints, endpoint order and sweep do not need to change; the upper root's
normalized winding advances across the periodic parameter seam while its total parameter remains
inside the same certified cell. The historical H1/H2 193-row golden remained green because it
contained no computed-Fillet source-edit/branch-traversal row.

The cause was a hidden 12.5% seed window applied to every non-affine parent during persisted
evaluation. A line paired with a Circle or CircularArc now searches the complete certified
explicit tangent-orientation cell: constant circular curvature makes that traversal branch-local.
Generic nonlinear curves retain their seed-connected guard, and direct manipulation plus radius
continuation are unchanged.

The positive regression
`m70b_f004_line_circle_persisted_evaluation_traverses_complete_radial_branch_cell` requires both
exact payload-derived cases to publish independently valid Current Fillets at the expected root
and winding inside the unchanged Local cell, beyond the former seed window, without changing
retained sketch/feature identity. The locked all-feature owner suite passes 42/42. Both F004 golden
rows keep their exact inputs and now record `PASS`.

Targeted repair recheck after a replacement candidate is published:

1. Load payload `4752:daa87c91c75abf9f` and confirm the radius-1 line-circle Fillet is visible and
   Current rather than `NoLocalRoot`.
2. Load payload `4750:beda1885b15e38b5` and confirm its periodic-seam branch is visible with the
   intended orientation.
3. Move each source line through modest accepted edits that keep the same explicit branch cell and
   confirm the Fillet follows without disappearing or switching side.
4. Exercise a generic nonlinear Fillet separately and confirm it does not jump to a remote root.

Expected: both exact line-circle payloads evaluate inside their stored explicit cells, including
the seam winding, while unrelated nonlinear branch-locality remains conservative.

Result: **RESOLVED HEADLESS — EXACT-PAYLOAD HUMAN RECHECK PENDING**

M70B remains active until the supervising human records explicit approval here. A scoped approval
may accept M70B-U1 through M70B-U5 after objective findings receive owning-layer regressions and a
targeted recheck; it must not invent an unrecorded exhaustive replay. M71 remains deferred until
M70B is closed. F003/F004 are resolved at their headless owners and the current fixture records all
197 rows as `PASS`; aggregate golden, full native/WASM qualification, a replacement publication and
the targeted human rechecks above remain before closure.
