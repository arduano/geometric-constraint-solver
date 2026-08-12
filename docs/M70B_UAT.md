<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B focused UAT — Workspace reproduction handoff

Status: complete under the supervising human's requested scoped sign-off on 2026-08-12. Human UAT
opened `M70B-F003` and `M70B-F004`; both findings have authorized owner repairs, and later movement
finding `M70B-F005` has its certificate-transport repair.
`M70B-F001` and `M70B-F002` retain their owning-layer corrections and complete replacement
evidence. M70B-H1 historically added a 193/193 passing constraint/dimension-authoring and scene
oracle; its complete release gate and fresh byte-verified Tailscale publication now pass. M70B-H2
only generalized that test infrastructure and added the repo-local defect workflow; its clean release
qualification passed without replacing or altering the then-served H1 product candidate. Test-only
H3 historically preserved those 193 row records and added four isolated `feature.fillet` rows:
point and curve-pair Coincident-closure authoring record F003, plus lower same-cell and periodic-seam
line-circle evaluation record F004. H3 recorded 193 `PASS` and four reviewed `DEFECT`. The
repaired fixture retains the same 197 cases and exact four input fingerprints. F005 appends one
source-rotation row at `input-04658a77db2dc779`, preserving all earlier records; the current
fixture contains 198 `PASS` at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. F005's exact owner
regression, 45-test feature suite, nine-test retained movement suite, focused golden tests, all
aggregate golden modes, formatting, warnings-denied all-workspace Clippy, locked all-feature
workspace tests and the relevant WASM check pass. Clean F005 source
`d400c4a8201f6afc531f5b504424d6430dbf3937` passes the complete release gate, and its fresh
immutable seven-file Tailscale publication is byte-verified. The supervising human subsequently
reported the F005 movement behavior fixed and requested sign-off once the closing regressions were
satisfactory. Clean source `48e3cc3` passes the complete release gate with the focused two-
previously-Current transaction and CircularArc transport/domain regressions, while the 198/198
golden and F005 release bytes remain unchanged. The resulting scoped approval closes M70B without
claiming an unrecorded exhaustive replay of every prepared step below.

Prior `M70B-F001` candidate source: `b4ec279e221df38816b7376a6978712e21df02c2`

`M70B-F002` replacement candidate source: `2e0f6c348ea0d3d9ee0bc2fd556f402a29d7059b`

Historical `M70B-H1` candidate source: `dd645d99e705e56c80ab2a4a136f7a4d03baafbf`

Qualified `M70B-H2` test-infrastructure source: `47584bdb607c722df508eae56584726954a03205`

Closing regression/qualification source: `48e3cc3`

Historical `M70B-H1/H2` golden SHA-256:
`803c443d12a7362993fd557bd96d9db496ce162579d0ae08e2feff57b009e19b`

Historical `M70B-H3` discovery golden SHA-256:
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`

Current repaired golden SHA-256:
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`

Historical published `M70B-F003/F004` replacement source:
`0ef60ef47035e8b1fb1eece2c38d05ccdfdc4abf`

Historical published `M70B-F003/F004` integrated release-gate result: **PASS**

Historical `M70B-F003/F004` Tailscale endpoint: `http://100.94.63.83:8080/`

Historical published `M70B-F003/F004` read-only snapshot:
`/tmp/geosolve-m70b-f003-f004-uat.lKC2xY`

Historical `M70B-F003/F004` release distribution manifest aggregate:
`96cc64dec998074ede56e3e38fb919a4854d0e0dbb8030138393e01a3d0844d3`

Published `M70B-F005` replacement source: `d400c4a8201f6afc531f5b504424d6430dbf3937`

Published `M70B-F005` integrated release-gate result: **PASS**

Published `M70B-F005` Tailscale endpoint: `http://100.94.63.83:8080/`

Published `M70B-F005` read-only snapshot: `/tmp/geosolve-m70b-f005-uat.Q5c9Wi`

Published `M70B-F005` release distribution manifest aggregate:
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`

The snapshot directory is mode `0555`, every file is mode `0444`, and PID `1841268` is bound only
to the Tailscale address. Every served asset and `/` byte-matches the immutable snapshot.

Prior `M70B-F001` Tailscale endpoint: `http://100.94.63.83:8080/`

Prior `M70B-F001` release distribution manifest aggregate:
`b91f25a600e09f99c67f7b8a77d2bc6a38d7a1517fead2b70942ed5681337c28`

Historical `M70B-H1` Tailscale endpoint: `http://100.94.63.83:8080/`

