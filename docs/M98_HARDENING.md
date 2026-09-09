<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation findings

M98 remains in implementation; these are focused owner results, not integrated
qualification or milestone acceptance. The hardening skill routes these findings to
folder persistence/transport because the existing Rust workbench retains its contracts.

## M98-F001 — observed disk state authorized an undisplayed Undo

Reproduced at `a913a6d` through `FolderWorkbenchAdapter`: display radius 10, begin an
Inspector draft, externally save radius 15, fetch a hidden background snapshot, Undo.
The request incorrectly used radius 15's hash despite no new snapshot reaching the UI.
The earlier actual-WASM reproduction overwrote the saved radius with radius 10.

The exact adapter regression failed with `radius-15` instead of `radius-10` before
repair. Snapshots now carry their own transport identity. Only explicit host installation
advances editing authority; installation rejects older snapshots and source replacements
under a pending field/source draft. Every queued command captures its authority before
later observations. Typed authoring field begin/change/commit/cancel hooks cover numeric
parameters, dimensions, names and multiline descriptions; application-wide DOM capture
and command-name suffix inference were removed. New input during a commit retains its
own immutable draft basis. Undo, metadata and extraction use the same installed basis.

Focused commands (frontend directory):

- `npx vitest run src/lib/folder-adapter.test.ts`: 5/5 pass after the original 1/2 failure.
- `npx vitest run src/lib/folder-adapter.test.ts src/components/authoring-metadata.test.tsx src/components/dimension-inspector.test.tsx src/components/side-panels.test.tsx`:
  23 tests in the three existing matching files passed at the initial two-adapter-test stage;
  no standalone `side-panels.test.tsx` exists.
- `npx tsc -b --pretty false`: passed.

Session epochs, editor leases and cross-process operation identities follow separately;
these focused tests do not claim those unimplemented guarantees.

## M98-F002 — broken derived cache prevented valid source startup

Reproduced at `a913a6d`: initialize a valid folder and put a single `0xff` byte in
`.geosolve/last-good.ts`. `openProject` threw `ERR_ENCODING_INVALID_ENCODED_DATA` before
reading valid disk source. The initial focused cache suite passed 1/3 and failed both
corrupt-cache cases.

Startup now reconstructs disk source first. Only a failed reconstruction attempts the
optional prior cache. Invalid cache bytes produce warnings; invalid cached TypeScript
restores the complete current-source diagnostic snapshot. A valid prior cache can retain
accepted geometry while the exact rejected disk source remains a draft. Cache update
failures are warnings rather than false source-save failures.

`node --test scripts/workspace-cache.test.mjs`: 4/4 pass using the retained actual WASM
runtime. This verifies the JavaScript persistence repair; the integrated M97/M98 WASM
will be rebuilt and qualified at nomination. Tests check exact source hashes and bytes,
last accepted identity, current rejected drafts and diagnostics without treating a cache
as authored authority. No solver equation, branch or golden-oracle change is involved.

## Session and publication integration — development evidence

The Linux publication journal and exclusive bridge lock are described in
[M98_WORKSPACE_STORAGE.md](M98_WORKSPACE_STORAGE.md). The bridge now binds a client ID,
epoch, editing lease and installed interaction revision. Read-only tabs have an explicit
handoff action; passive resize cannot alter the active editor's camera. Source publications
carry durable request identities, including client binding, so a repeated acknowledged
request returns its recorded outcome without executing the source edit twice. Navigation
avoids full rollback serialization and disk publication.

The independent [field lifecycle review](M98_FIELD_LIFECYCLE.md) also reproduced explicit
Refresh rebasing surviving field text. Refresh and handoff now retain immutable drafts;
new installed source cannot silently authorize their later submission. The review test
failed before repair and now passes. Folder source writeback retains leading licence
comments that sit outside the strict managed compiler's normalized directive.

Focused commands:

- `node --test scripts/workspace-bridge.test.mjs scripts/workspace-storage.test.mjs`:
  31/31 passed at the session/journal integration checkpoint.
- `node --test scripts/workspace-bridge.test.mjs`: 3/3 after passive viewer resize,
  explicit preamble preservation and exactly-once source-retry assertions. A baked circle
  independently verifies every sampled point is at radius 12 mm.
- Frontend `npx vitest run src/lib/folder-adapter.test.ts src/lib/folder-lifecycle-review.test.ts src/lib/authoring-edit.test.tsx`:
  10/10; `npx tsc -b --pretty false` passed.
