<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M90 goals: one typed, executed sketch language

## Outcome

Status: **Closed by explicit scoped supervising-user approval on 2026-09-04. The full dirty-tree
gate and immutable Tailscale nomination pass. M90-U1 through M90-U10 transfer/defer, without pass or
waiver, into M91's composite UAT. No GitHub Pages deployment or public push was authorized or made.**

M90 is the clean break promised by M89. A managed `sketch.ts` is one typed authoring program whose
lexical IR and runtime recording can reproduce the complete standalone sketch. Source, structured
UX edits, canvas authoring and headless agent workflows use the same declaration vocabulary and the
same authenticated authority. Structural/source changes use the two-phase compiler transaction;
ordinary movement of an existing code-owned point is a persistent solver-instance overlay and does
not mutate or compile source.

## Required product contract

- The only managed directive is `"use geosolve sketch"`.
- The only admitted compiler formats are `geosolve-managed-sketch-ir-v3` and
  `geosolve-executed-sketch-artifact-v3`.
- Every public geometry, constraint, dimension, operation and aggregate has a named method and a
  typed named argument object. Computed Fillet uses `computed.filletSet` with explicit parent and
  branch state.
- The public surface has no generic recipe declaration, tuple-key input table, edit lens, result
  manifest or operation-output transport payload.
- Parsing owns lexical names, comments, order, groups, expressions and source spans. Instrumented
  execution owns declaration results and source-value consumer provenance. Rust independently
  authenticates both before materialization.
- A canvas-authored declaration is inserted into `sketch.ts` with a monotonic generated name. No
  accepted source project may retain a GUI-only geometry, constraint, dimension, operation or
  Fillet.
- Value edits, insertion, reorder, suppression and deletion are Rust-prepared mutations. A browser
  or pinned Deno compiler may produce a candidate receipt, but Rust alone validates the permitted
  semantic delta, materializes native Intent, solves, validates and publishes one history entry.
- Canvas insertion closes over every SDK helper required by its authenticated generated values.
  The current reverse projector may add only missing `mm` and `rad` named bindings to the existing
  `@geosolve/sketch-code` import that owns `sketch`; it preserves all existing import declarations
  and binding order, never duplicates a helper and never changes imports for a non-insertion
  mutation. Rust derives and authenticates that same exact import delta in the prepared ticket.
- An ordinary drag of an existing code-owned point publishes synchronously through
  `CodeInteractionOverlay`. It preserves accepted-continuation semantics and adds exactly one
  outer/native history action while leaving `sketch.ts`, lexical IR, executed artifact and compiler
  identity unchanged. It neither opens a managed-mutation ticket nor invokes the browser compiler.
- Compilation is reserved for explicit source/scalar edits, declaration insertion, reorder,
  suppression, deletion and computed-Fillet radius edits. Solver DOF or continuation behavior does
  not implicitly promote a point drag into a code-authoring transaction.
- Native point and curve-control release certifies the exact finite solver-accepted terminal preview
  as numerical continuation under the independently reconstructed candidate design. A disconnected,
  underconstrained cold solve may not replace it with another arbitrary valid representative;
  continuation grants no semantic, branch, history or publication authority.
- Undo and Redo use the target position's stored authenticated accepted materialization as numerical
  continuation, reconciling only candidate-owned retained allocator high-water before exact native
  projection and independent validation.
- Compiled projects can be inspected, solved and rendered without a browser, JavaScript runtime or
  web server. Raw-source mutation uses the pinned Deno sidecar and the same prepared receipt wire.
- All twelve bundled samples are normalized and checked in with their V3 compiler envelopes. There
  is no managed-v1/v2 upgrade or compatibility path.
- Canonical browser project persistence keeps the raw exact workspace in serialized IndexedDB
  transactions. A legacy `geosolve.project.v1` value migrates only after the same bytes commit,
  and an uncertain read fences autosave until explicit user Save rather than risking unread data.
