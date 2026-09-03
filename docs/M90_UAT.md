<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M90 UAT: typed code, canvas projection and headless parity

Status: **Closed by explicit scoped supervising-user approval on 2026-09-04. M90-F005/F006 repairs,
collateral qualification, the complete dirty-tree release gate, optimized release-WASM build and
immutable Tailscale nomination pass. M90-U1 through M90-U10 remain unexecuted and transfer/defer,
without pass or waiver, into M91's composite UAT. The exact closing candidate remains Tailscale-only
at `http://100.94.63.83:18090/`; no GitHub Pages deployment or public push was authorized or made.**

## Withdrawn historical candidate

Do **not** use the historical snapshot below for this scorecard. M90-F001 withdrew those bytes from
continuing UAT. Historical unit
`geosolve-m90-uat-18090.service`, PID `2747435`, invocation
`767e65048ece4573834acf8f580f87bc`, served the frozen snapshot
`/tmp/geosolve-m90-uat.vuI7sBKt` as its working directory. Its external manifest remains
`/tmp/geosolve-m90-uat.vuI7sBKt.sha256`, with ordered aggregate
`b5bae1aca28787f026a11100c94e425d1c5e057ce3539170f4399b7cd8b05dc2`. The snapshot contains nine
regular files, two directories and zero symlinks. Its `20,007,307`-byte
`assets/geosolve_demo_web_bg-BAUG7n7P.wasm` has SHA-256
`51fc04d4129dd73791afb20b4403efe1f4fb95af6d607037b2a95956865fe7f5`.

