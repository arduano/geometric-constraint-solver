<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M90 implementation: typed APIs and executed reversible authority

Status: **Implementation and complete dirty-tree qualification are finished; M90 closed by explicit
scoped supervising-user approval on 2026-09-04. M90-U1 through M90-U10 transfer/defer into M91's
composite UAT without pass or waiver. Closing publication is Tailscale-only.**

## Architecture

M90 completes ADR 0043 as a clean break. `packages/geosolve-sketch-code` parses the closed managed
TypeScript subset, prints its canonical source and executes it through an instrumented recorder. The
compiler returns both the complete lexical IR and executed artifact. Stable source-site IDs bind
runtime consumers back to exact UTF-8 source spans; stack traces are diagnostic evidence only and
never select an edit target.

`geosolve-sketch-code` validates source, IR and artifact digests, reconstructs the Rust-owned result
shape for every named declaration family, expands the equation-free program into Intent and sends it
through the ordinary native solver and independent validation path. Neither the TypeScript compiler
nor the demo web adapter duplicates solver equations.

## Public authoring surface

The root package exports named builders grouped as `geometry`, `constraint`, `dimension`,
`operation`, `aggregate` and `computed`. Arguments use semantic property names—for example
`quadraticBezier({ start, control, end })`, `cubicBezier({ start, firstControl, secondControl,
end })`, and `coincident({ first, second })`. The former `[name, kind, value]` input tables and
generic recipe APIs are not public or admitted by V3 authority.

Runtime result references remain strongly branded by project and feature kind. Named methods are
compile-time generated from the same central Rust authoring/declaration inventory that authenticates
their result descriptors; parity tests prevent TypeScript and Rust catalogs from drifting.

## Reversible transactions

Rust prepares `geosolve-prepared-managed-mutation-v1` tickets for exact value compare-and-swap,
insertion, reorder, suppression and deletion. The browser or pinned Deno host applies the mutation,
normalizes and executes the candidate, then returns the complete compiler envelope. Rust validates
the ticket, accepted identity, exact semantic delta and compiler envelope before cold native
materialization. Only a finite, independently valid candidate publishes source, IR, artifact,
accepted scene and one outer history row.

Insertion semantics include their generated SDK-helper closure. The TypeScript host recursively
walks authenticated draft values, derives only `mm` and `rad`, and appends each missing binding once
to the existing `@geosolve/sketch-code` named import that contains `sketch`. Existing import and
binding order is preserved. Rust performs the same derivation before calculating the candidate
semantic digest, so the receipt may contain exactly that import delta and no unrelated addition,
removal or reorder. Unit-free insertion and all non-insertion mutations preserve imports exactly.

That protocol backs explicit code-panel/source and scalar edits, Inspector source values, computed
Fillet radius edits, canvas declaration insertion, reorder, suppression and deletion. Runtime
value-consumer provenance supplies fan-out automatically, so a shared lexical value edits every
authenticated consumer without an `editLens` declaration.

Ordinary movement of an existing code-owned point has a different owner. Pointer preview and
terminal publication stage the semantic point set into `CodeInteractionOverlay`, incrementally
materialize from the accepted continuation, validate terminal parity and synchronously publish the
prepared project overlay. A completed drag adds exactly one outer/native history action but does
not change source, lexical IR, executed artifact or compiler identity and never creates a pending
managed mutation. This separation keeps solver-instance placement independent of code authoring;
only an explicit source/scalar or structural edit crosses the compiler host.

## M90-F001 ownership regression

Exact user reproduction: open **Compass Rose**, drag a point, then observe that the complete canvas
is blocked by `pointer input is unavailable while a managed-source mutation is compiling`.
Independent reproduction confirmed the same route for every Compass Rose point. This is a
code-workbench/Rust-bridge ownership defect, not a solver, rank/DOF or equation defect.

The regressed pointer-up path converted the point release into
`ManagedSketchMutation::SetValues` and sent it to the browser compiler. Native preview had already
accepted one valid continuation, but cold source rematerialization from the rewritten seed selected
a different valid configuration of the underconstrained sketch. Exact terminal parity correctly
rejected that divergence. The rejected compiler ticket was then retained as
`pending_managed_mutation`, so the bridge's global pending guard reported the misleading
"compiling" message and rejected every subsequent pointer input.