- Bounded compressed reproduction transport remains separate from project persistence; exact saved
  projects do not inherit reproduction-capsule size limits or synchronous WASM compression cost.

## Required replay inventory

- all 25 geometry variants;
- all 33 standalone constraints, while the two host-external constraint variants remain explicitly
  input-gated rather than acquiring fabricated standalone authority;
- all eight dimensions;
- all ordinary operations and both aggregates;
- computed `FilletSet`, including keyed members and non-default branch/contact state;
- all twelve bundled samples through cold materialization and browser-free static rendering.

Every accepted replay requires finite native state, current active computed features and independent
hard-residual validation at normalized residual `<= 1e-9`.

## M90-F001 disposition

The code-workbench ownership repair is implemented and its focused qualification passes. Existing
code-owned point terminals now publish synchronous persistent overlays from accepted continuation;
failed or mismatched delegated terminals clear their semantic route and restore exact accepted
editor/selection/project/source/code-session/persistence/history authority. The forced selected-
reference rejection proves that the next gesture is immediately usable. This changes no solver
equation, rank/DOF rule, tolerance or branch state.

The former release-WASM candidate remains withdrawn. The subsequently frozen, byte-verified F001
provisional dirty-tree replacement is retained only as pre-F002 historical evidence; M90-F002 below
withdraws it from continuing UAT and its service is retired. The harness-terminated release-gate
attempt at exit `143` is not a pass or clean-source qualification. At that historical checkpoint,
replacement release-gate passage, M90-U1 through M90-U10 and milestone closure remained pending.

The replacement identity is snapshot `/tmp/geosolve-m90-uat.xk0AGnz0`, external manifest
`/tmp/geosolve-m90-uat.xk0AGnz0.sha256`, aggregate
`030e9f4aa98690b8cd35cdbb51a29220674f1bfcfba310192afc467f5afc38a4`, historically exact-served by
`geosolve-m90-f001-replacement-uat-18090.service` at the Tailscale-only endpoint
`http://100.94.63.83:18090/`. The unit is now inactive/dead; its historical PID was `3332035` and
invocation was `258e6d4da4b14661bd6d8e44856c64a5`. `PLAN.md` and `docs/M90_UAT.md` own its
complete WASM, route-ledger, file-mode and freeze-evidence identity.

## M90-F002 disposition

The browser quota repair is implemented and mechanically qualified. Canonical project persistence
is raw and exact in serialized IndexedDB transactions rather than quota-limited `localStorage`.
IndexedDB wins restore precedence; legacy bytes migrate one way only after a successful exact write,
stale duplicates are removed, and uncertain reads pause autosave until explicit user Save so unread
authority cannot be overwritten. Storage failures remain actionable UI diagnostics. This preserves
complete Undo history and changes no solver or source-authoring semantics.

Release-WASM measurement rejected synchronous persistence compression: medians reached about
`117 ms` at four Compass drags/`5.24 MiB` and `415 ms` at twenty drags/`21.49 MiB`, while a valid
`6 MiB` high-entropy patch exceeded the reused reproduction compressed-body limit. Raw persistence
therefore has no reproduction-capsule size bound; only reproduction export remains compressed.

Frontend Vitest `73/73`, the production release-WASM build and distribution validation, demo-web
native `282/282`, focused sketch-code collateral, format, warnings-denied Clippy and diff hygiene
pass.
An optimized release-WASM Chromium regression passes `1/1` with five drags, more than `5 MiB` of
exact IndexedDB workspace data, no quota alert or legacy project key, exact reload and working Undo.
The post-F002 build is frozen at `/tmp/geosolve-m90-uat.TN2NP9eF`, byte-verified at staging and live,
and was historically served by `geosolve-m90-f002-replacement-uat-18090.service` at
`http://100.94.63.83:18090/`; `PLAN.md` and `docs/M90_UAT.md` own its exact identity. At that
historical checkpoint all M90-U1 through M90-U10 rows and explicit milestone closure remained
pending/not run.

## M90-F003 disposition