Strict local and live HTTP ledgers were byte-identical with SHA-256
`17477e87e897b5ac080547df41b528bc16c252d4c67a4634ad21a384f4ccd29b`. The historical pre-F001
pinned provisional dirty-tree release gate exited `0`; its log has SHA-256
`2dd3663430c730eea84303954598f3a0696868aa4d433f3e32836a42024b250b`. The unchanged golden
survey/check/require-clean passes `271/271` with aggregate
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`. This is provisional
dirty-worktree historical reproduction evidence, not a current nomination, clean-source
qualification or human acceptance. The unit is now inactive/dead with no PID, and no listener is
attributed to that unit. After the later post-F004 service stopped on reboot and before the F006
replacement below, no current M90 candidate was live on port `18090`.

## Withdrawn pre-F002 F001 replacement

Do **not** use the snapshot below for the still-pending scorecard. Historical Tailscale-only unit
`geosolve-m90-f001-replacement-uat-18090.service` served it with PID `3332035`, invocation
`258e6d4da4b14661bd6d8e44856c64a5`, bound to `100.94.63.83` from frozen snapshot
`/tmp/geosolve-m90-uat.xk0AGnz0`. External manifest
`/tmp/geosolve-m90-uat.xk0AGnz0.sha256` has ordered aggregate
`030e9f4aa98690b8cd35cdbb51a29220674f1bfcfba310192afc467f5afc38a4`. The snapshot contains nine
mode-`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,958,913`-byte
`assets/geosolve_demo_web_bg-B6mOdH7K.wasm` has SHA-256
`9607cfd48f1ec23b2c29e120704277bbb70247bdbed6a67945c5cc64b7af8761`.

All ten staging and live routes are byte-identical with correct MIME types, no redirects and no
compression; their ledger has SHA-256
`56a5aff7b23000e1b009f2eb479b9545fcfb17dbe5d1a4f9721f7c3951a761ae`. The optimized release-WASM
Compass Rose two-consecutive-drag Playwright regression passes `1/1` against both staging and live.
Freeze and verification evidence is
`/tmp/geosolve-m90-f001-replacement-freeze-evidence.UpMqFyrm`.

This is immutable **pre-F002 provisional dirty-tree replacement evidence**. The monolithic release
gate was harness-terminated at exit `143` and is not a completed passing gate or clean-source
qualification. M90-F002 withdraws these bytes from continuing UAT because they still use the
quota-limited project-persistence path. The unit is now inactive/dead and its immutable snapshot
remains historical evidence. It accepts no human row and does not close M90.

## M90-F001 repaired preflight

Exact user reproduction: open **Compass Rose**, drag a point, then observe the complete canvas
blocked by `pointer input is unavailable while a managed-source mutation is compiling`. The
confirmed owner is the code-workbench bridge, not solver/DOF behavior. Pointer-up incorrectly
prepared a `SetValues` source mutation; underconstrained cold replay selected a different valid
configuration from the already accepted native continuation; exact parity rejected it; and the
pending compiler ticket remained installed, globally gating later input.

Focused native/bridge regressions now prove that an ordinary existing code-owned point drag,
including selected-reference detachment, publishes synchronously as a persistent
`CodeInteractionOverlay`: finite independently valid accepted geometry, exactly one outer/native
history action, unchanged source/lexical IR/executed artifact/compiler identity, no compiler
invocation, no `pending_managed_mutation`, and an immediately usable next pointer gesture. A forced
selected-reference terminal rejection proves that a failed or mismatched delegated terminal clears
its semantic route and restores exact accepted editor/selection/project/source/code-session/
persistence/history authority before that next gesture. Explicit source/scalar edits, insertion,
reorder, suppression, deletion and computed-Fillet radius edits continue through managed
compilation. No solver equation, rank/DOF rule, tolerance or branch state changed.

The four focused demo-web bridge regressions plus the sketch-code instance-overlay regression pass
`1/1` each; full demo-web `--lib` passes `282/282`. Sketch-code all-feature, affected multi-crate,
frontend Vitest `56/56`, TypeScript runtime `31/31` plus types/managed/build, release-WASM exact
two-consecutive-drag Compass `1/1`, locked WASM parity/`actual_wasm`/check, format, diff and
warnings-denied workspace Clippy pass. The golden survey is clean; after a transient combined exit
`101`, both affected scene rows pass exact rerun and complete unchanged `--check`/`--require-clean`
pass. The monolithic dirty release gate was harness-terminated at exit `143` and is **not** a pass.
The replacement freeze, exact verification and publication above pass provisionally. At that
historical checkpoint the replacement release gate and human retest remained pending.

## M90-F002 repaired preflight

Exact reproduction: open Compass Rose and make four accepted point drags. Complete Undo authority
grows the raw project to about `5.24 MiB`; the former synchronous write to
`localStorage["geosolve.project.v1"]` then crosses Chromium's approximately `5 MiB` Web Storage
quota and reports `QuotaExceededError`. This is a browser project-persistence defect, not a solver,
rank/DOF, branch or overlay failure.

Canonical project autosave now writes raw exact bytes to IndexedDB. Writes are serialized; restore
prefers IndexedDB; one legacy value migrates only after IndexedDB commits it; stale legacy bytes are
then removed; uncertain reads pause automatic saving until explicit Save; and read/write/removal
failures remain visible. Synchronous compression was rejected because it reached about `117 ms` at
four drags and `415 ms` at twenty, and reuse of reproduction-capsule limits rejected a valid
high-entropy project. Reproduction export remains compressed, but saved projects are raw and are
not subject to that codec's bounds.

Frontend Vitest `73/73`, production release-WASM build and nine-file distribution validation,
demo-web native `282/282`, focused sketch-code collateral, format, warnings-denied Clippy and diff
hygiene pass. Optimized release-WASM Chromium passes `1/1`: five drags create more than `5 MiB` of
exact IndexedDB data with no quota alert or legacy project key, then exact reload and Undo both
work. This automated evidence
accepts no UAT row.

## Withdrawn pre-F003 post-F002 replacement

Do **not** run the still-pending scorecard against these bytes. Tailscale-only unit
`geosolve-m90-f002-replacement-uat-18090.service` historically served frozen snapshot
`/tmp/geosolve-m90-uat.TN2NP9eF`. External manifest
`/tmp/geosolve-m90-uat.TN2NP9eF.sha256` has ordered aggregate
`06fb7b77e64cf1ead91b14accded0191a65a9b344fc538139e7a8eeb10b7c35f`. The snapshot contains nine
mode-`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,958,865`-byte
`assets/geosolve_demo_web_bg-D0927Gtv.wasm` has SHA-256
`b949a8ed46a39693e8f39132ce8eaae1bb0b29d07ead9b01603706ea8bef2a6a`.

All ten staging and live routes are byte-identical with correct MIME types, no redirects and no
compression; their ledger has SHA-256
`5247ef967f0a5f701181d08fa2fa9eb94a6302e17e5731ec8eacdefa3fb52ae6`. The exact frozen
release-WASM five-drag Compass quota regression passes `1/1` against both staging and live. Freeze
and verification evidence is `/tmp/geosolve-m90-f002-freeze-evidence.wZMAFe3d`.

M90-F003 withdraws this immutable snapshot from continuing UAT because it reproduces the missing
generated-unit import and direct-circle presentation-field mismatch. The historical service is
inactive/dead; its PID was `3839015` and invocation was
`0f6fca0683fa4ceead74c1355709b019`. The snapshot is historical pre-F003 reproduction evidence
only, not a current candidate, completed replacement release gate, clean-source qualification or
human acceptance. At that historical checkpoint M90-U1 through M90-U10 remained **Not run** and M90
remained open.

## M90-F003 repaired preflight

Exact user reproduction: choose **Start from code**, then draw one Center-Radius Circle with two
clicks. The prepared source correctly contains `radius: mm(...)`, `label` and `role`, but the empty
starter still imports only `sketch`; candidate compilation reports
`managed sketch mutation is invalid: unsupported managed sketch value expression` and publishes
no circle. This is a managed-source mutation/receipt defect, not a solver or circle-equation defect.

The repair makes the generated helper set part of the authenticated insertion. TypeScript and Rust
recursively derive only `mm` and `rad`, append each missing helper once to the existing
`@geosolve/sketch-code` import containing `sketch`, preserve every existing import and binding in
place, and bind the exact resulting imports into semantic-digest/receipt validation. Unit-free and
non-insertion mutations leave imports unchanged. Missing, extra, reordered or unrelated import
changes reject. The native direct-circle lowerer now also accepts the reverse projector's optional
`label` and `role`, preserves the display name and geometry role, and continues to reject every
other field.

The passing focused preflight starts from the exact authored-empty source and covers TypeScript
circle cold recompilation and runtime radius provenance; repeated/batched, already-imported,
unit-free and `rad` insertion; Rust exact-delta authority and import-forgery rejection; native
`label`/`role` lowering; and the retained bridge's complete two-click transaction. The bridge row
requires one finite accepted circle, Current features, independently validated normalized Hard
residual at most `1e-9`, exactly one `import { sketch, mm }`, one history action, Undo, no pending
mutation and an immediately usable next pointer gesture. An optimized release-WASM browser test
proves the same source-backed circle path before nomination.

TypeScript package tests pass their fixture checks, `34` runtime tests, types and managed source;
Rust prepared-mutation tests pass `34/34`, all-feature sketch-code and the exact retained bridge
pass, and golden `--check`/`--require-clean`, format, warnings-denied Clippy and diff hygiene pass.
Frontend Vitest passes `73/73` with TypeScript, manifest, licence, build-contract, isolated
production UI build and nine-file distribution checks. Optimized release-WASM Chromium passes the
F003 circle and carried F001/F002 regressions `1/1` each on both staging and live. Automated
preflight accepts no scorecard row.

## Withdrawn pre-F004 post-F003 replacement

Do **not** run the still-pending scorecard against these bytes. Historical Tailscale-only unit
`geosolve-m90-f003-replacement-uat-18090.service` served them with PID `305121`, invocation
`f456b1c8cea044638bae1119709b94d9`, bound to `100.94.63.83` and serving only frozen snapshot
`/tmp/geosolve-m90-uat.yIPVNICT`. External manifest
`/tmp/geosolve-m90-uat.yIPVNICT.sha256` has ordered aggregate
`d31e311c4e0e69974690d299819df6a33b7ae13b7eff5d037406e602c033f33a`. The snapshot contains nine
mode-`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,967,322`-byte
`assets/geosolve_demo_web_bg-DuuevZyx.wasm` has SHA-256
`c3160d7f8f6fc49db6294588cedd38ba5b520a80743d3977039957074fa8ca31`.

All ten staging and live routes are byte-identical with correct MIME types, no redirects and no
compression; their ledgers have the same SHA-256
`47a0229dbf46ea0549f4e424a6ce7ccd452810eb24161db61c4cb33436107726`. The exact frozen F003
circle regression and carried combined F001 overlay/F002 persistence regression each pass `1/1`
against both endpoints. Freeze and verification evidence is
`/tmp/geosolve-m90-f003-freeze-evidence.Y4jLQxD3`.

M90-F004 withdraws this immutable snapshot from continuing UAT because it predates deterministic
same-family declaration ownership for a multi-constraint canvas insertion. The historical service
is inactive/dead and the snapshot remains exact pre-F004 reproduction evidence. It is not a
completed replacement release gate, clean-source qualification or human acceptance. At that
historical checkpoint M90-U1 through M90-U10 remained **Not run** and M90 remained open.

## M90-F004 repaired preflight

Exact user reproduction: begin with two accepted Center-Radius Circles and draw one Segment whose
endpoints snap to the two circumferences. The managed candidate correctly contains one Segment, two
Point-on-Curve declarations and inferred Horizontal, but pre-repair terminal parity reports
`declaration relabel witness is not owned by its allocated source declaration`. The authenticated
compiler work had already finished; retaining that rejected ticket caused the misleading later
message `pointer input is unavailable while a managed-source mutation is compiling`.

The cause was ordering drift for simultaneously ready same-family declarations. GUI insertion
assigned reserved `constraintN` names in insertion order, while native materialization and cold
source replay allocated by durable Intent symbol. The two Point-on-Curve declarations exchanged
generated names and owned contact/source objects, so strict provenance correctly rejected the
mismatch. Canvas insertion now assigns those already reserved names in the same durable native-
symbol order. The frontend also aborts this exact unchanged authenticated terminal-native rejection
so the pending guard cannot survive it; stale or unauthenticated receipts remain pending.

The exact bridge regression passes `1/1` and proves one four-declaration batch, no early
publication, exactly one accepted source/history revision, two explicit periodic Point-on-Curve
contacts with retained parameter/domain/winding/neighbourhood/orientation, inferred Horizontal,
finite geometry, Current features, independently validated normalized Hard residual at most
`1e-9`, immediate next-pointer availability and exact Undo. Full demo-web passes `284/284`,
sketch-code all-feature passes, TypeScript runtime remains `34`, frontend Vitest passes `74/74`,
format/diff/warnings-denied Clippy pass, and the unchanged 271-row golden passes
`--check`/`--require-clean`. No golden expansion is warranted. The final optimized release-WASM
build and distribution checks pass; the combined F001/F002, F003 and F004 browser bundle passes
`3/3` against both frozen staging and live Tailscale. Automated preflight accepts no scorecard row.

No solver equation, rank/DOF rule, tolerance or explicit branch state changed.

## Withdrawn pre-F005 post-F004 replacement

Do **not** run the still-pending scorecard against these bytes. Tailscale-only unit
`geosolve-m90-f004-replacement-uat-18090.service` historically served frozen snapshot
`/tmp/geosolve-m90-uat.O4xZJBxg`. External manifest
`/tmp/geosolve-m90-uat.O4xZJBxg.sha256` has ordered aggregate
`fd0a4635edcc6bd24d36eeca831a57bbb62cdf1d67589c6245af9e7b88bf52be`. The snapshot contains nine
mode-`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,987,485`-byte
`assets/geosolve_demo_web_bg-CoONmEL1.wasm` has SHA-256
`e61ff5e9183883c1872293ad5d4c38c06175bc12575668f3f770282387bcf457`.

All ten staging and live routes are byte-identical with correct MIME types, no redirects and no
compression; their ledgers have SHA-256
`47589602797db38fb23a70da0d1cc31c7b032b7ab0907f3688d2a39886ebe921`. The combined
F001/F002, F003 and F004 optimized release-WASM browser bundle passes `3/3` against both endpoints.
Freeze and verification evidence is `/tmp/geosolve-m90-f004-freeze-evidence.F0HdfUMF`.

Historical PID was `792138` and invocation was `ff367dd5f5bb4f24a8661dd83a26168b`, bound to
`100.94.63.83` at `http://100.94.63.83:18090/`. The supervising user's reboot stopped the unit.
M90-F005 withdraws the bytes from continuing UAT because they predate its repair. This remains
immutable historical provisional dirty-tree evidence, not a completed normal release gate, clean-
source qualification or human acceptance.

