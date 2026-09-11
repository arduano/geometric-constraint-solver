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

## M98-F015 — rejected folder evaluation reset the live canvas view

The delayed-loading browser witness exposed a rejected external generator resetting a
969 × 876 CSS-pixel canvas to the default 1000 × 700 view. The accepted circle remained
mathematically unchanged but jumped from the measured host center to (500, 350).
The focused Node reproduction independently resized to 969 × 876 at DPR 2, zoomed around
(127, 283), middle-panned by (60, 25), and rejected an external generator. Its exact
viewBox comparison failed with `[0,0,1000,700]` instead of `[0,0,969,876]` before repair.

The folder's three transaction rollback paths called fresh adapter construction, which
correctly reconstructs accepted authored state and history but also initializes and fits
a new camera. They now dispatch `workspace.checkpoint.restore`: Rust independently
decodes and validates the complete persisted checkpoint, preserves the existing camera,
measured host extent and DPR, and recomposes geometry from the restored authority. It
does not restore a cached scene or accept client-supplied view fields. The command is
available to the owning bridge, including cleanup of a pending managed ticket, and is
refused through the folder HTTP dispatch route. The actor records the restored checkpoint
so a later worker failure cannot recover an intervening rejected candidate.

The native regression verifies exact view/geometry and checkpoint retention, usable
Undo/Redo, strict malformed-payload rejection and unchanged camera across later resizes.
Node regressions cover external rejection, a real publication failure, forbidden HTTP
checkpoint replacement and a later actor crash after rollback. Initial startup and cache
loading retain their existing construction and Fit behavior. No solver equations,
residual tolerances, source authority or golden rows change.

Focused commands (from the M98 worktree):

```bash
node --test --test-name-pattern='M98-F015' scripts/workspace-http.test.mjs
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --lib workbench::bridge::workspace::tests'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'node crates/geosolve-demo-web/frontend/scripts/build-wasm.mjs --release'
node --test scripts/workspace-http.test.mjs scripts/workspace-workbench.test.mjs
```

The first command failed before repair at the exact viewBox assertion. Native owner
checks pass 2/2, strict Clippy and formatting pass, and release demo WASM rebuilds.
HTTP/actor checks pass 12/12 after that rebuild, including real publication rejection
and restored-checkpoint crash recovery. Evidence is retained under
`target/m98/f015-{native-pending-final,clippy-r2,wasm,node-final}.log`. Replacement integrated
qualification remains required; the loading browser witness is qualified separately.

## Final qualification before the loading amendment

The complete clean-source gate passes on `6509e9c160a74c499668b63a5ef9bddefa593223` in
`20260909T144235-377abb33`: all 261 obligations, including corrected offline archives and
browser synchronization, unchanged golden, native/WASM parity and performance. Frozen
production and installed-folder previews pass fresh transport/readiness checks.
[M98 qualification](M98_QUALIFICATION.md) records final authority, exact evidence and the
separate transient preflight `/proc` harness race. Supervising-user acceptance remains open.

## M98-F016 — asynchronous canvas navigation replayed stale input

Reported on loading-amendment source `bf417de` / qualified product `a68fffa`: hover,
zoom and clicks took 2–5 seconds to register. No user payload or exact endpoint was
supplied. Private browser copies preserved the current manifold's seven authored
files; neither the user's editable session nor accepted M97 was used for mutation.

Independent Chromium reproduction retained 79 pointer requests from 90 mouse samples
under 40 ms simulated network latency. Requests finished 2.866 seconds after movement
stopped. Twelve wheel samples over 192 ms at 100 ms latency and 4 Mbps kept painting
successive zoom frames for over 14.6 seconds after input stopped; the final request was
still outstanding at the 15-second observation bound. The preceding pre-loading product
also reproduced these folder delays. This finding establishes an asynchronous input
queue defect, without attributing the user's entire reported regression to loading.
Local standalone hover/wheel and deferred dimension updates took tens of milliseconds.

`CanvasInputQueue` coalesced samples inside a single animation frame, then appended
one operation every frame even while the previous worker/HTTP request was pending.
Its owning regression holds one request, feeds 20 separate frames and clicks: the
original implementation delivered 23 pointer calls instead of first/latest/down/up.
Pending adjacent idle Select hover and fixed-origin middle pan now retain the newest
sample across frames. Pointer identity, buttons, modifiers and intervening commands
remain barriers. Wheel batches retain every ordered delta and anchor, with the native
256-sample bound. Semantic drag/authoring samples, click/release/cancellation ordering,
disposal generation checks and retained scene authority remain intact.

Folder responses also transport about 634 KB for each changed manifold frame. Standard
negotiated JSON compression is included in this repair to reduce transfer cost without
changing the decoded snapshot/authority protocol. Full snapshot transport and broader
folder React publication remain; no client-side camera or solver equations are added.

Focused qualification and replacement preview evidence are tracked in
[M98 navigation latency](M98_NAVIGATION_LATENCY.md). M98 acceptance remains open.


## M98-F017 — folder navigation waited for server execution and full UI publication

The user reports folder interaction remains substantially slower than M97 after qualified
F016 product `b1243a6`, and explicitly authorizes local navigation, picking, selection and
highlighting with server-owned sketch edits. Private-copy Chromium reproduction in
`target/m98/navigation-path-comparison.json` confirms every folder navigation update still
crosses serialized HTTP, while the standalone path uses local WASM. At 100 ms simulated
latency and 512 KiB/s, folder response time is 236–243 ms and its final frame arrives 710 ms
after input stops. Native frame deltas also lose their canvas-only marker through the folder
transport, unnecessarily refreshing React source, Explorer and Inspector state.

This is distinct from F016's bounded input queue: retaining fewer queued requests cannot
remove a network round trip from each visible response. The owning boundary is detached
Rust interaction plus browser routing. The authorized repair exports bounded accepted
presentation data to a dedicated browser WASM worker that shares exact camera, reprojection,
picking, dimension and ordered selection behavior. Authoritative edits remain serialized
on the server and validate source, epoch, lease, revision and accepted scene identity.
Transported geometry confers no solver or edit authority.