The exact user reproduction starts an empty coded sketch, draws one center-radius circle with two
clicks and receives generated source containing `radius: mm(...)` while the starter import remains
`import { sketch } from "@geosolve/sketch-code";`. Candidate compilation then fails with
`managed sketch mutation is invalid: unsupported managed sketch value expression`, leaving the
accepted project unable to publish the circle. This is a managed-source mutation/receipt defect,
not a geometry-solver failure.

The repair makes generated helper imports part of the authenticated insertion semantics. Both the
TypeScript mutation host and Rust prepared-mutation owner recursively derive the closed generated
unit set (`mm`, then `rad`), append only missing bindings to the existing SDK import and include the
result in candidate semantic-digest authority. A receipt with an extra, removed, reordered or
otherwise unrelated import change rejects. Unit-free insertion and every non-insertion mutation
retain imports exactly. The direct native circle lowerer also accepts the reverse projector's
optional `label` and `role` presentation fields and preserves them in native Intent; this removes
the second cold-materialization blocker exposed after the missing import was corrected.

Focused TypeScript, Rust prepared-mutation/native-lowering and retained-bridge regressions own the
exact empty-starter circle path, import idempotence and `rad` closure, receipt forgery refusal,
finite independently validated publication, one history action, Undo and immediate next-pointer
availability. Those focused checks pass, and real optimized release-WASM browser regressions repeat
the F003 circle and carried F001/F002 paths on staging and live. The post-F002 snapshot is now
historical pre-F003 evidence. Fresh snapshot `/tmp/geosolve-m90-uat.yIPVNICT`, manifest
`/tmp/geosolve-m90-uat.yIPVNICT.sha256` and aggregate
`d31e311c4e0e69974690d299819df6a33b7ae13b7eff5d037406e602c033f33a` are byte-verified and served
historically by `geosolve-m90-f003-replacement-uat-18090.service` at
`http://100.94.63.83:18090/`; M90-F004 withdraws those bytes from continuing UAT and the service is
retired. `PLAN.md` and `docs/M90_UAT.md` own the complete historical identity. No M90-UAT row is
accepted; at that historical checkpoint the replacement release gate remained pending and M90
remained open.

## M90-F004 disposition

The exact reproduction begins with two accepted Center-Radius Circles, then draws one Segment whose
endpoints snap to the two circumferences. One managed insertion must contain the Segment, two
Point-on-Curve declarations and inferred Horizontal. Before repair, cold receipt resolution failed
strict terminal parity with `declaration relabel witness is not owned by its allocated source
declaration`; the unchanged pending ticket then left subsequent pointer input falsely blocked as
compiling.

The cause was deterministic declaration/native ownership drift. The GUI assigned simultaneously
ready same-family constraints their reserved `constraintN` names in insertion order, whereas the
unordered Intent patch and cold source replay allocate them by durable native symbol. The two
Point-on-Curve declarations therefore exchanged generated names and owned contact/source objects.
The repair assigns already reserved names within each managed namespace/name family in that same
durable native-symbol order. Strict relabel authentication remains intact. The frontend additionally
consumes this exact authenticated terminal-native rejection so a failed ticket cannot latch pointer
input; stale or unauthenticated receipts remain retryable.

The exact bridge owner passes `1/1` and requires one four-declaration transaction, no early
publication, exact Point-on-Curve parameter/domain/winding/neighbourhood/orientation state, finite
accepted geometry, Current features, independently validated normalized Hard residual at most
`1e-9`, one source/history revision, immediate next-pointer availability and exact Undo. Full
demo-web passes `284/284`, sketch-code all-feature passes, TypeScript runtime remains `34`, frontend
Vitest passes `74/74`, format/diff/warnings-denied Clippy pass, and the unchanged 271-row golden
passes `--check`/`--require-clean`. No golden expansion is warranted because the focused owner plus
thin frontend recovery test cover the defect without revealing a new systemic matrix dimension.