## M90-F005 repaired preflight

Exact supplied workspace `/home/arduano/Downloads/project (1).json` is `956,305` bytes with SHA-256
`c5f748d31c90f8fd575ab2acaddfb8b7d20bbc9f31189dc0716995ad05a46ee7`. Checked-in capsule
`crates/geosolve-demo-web/tests/fixtures/m90_f005_native_drag_repro.txt` is `86,736` bytes with
SHA-256 `1b1dba9d039ee8756030174731ab3a03e7f77a8554853a96c29e4b10f8c49cf7` and decodes byte-for-byte
through the ordinary reproduction/workbench route. The exact symptom is a finite constrained
preview followed by pointer-up rejection and restoration of the old point position.

Two independent causes are repaired. First, authored scalar units now follow the owning leaf:
periodic contact `Parameter` is dimensionless even though native storage uses `ScalarUnit::Angle`.
One mapping governs ownership validation, bootstrap application and point/curve-control reverse
projection. Second, point release projects the exact already accepted terminal preview onto the
cold candidate design and independently certifies that numerical continuation; a disconnected,
underconstrained fresh solve no longer has to choose the same arbitrary representative. Curve-
control release uses the same seam. Undo/Redo uses the target history position's stored
authenticated accepted materialization as continuation, reconciling only candidate-owned retained
allocator high-water before exact projection.

