<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation findings

M98 and the F041–F043 repairs are mechanically qualified and delivered from `b49e339`;
[the complete qualification and delivery record](M98_QUALIFICATION.md#qualified-uat-repairs)
passes 293/293 obligations. U02 remains Fail pending human recheck; milestone acceptance
and closure remain open. The records below preserve focused findings and earlier failed
attempts separately from that final result. The hardening skill routes each defect to
its owning Rust, interaction, persistence or presentation boundary.

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
First-use GPU latency remained an explicit limitation at this F028–F030 delivery
checkpoint. The generated-polyline Undo finding remained separate and open then;
M98-F031 below records its subsequent repair.

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
The toolbar amendment's [integrated qualification and preserved delivery](M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
subsequently qualify this repair.

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
were development checks at that checkpoint; [final integrated qualification](M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
subsequently covers the toolbar amendment.

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
qualification were subsequent requirements at this checkpoint; the toolbar amendment's
[final record](M98_QUALIFICATION.md#qualified-shared-toolbar-parity) records their completion.

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
restoration, and pointer-capture recovery. These focused observations preceded integrated
qualification; [the final nomination](M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
records the independently authenticated whole-suite results.

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

### M98-F040 — First-hover shader compilation delays queued navigation

Clean `8ac6d11` run `20260911T153007-f3ebd337` passes 49/49 ordinary browser
workflows and 15/16 collaboration workflows, but manifold navigation p95 is
506.9 ms against the unchanged 500 ms budget. The first sample includes a hover
before wheel delivery. An isolated trace identifies a 169 ms main-thread task;
native wheel replies take 8–11 ms. A shader-source diagnostic confirms that
horizontal/vertical blur program linking and metadata queries cause the stall
(111.7/29.5 ms for the largest two calls). Geometry and server acceptance remain
correct. This is a browser renderer latency defect, not a solver failure.

Owner: `canvas-renderer-pixi.ts` and `canvas-renderer.ts`. A queued frame also
waited for another RAF after its predecessor completed GPU validation. Immediate
submission of the newest coalesced input removes that gap while preserving a
single in-flight draw, exact submitted-frame/surface witnesses, failure latching,
hidden suspension and context epochs. This alone passes manifold at 438.5 ms but
still misses four-editor navigation at 560.3 ms; it is insufficient by itself.

The renderer now prepares the exact native-shadow horizontal/vertical blur
programs through Pixi's public shader binding API during the first ordinary
render, after backend installation. It repeats setup after context restoration.
No synthetic primitive, offscreen draw, RenderTexture, visual-quality reduction
or timing-budget change is introduced. Startup includes shader initialization;
readiness still requires the real frame's GPU fence and error validation. Owned
preparation shaders release their own resources without destroying shared cached
programs. This differs from the earlier reverted synthetic-draw warmup experiment.

Three focused shader setup/restoration/failure tests fail before the correction.
All 44 focused renderer tests and TypeScript checking pass afterward, including
synchronous reentrant input and failure of an immediate queued successor. The
three focused collaboration workflows pass: cold/warm drag p95 450/173.9/116.5 ms,
zero reversals, four-editor navigation 363.1 ms p95 and manifold navigation 461.8 ms
p95 with 351.9 ms text acknowledgement. Existing budgets and all samples remain.
All four actual-browser renderer workflows pass (56.9 s), including independent
batch and horizontal/vertical blur shader-loss injections with line/text pixels.
[The final nomination](M98_QUALIFICATION.md#qualified-shared-toolbar-parity) records
integrated F040 qualification and preserved preview delivery.

Evidence: `target/m98/coordination/tool-parity/` contains
`immediate-frame-browser-r1.log`, `shader-profile-r1.log`,
`shader-prepare-before.log`, `shader-prepare-after.log`,
`renderer-schedule-review-result.md` and `hover-shader-review.md`.

Focused F040 commands ran in the pinned Nix shell:

```bash
cd crates/geosolve-demo-web/frontend
npx vitest run src/lib/canvas-renderer.test.ts src/lib/canvas-renderer-pixi.test.ts
npm run check:types
GEOSOLVE_BROWSER_COMPILER_HARNESS=1 GEOSOLVE_DIST=../../../target/m98/geosolve-shader-ready-r1 npx vite build
```

From the repository root,
`GEOSOLVE_DIST=target/m98/geosolve-shader-ready-r1 GEOSOLVE_BROWSER_TRACE=1 node --test --test-name-pattern="^dense manifold|^shared circle dragging|^four real browser" scripts/collaboration-browser.test.mjs`
passes 3/3 with no skips/retries. Renderer qualification uses
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome GEOSOLVE_E2E_PORT=18120 GEOSOLVE_E2E_ARTIFACT_MANIFEST=/home/arduano/programming/geometric-constraint-solver-worktrees/m98-file-workspace/target/m98/coordination/tool-parity/shader-ready-harness-r1.json npx playwright test tests/e2e/canvas-renderer.spec.ts --workers=1`
from the frontend directory and passes 4/4. Two preceding harness-only attempts
failed before assertions: a relative manifest path and a bundled Chromium missing
host libraries. The final attempt uses the same installed Chrome as collaboration
qualification; it neither rebuilds the artifact nor changes an assertion.

### Final toolbar amendment qualification

Clean final product `b005b9e1bdb1a120ec9363d4c8684d1f8f5c3d00` passes **293/293 obligations** in
`20260911T164104-576e7789` (20 fresh, 273 authenticated reused; 37m48.118s).
[Final qualification and preserved delivery](M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
authenticate F031–F040, optimized artifacts, unchanged reviewed golden, all current toolbar
routes and exact browser paint/navigation. Both existing shared previews use the exact
frozen artifacts without rebuilding and preserve current documents, drafts, invitations
and personal histories. Earlier failed runs remain failed evidence. Qualification receipt:
`target/release-gate/runs/20260911T164104-576e7789/qualification.json`. Supervising-user acceptance and M98 closure remain open.


## M98-F041 — Free polyline corner rejected after valid drag preview

**Current disposition, 2026-09-13:** repaired, qualified and delivered in `b49e339`.
The focused/pending checkpoints below are historical. See [final evidence](M98_QUALIFICATION.md#qualified-uat-repairs).

The supervising user reported U02 failing in Shared playground on 2026-09-12:
interior corners snap back on release; moving the connected endpoint first permits
the next corner drag. Source `f94b09a` serves qualified product `b005b9e`. Exact
read-only capture of the live journal authenticates 31 records and 29 attachment
blobs; all fifteen admitted point gestures replay identically through the installed
domain (nine rejections, six accepted operations). Accepted outputs match historical
model/input identities and rejected attempts preserve accepted state. The captured
playground source SHA-256 is
`1b8c71b049bce988371449eb8bfc0f32aeb38bb73ebcca87169d98357dd6bdbd`.

Public Rust `EditableSession` and installed engine WASM reproduce a cold corner
`[-25,-20]` to `[-30,1]` over four accepted samples. Finish succeeds, but commit
reports `candidate evaluation rejected and policy requires accepted publication`.
Stopping at `[-30,-1]` passes. Moving the end `[-25,0]` to `[-23,2]` first allows
the same corner destination. Moving the start first does not.

Classification: **DEFECT**, accepted continuation/branch transport. Free native
segments use their current geometric direction while retaining dormant branch
metadata. Source recompilation derives a new reference vector; the unchanged
positive-cell audit then rejects a rotation past 90 degrees. This is independent
of browser, network and solver convergence. Explicit branches and active branch
constraints must retain strict validation.

The exact owning regression
`a_cold_polyline_corner_can_rotate_past_its_original_segment_hemisphere` in
`geosolve-sketch-engine/tests/point_gesture.rs` fails at cold publication (endpoint-first
control passes). It checks stationary endpoints, finite geometry, exact restoration
and Undo/Redo. Pinned command:

```bash
cargo test --locked -p geosolve-sketch-engine --test point_gesture a_cold_polyline_corner_can_rotate_past_its_original_segment_hemisphere -- --exact --nocapture
```

Before repair: exit 101, 0 passed/1 failed/14 filtered.
`node target/m98/coordination/corner-drag/exact-replay.mjs` exits 0 and reproduces
all fifteen historical outcomes. Evidence is retained under
`target/m98/coordination/corner-drag/`, including `native/diagnosis.md` and
`exact-replay/REPORT.md`. Repair, replacement qualification and U02 human recheck
remain pending; existing user previews are unchanged.


### F041 repair and focused qualification

The sketch owner adds `transport_unenforced_source_line_branches`: an explicit
three-document projection from trusted origin, native terminal and exact new source
seeds. Named branch bits must match the origin exactly; only an unenforced reference
may follow a finite endpoint rotation outside its old hemisphere. Batched changes
retain every coordinate, scalar and unrelated durable field. Generic branch-cell
comparison remains strict, and cold materialization still independently validates
residuals before publication. No residual equation, solver tolerance, wire format,
source schema or golden value changes.

`geosolve-sketch-code::transport_code_point_terminal_branches` authenticates source
Polyline ownership and excludes authored `branchDirections`. It selects only spans
with changed source endpoints and an actual hemisphere crossing; ordinary drags
return no transport documents. This addresses review feedback about unconditional
whole-document validation and unrelated dormant spans. Engine and standalone
workbench publication/parity use this shared source-owner seam.

Focused commands ran in the pinned Nix shell:

```bash
cargo test --locked -p geosolve-sketch --lib projectional_branch_audit_tests
cargo test --locked -p geosolve-sketch-engine --test point_gesture
cargo test --locked -p geosolve-demo-web --lib m98_f041_cold_corner_publication_preserves_native_terminal_and_history
cargo test --locked -p geosolve-sketch-code --lib ordinary_point_terminals_do_not_allocate_branch_transport_documents
node packages/geosolve-engine/scripts/build-wasm.mjs
node --test --test-name-pattern=M98-F041 scripts/collaboration-domain-properties.test.mjs
cargo clippy --locked -p geosolve-sketch-code -p geosolve-sketch-engine -p geosolve-demo-web --all-targets --all-features -- -D warnings
cargo fmt --all
```

All pass: 11 sketch branch audits, 15 engine point-gesture tests, one standalone
workbench regression, one no-transport-path test and two actual-WASM shared-domain
regressions. The two shared cases use source-derived and explicitly authored
branch vectors; both verify peer edits, personal Undo/Redo and exact cold rebuild.
Explicit vectors remain `[[1,0],[0,1]]` while the free geometry rotates. Logs under
`target/m98/coordination/corner-drag/`: `focused-native-after-r2.log`,
`focused-wasm-build.log`, `domain-after.log`, `focused-clippy.log`.

These focused results do not yet qualify a replacement preview or close U02/M98.

The replacement domain also passes
`node target/m98/coordination/corner-drag/exact-replay-after.mjs`: all fifteen
original gestures independently replay from their authenticated historical bases.
All nine formerly rejected corners accept; all six formerly accepted models/input
identities remain exact. Every result has finite points, zero independently
validated hard residual, stationary unrelated points and exact cold restoration.
Receipt: `target/m98/coordination/corner-drag/exact-replay-after-r4/report.json`.
Three preceding harness attempts remain recorded (JSON property-order equality,
existing scratch directory and per-folder worker binding); those are harness
errors and caused no product changes or alteration of the original captures.


### F041 first integrated nomination and browser readiness repair

Clean `a37a8be` nomination `20260912T191359-ad253e32` retains 278 passing
obligations, including 49/49 ordinary browser workflows with no retries/skips and
the unchanged 271-case golden, but fails collaboration browser qualification at
14/16. It is not a qualified replacement and no preview was upgraded.

The invalid-Apply drawing check captured a stale frame before Split layout
finished presenting its resized viewport; the failed coordinates differ by a
282 px horizontal translation. The browser helper now waits for the accepted
frame and renderer dimensions to match the actual canvas, retaining the complete
before/after geometry equality. The focused invalid-draft/Apply/peer-typing case
passes on the same prepared product in `recovery-after-r1.log`.

The same run exceeds the unchanged 500 ms dense-manifold budgets: first
navigation 608.9 ms and text acknowledgement 745.6 ms. A focused unchanged-artifact
trace reproduces first-navigation 545.7 ms, subsequent moves 342.7–419.0 ms and
text acknowledgement 458.6 ms. An additional instrumented trace locates most
navigation delay after the local wheel reply (12–47 ms), during presentation;
its extra observations are diagnostic, not qualification. Neither timing limits
nor first-use samples were weakened. Logs remain in
`target/m98/coordination/corner-drag/{release-gate.log,manifold-focus-r1.log,manifold-diagnostic-r2.log,recovery-after-r1.log}`.
Replacement qualification and human U02 recheck remain open.

The resumed nomination `20260912T202633-ed209d3a` on `55faa24` preserves
unchanged native/WASM/browser evidence but fails the folder-browser selection
check at 10/11. The test chose click coordinates immediately after middle-button
pan, before that pan was painted. It now independently requires the requested
18 × 12 px translation within the existing one-second limit before sampling the
curve. Full selection, local navigation, held-edit and source/authority retention
assertions remain. The focused case passes in `folder-focus-r2.log` (1/1); an
earlier full-frame wait experiment also passed in `folder-focus-r1.log`.
No product change or new release claim follows from these helper corrections.

The complete frame/worker/fence diagnostic `manifold-draw-r3.log` passes its
unchanged dense-manifold assertions: navigation p95 465.7 ms, text ACK 468.2 ms,
zero navigation RPCs and successful held-edit publication. Its captured
`manifold-draw-r3.json` confirms a hover submission precedes the wheel submission;
local Rust wheel replies remain fast while software-rendered frames serialize.
Earlier failed timing measurements remain evidence of host/presentation variability;
this instrumented pass does not replace integrated qualification or establish a
performance improvement. No rendering behavior, resolution or timing budget changed.

The third nomination, `20260912T205123-b192bffb` on `3d39e35`, passes both
readiness corrections but fails collaboration browser qualification at 13/16.
First circle-drag event-to-paint p95 is 520.5 ms, four-editor navigation is
650.5 ms and manifold navigation is 670.4 ms, each against the unchanged 500 ms
limit. Manifold text acknowledgement passes at 440.9 ms; Gridfinity navigation
passes at 343.0 ms. The failed nomination remains preserved in
`target/m98/coordination/corner-drag/release-gate-r3.log` and its run's
`stages/collaboration.browser/scratch/m98/node.tap`. No preview replacement or
new qualification follows from the passing corner regressions alone.

## M98-F042 — Interactive supersampling serializes expensive canvas draws

**Current disposition, 2026-09-13:** repaired, qualified and delivered in `b49e339`.
The focused/pending checkpoints below are historical. See [final evidence](M98_QUALIFICATION.md#qualified-uat-repairs).

The F041 qualification failures above independently reproduce delayed first-use
dragging and navigation on clean `3d39e35`. The complete manifold trace shows
an old-camera hover draw beginning around 45 ms, followed by the local wheel
reply around 61.5 ms; GPU completion takes 232 ms for hover and another 181.4 ms
for zoom. Local wheel work takes 12–47 ms. At display DPR 1 the backend always
uses a 2× framebuffer, approximately 3.4 million pixels for this canvas, with
antialiasing and native shadow filters. This is a presentation cost; F041 branch
transport does not run during navigation.

The presentation repair uses the actual display DPR for changing frames,
including peer updates and delayed native previews, and holds that quality while
pointer/wheel input is queued or a pointer remains captured. It restores the existing minimum 2×
supersampling after 1.5 s of idle time following completed rendering. Activity
alone does not submit another old-camera frame. Each GPU submission retains its
own quality, frame, surface and context identity; only validated completion
publishes its witness. Text textures retain their original supersampled quality
across framebuffer changes. DPR 2 and higher require no quality-only redraw.
Native geometry, CSS sizes, input ordering, branch policy, server authority and
all existing performance budgets remain unchanged.

The extended existing browser pixel/idle case fails on the previous prepared
artifact at the intended active-resolution assertion (`2` versus expected `1`),
after its independent 16 × 12 px pan translation passes. Baseline log:
`target/m98/coordination/corner-drag/active-raster-before.log`. The case also
requires real point pixels during a held pan, exact geometry and source retention,
restored static quality and the unchanged no-redraw assertion. Pixel captures
bind one completed frame/surface identity across the screenshot and use that
captured scene for every sample coordinate. Replacement checks and integrated
qualification are pending; no preview has been upgraded.

The first local-input-only raster prototype passes 93 focused renderer/Pixi/
viewport tests and TypeScript checking, but its isolated browser measurement
still fails all four existing latency cases. Cold circle preview p95 is
549.2 ms (zero reversals), four-editor navigation 750.1 ms, manifold first
navigation 561.4 ms and Gridfinity first navigation 625.3 ms. Subsequent dense
navigation is 219–361 ms and 197–272 ms respectively; text acknowledgement
passes. Inputs, isolated build and all measurements remain in
`target/m98/coordination/corner-drag/interaction-raster-{inputs,build,performance}-r1.*`.
This failed prototype is not a qualified improvement. Review additionally
found a queued idle-refinement draw surviving restarted input and shared
authoring work outliving the viewport queue. The follow-up policy covers all
changing accepted frames without awaiting shared authoring on the navigation
path, cancels only unsubmitted refinement work, and exposes pending/settled
presentation status for exact pixel capture barriers.

The revised policy passes
`npm test -- --no-cache src/lib/canvas-renderer.test.ts src/lib/canvas-renderer-pixi.test.ts src/components/canvas-viewport.test.tsx`
(98 tests), `npm run check:types` and `git diff --check`. Its isolated harness
passes all four existing `canvas-renderer.spec.ts` browser workflows in 1.2 min,
including the extended active/static pixel witness, the unchanged idle assertion,
DPR/hidden layouts, exact picking and CSS displacement, context restoration and
first-use shader-loss text/stroke pixels, and lost-capture recovery. The command
uses the pinned Nix shell and Chrome, `GEOSOLVE_E2E_PORT=18126`,
`GEOSOLVE_E2E_ARTIFACT_MANIFEST=…/interaction-raster-harness-r2.json`, and
`npx playwright test tests/e2e/canvas-renderer.spec.ts --workers=1` with a fresh
output directory. Full output and browser evidence are retained in
`target/m98/coordination/corner-drag/interaction-raster-pixels-r2{.log,/}`.
These focused passes do not yet establish the required latency or integrated
release qualification.

The r2 performance run passes circle dragging (483.9 ms cold, 120.5/127.4 ms
warm, zero reversals), manifold navigation (485.5 ms), Gridfinity navigation
(349.2 ms) and all text acknowledgements. Four-editor navigation still fails at
842.7 ms. Its first wheel API call spends 506.9 ms before returning; the local
wheel reply takes only a few milliseconds. The retained diagnostic
`four-raster-trace-r3.json` shows all four documents remain visible, disproving
the proposed hidden-tab activation explanation. Three peer canvases submit
2× idle refinements just after the first pointer move, while the originating
canvas finishes its own 2× draw. Those submissions precede the interactive
hover and zoom draws.

Both instrumented four-editor copies time out later in their browser flow;
their partial timings are diagnostic only. The second copy checkpoints each
navigation's GPU/visibility data before that later failure. The uninstrumented
r2 suite retains its successful typing/model assertions and failed latency
assertion separately. The final policy extends the quiet interval from 500 ms
to 1.5 s to avoid competing refinements during short pauses between editors.
Its added four-renderer regression and the complete focused suite pass 99/99,
with TypeScript checking passing in `interaction-raster-owner-r3.log`.
Original latency limits, input sequences and static pixel assertions remain
unchanged; r3 browser validation and integrated qualification are pending.

The uninstrumented r3 run passes four-editor navigation (416.4 ms), Gridfinity
(201.4 ms), warm circle dragging and text acknowledgement, but cold circle
dragging (583.5 ms) and first manifold navigation (521.8 ms) remain over budget.
No unchanged retry or new nomination follows from those partial passes.

Two additional presentation regressions fail on that implementation:
`point-begin-paint-before.log` proves the controller paints the unchanged native
point-begin pose before the first queued movement, and `hover-paint-before.log`
proves idle hover immediately schedules a full draw. The controller now awaits
and validates native point setup, preserving every movement and terminal sample,
then paints the first advanced pose. Idle Select hover may coalesce paint for
at most 32 ms; repeated hover does not extend the window, and a changed navigation,
editing or source frame replaces it immediately. The viewport carries this hint
through both direct canvas replies and delayed React commits. Picking, native
input evaluation and server replay are unchanged. Pending/settled diagnostics
include deferred hover; resize, loss and disposal retire that work.

All 118 focused renderer/Pixi/viewport/controller tests and TypeScript checking
pass in `interaction-raster-owner-r4.log`. The exact isolated r4 bundle retains
the previously prepared native modules. Final pixel checks and integrated
qualification remain pending.


The r4 original performance run passes only Gridfinity. Cold circle preview
is 635.8 ms (warm 158.1/83.4 ms, zero reversals). Manifold navigation improves
to 437.7 ms, but its text acknowledgement fails at 677.0 ms. Four-editor
navigation samples remain below 500 ms, then the workflow times out before
its text acknowledgement. These results remain unqualified in
`interaction-raster-performance-r4.log`.

Two isolated diagnostic copies preserve the original circle assertions and add
worker/submission timestamps. `cold-worker-trace-r4.json` measures native first
prediction ready at 217.6 ms after initial hover, while first moved paint occurs
at 580.7 ms (535.7 ms event-to-paint, fail). `cold-gpu-trace-r4.json` measures
322.7 ms for the first actual hover/blur draw, then 94.8 ms and approximately
26 ms for later draws at the same native DPR. That instrumented run happens to
pass at 452.1 ms; it is diagnostic evidence, not a replacement qualification.
The first hover uses eight blur passes on a small 25 × 24 target. Existing
startup `shader.bind` preparation does not eliminate this first actual pipeline
cost. Offscreen pipeline preparation before the first native scene is being
investigated, retaining one final GPU fence/error validation and truthful
completed native pixels. Static review of r4 finds no blocking correctness
regression; `render-review-current-result.md` records its scope and minor
identical-frame hover-priority observation. No live preview has been upgraded.

The separate four-editor diagnostic `four-click-trace-r4b.log` completes all
original navigation, typing, held-solve and reload assertions (472.1 ms navigation,
124.2 ms text). Additional step markers and a bounded click timeout did not
reproduce the prior workflow stall; no production or qualification-harness
change is inferred from that pass. An earlier diagnostic copy failed import
resolution before opening a browser; both outputs are retained.

The startup follow-up renders the existing strength-3/quality-4 blur through a
private 64 × 64 antialiased target before the first native scene, and repeats
only after context restoration. Program preparation remains explicit. Temporary
graphics (including their owned contexts), target and texture source are cleaned
up on success or failure; the final native scene fence/error validation covers
all submitted work. No warm-up pixels or synthetic frame are published as native
presentation. Allocation/submission/context-loss failures remain unavailable
until restoration. Five resource-owner regressions cover this path; the original
four fail before implementation. `npm test -- --no-cache
src/lib/canvas-renderer-pixi.test.ts` passes all 28 tests and
`npm run check:types`/`git diff --check` pass. Full command details and before/after
outcomes are in `target/m98/coordination/corner-drag/gpu-warm-result.md`.
The isolated r5 build and actual-browser checks remain pending.


R5 passes the original circle (416.6 ms cold, 100.9/109.5 ms warm, no reversals),
manifold (346.3 ms navigation, 456.0 ms text) and Gridfinity (239.6/343.0 ms)
performance workflows. Four-editor navigation still fails one sample at
532.2 ms; text 134.8 ms and all functional assertions pass. The same isolated
artifact passes all four real canvas pixel/DPR/layout/context/capture workflows
in 1.4 minutes (`interaction-raster-pixels-r5.log` and its evidence directory).
These focused results do not qualify the release.

Further cross-tab traces establish a narrower scheduling defect. The first r5
trace passes all original four-editor assertions (diagnostic only); the second
checkpoints all navigation evidence then times out later in the workflow.
In `four-frames-trace-r5.json`, wheel input arrives at 0 ms, an unsubmitted
old-camera hover draw starts at 11.2 ms, native wheel sends/replies at
12.6/16.3 ms, and the old camera still paints at 152.2 ms before the zoom draw
completes at 260.1 ms. All those surfaces are already at display DPR. This is
redundant pending hover work, independent of static refinement.

Five focused assertions fail in `wheel-hover-before-r6.log`. The viewport now
marks wheel input as superseding hover as soon as its DOM event arrives. The
renderer retains but suppresses only unsubmitted hover (timer, queued RAF or
late reply) until an immediate native response or input-queue completion.
A no-change/clamped response releases the same retained scene; queue completion
also releases it after a null/error response. Immediate scene/surface work and
already-submitted GPU draws remain intact. Native pointer/zoom evaluation and
ordered wheel anchors are unchanged; the ordinary 32 ms hover window and
1.5-second static refinement policy remain unchanged. All 82 renderer/viewport
focused tests and TypeScript checking pass in `wheel-hover-after-r6.log`.
Final r6 browser validation and integrated nomination remain pending.


R6 original performance passes four-editor navigation (336.2 ms, text204.7 ms),
manifold (350.3/478.7 ms) and Gridfinity (217.7/290.9 ms). Cold circle dragging
fails at668.0 ms while warm99.0/116.9 ms and zero reversals pass. All original
limits remain unchanged. A separate cold GPU diagnostic (`cold-gpu-trace-r6.json`)
passes at399.7 ms but still measures309.5 ms in the first hover draw, followed
by44.5 ms and approximately26 ms. The framebuffer was already display DPR.
Private offscreen preparation does not remove that first canvas composition cost.

An added owning assertion fails in `canvas-pipeline-before-r7.log`: preparation
never exercises blur output on the actual canvas framebuffer. Startup now covers
both the private filter target and final canvas composition, whose framebuffer
format/sample count can require a different driver pipeline. The initial1×1
canvas intersects a small preparatory shape; the final native scene clears the
canvas in the same submission. Only the final native fence/error validation
publishes readiness/frame evidence. Thus R7 supersedes R5's strictly-offscreen
warm-up wording: temporary canvas preparation is cleared, never published as a
native completed frame. Both preparation-target failure paths retain cleanup
and restoration-only retry. Identical-frame priority promotion remains scoped
to hover actually suppressed by wheel input. All130 renderer/Pixi/viewport/
controller tests and TypeScript checking pass in `canvas-pipeline-after-r7.log`.
R7 isolated build and actual-browser validation are pending; no new integrated
gate or live-service replacement has run.


R7 original performance completes 2/4 workflows: circle 389.9 ms cold and
86.0/83.9 ms warm with zero reversals, and Gridfinity 225.4 ms navigation and
337.3 ms text pass. All four-editor and manifold navigation samples complete
within 500 ms, but both workflows time out at 180 seconds before a text acknowledgement. Their complete
runs fail; partial navigation measurements do not replace the failed workflow.
An action-level four-editor trace with early exception/cleanup markers is now
prepared to locate this intermittent stall, which predates R7. No integrated
nomination follows from these partial results. The pixel witness additionally
uses its already-captured bounds when sampling pixels, removing a post-capture
layout reread without altering any pixel assertion; final browser validation
of this helper was subsequently completed below.

### R7 final focused review and pixel evidence

Independent read-only review finds no blocking correctness issue in the twelve
F042 files (`review-f042-result.md`). The original browser source, actions and
assertions also pass all four selected performance workflows with `DEBUG=pw:api`
in `original-debug-r7.log` (165.3 s): circle 440.4 ms cold and 118.6/100 ms warm,
zero reversals; four-editor navigation/text 359.5/140.8 ms; manifold
421.4/397.9 ms; Gridfinity 232.5/315.6 ms. These diagnostic passes do not explain
the prior intermittent cancellations. Read-only harness review notes that the
original click timeout is 30 seconds; an earlier body error followed by stalled
cleanup can also produce the outer 180-second cancellation. No cause or repair
is inferred from that observation gap (`stall-review-result.md`).

The final captured-bounds pixel helper and R7 artifact pass all four existing
canvas workflows in 1.4 min, using the pinned Nix shell and Chrome:
`python3 target/m98/coordination/corner-drag/measure-pixels-r7.py` invokes
`npx playwright test tests/e2e/canvas-renderer.spec.ts --workers=1 --reporter=list,json`
with the R7 harness manifest, port 18126 and fresh report/output paths.
Active/static point pixels, exact pan displacement and unchanged source, DPR 2
resize and picking, hidden layouts, genuine context restoration including
first-use batch/blur losses, and lost-capture recovery all pass. The JSON report
retains all passing witnesses and three actual restored-canvas PNG attachments;
copies are under `interaction-raster-pixels-r7-attachments`. Restored line,
point and dimension text pixels were also visually inspected. All logs here
are under `target/m98/coordination/corner-drag/`.

The subsequent untraced original run (`original-final-r7.log`) passes circle and
four-editor workflows, but manifold navigation totals about 7.6 seconds and
typing starts just 40.9 ms before the held solve resumes. It fails the retained
`released === 0` assertion; its 794.2 ms text ACK overlaps active solving and
does not independently establish a text-owner regression. Gridfinity navigation
completes, then its workflow reaches the outer timeout before ACK. Both failures
remain unresolved by the passing diagnostic runs.

A temporary diagnostic copy retains all actions/assertions and adds early body
errors, cleanup markers and late shutdown statistics. Its run
(`flow-observed-r7.log`) completes all actions and cleanup without a stall. Circle
599.9 ms cold and four-editor 601.5 ms navigation fail their unchanged budgets;
later manifold 275.7/384.2 ms and Gridfinity 149.0/228.7 ms navigation/text pass.
The accompanying `/proc` observations (`flow-observed-r7-host.jsonl`) establish
competing Rust compilation in another repository consuming approximately 5–10
cores during the early cases, with CPU scheduling pressure at 20–35 percent.
That compilation has ended by the later cases. These loaded measurements do
not justify another speculative product change or qualify the candidate. The
independent text-path review (`text-latency-review-result.md`) also separates
the expired solve hold from an isolated text-owner measurement.

No diagnostic changed the production artifact or existing latency/semantic
assertions. Clean-source integrated qualification must still establish the
complete original workflows; every live preview upgrade remains pending.

### F042 integrated nomination and screenshot scrolling

Clean `c7d4d03221a8563efc2667b2b8a3359eaee74de5` passes preflight in
`20260912T233433-3b1ff566`. Integrated run `20260912T233651-9fd8a6d4`
retains 290 passing obligations, then fails the ordinary browser stage:
48/49 workflows pass, with no skips or retries. Collaboration browser and final
performance do not execute, so this run does not qualify a replacement product.

The manifold's initial pixel witness fails its strict before/after capture
comparison. The complete frame, completed surface and raster quality remain
identical; only canvas page coordinates change from y = -17 to y = 0.
Playwright's ordinary locator screenshot scrolls its target into view after
the helper has frozen those bounds. The retained error and trace are under
`target/release-gate/runs/20260912T233651-9fd8a6d4/stages/browser/scratch/browser/full/`.
This is a capture-harness ordering error, with no demonstrated native scene or
renderer failure.

The helper now completes `scrollIntoViewIfNeeded()` before waiting for settled
quality and freezing frame, surface and bounds. The complete before/after
comparison, captured-bound coordinate sampling and every pixel assertion remain
unchanged. Focused verification uses the exact failed-run harness artifact and
the original full manifold workflow plus all four canvas workflows; no product
rebuild or solver change is needed for this correction. All **5/5 pass in
5.5 min**, including the full 4.2-minute manifold edit/history/reload workflow.
The pinned Nix command is
`python3 target/m98/coordination/corner-drag/measure-capture-scroll.py`; it invokes
Playwright on `m92-sample-audit.spec.ts` and `canvas-renderer.spec.ts`, selecting
`pc-water-manifold|M94 canvas`, with one worker and list/JSON reports. Evidence:
`target/m98/coordination/corner-drag/capture-scroll-after{.log,.json,/}`.
`git diff --check` passes. Qualification and preserved preview delivery remain
pending; the runner must authenticate any reused unaffected successes.

### Collaboration ACK completion witness repair

Clean `e6070c968f1fbfd38f88b802a9280709ea3e40a8` reaches 291 passing obligations
in `20260913T001723-3bb4f88c`: ordinary browser **49/49 passes**; collaboration
browser **15/16 passes**, with the manifold cancelled at 180 seconds. Final
performance does not run. No replacement is qualified or delivered by that run.

The R8 full-order diagnostic independently locates both a four-editor cancellation
and a manifold timeout at `Response.finished()`, after keyboard insertion and
HTTP 200 headers. Cleanup completes promptly. A minimal real-Chromium HTTP fixture
using the actual collaboration client reproduces the same indefinite wait on its
second text request: `writeText()` returns the complete parsed ACK, then Playwright
reports `requestfailed: net::ERR_ABORTED`. Its `Response.finished()` promise remains
unsettled until page closure. This is a completion-observer failure, not a solver
or source-editor stall. Evidence under `target/m98/coordination/corner-drag/`:
`suite-stall-r8c.log`, `text-http-r9c.log` and `typing-stall-review-result.md`.

Skipping cancellation of a completed reader does not fix the reproduction
(`text-http-r9d.log`). The isolated 30-request-per-mode matrix also reproduces
post-body aborts with a plain streaming `fetch` reader and no GeoSolve client:
client 3/30, reader 2/30, retained response 1/30. All application reads and parsed
ACKs succeed; these counts describe this diagnostic, not a frequency guarantee.
Other modes show no abort in that bounded run (`text-http-matrix-r9a.log`).
Instrumented original suites R9 and R10 each pass 14/14; neither passing run is
used to dismiss the independently reproduced failure. No production cancellation,
fetch, timeout, solver, protocol or canvas code changes for this harness repair.

`collaboration-browser-ack.mjs` observes the final outbox write per key only after
the IndexedDB transaction commits. Matched HTTP 200 plus the same text request
being present then absent in later committed snapshots proves that the ordinary
client drained and parsed its response and durably removed its pending request.
The observer binds the actual endpoint, current tab, outbox key, document/epoch
and user; it retains bounded IDs rather than source bytes. It neither changes
writes nor replaces the application's response stream. Both latency workflows
wait at most three seconds for this stronger application acknowledgement. Their
original keyboard actions, 500 ms budgets, source/peer checks, accepted revision
and ten-second solve-hold assertions remain unchanged.

The added browser regression uses the real packaged collaboration client and real
HTTP/IndexedDB. Complete ACKs pass; held, truncated and malformed HTTP-200 bodies
cannot acknowledge; an aborted removal transaction cannot acknowledge. A transient
removal overwritten within the same transaction also cannot acknowledge. Releasing
the held body then produces a valid witness. Pinned command:

```bash
node --test --test-concurrency=1 --test-name-pattern="shared text ACK witness" scripts/collaboration-browser.test.mjs
```

It passes **1/1**, with five controls, in 3.0 seconds (`ack-witness-r11a.log`).
The amended complete original suite and integrated qualification remain pending.

The first amended full suite (`suite-stall-r11a.log`) completes **14/15** with no
cancellation. Only manifold ACK latency fails: **502.85 ms**, above its unchanged
500 ms limit; navigation is 296.22 ms p95. Four-editor navigation/ACK is
246.75/168.58 ms. The observer initially retrieved a remote JSHandle's value and
then disposed it, adding two separate browser calls to the measured path. It now
returns its complete witness in one bounded browser evaluation, with both browser
and Node deadlines. The measured start/end and all correctness predicates remain
unchanged; there is no timing subtraction or relaxed budget. The five-control
regression passes again in 3.0 seconds (`ack-witness-r11b.log`).

R11b completes 14/15 without cancellation, but its manifold fails the still-held
solve assertion; the ten-second deadline already expired. No cause is inferred
from that missing timing data, and failure diagnostics now retain the job and
navigation timeline before asserting. The subsequent R12 timing diagnostic
(`suite-stall-r12a.log`) completes 3/4 selected tests. Its manifold reaches all
concurrency/source assertions, then fails the observer-inclusive ACK measurement:
545.43 ms. The actual committed browser timestamp is **402.00 ms** after the
pre-keyboard browser marker; response headers arrive at about 393 ms. Thus even
one browser evaluation can return the observation well after the event it records.
Passive host pressure is retained in `r12a-host.jsonl`; no unrelated job was stopped.

The final ACK measurement uses two timestamps from the same browser clock: the
marker before keyboard dispatch and the observed native outbox transaction commit.
The marker is earlier than the prior Node-side start, and durable removal follows
complete response drain/parse. This retains the 500 ms requirement at the actual
application boundary without adding measurement-extraction delay. The original
Node elapsed interval remains explicit `textObservationMs` telemetry. Both raw
clock values and the complete identity witness are retained; queued time must
follow this typing marker and acknowledged time must follow the queued write.
Navigation budgets, the ten-second hold assertion, input actions, source/peer
checks and accepted-state checks remain unchanged. This is a browser measurement
correction, not a claim that the expired R11b hold passed.

R12b (`suite-stall-r12b.log`) completes 3/4 selected tests with the corrected
measurement. Four-editor ACK/navigation pass at 106.0/289.4 ms; manifold navigation
passes at 278.4 ms, but its **actual committed ACK is 568.5 ms**, still above the
unchanged budget. This is an independently measured text-path cost, not observer
latency. Replacement qualification remains blocked on reducing that cost.

## M98-F043 — Dense typing eagerly encodes unchanged character ownership

**Current disposition, 2026-09-13:** repaired, qualified and delivered in `b49e339`.
The focused/pending checkpoints below are historical. See [final evidence](M98_QUALIFICATION.md#qualified-uat-repairs).

R12b independently reproduces a 568.5 ms actual durable manifold text ACK while
its model edit remains held and navigation passes. The Rust text-history owner
constructs cursor bytes for every visible character before and after each edit,
then retains only the changed span and its anchors. For the manifold's
27,353-scalar file, a short prepended comment therefore converts over 54,000
operation IDs through formatted text, cursor parsing and byte allocation.

An isolated actual-WASM benchmark uses five fresh identical five-file manifold
bases, 32-byte session actors, the same Unicode comment and genuine authenticated
`stageUserTextChanges`. Before repair, native server staging takes 119–178 ms;
checkpoint serialization is only 0.15–0.65 ms, so the proposed checkpoint-cache
optimization is rejected. The benchmark retains source sizes, every component
measurement and exact client/source-stage SHA-256 identities in
`target/m98/coordination/corner-drag/text-owner-before-r13c.{json,log}`.
Its initial wrong-operation-shape harness error and the earlier short-actor
measurement remain separate records, not production findings.

The focused Rust owner measures cursor conversion at 511 ms and span derivation
at 43 ms in the unoptimized debug build (`text-witness-before-r13.log`). The
repair compares public native operation IDs first, then runs the unchanged public
cursor encoder only for changed characters and retained anchors. It introduces
no cache, custom cursor codec, public API, wire/schema, solver equation, tolerance,
branch, source-authority or golden change. Imported multiscalar/nonstandard
operations retain the original complete cursor path and limits.

Two new owner regressions freeze the exact complete span witnesses, including
Unicode, same-value foreign ownership, disjoint concurrent edits, four actor
lengths, reverse comparisons, cold restoration and imported multiscalar fallback.
The existing full cursor path remains an independent exact oracle. In the focused
run its reference conversion/derivation is 566/47 ms and the new complete path
is 94 ms; all five atom/span tests pass. These are debug-owner measurements,
not browser latency claims.

Pinned commands completed:

```bash
cargo test --locked -p geosolve-collaboration --lib dense_typing_span_witnesses_retain_exact_ownership_and_unicode -- --nocapture
cargo fmt --all
cargo test --locked -p geosolve-collaboration --lib atom_iteration_tests -- --nocapture
cargo test --locked -p geosolve-collaboration
cargo clippy --locked -p geosolve-collaboration --all-targets --all-features -- -D warnings
```

All pass: one baseline owner characterization, five focused atom/span tests,
92 complete collaboration tests and warnings-denied Clippy. Logs:
`text-witness-{before,after}-r13.log`, `text-native-after-r13.log`,
`text-clippy-r13.log`. WASM parity/performance, complete replacement qualification
and preserved preview delivery remain pending. Human U02 is still Fail pending
recheck, and M98 is open.

`node packages/geosolve-collaboration/scripts/build-wasm.mjs` passes. Repeating
the exact five-base benchmark against that new WASM preserves both the complete
client checkpoint and source-stage SHA-256 in every trial. Median server staging
falls from **140.86 to 95.32 ms (32.3%)**; individual new trials are
156.80/116.82/90.96/91.57/95.32 ms. This is isolated owner evidence with explicit
cold/warm variation, not a browser guarantee. Evidence:
`text-owner-after-r13.{json,log}` and `text-owner-parity-r13.json`.
`node --test --test-concurrency=1 packages/geosolve-collaboration/test/*.test.mjs`
also passes **44/44** in 9.1 seconds (`text-wasm-tests-r13.log`), including genuine
native-WASM personal-history restoration, Unicode concurrency, 520 edits across
the retained Undo horizon, bounded transport and durable source publication.

Focused original browser workflows now pass **4/4** in
`suite-stall-r13a.log`: four-editor ACK/navigation is **108.8/226.9 ms**,
manifold **372.1/320.1 ms**, and Gridfinity **216.2/167.3 ms**. Navigation is
p95, all ACKs are genuine committed client events, and all unchanged 500 ms and
ten-second concurrency assertions pass. The fourth case proves all five ACK
failure controls. The measured observer intervals remain visible separately
(122.5/514.1/300.2 ms), rather than being confused with the committed events.
Pinned command: `python3 target/m98/coordination/corner-drag/measure-ack-focused-r12.py r13a`.
It selects the original four-editor/dense/ACK-control tests, using the newly
built server WASM and the unchanged earlier browser artifact. The integrated
runner must still qualify and freeze the complete final artifact; this focused
result is not a preview-delivery claim.


## R13 final qualification and installed corner witness

Clean `b49e339e4fbeeebf8b0d513de109875983f2cb58` passes all 293 obligations in
`20260913T020232-eaefaaaf` (44 fresh, 249 authenticated reused; 42m41.084s).
All 49 ordinary and 17 collaboration browser workflows pass; performance passes in
222.7s and the 271-case golden is unchanged. Integrated four-editor/manifold/Gridfinity
ACKs are 88.6/314.6/200.1 ms and navigation p95 is 273.7/296.8/143.0 ms. Cold/warm
drag preview p95 is 351.1/83.3/83.1 ms, with zero reversals. Original budgets remain intact.

The subsequent ignored installed cold-corner helper initially failed both nominal-coordinate
assertions: Chromium rounded the requested fractional pointer position, producing the actual
terminal `[-29.999997128420954, 4.999997538646531]`; the accepted position differs only in
the last floating-point digits. `browser-after-r13.log` and
`browser-after-diagnostic-r13.log` retain those failures and the observed native gesture. The first DOM
witness attempt listened on the canvas, while the viewport parent owns pointer capture;
`browser-after-input-witness-r13.log` retains that helper failure. The corrected observer
listens during window capture and independently projects the actual DOM terminal through
the unchanged accepted scene scale. It requires input within 0.001 CSS pixel of the requested
target, unchanged canvas bounds, native sample agreement within 1e-9 and accepted movement
within the original 1e-6 model-space limit. No production code, release artifact or gate
assertion changed, so the integrated gate was not rerun for this isolated helper repair.

`browser-after-capture-witness-r13.log` passes both installed client/server prediction cases
in 53.689s: cold release, stationary unrelated points, unchanged source, peer visibility,
personal Undo/Redo and exact reload. `browser-after-{client,server}.{json,png}` retain the
observed DOM events, native gesture and accepted point; the client image was visually reviewed.
The original fifteen exact native gestures remain their separate unrounded owning oracle.

The four shared services, both folder services, generator website and launcher now serve the
qualified replacement. Fresh preservation checks retain all current human work; exact
HTTP/MIME and real-browser readiness pass. The old pristine profile export stays labelled
as `b005b9e` evidence. The final report records every command and receipt. U02 remains
**Fail pending human recheck** and M98 is open.