The historical provisional dirty-tree replacement is `/tmp/geosolve-m90-uat.O4xZJBxg`, manifest
`/tmp/geosolve-m90-uat.O4xZJBxg.sha256`, aggregate
`fd0a4635edcc6bd24d36eeca831a57bbb62cdf1d67589c6245af9e7b88bf52be`. Historical unit
`geosolve-m90-f004-replacement-uat-18090.service` served it at
`http://100.94.63.83:18090/` until the supervising user's reboot stopped the service. M90-F005
withdraws those bytes from continuing UAT while retaining the immutable identity and qualification
ledger in `PLAN.md` and `docs/M90_UAT.md`. No solver equation, rank/DOF rule, tolerance or explicit
branch state changed. At that historical checkpoint the replacement release gate, M90-U1 through
M90-U10 and explicit closure remained pending, and M90 was open.

## M90-F005 disposition

The exact supplied native project reproduced release snapback across seven connected points after
ordinary finite previews. Original `/home/arduano/Downloads/project (1).json` is `956,305` bytes at
SHA-256 `c5f748d31c90f8fd575ab2acaddfb8b7d20bbc9f31189dc0716995ad05a46ee7`; checked-in capsule
`crates/geosolve-demo-web/tests/fixtures/m90_f005_native_drag_repro.txt` is `86,736` bytes at
SHA-256 `1b1dba9d039ee8756030174731ab3a03e7f77a8554853a96c29e4b10f8c49cf7` and decodes byte-exactly.

The repair first makes authored scalar units follow their owning leaf semantics: a periodic curve's
native angle-backed contact `Parameter` remains dimensionless. The same mapping governs ownership
validation, bootstrap application and point/curve-control projection. Point release then certifies
the exact solver-accepted terminal preview as the candidate's numerical continuation rather than
requiring a disconnected, underconstrained fresh solve to choose the same arbitrary representative.
Curve-control release uses the same seam. Undo/Redo certification uses each target history
position's stored authenticated accepted materialization, reconciling only candidate-owned retained
allocator high-water before exact projection.

Focused projectional applications pass `12/12`; exact bridge regression
`m90_f005_supplied_native_workspace_constrained_drags_publish_and_undo` passes `1/1` for point IDs
ending `066d`, `0670`, `0673`, `0674`, `067e`, `0685` and `0686`. It requires exact terminal
publication, independent hard-residual validation, contact-metadata retention, one history action,
exact accepted-document Undo, exact terminal Redo and persistence/reload. No equation, rank/DOF
rule, tolerance or explicit branch state changed.

F005 collateral passes constraint-editor all-features with unit layer `439/439` plus every
integration suite, demo-web `286/286`, frontend Vitest `74/74`, warnings-denied Clippy, format/diff,
the unchanged reviewed 271-row golden, optimized release-WASM build and nine-file distribution
validation. Its supplied-workspace browser row is included in the `4/4` bundle passing against both
staging and live. Automated qualification accepts no M90-UAT row.

## M90-F006 disposition

Frozen browser preflight exposed a thin presentation-host defect. Successful `project.import`
replaced the bridge's live host extent and pixel ratio with defaults while the unchanged DOM box
generated no new `ResizeObserver` sample. SVG letterboxing and Rust pointer normalization therefore
used different coordinate spaces. Successful import now retains the authenticated live `host_size`
and `pixel_ratio` before atomic bridge replacement; failed restore remains entirely non-mutating.
Exact regression `successful_project_import_retains_live_viewport_and_pointer_alignment` covers a
non-default letterboxed viewport and semantic point hover.

The first corrected browser attempt compared screen coordinates across a deliberately fresh camera
fit. Exact persisted authority was unchanged, so it was a `HARNESS_ERROR`. Fitting both presentation
sides makes the row deterministic; it passes `5/5` repeated with persistence and Undo. The Rust F005
owner separately proves Undo and Redo.