Historical `M70B-H1` read-only snapshot: `/tmp/geosolve-m70b-h1-uat.viSB9G`

Historical `M70B-H1` release distribution manifest aggregate:
`f33cc593dbe719f192a5a08ea293678f4c053adbe6b9bf4f44f8bae662f53019`

H1-H3 were test-only, so their served release bytes and aggregate intentionally remained the prior
F002 product bytes qualified and republished by H1. The F003/F004 repairs change headless
production behavior; their clean qualified and byte-verified replacement now supersedes H1 as the
historical product candidate. F005's clean qualified and byte-verified replacement now supersedes
F003/F004 as the served UAT candidate.

## Preconditions

- [x] `docs/M70B_IMPLEMENTATION.md` records passing focused/direct `M70B-F002` qualification.
- [x] The complete integrated release gate passes on the `M70B-F002` clean nominated source.
- [x] An `M70B-F002` replacement read-only release distribution was served through Tailscale at its
  historical checkpoint.
- [x] Every `M70B-F002` served asset and `/` matched the frozen local bytes at that checkpoint.
- [x] M70B-H1's checked golden and clean-oracle gate pass with 193/193 classified rows.
- [x] The clean M70B-H1 source passes the complete integrated release gate.
- [x] A fresh M70B-H1 read-only distribution was served and byte-verified through Tailscale at its
  historical checkpoint.
- [x] Historical M70B-H3 preserved all 193 H1/H2 rows and reviewed exactly four `DEFECT` rows
  carrying only `M70B-F003`/`M70B-F004` at SHA-256
  `a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`.
- [x] The F003/F004 repair checkpoint retained all 197 case IDs and exact input fingerprints,
  recorded 197 `PASS` and had SHA-256
  `035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`.
- [x] F005 exact payload `4228:0823d31f269300af` appends only source-rotation fingerprint
  `input-04658a77db2dc779`; the current fixture records 198 `PASS` at SHA-256
  `bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`.
- [x] The focused F003 editor integration suite, 45-test all-feature F004/F005 owner suite and
  nine-test retained F005 movement suite pass.
- [x] Current aggregate golden `--check` and `--require-clean` runs pass.
- [x] Formatting and focused warnings-denied Clippy pass for F005.
- [x] Full workspace Clippy/tests and the relevant WASM build pass for the F005 replacement.
- [x] Clean `main` source `0ef60ef47035e8b1fb1eece2c38d05ccdfdc4abf` passes the complete
  integrated F003/F004 release gate.
- [x] Its immutable F003/F004 seven-file replacement distribution was served only through Tailscale
  at that historical checkpoint, and every asset plus `/` byte-matched the frozen snapshot.
- [x] A clean F005 replacement passes the complete release gate and a fresh immutable distribution
  is served and byte-verified through Tailscale.
- [x] The closing focused retained-coordinator regression passes with two distinct features that
  begin `Current`, only one failing during projected dragging, complete-scene retention,
  failing-feature-only attribution, reverse recovery and last-valid release.
- [x] The closing public feature-owner CircularArc/affine regression passes in both parent orders,
  crosses a stale Local witness on the same branch and rejects a regular supporting-circle root
  beyond the finite arc endpoint.
- [x] The complete clean release gate re-passes after that test-only cut; if the release bytes remain
  identical, the current frozen F005 publication remains authoritative without a republish.
- [x] The browser hard-refreshed the byte-identical F005 candidate used for the accepted movement
  recheck; the test-only closing cut requires no replacement browser build.

Use only the ordinary GeoSolve Sketch Workbench. The reproduction overlay is global workbench UI;
there is no scenario mode, protected fixture, alternate coordinator or restored legacy page. Direct
Native Rust tests remain authoritative for exact bytes, bounds, workspace/high-water fidelity and
atomicity; the same codec path must also compile for WASM. Human review assesses discoverability,
text handoff, visible restoration and failure recovery.

## M70B-H3/F005 — Reviewed discovery golden and current repaired fixture

The current aggregate driver executes the original 193 H1/H2 rows plus five isolated
`feature.fillet` rows:

- `feature.fillet.authoring.coincident-closure.point` — `M70B-F003`;
- `feature.fillet.authoring.coincident-closure.curve-pair` — `M70B-F003`;
- `feature.fillet.evaluation.line-circle.same-cell-lower` — `M70B-F004`, winding zero; and
- `feature.fillet.evaluation.line-circle.same-cell-seam` — `M70B-F004`, winding one; and
- `feature.fillet.evaluation.line-circle.source-rotation.retained-start` — `M70B-F005`, moved
  affine source with overlapping fresh certificates.

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