- `node --test scripts/workspace-loader.test.mjs`: 7/7, including freshly compiled
  manifold patches, ordinary generator loops, dependencies, cancellation and supersession.

These use the retained prototype workbench WASM until the integrated build is ready.
Subsequent folder-v2/native admission wiring is under development and is not covered by
those earlier results. Full release qualification remains outstanding.

## M98-F003 — worker loss left dead owners and orphaned recovery timers

Reproduced on `5373f17` plus the pending folder integration by terminating the actual
WASM worker after capturing the 10 mm circle. The next operation waited for its timeout
instead of restoring immediately. Losing two concurrent native responses also created two
replacement workers; the retired generation's timer could destroy the first recovery.

One retirement path now owns timeout, error and every unexpected exit, cancels all pending
timers, detaches the old owner before termination and reconstructs once from its checkpoint.
A failed reconstruction permits an explicit construct retry. Accepted publication updates
the checkpoint even when optional disk caches are disabled. Navigation does not capture it.

`node --test scripts/workspace-workbench.test.mjs scripts/workspace-actor-review.test.mjs`
passes 9/9 after three protocol failures and two independently reproduced actual-WASM
failures. The actual native tests compare exact saved project, source and independently
sampled 10 mm circle geometry after recovery. Protocol tests additionally cover exit zero,
uncaught error, ordering and disposal. Evidence: `target/m98/workbench-recovery-after-r2.log`.

## M98-F004 — export failure escaped native transaction rollback

A real radius 10→12 source edit followed by an injected `exportWorkspaceDesign` failure
left disk at 10 but accepted native geometry at 12. Native dispatch and semantic/project
exports now share the publication rollback scope. Failure restores the complete previous
checkpoint, including accepted source and geometry, and invalidates the interrupted edit's
interaction authority. The regression verifies the geometry independently through profiles.

## M98-F005 — failed acknowledgment advertised mismatched saved state

An injected `before-acknowledge` fault left a durable published radius 12 source/design and
receipt while native rollback restored radius 10. The host incorrectly advertised `ok:true`.
It now reports publication interruption whenever disk and restored accepted identity differ.
A repeated durable operation first reconstructs current disk authority; retained published
bytes are not overwritten by rollback. The browser preserves the original operation ID
and immutable request on disconnect, offers Check save status, and retries only read-only
requests or explicit receipt-protected edits. Relative pointer/navigation and handoff actions
are never automatically replayed. A failed status lookup cannot overwrite retained edit intent.

## M98-F006 — derived helper digests could suppress required Undo publication

Corrupt only a historical helper's cached `sha256` to match its current file, leaving the
historical contents unchanged. Undo independently compiled the old entry/helper correctly,
but then trusted that cached digest to skip writing the old helper. It returned accepted
geometry at centre 0 while disk still reconstructed centre 20. Publication now plans from
`readWorkspaceSnapshot`'s independently recomputed historical files; cached digests supply no
authority. Current source/design/runtime reconstruction still precedes optional cache restore.

Focused F004–F006 command:

```bash
node --test --test-name-pattern='failure|corrupt cached helper' scripts/workspace-transaction-review.test.mjs
```

4/4 pass (including the staging-failure control) in
`target/m98/workspace-transaction-after-r2.log`. Frontend reconnect/draft tests:
`npm exec -- vitest run src/lib/folder-adapter.test.ts src/lib/folder-lifecycle-review.test.ts`
passes 9/9 and `npm exec -- tsc -b` passes from the frontend directory. These are focused
integration results; complete release qualification remains pending.

## M98-F008 — Center on origin serialized authoring history

Actual HTTP navigation on the small folder failed the independent guard against
`persistProject` calls. The explicit navigation inventory omitted the UI's `view.origin`
and listed nonexistent zoom commands. The host consequently treated centering as authored
work and refused it entirely in generator mode. The inventory now uses the actual command;
geometry, source, caches and history must remain unchanged. Full sample navigation and
queue measurements are recorded separately by `workspace-navigation.test.mjs`.

## M98-F009 — unreadable complete source retained misleading revision metadata

The migrated actual-browser workflow saved malformed TypeScript after accepting radius 14.
It correctly retained visible geometry, but `currentHash` still equaled `acceptedHash` and
its structured file/line fields were lost. Failed dependency capture now reports a null
current complete revision, retains accepted identity separately and exposes the loader's
path/line/column/code. Repaired disk reconstructs and installs its full identity normally.
The current browser regression passes with exact retained canvas geometry and a located
`sketch.ts:13:15` syntax diagnostic.

## M98-F010 — invalid source prevented complete-folder restart