The implemented repair restores synchronous `CodeInteractionOverlay` publication for these drags,
including selected-reference detachment, while keeping all explicit managed-source mutation routes
compiled and authenticated. Successful ordinary terminals use accepted continuation, publish one
persistent overlay and one native history action, retain exact source/IR/artifact/compiler identity
and create no pending mutation or compiler request. A failed or mismatched delegated terminal
clears its semantic route and restores the exact accepted editor, selection, project, source,
code-session, persistence and history authority. The forced selected-reference rejection proves
the next pointer input remains available. No solver equation, rank/DOF rule, tolerance or branch
definition changed.

As defense in depth for genuine compiler-backed edits, the frontend now aborts the exact unchanged
pending ticket after an authenticated terminal native rejection, clearing the global pointer guard.
Stale/authentication failures and receipts for another ticket remain pending and retryable. This
fallback is not used by ordinary point drags because they no longer create a compiler ticket.

## M90-F002 exact browser persistence

Exact reproduction confirmed that four accepted Compass Rose drags grow the raw saved workspace to
about `5.24 MiB`. Each Undo entry intentionally retains a complete roughly `1 MiB` authority
snapshot; the former synchronous `localStorage.setItem("geosolve.project.v1", ...)` therefore
crossed Chromium's approximately `5 MiB` per-origin Web Storage quota and surfaced
`QuotaExceededError`. Trimming history or storing a lossy projection was rejected because either
would weaken canonical project authority.

`frontend/src/lib/project-storage.ts` now owns canonical browser project persistence. It stores the
raw project string in IndexedDB database `geosolve.browser-projects.v1`, object store `projects`,
key `current`. All reads, writes and removals share one promise queue, so a slow older write cannot
overtake a newer revision. `App.tsx` waits for the authoritative read before enabling autosave,
deduplicates byte-identical automatic writes, and preserves explicit Save as the recovery action
after an uncertain read.

Migration is fail-safe. A present IndexedDB value wins and any stale legacy project key is removed.
When IndexedDB is confirmed empty, a legacy value is copied first and removed only after the exact
IndexedDB write commits. Failed migration continues to restore and retain the legacy bytes. If
IndexedDB or the legacy slot cannot be read with certainty, the fallback may render but automatic
saves remain fenced, so it cannot overwrite possibly newer unread data. Invalid restore removes
saved authority only after a fresh fallback is ready. Read, write, removal and stale-key-cleanup
failures use the existing durable application alert.

Synchronous Rust/WASM zlib persistence compression was prototyped and then removed. It shrank
ordinary data by roughly `90%`, but `persistProject()` executes synchronously on the main thread:
measured medians were about `27 ms` at the initial `1.18 MiB`, `117 ms` at four drags/`5.24 MiB`,
`195 ms` at eight drags/`9.30 MiB` and `415 ms` at twenty drags/`21.49 MiB`. More importantly, a
valid workspace containing a `6 MiB` high-entropy custom patch produced a `14,329,357`-byte
compressed reproduction body and was rejected by the reused `12,582,912`-byte limit. Exact project
persistence therefore stays raw and does not inherit reproduction bounds. The reproduction export
codec remains compressed. Raw A/B measurements were about `49 ms` serialization plus `13 ms`
IndexedDB write at four drags, and `138 ms` plus `59 ms` at twenty drags.

## M90-F003 generated-helper closure

Exact user reproduction: choose **Start from code**, leave the authored-empty starter unchanged,
select Center-Radius Circle and click its center and radius. Reverse projection prepared this
declaration correctly:

```ts
const geometry1 = $.geometry.centerRadiusCircle("geometry1", {
  center: [-3.633667933314297, 5.39811540404595],
  label: "center-radius-circle-0000000000000001",
  radius: mm(1.2599398842396183),
  role: "profile",
});
```

However, the mutation printer retained the starter's `import { sketch }` unchanged. The parser
admits a unit call only when its helper is statically imported, so candidate compilation reported
`managed sketch mutation is invalid: unsupported managed sketch value expression`. Simply adding
`mm` to the checked-in starter would hide the same defect for every other generated length and for
generated angles.

The implemented systemic repair closes insertion over generated helpers on both sides of the
compiler boundary. TypeScript adds missing helpers before printing and recompiling; Rust derives
the identical expected import list before binding the prepared ticket's semantic digest and before
validating the receipt. The admitted generated set is deliberately only `mm` and `rad`, in that
canonical order. Existing imports stay where they are, already imported bindings remain where they
are, each missing binding is appended at most once to the SDK import containing `sketch`, and an
unsupported unit or absent owning import fails closed. Receipt validation rejects a missing helper,
an extra helper, binding reorder, removed binding, changed custom import or added unrelated import.