Current immutable snapshot `/tmp/geosolve-m90-uat.EtWyWQlt`, manifest
`/tmp/geosolve-m90-uat.EtWyWQlt.sha256` and freeze evidence
`/tmp/geosolve-m90-f006-freeze-evidence.HqyaA7Qp` have aggregate
`b3fd72b9ec98d318d7bfa7bf8c09d0fcbd3856ea0723d81e01e64945e301debe`, nine mode-`0444` files,
two mode-`0555` directories and no symlinks. Its `19,990,463`-byte
`assets/geosolve_demo_web_bg-Dc5MH04n.wasm` has SHA-256
`bad16242c2ec0fa0c6c0ba6882428372c1bf0b7dd1a70235467b5febdfc80712`. Staging/live HTTP ledgers
match at SHA-256 `41d11e1c56bad8dcc57edf229f0bfec20d8f5e602b3c385e5e68d54dd42c816a`, and the optimized
release-WASM browser bundle passes `4/4` on both. Tailscale-only unit
`geosolve-m90-f006-replacement-uat-18090.service`, PID `462021`, invocation
`4a17e69e926446eba21439ac4dd6f4e6`, exact-serves that snapshot at
`http://100.94.63.83:18090/`.

## M90 closeout disposition

The final full dirty-tree gate command
`env GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from `23:20:45` through `23:47:22 AEST` on 2026-09-03 and exited `0` after `1,596,726 ms`.
Its `573,421`-byte log `/tmp/geosolve-m90-f006-full-gate.hKSTh4/release-gate.log` has SHA-256
`bda7f5f92f15a5f0a0cf26ed93cb514943d9a9d1ad49bf0ba0e148c9239b205d`. The complete gate preserves
the reviewed 271-row golden unchanged, passes the release-only 256-moving-body performance row in
`137.82 s`, frontend Vitest `74/74`, and the optimized nine-file distribution validation. This is
complete dirty-tree qualification, not clean-source qualification.

The supervising user explicitly approved scoped closure on 2026-09-04. M90-U1 through M90-U10
remain unexecuted and transfer/defer—not pass or waive—into M91's one composite UAT. Automated
qualification accepts no human row. The immutable F006 snapshot and service above remain the exact
Tailscale-only closing publication; no GitHub Pages deployment or public push was authorized or
made. M90 is closed.

## Explicit non-goals

- Profile Offset may retain its existing explicit helper/root declaration closure; M90 does not
  broaden the Offset feature set.
- Custom patch modules remain compiled, pinned extension points and are not required to be
  reversible into managed lexical IR.
- TypeScript records declarations but does not own solver equations, native materialization,
  convergence, branch validation or publication.
- M90-F001 changes no solver equation, rank/DOF rule, tolerance or branch definition; its
  implemented repair restores the code-workbench ownership boundary for existing-point movement
  and atomically restores accepted authority after any failed delegated terminal.
- M90-F002 changes no solver equation, history fidelity or canonical project contents. It changes
  only the browser persistence medium and its migration/error coordination; small non-authoritative
  UI preferences and unapplied drafts may continue to use Web Storage.
- M90-F003 changes no solver equation, residual, circle seed, rank/DOF rule, tolerance or branch
  state. It closes generated SDK imports inside the authenticated managed insertion and admits the
  reverse projector's existing optional circle presentation fields at native lowering.
- M90-F004 changes no solver equation, rank/DOF rule, tolerance or branch state. It makes
  same-family declaration naming agree with existing native allocation authority and consumes only
  an authenticated terminal-native rejection at the browser boundary.
- M90-F005 changes no solver equation, rank/DOF rule, tolerance or branch state. It aligns authored
  scalar units with leaf semantics and independently certifies an already accepted numerical
  continuation under the exact candidate design; continuation never grants semantic or publication
  authority.
- M90-F006 changes only presentation-host viewport retention across successful import. It changes no
  document, solver equation, branch, tolerance, history, persistence or accepted-authority
  semantics.
- No backward compatibility is retained for old managed source or generic declaration APIs.