[Local canvas implementation and qualification](M98_LOCAL_CANVAS.md) tracks focused native,
transport and browser evidence. This amendment is qualified and delivered from `d5f9e40`;
[final qualification](M98_QUALIFICATION.md#qualified-local-canvas-boundary) records all 261
passing obligations and the verified replacement previews.

## M98-F018 — Redo rejected a canvas-authored polyline's historical project

During F017 browser qualification on development r2, Polyline previews, three clicks,
Finish with one source write, local navigation and Undo succeeded. Redo returned HTTP 400:
`Historical dependency bytes do not match the accepted Undo/Redo project`. The transaction
retained the previous accepted model and source. Exact evidence is
`target/m98/local-canvas-authoring-r2b/local-folder-authoring-debug.json`.

The backend-owner regression reproduces the same failure through public `openProject`
requests with local interaction disabled, establishing that the folder-history failure is
independent of the new client routing. The historical source graph is independently compiled
before acceptance; that comparison must retain its dependency and authoring invariants.
The native canvas authoring project raises `declaration_name_high_water`, a monotonic
name allocator intentionally absent from printed source. Undo preserves that counter,
so hashing it also prevented recovery of the exact original source snapshot. Source-history
identity now excludes this single lifecycle field; comparison retains every compiled source,
IR, custom-file, artifact and lock field. Native restored projects must still exactly match
the recorded project before their allocator/history is admitted. The counter itself is never
lowered or rewritten by this comparison.

The focused real polyline backend regression passes exact original-source Undo, authored
project/design Redo, reopen/Undo and subsequent `geometry2` allocation. Forged historical
source, including matching forged hashes, remains subject to independent compilation.
The complete gate and actual browser source Undo/Redo workflow pass in
`20260910T022428-a1d7c652`; the qualified replacement is delivered.

## M98-F019 — initial collaborative Inspector used server-local authority

At collaborative revision zero, open the complete manifold and change Channel width
from 12 to 11. The native browsing descriptor refused `These properties belong to
an older source revision`; no semantic command reached the server and accepted
source/model remained unchanged. The browser adapter had captured the initial
server Inspector's authority before waiting for its independent browsing worker.
That worker correctly refused a different native session's authority.

The focused adapter regression
`describes an initial parameter edit with the local Inspector authority after its worker finishes opening`
first observed `server-inspector` instead of `local-inspector`
(`target/m98/collaboration-inspector-authority-repro-r2.log`). The adapter now resolves
local chrome after initialization, checks model/view compatibility, then describes
the edit with that native authority. Inner metadata payload authority checks remain.
The three focused adapter/controller/source-projection files pass ten cases and
strict TypeScript (`collaboration-inspector-authority-repair.log`). The new development
artifact includes the correction; dense browser requalification remains pending.

## M98-F020 — full-canvas busy filter amplified multi-editor compositing

The four-editor held-solve browser case on development-r1 passed source/model/reload
checks but exceeded the unchanged 500 ms navigation target: p95 935.222 ms. Native
wheel execution was 1.7–2.9 ms. A bounded A/B experiment removed only the solving
overlay's `backdrop-filter: grayscale(1)`, retaining its gray translucent surface and
animated spinner. Navigation p95 improved to 527.593 ms; text ACK was 191.944 ms
(`collaboration-browser-css-ab-r6.log`). This implicates browser compositing rather
than solver authority. The product now uses the existing gray layer without filtering
the entire WebGL surface. Development-r2 passes the actual four-editor product test without CSS injection:
p95 navigation488.092ms and text ACK112.877ms. Dense sample responsiveness remains
under investigation; this is focused evidence, not integrated qualification.

## M98-F021 — latest point replay omitted named scalar dependencies

The actual compiler fixture `point-gesture-parameter.json` has lexical `width` owning
public parameter `boreRadius`, and lexical `port` owning public circle `bore` whose
radius references that parameter. Even replay against identical accepted sessions
failed `point replay source owner missing at basis`. The native dependency guard
indexed geometry declarations but followed references to scalar bindings absent
from that index (`named-parameter-replay-repro-r3.log`).

The guard now traverses named scalar bindings, preserves their semantic symbols,
references, units and value structure, and includes their lifetimes in the server
witness. Continuous dimensional values can change while discrete/reference changes
still reject. Native regression
`latest_point_replay_retains_named_parameter_dependencies_with_distinct_lexical_names`
passes through the public session API, retaining exact project source, accepted
geometry and independently validated residuals (`named-parameter-replay-repair.log`).
Actual-WASM latest-replay passes five cases, including a later parameter value
write and reference-to-literal refusal (`extraction-replay-actual-wasm-r4.log`);
scoped strict Clippy and release engine/demo builds pass. No equations, tolerances,
implicit branches or golden expectations changed.

## M98-F022 — semantic scalar writes omitted binding owners

The actual dense manifold browser edit of public `channelWidth`, path `[]`,
from 12 to 11 mm reached the domain worker but rejected with `managed value owner
is absent or ambiguous` (`collaboration-browser-dense-r3.log`). The shared
`derive_managed_value_mutation` preparation helper indexed only declarations,
although exact mutation application already supported named and ordinary scalar
bindings. This differs from F021's historical point-replay dependency guard.

The public Rust regression `semantic_value_preparation_resolves_parameter_symbols_and_ordinary_scalar_bindings`
independently reproduces the failure with existing genuine compiler fixtures
(`parameter-value-preparation-repro.log`). It covers public `width` with distinct
lexical `x`, ordinary `sharedRadius`, exact prior values and retained compiler
authority. Preparation now uses a named parameter's public symbol or an ordinary
binding's lexical identity, matching exact mutation application; a named parameter's
lexical alias does not acquire public identity. Focused native repair1PASS and scoped strict all-target Clippy pass
(`parameter-value-preparation-repair.log`). Release engine/demo WASM builds pass
(`parameter-value-wasm-r4.log`). Actual dense browser flows now publish manifold
width12→11 and Gridfinity41.5→41 (`collaboration-browser-dense-r4.log`), preserving
source/model invariants; the same run still fails responsiveness budgets. No solver
equations or tolerances change.

## M98-F023 — extraction metadata JSON order changed exact compiler meaning

The actual-WASM extraction of a circle radius reached native publication but
rejected `candidate IR differs from the one operation authorized by Rust`
(`extraction-native-publication-repro.log`). Native preparation orders new options
as label/description/isKeyParameter; the compiler used the incoming JSON object's
key order, which changes across native JSON serialization. The package regression
`parameter extraction has identical compiler receipts for every metadata key order`
independently reproduces differing receipts (`extraction-metadata-order-repro.log`).

New extraction metadata now uses the existing deterministic presentation encoder.
All six key orders produce identical genuine compiler receipts, and actual-WASM
extraction publishes with original comments, two fresh allocations, whole-binding
promotion, exact source/design reopening and finite independently validated geometry
(`extraction-atomic-source-repair.log`). The cold-session geometry comparison resolves
fresh native local IDs through unique semantic owners while retaining the complete
geometry and incidence graph. In-session identity assertions remain exact.

## M98-F024 — extraction Undo compiled a dangling intermediate reference

Concurrent extractions succeeded, but Alice's Undo failed `reference to unknown or
forward binding parameter1` (`extraction-undo-repro.log`). Structural reconstruction
removed the new parameter before restoring its consumer's old literal. The focused
source-composition regression independently reproduces the dangling reference
(`extraction-atomic-source-repro.log`).

The internal SDK helper `rewriteManagedSketchIrValues` applies the existing bounded
value CAS rules while old and recreated bindings exist. The domain then removes
deleted owners and recompiles the complete candidate. The helper conveys no compiler
receipt or publication authority. Two extraction/source cases and the actual HTTP
concurrent-extraction test pass (`extraction-atomic-source-repair.log`), including
personal Undo/Redo, distinct allocations, another editor's retained extraction and
refusal to remove their later same-value parameter contribution. Full collateral and
integrated qualification remain required.

## M98-F025 — shared-text ownership scans delayed dense typing

At `8e8331a` plus the recorded development-r3 integration, a real manifold text
insertion took 1,647 ms to acknowledge; Gridfinity took 513 ms. Both exceed the
unchanged 500 ms target. Diagnostic r5 localized 567 ms to server
`stageUserTextChanges` and 218 ms to the client native edit. The text history owner
looked up a stable cursor separately for every scalar, and current-frontier typing
unnecessarily forked and re-imported the document.

`geosolve-collaboration::text::history` now uses public bulk sequence iteration and
the public cursor parser. An exact cursor-byte oracle compares 25,000 Unicode
scalars through concurrent changes, deletion and reload, plus imported multiscalar
and conflicting overwrite representations. Those uncommon representations retain
the original lookup path. Current-frontier edits use existing atomic edit admission;
stale-frontier and file-lifetime checks remain intact. No contribution guard or
surrogate-boundary check was removed.

Commit `3f2e085` passes all 113 native collaboration/adapter tests and strict scoped
Clippy (`collaboration-text-native-collateral.log`). The rebuilt actual-WASM package
passes all 44 cases, including 520 durable edits beyond the bounded Undo horizon
(`collaboration-combined-collateral-r4.log`). Debug cursor traversal improved from
778–904 ms to 103–110 ms with identical witnesses. This is owner-level performance
evidence; the coherent r4 browser results are recorded below.

## M98-F026 — suppressed geometry prevented collaborative publication

The new real runtime suppression tests reproduced rejection with
`point gesture has no independently accepted native position`. The compiler had
correctly suppressed the circle, but `EditableSession::point_gesture_targets`
still required accepted positions for its inactive native points. The public
engine regression `explicitly_suppressed_geometry_has_no_advertised_point_gesture_targets`
reproduces the exact failure using the
genuine compiler fixture `point-gesture-suppressed.json`.

The registry now omits only graph nodes explicitly marked suppressed. Missing
active geometry still rejects. The regression independently checks validated
acceptance, empty suppressed output, refused dragging and restored target handles.
Native regression and strict engine Clippy pass (`capability-point-target-r1.log`);
combined release WASM passes. Both runtime suppression/Undo/restart and same-value
ownership cases pass in `collaboration-combined-collateral-r4.log`.

## M98-F027 — detached navigation admitted a partially authored geometry draft

Existing regression `local_canvas_navigation_handoff_preserves_armed_tool_and_uses_new_camera`
failed during visibility collateral: after the first construction click and
pointer-up, no pointer was captured, but a staged native draft still existed.
`apply_interaction_state` guarded active pointers and pending mutations without
checking that retained draft state.

Admission now also refuses a geometry draft with `completed_stages > 0`. An empty
armed tool remains navigable, and cancellation restores ordinary admission. The
unchanged regression passes after repair; the seven other local-canvas collateral
cases passed. Four visibility owner tests and strict all-target demo Clippy also
pass (`capability-final-native-r1.log`, `capability-final-style-r3.log`). No solver,
branch, golden expectation or acceptance assertion changed.

## Combined browser qualification checkpoint

Development-r4 now passes dense manifold navigation p95 422.32 ms / text ACK
345.58 ms and Gridfinity 195.04 / 370.62 ms, retaining the 500 ms thresholds and
zero navigation computation requests. The 32-client text case records p95 274.90 ms;
its 2,844,454 received bytes include 882,816 SSE bytes. A real stalled TCP reader
is retired below 128 KiB while healthy text ACK is 30.16 ms. Evidence:
`collaboration-browser-development-r4.log`.

Two first-run browser harness failures were corrected without product changes:
Node timers can fire fractionally early, so the hold now rearms until its monotonic
10,000 ms deadline; point gestures persist native design overrides, so the remote
case verifies exact changed design and unchanged raw source, rather than requiring
an unauthored source rewrite. Its exact terminal, one durable publication, reload
and cold restart pass in `collaboration-browser-followup-r4.log`. Peer CodeMirror
also receives text before release. The repeated four-context navigation measurement
is 615.02 ms despite 2–4 ms native wheel work; this remains an open performance
concern at this checkpoint, not a passing result or waived target. The following
checkpoint records its correction and recovery coverage.

### Final toolbar correction and browser recovery

The same settled-layout harness measured four-editor navigation p95 624.49 ms with
the ordinary toolbar CSS and 348.08 ms with only its backdrop filter disabled.
Removing `backdrop-blur-sm` from `canvas-controls.tsx` preserves the existing
95%-opaque surface, shadow and layout. This extends M98-F020's presentation repair;
native navigation remains 2–4 ms and no numerical or authority contract changes.

Ordinary development-r5 passes navigation p95 360.53 ms and text ACK 162.08 ms during
a 10,000.095 ms held solve, with independent cameras and zero navigation computation
RPCs (`collaboration-browser-product-r5.log`, 1 pass / 39.04 s). Diagnostic CSS
overrides are unset. Integrated performance scheduling remains required; these are
focused measurements rather than authenticated release evidence.

Both real browser recovery cases pass in `collaboration-browser-recovery-r3.log`
(2 pass / 45.04 s): exact persisted IndexedDB command retry after lost admission
ACK, cold server restart and page reload; and shared invalid source with retained
accepted canvas, canvas editing, rejected Apply and later immutable Apply while
typing remains unapplied. Join observation reads actual HTTP bodies, with separate
empty browser-error and observer-error assertions. No response or storage is faked.

The complete focused browser inventory now has passing evidence. Clean integrated
nomination and verified replacement delivery remain open.

### Integrated qualification harness corrections

Clean run `20260910T191453-62d4d77b` on `17f900a` records 256 passing stages,
including the complete native workspace, headless sample workflows, release WASM
and optimized WASM lifecycle. It stops at `prepare.m98` with
`incomplete M98 owning test inventory: collaboration.package`; it is not a
qualified release. The registry required nonexistent `host.test.mjs`, while the
actual authority owner is `authority.test.mjs`. Synthetic fixtures generated from
the same registry had concealed that drift. The corrected registry preserves its
five required families. A regression discovers all ten groups in the real
repository and proves that deleting the authority file still rejects the inventory.
All 17 runner owner tests pass (`collaboration-inventory-owner-r1.log`).

The ordinary artifact readiness checker separately assumed that the first sorted
WASM file was the workbench module. With collaboration, demo and engine modules,
it selected collaboration instead. The actual development-r5 endpoint reproduced
24 passing byte/MIME routes followed by `readiness did not load the nominated
WASM module` (`artifact-readiness-repro-receipt.json`). The checker now requires
exactly one nominated demo module and its exact loaded URL. Missing, ambiguous,
wrong-origin and optional-only module observations still fail. All 25 artifact
owner tests pass (`artifact-readiness-owner-r1.log`); the unchanged development-r5
bytes then pass all 24 routes and actual-WASM readiness
(`artifact-readiness-repaired-receipt.json`). No runtime or mathematical code changed.

Focused commands use the pinned Nix shell:

```bash
python3 -m unittest discover -s scripts/tests -p test_release_m98.py
node --test crates/geosolve-demo-web/frontend/scripts/test-release-artifact.mjs
node target/m98/reproduce-artifact-readiness.mjs
node target/m98/reproduce-artifact-readiness.mjs repaired
```

The quickstart and packaged CLI README now include the fourth collaboration archive
in offline installation. Because that README is a package input, its archive must
be freshly qualified. The resumed integrated runner will authenticate unchanged
passing evidence and execute affected or unfinished stages; the failed run remains
failed and does not qualify any replacement preview.

### Folder camera history baseline waited for the wrong event

Run `20260910T195659-44d843d6` records 261 passing stages and one failed folder
browser file. Its 49 ordinary browser workflows all pass without retries or skips;
ten of eleven folder workflows pass. The failing local-authoring history test
captures its camera after browser pointer-up delivery, using a viewport/server
inequality that the preceding wheel event already satisfies. The native pan reply
can still be pending at that point.

The expected centre is `[-1.754866693126985, 1.571693828701786]`; both the actual
client and server centres after Undo are `[-2.0873650025576267, 1.8155259222842568]`.
At zoom `45.113011328344676`, their displacement is exactly the requested
15-pixel right / 11-pixel down pan. Undo retained the completed local camera; the
harness had captured a pre-pan baseline. This is a harness race, not a new native
interaction defect.

The existing test now matches worker request/reply IDs, waits for that pan's
pointer-up acknowledgement and compares its input to the actual DOM terminal
before capturing the history baseline. Strict camera equality, exact point-drag
samples and publication/history assertions remain unchanged. The full focused
authoring workflow passes in 10.22 s (`folder-camera-harness-r1.log`) against the
second run's unchanged prepared browser artifact:

```bash
GEOSOLVE_DIST=<prepared-geosolve-harness> GEOSOLVE_BROWSER_EVIDENCE=target/m98/folder-camera-harness-r1 node --test --test-name-pattern="local folder authoring previews" scripts/workspace-browser.test.mjs
```

The failed run remains failed. The next integrated nomination must account for
the corrected harness and all remaining collaboration, package and release stages.


### Final collaboration nomination and delivery

Clean `513463f` passes all 288 obligations in `20260910T204328-4a05c31a`,
including the corrected folder authoring/history workflow and all eight collaboration
browser/fault/load cases. The two previous attempts retain their failed outcomes.
[Final qualification](M98_QUALIFICATION.md#qualified-multi-editor-collaboration) records
32 fresh and 256 authenticated reused results plus exact frozen/installed delivery.

The initial task-local delivery verifier expected standalone frame provenance
`accepted`; shared local navigation correctly reports `accepted-presentation`, as
covered by the native local-interaction owner. Correcting that ignored helper only
allows the same qualified bytes to pass editor/viewer, three-WASM, independent-camera
and source/model retention checks. Failed and passing receipts are retained. Both
24-route Tailscale verifications pass and all seven original authored files, original
authority and existing preview services remain unchanged. No new product defect,
equation, tolerance, branch or golden change was introduced by this delivery.


### Lightweight shared-playground review findings — open

The [playground delivery](M98_PLAYGROUND.md) records two newly observed limitations
of the qualified collaboration product: 3.7-second accepted peer publication even on
two circles, and rejected generated-polyline point Undo with a missing `invocation`
property. Exact live journals, source and browser evidence are preserved. Detached
domain profiling separates millisecond native solving from repeated cold compiler,
session and scene reconstruction. No production repair or new qualification is
claimed. The circle playground avoids the generated-polyline inverse path, passes
two-tab dragging/Undo and preserves the existing manifold preview.

### M98-F028 — Shared drag preview overwritten by accepted pointer frames

The user reports large back-and-forth flicker and input delay while dragging either
centre in the two-circle shared playground on qualified `513463f` (documentation
HEAD `48ff14f`). The adapter's exact frame-stream regression reproduces accepted
coordinates replacing every provisional preview. An isolated actual-browser fixture
against the installed qualified runtime and artifact independently records five
backward jumps during its first forward drag and two during its next drag. The
fixture holds the first server terminal and sends ordinary pointer/presence updates.
No live user document is changed. Classification: **DEFECT**, owned by the browser
collaboration adapter/controller; native preview geometry is already valid.

`installLocal` unconditionally paints accepted geometry over authoring prediction;
terminal handling also restores the pre-edit frame before refreshing the accepted
scene. The correction gives the provisional frame ownership through gesture and
pending terminal resolution, and retains native terminal presentation for camera
reprojection. Cancellation, rejection and accepted-model replacement release that
ownership. Ordered native path samples are preserved while intermediate rendering
is batched. Superseded browsing errors no longer become visible error notices.

Owning regressions cover every emitted/returned frame, held acceptance, cancellation,
rejection, remote replacement and stale gesture replies. The browser regression
`shared circle dragging paints continuously through pending acceptance and reaches
peers promptly` samples actual presented geometry/provenance throughout three drags,
including a held server commit. It includes native noninteractive provisional items;
the initial diagnostic helper wrongly observed only accepted draw IDs and its failed
receipt remains retained. Evidence is under `target/m98/coordination/drag-repair/`.
Integrated qualification and replacement delivery were pending at this development
checkpoint; the final F028–F030 qualification below supersedes it.

Native authoring reconstruction also begins immediately after authenticated join,
overlapping shared text and scene loading. Complete unchanged model replacement is
idempotent, and queued scene refreshes recheck their revision before replacing local
state. Three startup/duplicate-refresh regressions reproduce the old behavior; the
24-case adapter/controller suite passes the correction. The earlier six-file owner
run passes all 34 cases, including actual-WASM reprojection and accepted picking.

Focused browser runs now record zero backward jumps and approximately 80–101 ms
warm event-to-preview p95, with warm release-to-peer around 332–412 ms. Initial
browser/GPU use remains roughly 318–379 ms. A diagnostic observed a 227 ms first
highlight WebGL error-check stall; preparing the same filter offscreen did not improve
the full cold interaction and that experiment was completely reverted. Renderer code
and its existing qualification assertions remain unchanged. The new browser case
uses the existing M98 500 ms local-navigation budget on first use, a tighter 200 ms
budget on subsequent drags, and 1000 ms for warm release-to-peer. Its provisional
200 ms cold target was not achieved and is not claimed. Bounded GPU/long-task traces
and the failed development receipts remain available. Pan, wheel and resize during
independently held authoring and server completion pass at 30–171 ms, with zero
model/scene/preview requests. Final integrated measurements supersede these focused
development timings.

Final review reproduced the same held-navigation problem in the optional server
prediction route: its renderer forwarded a frame but omitted the already available
opaque native presentation. The actual browser's held server-prediction case failed
with `wheel waited for held prediction` (`remote-before.log`). Forwarding that same
native presentation enables the navigation worker's existing exact-view reprojection;
it adds no server authority or solving to navigation. The held authoring/terminal
pan, wheel and resize regression now exercises both client and server prediction.
Nomination `20260910T224255-cf8bbde7` was deliberately interrupted after successful
preflight and workspace preparation to include this correction. It remains incomplete
and does not qualify a release; unchanged completed evidence is eligible only through
the authenticated runner.

### M98-F029 — Repeated cold domain and scene reconstruction on each shared edit

The same isolated browser reproduction records roughly 3.3–3.7 seconds from release
to peer geometry for two circles. Earlier detached profiling isolates 6.87 ms native
preview and 64.30 ms terminal preparation, with 2.82 seconds in repeated server
reconstruction. Classification: **DEFECT**, owned by the collaboration domain worker
lifecycle, rather than a solver convergence failure.

Each open document now owns a serial, bounded, terminable worker with genuine compiler
receipts and exact-input authenticated native sessions. Sessions leave the cache
before mutation; historical replay and target-generation authentication remain intact.
Workbench scene construction retains its loaded worker, reconstructs each distinct
candidate's native presentation context and validates complete exported project/design
identity. Cached candidates never become durable authority before the
existing publication transaction. Cold ephemeral execution remains an independent
parity path. Cancellation/timeout discard the worker and its retained state.

Final focused two-circle measurements including historical replay, scene composition
and reconciliation are 127–213 ms warm (134 ms median), versus 2.13 s cold, with exact
accepted model/design/result parity. Five retained owner cases, the unchanged scene
owner and both before-append/after-fsync durability scenarios pass. Explicit result
ownership releases intermediate and evicted native results; fresh writable sessions
bound native history. Tests cover cache isolation, failed durability, timeout/restart
and full scene parity. A resize-first refinement failed the existing fitted-point
picking test; preserving native constructor and first-resize semantics corrects it.
The failed collateral receipt remains failed; all other 39 collateral cases passed.
Integrated qualification and preview delivery were pending at this development
checkpoint; see the final F028–F030 qualification below.
No equation, tolerance, branch choice or golden-oracle bytes change. Generated-polyline
Undo remains the separately recorded open finding.

### Qualification follow-up — provisional navigation witness

Run `20260910T224940-a860c18d` on `8c2e2df` completed with unchanged source but
failed collaboration browser qualification: nine of eleven cases passed. All three
new drag/held-navigation regressions passed, as did the independent 49-workflow
ordinary browser stage. This failed run does not qualify a release. Only its
authenticated unchanged-input successful stages may supply later evidence.

The older `remote server preview leaves browser navigation local and its finished
point gesture commits durably` witness filtered every noninteractive draw item.
Genuine native provisional geometry is intentionally noninteractive, so the witness
compared empty arrays while geometry zoomed correctly. The corrected helper observes
actual `wb-point`/`wb-curve` coordinates and radii in either state, excluding presence
and UI chrome. It retains every timing, zero-RPC, accepted-authority, exact terminal,
unrelated-point, reload and cold-restart assertion. This is a harness error, not a
second product defect. Against the same prepared `8c2e2df` artifact the focused
case passes, with 98.44 ms navigation p95 and zero navigation RPCs:

```sh
GEOSOLVE_DIST=target/release-gate/prepared/d51eb97d1929d48d2976470a92283560bf158d8f953382ce50fa670ce6035a9e/browser/geosolve-harness node --test --test-name-pattern="remote server preview leaves" scripts/collaboration-browser.test.mjs
```

The separate invalid-draft/Apply workflow exposed the genuine native selection
reconciliation defect below. Its original browser assertions remain unchanged.

### M98-F030 — Accepted reconstruction discarded personal selection and pins

The same integrated run loses the selected `bore` after captured Apply reaches
accepted revision 2, removing its radius row from Inspector. The public actual-WASM
`InteractionHandle.replace` boundary independently reproduces selection length
**1 → 0** when the exact same authored project/design is constructed twice in one
native module. Camera remains unchanged. Each reconstruction allocates fresh native
document/entity IDs, while its exact source-owned presentation symbol remains equal.
Classification: **DEFECT**, owned by Rust `LocalInteraction::replace_json`; this is
personal browsing state, not shared semantic target authority.

The old replacement path validates raw previous native IDs against the next scene.
Retaining the backend worker exposes this previously accidental dependency on
restarted allocators. Exact reproduction and identity receipts are retained in
`target/m98/coordination/drag-repair/recovery-review.md` and
`recovery-native-before.log`. A new native regression also demonstrates that a
colliding old numeric ID can incorrectly retain a different point.

Rust now retains the authenticated presentation bindings and uses a separate partial
map for replacement: exact surviving source symbols, compatible complete binding
shapes and unambiguous correspondence in both directions. Translated selections
and native/implicit curve occurrences must validate in the replacement scene.
Missing or incompatible owners are discarded. Dimension pins/focus follow the same
map; camera, visibility and navigation state remain personal. The strict complete
mapping used by prediction is unchanged, and these bindings confer no shared edit
authority. Five new owner tests cover surviving state, deletion/incompatibility,
ambiguity, exact implicit occurrence and malformed-seed transactionality.

`cargo test --locked -p geosolve-demo-web --lib local_interaction -- --nocapture`
passes all 23 cases (190.37 s). Narrow warnings-denied Clippy, TypeScript and diff
checks pass. The thin actual-WASM Inspector replacement regression demonstrably
fails on the previous WASM. The rebuilt actual-WASM browsing suite passes all four
cases. Its first post-build run exposed a new-test expectation that selecting only
a circle center marked the whole declaration selected; the expected `partial`
state now matches the existing native contract while requiring exact center
identity and a populated radius Inspector. Final qualification is recorded below.
An incidental development assertion assumed an allocator collision during parallel
tests; that assumption was removed while retaining all ownership assertions and
the separately captured real collision reproduction. No mathematical behavior,
accepted source/design export, branch semantics or golden bytes change.

Focused build/parity commands for F030 (pinned shell):

```sh
npm --prefix crates/geosolve-demo-web/frontend run build:release-artifacts -- --out "$PWD/target/m98/coordination/drag-repair/native-replacement-artifacts"
cd crates/geosolve-demo-web/frontend && npx vitest run src/lib/collaboration-browsing-worker.test.ts
```

Release WASM and both browser bundles build successfully. The four-test parity suite
passes after the above expectation correction. Original failed receipts are retained.

The two previously failing browser workflows both pass against that rebuilt harness
(37.76 s total), preserving the original invalid-draft/Apply assertions. Accepted
radius is 6 at revision 2 while later shared working source remains at radius 8;
the selected Inspector persists. Held server-preview navigation p95 is 97.50 ms
with zero navigation RPCs, one exact durable terminal and successful cold reopen.

```sh
GEOSOLVE_DIST=target/m98/coordination/drag-repair/native-replacement-artifacts/geosolve-harness node --test --test-concurrency=1 --test-name-pattern="browser preserves invalid draft|remote server preview leaves" scripts/collaboration-browser-recovery.test.mjs scripts/collaboration-browser.test.mjs
cargo fmt --all -- --check
```

Both focused commands pass. The following integrated qualification and preserved
preview delivery complete the repair.

### Final F028–F030 qualification and preserved delivery

Clean `9b64c29` passes all 288 obligations in `20260910T233626-b8af5961`:
34 fresh, 254 authenticated reused, 36m7.710s. All 49 ordinary and 11 collaboration
browser workflows pass with no failures, skips or retries. The 363-case native
workbench stage, 271-case unchanged golden, 140 runtime, 44 package and 62 frontend
collaboration assertions pass. [Full qualification and exact commands](M98_QUALIFICATION.md#qualified-shared-dragging-repair)
record authentication and retained failed attempts.

Final drag trials record zero reversals, warm local p95 63.0/75.8 ms and warm
release-to-peer 351.1/383.3 ms. First use is 426.8 ms locally; held authoring/terminal
navigation is 59.7–166.0 ms with zero navigation RPCs in both prediction modes.
First-use GPU latency remains an explicit limitation. The generated-polyline Undo
finding remains separate and open.

On 2026-09-11, both existing shared previews receive exact frozen/installed artifacts.
Playground revision 6 and manifold revision 1, all 32/25 retained files, three users'
histories per document, drafts, models and invitations compare unchanged before restart
and after browser verification. Each service passes all 24 exact HTTP/MIME routes and
two-editor readiness with the three exact nominated WASM files and zero page errors.
Two ignored delivery-helper mistakes were corrected without changing the qualified
product; their original failed observations remain recorded in the qualification report.
Static 18110 and the original 18108 manifold remain preserved. M98 acceptance/closure
remains open; F028–F030 implementation, mechanical qualification and delivery are complete.

### M98-F031 — Generated point history confused semantic and allocated addresses

While implementing the authorized toolbar amendment, a focused domain regression reproduces
the previously observed generated-polyline Undo failure on baseline `0af5b82`. Alice drags
the first keyed polyline point from `[0,10]` to `[4,16]`; Bob independently changes a circle
radius; Alice's personal Undo rejects with `Cannot read properties of undefined (reading
'invocation')`. Classification: **DEFECT**, owned by the collaboration domain's native
point-history adapter. No solver failure or geometry tolerance change is involved.

Both a stable generated-member lens and a legacy allocation-specific native address have
an `owner.address` field. The adapter treated either as the legacy shape. It now checks the
native owner discriminant before retaining that legacy path; generated semantic lenses
resolve uniquely against the current native writable point inventory and source codec.

`node --test --test-name-pattern='generated polyline point history'
scripts/collaboration-domain-properties.test.mjs` first fails with the exact error above.
After repair, `node --test scripts/collaboration-domain-properties.test.mjs` passes all
seven tests (45.35 s). The new regression verifies exact original point drafts after Undo,
exact moved drafts after Redo, unchanged independently edited source and cold accepted-model
rebuild. Existing fresh-allocation, codec mismatch, ownership and metadata checks also pass.
Integrated qualification and refreshed previews remain pending with the toolbar amendment.

### M98-F032/F033 — Complete construction source recipes

The full 25-variant native construction/compiler inventory on `0af5b82` plus the
in-progress toolbar integration reproduced two source-authoring defects. The
ThreePointCircle clicks `[10,10]`, `[14,10]`, `[12,13]` completed natively, but source
projection synthesized equilateral control points from its derived center/radius.
Compiling that new triplet changed the exact native seeds and terminal validation
correctly refused publication. The retained construction continuation now keeps the
three native-accepted defining clicks through correction and Step Back; independent
server replay derives the same explicit source recipe.

TangentArc source insertion omitted its required center and top-level contact
orientation. Genuine compiler admission rejected the missing center; an opposed
contact also needed its explicit orientation retained. The source insertion owner now
projects both from the authenticated native curve/contact state. Neither correction
changes solver equations, primitive definitions, tolerances or terminal equality.

`cargo test --locked -p geosolve-sketch-engine --test construction_parity` passes eight
tests, including all 25 genuine compiler/native/cold/history recipes (one explicitly
ignored fixture writer). The existing construction suite passes five tests;
`cargo test --locked -p geosolve-sketch-code --test compact_geometry_recipe_roundtrip
 tangent_arc_keeps_its_named_source_contact -- --exact` passes. Actual WASM
`node --test packages/geosolve-engine/test/construction.test.mjs` passes all 32 tests,
including all 25 recipes and opposed tangent contact. The first rational-conic WASM
assertion expected three native DesignPoints; its representation has two endpoints and
scalar middle controls. Correcting that count was a **HARNESS_ERROR**, with all exact
compiler/native and geometry checks retained.

### M98-F034/F035 — Source ownership and Profile Offset label projection

The all-tool native compiler lifecycle fixture uses an open two-span Polyline through
`[200,0]`, `[220,0]`, `[220,20]`, with `closed` omitted. Native Fillet/Offset authoring
was valid, but `direct_single_curve_polyline_source` dropped this source owner because
it required an explicit boolean. The insertion owner now recognizes the documented
omitted-false default while continuing to reject invalid values. The fixture retains
that omission and exercises a 2 mm Fillet and Profile Offset.

Profile Offset additionally inserts a logical open-chain aggregate that has no native
object label, and its generated distance scalar uses the exact suffix ` distance`.
The engine terminal label projection incorrectly required native ownership for the
aggregate and accepted only dot-prefixed label suffixes. It now admits an aggregate
only when both sides have no native owner and exact kind, fields and inputs, with no
reservations, operation outputs or children. The space-prefixed scalar suffix is
restricted to matching ProfileOffset operation owners. Complete native ownership,
geometry, branches and residual validation remain unchanged.

`cargo test --locked -p geosolve-sketch-engine --test tool_operations` passes the
complete genuine compiler/native/cold/Undo/Redo matrix for all 13 constraints, five
dimensions, Fillet, Offset and a directly authored role change. The subsequent focused
run passes 11 tests plus the explicitly ignored fixture writer, including ordered
viewport changes, reset behavior and the cumulative trace regression below. These
are development checks; integrated toolbar qualification remains pending.

### M98-F036 — Operation previews omitted native operand presentation

A native Parallel draft with one picked Polyline span retained a pending operand but
its `presentation_json` returned only scene/bindings. Actual WASM independently showed
that radius hover and point-distance pending picks left presentation byte-identical.
Fillet/Offset geometry was present, but hover/pending styling and Offset chain cues
were absent. Classification: **DEFECT**, owned by the engine presentation boundary.

The focused `tool_presentation` regression first fails on the missing pending array.
The repair carries native pending curves, provisional items, compatible hover,
Offset availability and ordered chain/endpoints in a paint-only payload. The demo
renderer consumes those states in the prediction scene's namespace and reprojects
using the user's current camera. Every resulting item remains noninteractive;
accepted navigation and server publication authority stay independent. Native and adapter regressions pass, including current-camera Offset chain cues and
active provisional dimension visibility in Hidden mode. The actual-WASM tool matrix
and focused browser Offset/Fillet workflows also pass.

### M98-F037 — Cumulative operation trace exceeded its byte reservation

An actual-WASM reproduction submits 140 ordered selection batches, each with 256 datum
operands, retaining 1,299,092 serialized bytes despite a one-MiB trace reservation.
The per-request and 4,096-sample limits did not constrain cumulative sample size.
A public native engine regression independently reproduces the same boundary failure.

Operation continuation now accounts for cumulative serialized samples before dispatch
or sequence mutation, matching construction's existing bound. Exhaustion retains the
previous native draft and accepted origin; callers can still cancel normally. The
native `repeated_tool_selection_batches_cannot_exceed_the_reserved_trace_bytes`
regression passes in the 11-test focused operation suite. WASM-adapter and integrated
qualification follow with the toolbar amendment.

### M98-F038 — Empty resets and refused relations broke native retry

Actual-WASM review reproduces two operation-wrapper lifecycle failures: Reset on an
empty Radius or PointDistance collector silently exits its native mode, and a refused
Horizontal relation leaves its full operand list pending, blocking the next valid
pick. The minimal genuine source fixture contains a fixed skew segment
`[0,0] → [20,10]` and a separate horizontal segment `[0,30] → [20,30]`.
The owning native regressions independently fail both cases in the 13-test operation
suite before correction (11 pass, two fail; fixture writer explicitly ignored).

The reusable authoring owner's cancel method intentionally treats a second Escape as
mode exit. Tool reset now leaves an empty collector active, while the UI uses native
pending-state capabilities to implement first-Escape clearing and second-Escape exit.
Every Apply outcome clears terminal operands as required by the native owner.
Furthermore, the projectional editor deliberately retains valid-but-unsolved intent;
the operation wrapper now attempts a relation on a transient fork and installs only
an independently accepted result. A refused attempt retains the exact previous draft
editor, with its diagnostic available for correction. Forking adds no second solve.
The regression continues through a valid retry and source preparation, preventing a
refused relation from leaking into the later terminal. The final operation suite passes 13 tests plus its explicitly ignored fixture writer.


### M98-F039 — Synchronous GPU completion stalls canvas input

During toolbar qualification on `ec9362b`, collaboration browser run
`20260911T133551-a403dc7b` retained zero drag reversals but exceeded existing
500 ms budgets for cold drag (550.2 ms), server Offset resize (1199.7 ms) and
manifold navigation (512.8 ms). The ordinary browser suite ran concurrently.
The collaboration suite contained measured latency obligations but declared only
a shared memory resource; a focused runner regression reproduces that missing
isolation. It now requires exclusive gate execution, without changing thresholds.

A separate Fillet browser assertion observed toolbar readiness and immediately read
the last completed GPU frame. Those are asynchronous queues. Waiting for a genuine
provisional `wb-computed-fillet` item before publication preserves the original paint
requirement and passes on the exact optimized artifact. This is a harness correction.

Isolated repetition still failed three timing checks: cold drag 910.4 ms, server
Offset wheel 588.1 ms and manifold navigation 1251.6 ms. The retained drag trace
records a synchronous `gl.getError()` call taking 455.8 ms, with subsequent calls
at 70–111 ms. All three drags retain zero reversals. Running the same drag test
against the prior qualified `9b64c29` frontend on this host also fails: cold
804.8 ms, warm 225.3/266.8 ms, and a 421.5 ms error-query stall. Current host load
therefore exposes an existing browser presentation bottleneck; the evidence does
not establish a new toolbar or solver regression. The baseline comparison uses
the same current isolated server and the exact previous frontend artifact.

Owner: `canvas-renderer-pixi.ts` and `canvas-renderer.ts`. The renderer flushes and
synchronously queries GL errors after every draw on the browser's input thread.
The query waits for earlier GPU work; simply removing it or moving it into a timer
would either weaken frame validation or relocate the same stall. The repair
observes WebGL2 fence completion asynchronously before retaining the existing
error check. Zero-timeout polls yield through scheduled 4 ms tasks; a still-unsignaled
fence after five seconds fails explicitly. Completed fences remain valid when a
background tab delays its first poll. At most one draw may be in flight, newer immutable inputs
coalesce, and only the exact validated frame can advance presentation evidence.
Context loss, disposal, failed fences and timeouts must not publish late frames.
No Rust solver, numerical tolerance, draw style, shader warmup or timing threshold
change is part of this repair. All 39 focused renderer tests and TypeScript checking pass. The first browser
trial passed Fillet, server Offset navigation and manifold navigation, but cold
drag narrowly missed 500 ms at 500.8 ms. Reducing the polling delay from 8 ms to
the browser nested-timer floor of 4 ms avoids unnecessary queued-frame latency.
The resulting cold drag is 416.2 ms; warm drags are 129.5/116.8 ms, all with zero
reversals. Warm release-to-peer is 443.4/358.7 ms. All four real-browser renderer workflows pass on the final isolated artifact:
pixels/idle behavior, DPR/hidden layouts, context loss including first-shader-loss
restoration, and pointer-capture recovery. These focused observations do not replace
the pending integrated qualification.

Preserved evidence under `target/m98/coordination/tool-parity/`:
`browser-isolated-r3.log`, `drag-prior-artifact-comparison.log`,
`frontend-gate-review.md`, `browser-scheduling-review.md`, `renderer-review.md`,
`browser-scheduling-before.log` (expected failure),
`browser-scheduling-after.log` (17 pass) and
`browser-scheduling-pipeline.log` (24 pass). The two failed product nominations
remain failed; successful independent receipts may be reused only by the runner.

Final focused commands use the pinned Nix shell.
`npx tsc -b --pretty false` and
`npx vitest run --no-cache src/lib/canvas-renderer.test.ts src/lib/canvas-renderer-pixi.test.ts`
pass in the frontend directory (39 tests). An isolated compiler-harness
`npm run build:ui` preserves the existing three WASM modules.
`node --test --test-concurrency=1 --test-name-pattern="shared circle dragging"
scripts/collaboration-browser.test.mjs` passes on that artifact.
`npx playwright test tests/e2e/canvas-renderer.spec.ts --workers=1` passes
all four cases in 1.1 minutes with the exact isolated manifest and port 18120.
Logs: `renderer-root-review-tests.log`, `renderer-poll-tests.log`,
`browser-async-gpu-r4.log`, `browser-async-gpu-drag-r5.log` and
`async-gpu-renderer-browser.log` in the coordination directory.