After that first blocker was removed, exact cold materialization exposed a second mismatch: canvas
reverse projection includes optional `label` and `role`, while the direct center-radius-circle
lowerer admitted only `center` and `radius`. The lowerer now admits only those two required fields
plus the two optional presentation fields, maps `label` to the native display name and validates/
maps `role` through the ordinary geometry-role field. No circle equation, seed, radius, branch,
rank or tolerance behavior changed.

The owning regression stack is intentionally layered. TypeScript starts from the exact sketch-only
authored-empty source, inserts a circle, cold-recompiles `import { sketch, mm }`, and retains radius
consumer provenance; repeated/batched insertion, pre-existing imports, unit-free insertion,
generated `rad`, unsupported units and custom-import preservation cover the systemic closure. Rust
prepared-mutation tests authenticate that exact import delta and reject forged alternatives; a
native lowering test owns optional circle `label`/`role`. The retained bridge test performs the
two-click empty-source gesture, resolves the real compiler receipt, requires one finite accepted
circle, Current features and independently validated normalized Hard residual at most `1e-9`, one
source/history publication, exact one-time `mm` import, Undo and immediate subsequent pointer
input. The final adapter check is the equivalent optimized release-WASM browser flow.

The post-F002 snapshot and service identity below are retained only as historical pre-F003
evidence. Focused TypeScript/Rust/frontend qualification and the exact retained bridge pass; the
optimized release-WASM F003 circle and carried F001/F002 browser regressions pass on staging and
live. The byte-verified post-F003 identity is recorded after the historical ledger below. No human
UAT row or milestone closure is implied.

## M90-F004 simultaneous declaration ownership

Exact reproduction starts from managed source containing two Center-Radius Circles. A Segment is
drawn between the two circumferences so native inference supplies two Point-on-Curve relations and
one Horizontal relation. The candidate editor correctly prepares all four declarations in one
`InsertDeclarations` mutation and publishes nothing before compiler receipt authentication.

Before repair, the receipt passed managed compilation and cold materialization but failed strict
terminal parity with `declaration relabel witness is not owned by its allocated source declaration`.
Several new constraint declarations become ready in the same unordered Intent patch. The GUI
candidate assigned its reserved monotonic `constraintN` symbols in insertion order, while native
materialization and cold source replay allocate those nodes by durable Intent symbol. The two
Point-on-Curve declarations consequently exchanged managed names and their owned contact/source
objects. Exact declaration-to-native provenance authentication correctly rejected that mismatch;
this was not slow or unfinished compilation.

`allocate_canvas_declaration_names` now groups new nodes by managed namespace/name family. When a
family has several members, it derives each prospective direct-declaration Intent symbol, sorts
the already reserved names by that durable symbol and assigns them back to the candidate nodes in
the same order native materialization will use. The global generated-name high-water remains
monotonic, declaration/source ordering stays explicit, and no native ID is inferred from
coordinates.
`authenticated_declaration_object_relabels` remains fail-closed: each terminal persistent node,
staged declaration, native owner and exact canonical label suffix must still match uniquely.

As defense in depth, `pending-managed-mutation.ts` treats an unchanged authenticated
`declaration relabel witness ...` failure as terminal native rejection. It aborts that exact ticket
and requires a settled snapshot before another pointer gesture. Receipt authentication/staleness
failures remain pending and retryable, so the adapter cannot turn a different or unauthenticated
failure into publication authority.

The owning bridge regression
`m90_f004_two_circles_snapped_segment_publishes_one_managed_batch` uses checked compiler fixtures
for the two-circle base and snapped-Segment result. It requires one four-declaration mutation, no
early source/revision/history publication, exact cold receipt publication, two periodic
Point-on-Curve contacts with preserved parameter/domain/winding/neighbourhood/orientation, one
Horizontal relation, explicit finite Segment branch, Current features, finite state, independently
validated normalized Hard residual at most `1e-9`, an immediately usable complete next pointer
gesture and exact Undo. The thin frontend regression separately recreates the old relabel rejection
and proves abort precedes the next pointer sample.

No solver equation, rank/DOF rule, tolerance or explicit branch state changes. The 271-row golden
does not expand: this focused declaration-ownership/lifecycle case is fully represented at the
lowest public Rust code-project/prepared-mutation/native-parity boundary plus the thin browser
recovery boundary and exposes no missing family, branch, transform, operand-order or authority-state
axis.