Both owning cold-reopen tests initially threw a located loader exception before the bridge
could open. Complete editable folders now open with diagnostics even when the current
source graph is invalid. Optional previous-source recovery recomputes every dependency
identity, independently recompiles the captured source graph, compares its canonical project
and restores only its semantic design. It does not restore unverified cached solved geometry.
A corrupted cached source is ignored; invalid authored disk bytes remain unchanged.

`node --test scripts/workspace-recovery.test.mjs`: 2/2 pass after the original 0/2 failure,
with exact independently sampled previous circle geometry in the valid-cache case and no
accepted revision in the corrupt-cache case. Evidence:
`target/m98/workspace-cold-recovery-after-r1.log`. This fallback requires a previously saved
complete source cache; absence or corruption leaves diagnostics available without inventing
an accepted result. Generator source still executes only through its bounded normal loader.

## M98-F011 — repairing rejected files inserted an empty history step

The complete manifold workflow edited a local patch, rejected an invalid patch value,
restored the exact accepted bytes, then used Undo. The first Undo left the edited patch
unchanged. A minimized 10→12 mm circle source workflow independently reproduced a refused
Undo with `No authored files represent the accepted design change`. Restoring already
accepted files had redundantly applied the same complete project to native history.

The scan now recognizes exact accepted dependency identity after repair. It retains the
already restored native checkpoint, clears rejection state and publishes disk observation
without a new authoring action. `node --test --test-name-pattern='repairing rejected'
scripts/workspace-project.test.mjs` passes 1/1 after the original failure. The regression
requires a single Undo to restore the exact original source bytes.

The F008 inventory review also reproduced unnecessary history capture for middle-button
pan down/up and Show in canvas. Pan now records navigation provenance for the exact
client/pointer gesture, including its terminal; primary drag terminals retain authoring
publication. Source-range selection remains guarded by installed interaction revision and
is classified as presentation. Neither action may serialize history or write files.

Complete manifold folder collateral:
`node --test scripts/workspace-manifold.test.mjs` passes 1/1 in 123.9 seconds after F011,
covering source-owned label writeback, external patch metadata changes, full 18-region export
with independent net area, rejected patch retention, Undo/Redo and restart. Individual label
edit/Undo/Redo measured 17.9/7.3/7.4 seconds in this development run. These costs are retained
as limitations, not described as optimized; `target/m98/workspace-manifold-lifecycle-r4.log`
records the exact timings. Its 300-second test bound covers this multi-operation lifecycle;
individual native worker timeout remains 120 seconds.

## M98-F012 — successful refresh discarded an uncertain save's operation ID

After a disconnected field save, a successful status lookup carrying newer disk state
replaced the retained full request with a field-draft array. The operation ID disappeared,
removing Check save status and its retry identity. Stale-field observation, Refresh and
handoff now retain an existing full operation record. The regression loses both save
responses, advances disk, successfully resolves the receipt and then refreshes; original
request bytes, operation ID and stored pending intent must remain equal.

Frontend `npm exec -- vitest run src/lib/folder-adapter.test.ts src/lib/folder-lifecycle-review.test.ts`
passes 10/10 after the new test failed, and `npm exec -- tsc -b` passes. Evidence:
`target/m98/folder-receipt-before.log` and `folder-receipt-after.log`.

## M98-F013 — new CLI projects could not use the documented apply workflow

The real `init → status → takeover → apply` regression only passed because its fixture
manually replaced the freshly created v1 manifest with v2. Removing that rewrite reproduced
`Complete file transactions require a v2 folder bridge`. Initialization now creates an
explicit editable folder-v2 manifest. The committed legacy sample and focused compatibility
fixtures still exercise v1 deliberately. Fresh CLI projects can use complete file edits,
semantic design and dependency history without hand-editing their manifest.

## M98-F007 — containment examined curve pieces behind its ray origin

The manifold GUI accepted width 10 mm but complete export refused it. A native-only closed
stair polygon and a separate radius-3 circle reproduced `ContainmentAmbiguity` despite at
least 12 mm independently measured boundary clearance. The containment routine isolated
roots along an infinite horizontal line before discarding left-side intersections. An
irrelevant tangent/endpoint wholly behind the query point could therefore fail the actual
rightward ray test.

`bf59119` prunes interval curve boxes proved wholly behind the witness before root isolation.
It changes neither tolerance nor accepted touching/uncertain geometry policy. The owning
`m98_f007_disjoint_contours_do_not_touch_when_containment_ray_is_tangent` checks translation-
independent model scaling/reflection, clearance, two finite positive faces, independent
area `700 + 9π`, and unchanged source. The scenario is recorded in `docs/SCENARIOS.md`.