Focused projectional applications pass `12/12`. Exact bridge regression
`m90_f005_supplied_native_workspace_constrained_drags_publish_and_undo` passes `1/1` for connected
point IDs ending `066d`, `0670`, `0673`, `0674`, `067e`, `0685` and `0686`; `...067f` remains the
unconstrained control. Every case requires a moved finite preview, exact terminal publication, no
managed compilation, one revision/history action, independently validated hard residual, contact-
metadata retention, exact accepted-document Undo, exact terminal Redo and persistence/reload.
Whole-workspace bytes are not required to match after Undo because CAS revision advances and Redo
history remains intentionally present.

No solver equation, rank/DOF rule, tolerance or explicit branch state changed. Constraint-editor
all-features passes with unit layer `439/439` plus every integration suite, demo-web passes
`286/286`, frontend Vitest passes `74/74`, warnings-denied Clippy, format/diff, the unchanged
reviewed 271-row golden, optimized release-WASM build and nine-file distribution validation pass.
The supplied-workspace row is included in the `4/4` browser bundle passing against staging and live.
Automated qualification accepts no scorecard row.

## M90-F006 repaired preflight

Frozen browser preflight exposed a presentation-host defect in successful `project.import`.
Replacing the bridge also reset its live host size and pixel ratio to defaults `1000 x 700` and `1`.
The DOM box itself was unchanged, so `ResizeObserver` emitted no new sample; SVG letterboxing and
Rust pointer normalization then used different coordinate spaces. Successful import now retains the
authenticated live `host_size` and `pixel_ratio` before atomic bridge replacement, while failed
restore remains completely non-mutating. Exact regression
`successful_project_import_retains_live_viewport_and_pointer_alignment` covers a non-default
letterboxed viewport and semantic point hover. No document, solver, branch, tolerance, history,
persistence or accepted-authority semantics change.