## M90-F005 exact native terminal continuation

The exact user payload is `/home/arduano/Downloads/project (1).json`, `956,305` bytes with SHA-256
`c5f748d31c90f8fd575ab2acaddfb8b7d20bbc9f31189dc0716995ad05a46ee7`. Bounded fixture
`crates/geosolve-demo-web/tests/fixtures/m90_f005_native_drag_repro.txt` is `86,736` bytes with
SHA-256 `1b1dba9d039ee8756030174731ab3a03e7f77a8554853a96c29e4b10f8c49cf7` and inflates byte-
identically through the ordinary reproduction decoder. The restored project is native-only; a
point preview moved and was accepted, but pointer-up rejected the terminal candidate and restored
the old scene for almost every connected point.

The first defect was an authored/native unit conflation. Native storage uses
`ScalarUnit::Angle` for a periodic curve contact parameter, but the owning Intent
`LeafField::Parameter` is dimensionless. `scalar_leaf_intent_unit` now supplies one closed mapping
for writable ownership validation, bootstrap application, point-drag reverse projection and curve-
control reverse projection. Generic `Value` inherits `Length`, `Angle` or dimensionless Parameter
storage; `Angle` is authored as angle; `Weight` and `Parameter` are authored as dimensionless;
angle-backed periodic `Parameter` is the deliberate cross-unit case. Unsupported leaf/storage
combinations remain invalid.

Correct unit projection exposed the independent terminal-continuation defect. The supplied scene is
disconnected and underconstrained. Sequential cold materialization of the patched candidate could
choose a different valid numerical representative for unrelated free degrees of freedom than the
retained solver had already accepted for the terminal preview. Exact
`direct_manipulation_preview_matches_cold` then reported `PreviewColdMismatch`, correctly refusing
to publish the divergent cold state but making the valid gesture appear to snap back.

`finish_point_drag_with_work` now plans the exact Intent patch while supplying the terminal accepted
preview document to a private accepted-continuation materialization path. Cold lowering still
reconstructs the complete candidate design, native objects, ownership and prepared input. The
retained session projects the supplied numerical continuation onto that exact design and
independently certifies it before parity and publication. The continuation supplies no source
declaration, Intent operation, temporary drag request, solver-success status, history row or branch
choice. Object/topology/source mismatch, non-finite state or excessive hard residual still rejects.
`finish_curve_control_drag_with_work` uses the same path.

History refresh also avoids arbitrary replay selection. `restore_history_position` decodes the
target position's authenticated accepted materialization and supplies it as the numerical
continuation while refreshing accepted evidence. Because Intent history deliberately retains the
largest persistent-identity cursor ever observed, an older authenticated document may carry the
same durable objects with an earlier allocator high-water. Before projection, only that candidate-
owned retained high-water metadata advances to the cold design's cursor. All native objects,
topology rows, source fields, contacts, branches and numerical values remain subject to exact
projection and independent certification. Undo therefore restores the pre-drag accepted document
and Redo restores the exact terminal document.

The bridge fixture independently recreates the workspace for seven connected points:
`7b80e0003fe358f731b6e557427a066d`, `7b80e0003fe358f731b6e557427a0670`,
`7b80e0003fe358f731b6e557427a0673`, `7b80e0003fe358f731b6e557427a0674`,
`7b80e0003fe358f731b6e557427a067e`, `7b80e0003fe358f731b6e557427a0685` and
`7b80e0003fe358f731b6e557427a0686`; `...067f` remains an unconstrained control. Each case requires
a moved finite preview, exact terminal publication, no managed compiler request, one revision and
Intent history action, independent hard-residual validation, contact metadata retention, exact
accepted-document Undo, exact terminal Redo and persistence/reload. Whole-workspace bytes are not
compared after Undo because the composite history intentionally advances its CAS revision and
retains one Redo action.

Focused projectional applications pass `12/12`, including the scalar-unit matrix, Point-on-Curve,
mixed line/circle tangency and disconnected contact-rich continuation regressions. Exact bridge
test `m90_f005_supplied_native_workspace_constrained_drags_publish_and_undo` passes `1/1`. No solver
equation, rank/DOF rule, tolerance or explicit branch state changes. Constraint-editor all-features
passes with unit layer `439/439` plus every integration suite, demo-web passes `286/286`, frontend
Vitest passes `74/74`, warnings-denied workspace and targeted Clippy pass, and format/diff plus the
unchanged reviewed 271-row golden pass. Optimized release-WASM build and nine-file distribution
validation pass; the supplied-workspace browser row is included in the `4/4` bundle passing against
both staging and live.