Focused native containment passes 1/1; existing M31 profiles pass 31/31, computed profiles
pass 7/7, and scoped strict Clippy and formatting pass. Both dedicated engine and demo
release WASM builds pass. On those published bytes:
`node --test scripts/workspace-transaction-review.test.mjs` passes 12/12 in 92.78 seconds,
including actual manifold GUI widths 12→10→11, independent sidecar reconstruction and
18 regions at every width with all five 3 mm bores retained. Evidence:
`target/m98/workspace-transaction-review-final-r2.log` and `topology-f007-wasm-r1.log`.
No solver equation or golden oracle bytes changed.

## M98-F014 — offline CLI packaged a recursive bundler launcher

Clean `e0c2167` failed integrated `package.m98` in run
`20260909T132628-47a2b33d`: the installed generator-circle `check` exceeded the
unchanged 90-second command bound. The initialized editable check had passed because
its source needed no bundling. Installing the exact retained archives into a new
empty-cache consumer independently reproduced the timeout in 91.1 seconds. Captured
stderr showed the loader's own 15-second timeout; inherited subprocess pipes kept the
outer process alive.

Release preflight installs esbuild without running its install script, leaving
`bin/esbuild` as a 9,351-byte JavaScript forwarding launcher. The packager copied that
launcher into esbuild's native fallback location. Its checkout `--version` check passed
by locating the optional platform dependency. In the installed CLI, it instead found
itself and recursively launched copies. Both archive inspection and a bounded direct
fallback invocation independently confirm the packaging error.

The correction belongs to archive preparation: resolve the actual matching Linux
platform executable, verify ELF identity and version with a bounded invocation, and
ship it beside unchanged upstream esbuild JavaScript. The offline installed-package
regression must verify that executable before checking the generated circle and custom
website. Existing timeouts and geometry assertions remain unchanged; no native engine,
worker authority, solver equation or golden bytes need modification. Focused and final
qualification results are recorded after the repair below.

Correction `72f03b2` passes the full offline smoke **2/2 in 19.1 seconds**, including
empty-cache installation, editable and generator CLI checks/bake, installed engine,
folder status and exact production index bytes, and Chromium using the clean-installed
SDK/engine website. Regression-first execution against the original gate archives fails
on executable magic in 2.1 seconds before recursion. Syntax and diff checks pass.

Executed focused command (from this worktree):

```bash
env -u NODE_OPTIONS -u NODE_PATH -u GEOSOLVE_DIST -u GEOSOLVE_M98_PACKAGES \
  GEOSOLVE_M98_PACKAGE_OUT="$PWD/target/m98/package-fixed-e0c2167" \
  GEOSOLVE_M98_DIST="$PWD/target/release-gate/prepared/f862ad58f8fc7bb4ed341d7ca820326b75fe2a655217065d40f0d7fb56985f8e/browser/geosolve-production" \
  node --test --test-concurrency=1 --test-reporter=tap scripts/package-m98.test.mjs
```

Final integrated replacement qualification remains required; this focused result does
not convert the failed run into a release pass.

## Qualification harness correction — Split resize baseline

Replacement run `20260909T142852-ee5601c1` on clean `f67c977` passed the 97-case folder
runtime suite but failed one browser retention assertion. The test opened Split and
captured screen-space geometry as soon as CodeMirror appeared, before the asynchronous
resize RPC published the new camera frame. The retained evidence shows the same curve
identity and all 129 points translated exactly −282 pixels in X with zero Y change;
accepted source hash, writes and external-apply count were unchanged.

The test now waits for a smaller Split canvas and a presented viewBox matching its actual
CSS extent, using the existing canvas renderer's 0.01-pixel extent check. Its exact
post-rejection geometry equality and all source/authority/diagnostic assertions remain.
This is a synchronization correction without product or golden changes or a new defect ID.
Focused command passes **1/1 in 7.41 seconds**:

```bash
GEOSOLVE_DIST="$PWD/target/release-gate/prepared/f862ad58f8fc7bb4ed341d7ca820326b75fe2a655217065d40f0d7fb56985f8e/browser/geosolve-harness" \
  node --test --test-name-pattern='external rename updates' scripts/workspace-browser.test.mjs
```

Evidence: `target/m98/folder-browser-layout-r1.log`. The failed replacement remains
failed; the runner must qualify the corrected harness and all outstanding obligations.