The first corrected browser attempt compared SVG screen coordinates from the drag-time camera with
coordinates after a deliberately fresh restore fit. Project bytes and accepted authority were
unchanged; this was `HARNESS_ERROR`, not another product finding. Fitting both presentation sides
makes the row deterministic. It passes `5/5` repeated while retaining persistence and Undo. The Rust
F005 owner separately proves both Undo and Redo.

## Current immutable candidate

Run the scorecard only against optimized snapshot `/tmp/geosolve-m90-uat.EtWyWQlt`, external
manifest `/tmp/geosolve-m90-uat.EtWyWQlt.sha256` and freeze evidence
`/tmp/geosolve-m90-f006-freeze-evidence.HqyaA7Qp`. Their ordered aggregate is
`b3fd72b9ec98d318d7bfa7bf8c09d0fcbd3856ea0723d81e01e64945e301debe`; the snapshot has nine
mode-`0444` files, two mode-`0555` directories and no symlinks. Its `19,990,463`-byte
`assets/geosolve_demo_web_bg-Dc5MH04n.wasm` has SHA-256
`bad16242c2ec0fa0c6c0ba6882428372c1bf0b7dd1a70235467b5febdfc80712`.

All ten staging/live routes byte-match with HTTP-ledger SHA-256
`41d11e1c56bad8dcc57edf229f0bfec20d8f5e602b3c385e5e68d54dd42c816a`. Optimized release-WASM
browser rows pass `4/4` on both: empty coded Center-Radius Circle, two-circle snapped Segment,
Compass persistence/reload and the supplied native contact-workspace drag. Tailscale-only unit
`geosolve-m90-f006-replacement-uat-18090.service`, PID `462021`, invocation
`4a17e69e926446eba21439ac4dd6f4e6`, exact-serves only this snapshot at
`http://100.94.63.83:18090/`.

## Final gate and scoped closeout