## M90-F006 imported viewport preservation

Frozen browser preflight exposed a presentation-host defect in successful `project.import`.
Replacing the whole bridge also replaced the live host extent and pixel ratio with defaults
`1000 x 700` and `1`. Because the DOM box itself had not changed, `ResizeObserver` emitted no new
sample. SVG letterboxing kept using the real browser dimensions while Rust pointer normalization
used the defaults, so a visibly aligned point could miss semantic hover.

Successful import now captures and restores the authenticated live `host_size` and `pixel_ratio`
after complete project restoration and before atomic bridge replacement. Failed restore remains
non-mutating. Exact regression `successful_project_import_retains_live_viewport_and_pointer_alignment`
covers a non-default letterboxed viewport and semantic point hover. This changes no document,
solver, branch, tolerance, history, persistence or accepted-authority semantics.

The first corrected browser attempt compared drag-camera screen coordinates with coordinates after
a deliberately fresh restore fit. Exact project and accepted authority were unchanged, so this was
`HARNESS_ERROR`, not another product finding. Fitting both presentation sides makes the row
deterministic; it passes `5/5` repeated with persistence and Undo. The Rust F005 owner separately
proves both Undo and Redo.

## Headless workflow

`geosolve-headless inspect` and `render` consume a bundled sample or canonical project JSON directly.
`prepare-edit` emits a Rust-authenticated compiler request; the pinned Deno mutation sidecar consumes
that request and emits a receipt; `resolve-edit` authenticates, cold-solves, independently validates
and writes the new project plus SVG/PNG/report files into a new output directory. Existing output is
never overwritten.

## Qualification record

As of 2026-09-03, the four focused demo-web bridge commands plus sketch-code
`managed_v3_instance_overlay_is_exact_cas_history_and_persistence_authority` pass `1/1` each. Full
demo-web `--lib` passes `282/282`; sketch-code all-feature and affected multi-crate suites pass;
frontend Vitest passes `56/56`; TypeScript runtime passes `31/31` with types, managed checks and
production build; release-WASM exact Compass two-consecutive-drag Playwright passes `1/1`; locked
WASM parity, demo-web `actual_wasm`, WASM check, format, diff and warnings-denied workspace Clippy
pass. Generic golden survey is clean; following one transient combined exit `101`, both affected
scene rows pass exact rerun and complete unchanged `--check`/`--require-clean` pass.

The monolithic dirty release gate was harness-terminated at exit `143` and is not a pass. A
subsequent optimized release-WASM build is frozen as provisional dirty-tree replacement evidence at
`/tmp/geosolve-m90-uat.xk0AGnz0`, external manifest
`/tmp/geosolve-m90-uat.xk0AGnz0.sha256`, aggregate
`030e9f4aa98690b8cd35cdbb51a29220674f1bfcfba310192afc467f5afc38a4`, with nine mode-`0444`
files, two mode-`0555` directories and no symlinks. Its `19,958,913`-byte
`assets/geosolve_demo_web_bg-B6mOdH7K.wasm` has SHA-256
`9607cfd48f1ec23b2c29e120704277bbb70247bdbed6a67945c5cc64b7af8761`.

All ten pre-F002 staging/live routes byte-match with correct MIME, no redirects/compression and ledger
SHA-256 `56a5aff7b23000e1b009f2eb479b9545fcfb17dbe5d1a4f9721f7c3951a761ae`; optimized
release-WASM two-consecutive-drag Compass passes `1/1` on each. Evidence is
`/tmp/geosolve-m90-f001-replacement-freeze-evidence.UpMqFyrm`. Tailscale-only
`geosolve-m90-f001-replacement-uat-18090.service` historically served that snapshot with PID
`3332035`, invocation `258e6d4da4b14661bd6d8e44856c64a5`; it is now inactive/dead.