F005 preserves those records and appends
`feature.fillet.evaluation.line-circle.source-rotation.retained-start` at
`input-04658a77db2dc779`. The current fixture records 198 `PASS`, zero defects, panics, timeouts or
harness errors at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Exact F005 owner regression
`m70b_f005_line_circle_source_rotation_transports_persisted_branch_cell`, the 45-test owner suite,
nine-test retained movement suite, focused golden tests, aggregate
`--survey`/`--check`/`--require-clean`, formatting, warnings-denied all-workspace Clippy, locked
all-feature workspace tests and the relevant WASM check pass. The clean F005 release qualification
and fresh byte-verified Tailscale publication also pass; the scoped close record appears below.

Result: **PASS — 198/198 GOLDEN, CLEAN CLOSING GATE AND SCOPED M70B APPROVAL**

## M70B-F005 — Persistent Fillet movement continuity

1. Copy the complete checked-in
   [`m70b_f005_repro.txt`](../crates/geosolve-demo-web/tests/fixtures/m70b_f005_repro.txt) capsule
   (identity `4228:0823d31f269300af`) and load it through **Load repro**.
2. Confirm the radius-1 Fillet between the circle and line is visible immediately.
3. Drag either line endpoint so the Fillet contact moves through the circle's nearby 90-degree/
   cardinal mark and back again.
4. Repeat with several small movements on both sides of that mark.
5. Move far enough that the chosen contact genuinely leaves the finite line segment, or toward an
   actual tangent/fold limit. Confirm motion pauses at the last complete valid line-plus-Fillet
   scene with a local warning rather than deleting the Fillet or jumping to the opposite contact.
6. Without releasing, move back into valid geometry and confirm the warning clears and the same
   Fillet branch resumes. Repeat once by releasing while still beyond the limit; confirm only the
   last valid preview is committed.

Expected: ordinary transverse movement stays continuous when a valid root crosses an old cached
certificate edge. The Fillet retains its circle side, line side, retained endpoints and sweep;
the full circle remains visible. There is no disappearance at a cardinal mark and no opposite-root
jump. A genuine parent/fold/barrier limit holds the last complete scene and release point, exposes
an attributed cue when possible, and recovers when dragged back. It never paints a moved line with
its persistent Fillet missing.

Result: **PASS — SUPERVISING HUMAN REPORTED THE F005 MOVEMENT BEHAVIOR FIXED**

Notes:

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Result: **NOT EXHAUSTIVELY REPLAYED — ACCEPTED BY SCOPED CLOSE**

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

Targeted repair recheck on the published replacement candidate:

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

Result: **RESOLVED HEADLESS — ACCEPTED BY SCOPED CLOSE WITHOUT AN UNRECORDED REPLAY**

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
retained sketch/feature identity. The locked all-feature owner suite passed 42/42 at the F004 repair
checkpoint and now passes 45/45 with F005 appended. The retained movement suite separately passes
9/9. Both F004 golden rows keep their exact inputs
and record `PASS`.

Targeted repair recheck on the published replacement candidate:

1. Load payload `4752:daa87c91c75abf9f` and confirm the radius-1 line-circle Fillet is visible and
   Current rather than `NoLocalRoot`.
2. Load payload `4750:beda1885b15e38b5` and confirm its periodic-seam branch is visible with the
   intended orientation.
3. Move each source line through modest accepted edits that keep the same explicit branch cell and
   confirm the Fillet follows without disappearing or switching side.
4. Exercise a generic nonlinear Fillet separately and confirm it does not jump to a remote root.

Expected: both exact line-circle payloads evaluate inside their stored explicit cells, including
the seam winding, while unrelated nonlinear branch-locality remains conservative.

Result: **RESOLVED HEADLESS — ACCEPTED BY SCOPED CLOSE WITHOUT AN UNRECORDED REPLAY**

The supervising human reported F005 movement fixed, requested the two closing regressions, and
asked for M70B sign-off once the result was satisfactory. Clean source `48e3cc3` passes
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, including the unchanged
198/198 clean golden, all locked workspace tests, native/WASM parity, warnings-denied checks, the
149.13-second sparse crossover and Trunk release assembly. The generated seven-file distribution
byte-matches `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` at aggregate
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`, so the human-reviewed F005
publication remains authoritative without a republish. This scoped approval accepts M70B-U1
through M70B-U5 and the resolved F001-F005 findings for the recorded milestone scope without
inventing an exhaustive replay of every scripted step. M70B is closed. M71 remains an unauthorized
candidate backlog awaiting an explicit scope decision.