The final full dirty-tree gate command
`env GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from `23:20:45` through `23:47:22 AEST` on 2026-09-03 and exited `0` after `1,596,726 ms`.
Its `573,421`-byte log `/tmp/geosolve-m90-f006-full-gate.hKSTh4/release-gate.log` has SHA-256
`bda7f5f92f15a5f0a0cf26ed93cb514943d9a9d1ad49bf0ba0e148c9239b205d`. The complete gate keeps the
reviewed 271-row golden unchanged, passes the release-only 256-moving-body performance row in
`137.82 s`, frontend Vitest `74/74`, and the optimized nine-file distribution validation. This is
complete dirty-tree qualification, not clean-source qualification.

The supervising user explicitly approved scoped closure on 2026-09-04. Every M90-U1 through
M90-U10 row remains **Not run** and transfers/defers—not passes or waives—into M91's one composite
UAT. Automated qualification accepts no human row. The immutable F006 snapshot and service remain
the exact Tailscale-only closing publication; no GitHub Pages deployment or public push was
authorized or made. M90 is closed.

## Scorecard

Every row is **Not run** and transferred/deferred into M91's composite UAT. A passing automated
check is not a substitute for any interaction row. This retained list is the transferred contract,
not a claim that the current post-F006 candidate received row-by-row human execution.

1. **M90-U1 — Not run.** Start an empty coded sketch and draw one Center-Radius Circle with two
   clicks. Confirm that `sketch.ts` gains one compact declaration, changes its SDK import to exactly
   `import { sketch, mm } from "@geosolve/sketch-code";`, shows one accepted circle and remains
   immediately interactive. Undo must restore the exact empty starter. Then open Compass Rose, draw
   a three-vertex Polyline and accept one inferred constraint; confirm compact named declarations
   with no `recipe`, transport input table or result manifest. Drag an existing point five times,
   confirm no quota alert, reload the exact last position and verify Undo. Import the exact
   M90-F005 capsule identified above and drag each connected point ending `066d`, `0670`, `0673`,
   `0674`, `067e`, `0685` and `0686`; each release must retain its terminal preview. Reload the last
   move, Undo to its exact pre-drag accepted geometry and Redo to its exact terminal geometry.

2. **M90-U2 — Not run.** Draw a Quadratic Bezier and Cubic Bezier. Confirm readable
   `start`/`control`/`end` and `start`/`firstControl`/`secondControl`/`end` arguments and source
   navigation to each declaration.

3. **M90-U3 — Not run.** Draw two Center-Radius Circles, then one Segment whose endpoints snap to
   the two circumferences. Confirm one source-backed transaction adds the Segment, two explicit
   Point-on-Curve declarations and any displayed inferred axis relation, immediately releases
   pointer input and Undo restores the exact two-circle project. Then author representative point,
   curve, datum, contact and curvature constraints. Confirm each becomes a named typed declaration
   and survives reload, Undo and Redo.

4. **M90-U4 — Not run.** Author and edit one dimension from the Inspector. Confirm the exact source
   value changes and the selection remains on the same declaration.

5. **M90-U5 — Not run.** Create a Fillet, edit its radius, suppress, restore, delete and Undo.
   Confirm one direct `computed.filletSet` declaration owns the complete lifecycle and explicit
   branch state.

6. **M90-U6 — Not run.** Reorder declarations in the declaration panel. Confirm actual `sketch.ts`
   order changes; reject a dependency-invalid move without changing the accepted scene or history.

7. **M90-U7 — Not run.** In Typed Panel, edit the shared radius used by several runtime consumers.
   Confirm every intended consumer updates and no explicit edit-lens metadata appears in source.

8. **M90-U8 — Not run.** Open every bundled sample and confirm finite accepted geometry, usable
   selection and no legacy upgrade prompt or source rewrite.

9. **M90-U9 — Not run.** Run the documented browser-free inspect/render flow on one bundled sample.
   Then run prepare-edit, the pinned Deno sidecar and resolve-edit; confirm the output project and
   PNG/SVG reflect exactly one value change.

10. **M90-U10 — Not run.** Enter invalid or unsupported source and an invalid numeric edit. Confirm
    both retain the prior complete accepted canvas, source authority and history while showing a
    positioned diagnostic.

These rows transfer unchanged into M91 for composite human execution and later Pass, Fail or Waived
disposition. M90's scoped closure does not retrospectively pass or waive any row; automated
qualification pre-accepts none.