M90-F002 adds passing project-store tests for exact large values, ordered writes, migration and
failure retention; application tests for startup/save/replacement/uncertain-read coordination; and
an optimized release-WASM Chromium regression. Frontend Vitest passes `73/73`, and the production
release-WASM build plus nine-file distribution validation pass. Full demo-web native tests pass
`282/282`, with focused sketch-code collateral, format, warnings-denied Clippy and diff hygiene also
green. The Chromium regression passes `1/1`: five Compass Rose drags advance IndexedDB beyond
`5 MiB` without
a quota alert or legacy project key, reload restores the exact saved fingerprint and point, and
Undo changes it as expected. No solver equation, rank/DOF rule, tolerance or branch state changed.

The post-F002 optimized build is frozen at `/tmp/geosolve-m90-uat.TN2NP9eF`, external manifest
`/tmp/geosolve-m90-uat.TN2NP9eF.sha256`, aggregate
`06fb7b77e64cf1ead91b14accded0191a65a9b344fc538139e7a8eeb10b7c35f`, with nine mode-`0444`
files, two mode-`0555` directories and no symlinks. Its `19,958,865`-byte
`assets/geosolve_demo_web_bg-D0927Gtv.wasm` has SHA-256
`b949a8ed46a39693e8f39132ce8eaae1bb0b29d07ead9b01603706ea8bef2a6a`.

All ten staging/live routes byte-match with correct MIME, no redirects/compression and ledger
SHA-256 `5247ef967f0a5f701181d08fa2fa9eb94a6302e17e5731ec8eacdefa3fb52ae6`; the exact frozen
five-drag quota regression passes `1/1` on each. Evidence is
`/tmp/geosolve-m90-f002-freeze-evidence.wZMAFe3d`. Tailscale-only
`geosolve-m90-f002-replacement-uat-18090.service` historically served only that snapshot with PID
`3839015`, invocation `0f6fca0683fa4ceead74c1355709b019`, on `100.94.63.83` at
`http://100.94.63.83:18090/`.

M90-F003 withdraws those bytes from continuing UAT because they predate generated-helper closure
and direct-circle presentation-field lowering. The F002 service is inactive/dead and its immutable
snapshot remains historical reproduction evidence only.

M90-F003 qualification passes TypeScript fixture checks, `34` runtime tests, type/managed checks and
the exact pinned-Deno mutation; Rust prepared-mutation `34/34`, all-feature sketch-code, the exact
retained bridge, format, warnings-denied Clippy, diff hygiene and golden `--check`/
`--require-clean`; frontend Vitest `73/73`, TypeScript, manifest, licence, build-contract, isolated
production UI build and nine-file distribution validation. Optimized release-WASM Chromium passes
the F003 circle and carried F001/F002 regressions `1/1` each against staging and live.

The optimized release-WASM output is frozen read-only at `/tmp/geosolve-m90-uat.yIPVNICT`, with
external manifest `/tmp/geosolve-m90-uat.yIPVNICT.sha256` and ordered aggregate
`d31e311c4e0e69974690d299819df6a33b7ae13b7eff5d037406e602c033f33a`. It contains nine mode-
`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,967,322`-byte
`assets/geosolve_demo_web_bg-DuuevZyx.wasm` has SHA-256
`c3160d7f8f6fc49db6294588cedd38ba5b520a80743d3977039957074fa8ca31`.

All ten staging/live routes byte-match with correct MIME, no redirects/compression and ledger
SHA-256 `47a0229dbf46ea0549f4e424a6ce7ccd452810eb24161db61c4cb33436107726`.
Freeze evidence is `/tmp/geosolve-m90-f003-freeze-evidence.Y4jLQxD3`. Historical Tailscale-only
`geosolve-m90-f003-replacement-uat-18090.service`, PID `305121`, invocation
`f456b1c8cea044638bae1119709b94d9`, served only that snapshot on `100.94.63.83` at
`http://100.94.63.83:18090/`. M90-F004 withdraws those bytes from continuing UAT; the service is
inactive/dead and the immutable snapshot remains historical pre-F004 evidence.

M90-F004 qualification passes the exact bridge owner `1/1`; full
`cargo test --locked -p geosolve-demo-web --lib` at `284/284`;
`cargo test --locked -p geosolve-sketch-code --all-features`; TypeScript fixture/runtime/type/
managed checks with `34` runtime tests; frontend Vitest `74/74`; warnings-denied locked workspace
all-target/all-feature Clippy; format and diff checks. The exact golden inventory plus `--check` and
`--require-clean` remain 271 rows at SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`. The final pinned/Nix
optimized release-WASM build, manifest/licence/build-contract checks and nine-file distribution
validation pass. The combined F001/F002, F003 and F004 browser bundle passes `3/3` against frozen
staging and live Tailscale bytes.

The historical post-F004 optimized release-WASM output is frozen read-only at
`/tmp/geosolve-m90-uat.O4xZJBxg`, with external manifest
`/tmp/geosolve-m90-uat.O4xZJBxg.sha256` and ordered aggregate
`fd0a4635edcc6bd24d36eeca831a57bbb62cdf1d67589c6245af9e7b88bf52be`. It contains nine mode-
`0444` regular files, two mode-`0555` directories and zero symlinks. Its `19,987,485`-byte
`assets/geosolve_demo_web_bg-CoONmEL1.wasm` has SHA-256
`e61ff5e9183883c1872293ad5d4c38c06175bc12575668f3f770282387bcf457`. Freeze evidence is
`/tmp/geosolve-m90-f004-freeze-evidence.F0HdfUMF`.

All ten staging/live routes byte-match with correct MIME, no redirects/compression and HTTP ledger
SHA-256 `47589602797db38fb23a70da0d1cc31c7b032b7ab0907f3688d2a39886ebe921`.
Tailscale-only `geosolve-m90-f004-replacement-uat-18090.service` historically served only that
snapshot with PID `792138`, invocation `ff367dd5f5bb4f24a8661dd83a26168b`, at
`http://100.94.63.83:18090/`. The supervising user's reboot stopped the service. M90-F005
withdraws those bytes from continuing UAT because they predate its repair; the snapshot and hashes
remain exact historical evidence.

F005/F006 collateral passes constraint-editor all-features with unit layer `439/439` plus all
integrations, demo-web `286/286`, warnings-denied workspace and targeted demo-web Clippy,
`cargo fmt --all -- --check`, `git diff --check`, the unchanged 271-row golden, sample manifest
`37`, runtime licences `71`, frontend build contract, Vitest `74/74`, optimized release-WASM build
and nine-file distribution validation.

Current immutable optimized snapshot `/tmp/geosolve-m90-uat.EtWyWQlt`, manifest
`/tmp/geosolve-m90-uat.EtWyWQlt.sha256` and freeze evidence
`/tmp/geosolve-m90-f006-freeze-evidence.HqyaA7Qp` have ordered aggregate
`b3fd72b9ec98d318d7bfa7bf8c09d0fcbd3856ea0723d81e01e64945e301debe`, nine mode-`0444` regular
files, two mode-`0555` directories and no symlinks. Its `19,990,463`-byte
`assets/geosolve_demo_web_bg-Dc5MH04n.wasm` has SHA-256
`bad16242c2ec0fa0c6c0ba6882428372c1bf0b7dd1a70235467b5febdfc80712`.

Staging/live HTTP ledgers match at SHA-256
`41d11e1c56bad8dcc57edf229f0bfec20d8f5e602b3c385e5e68d54dd42c816a`. The optimized
release-WASM browser bundle passes `4/4` against both: empty coded Center-Radius Circle, two-circle
snapped Segment, Compass persistence/reload and the supplied native contact-workspace drag.
Tailscale-only unit `geosolve-m90-f006-replacement-uat-18090.service`, PID `462021`, invocation
`4a17e69e926446eba21439ac4dd6f4e6`, exact-serves that snapshot at
`http://100.94.63.83:18090/`.

The final full dirty-tree gate command
`env GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from `23:20:45` through `23:47:22 AEST` on 2026-09-03 and exited `0` after `1,596,726 ms`.
Its `573,421`-byte log `/tmp/geosolve-m90-f006-full-gate.hKSTh4/release-gate.log` has SHA-256
`bda7f5f92f15a5f0a0cf26ed93cb514943d9a9d1ad49bf0ba0e148c9239b205d`. The gate keeps the reviewed
271-row golden unchanged, passes the release-only 256-moving-body performance row in `137.82 s`,
frontend Vitest `74/74`, and the optimized nine-file distribution validation. This is complete
dirty-tree qualification, not clean-source qualification.

The supervising user explicitly approved scoped closure on 2026-09-04. M90-U1 through M90-U10
remain unexecuted and transfer/defer—not pass or waive—into M91's one composite UAT. Automated
qualification accepts no human row. The immutable F006 snapshot and service remain exact Tailscale-
only closing publication authority; no GitHub Pages deployment or public push was authorized or
made. M90 is closed. `PLAN.md` owns the exact evidence and M91 intake ledger.
