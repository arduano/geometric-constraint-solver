<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 collaboration amendment

Implementation authorized after the local canvas delivery. This amendment extends M98
with a reusable Rust collaboration core and the existing workbench as its reference host.
The qualified single-editor product at `d5f9e40` remains the preview until a replacement
passes qualification. M98 supervising-user acceptance and closure remain open.

## Product contract

One server orders, validates and persists accepted model changes. Concurrent editors share
working TypeScript, while each retains independent camera, selection, Inspector and tool
state. Working source, accepted source/model and provisional client presentation are
separate states. Text synchronizes before parsing, including incomplete imports and invalid
syntax. Apply captures an immutable draft revision; later typing continues unapplied.
Canvas editing remains available while the working source is incomplete.

The reusable `geosolve-collaboration` Rust crate owns protocol and collaboration state,
with dedicated WASM bindings and a TypeScript package. Shared text uses Automerge 0.11
with explicit UTF-16 indexing on native and WASM and stable cursors. Compiler UTF-8 spans
must be converted explicitly. Automerge requires Rust 1.90; the new collaboration crates
will declare that scoped minimum rather than misrepresent the existing 1.89 declaration.

Semantic operations name stable targets, generations, operands and explicit branches.
No operation implicitly targets another editor's selection. Server admission order controls
accepted commits; each operation is prepared against the latest accepted model using the
existing compiler receipts, exact-input guards and independent residual validation. Later
valid property writes win. Deleted, ambiguous or generation-changed targets reject. A stale
deletion cannot acquire newly added dependents. The server allocates persistent IDs and
returns provisional-to-persistent mappings.

Source writeback uses localized authenticated edits and stable text anchors. An identifiable
property overlap follows server order. If incomplete source destroys patch ownership, keep
the accepted canvas edit and preserve the draft verbatim with explicit source reconciliation
pending. Unrelated editing continues. Apply rebases its captured changes over accepted
changes since its basis, validates the whole candidate and publishes atomically; unsafe
structural reconciliation or compilation failure retains draft and accepted geometry.

Undo means undo this user's latest contribution, using a checked inverse. It cannot replace
another user's newer property value or remove their new dependencies. Unavailable inverses
are reported. Redo uses the same checks. Text Undo changes working source and requires Apply;
Apply publishes a captured draft without claiming ownership of everybody else's text.

## Authority and transport

Versioned join/resume, text, semantic command, Apply, Undo/Redo, outcome, event, scene and
presence messages carry document/connection epochs and durable operation IDs. Duplicate
operations resolve their original outcome even after restart; incompatible clients reconnect
or update. A server-issued session binds host-provided user identity and editor/viewer role.
The demo supports trusted invited users; production identity providers and organizations
are outside this milestone.

HTTP commands and SSE provide the reference transport. Durable ordered document events are
separate from disposable presence. Accepted revisions cache one encoded immutable scene.
Refresh and presence coalesce; slow subscriber buffers, ingress and per-client resources
are bounded, with explicit resumable backpressure. Fast text/read paths do not wait for
compiler or solver workers. Model operations serialize in admission order without relaxing
exact prepared-input publication.

ACK follows durable persistence of the operation journal, source checkpoints, accepted
source/design snapshot and dedup outcome. Restart independently reconstructs accepted
geometry. External saves and CLI use the same gateway, diffed against the last exported
mirror. Stable file identity is separate from its path. Collaborative files are explicit
mirrors; preserve competing external bytes, journal publication and require explicit corrupt
history recovery. Existing single-editor serving remains available; collaboration is opt-in.

## Prediction and UX

Navigation remains in its current local Rust/WASM worker. A separate authoring worker predicts
construction and drag through shared Rust semantics over accepted versioned inputs, without
server publication authority. Commits carry semantic intent, relevant ordered gesture samples
and explicit branches. The server recomputes and validates every edit; client coordinates or
residual claims are never proof. A configurable server preview fallback uses the same semantics.

The workbench shows participants, remote cursors/selections, independent editing context,
synchronization state and concise overwrite/rejection feedback. Connected authoring is the
MVP: navigation works offline and pending work survives disconnection. Offline text mirrors
are a stretch; full offline semantic reconciliation is deferred if substantially harder.

## Ordered implementation

- [x] Rust document authority, operation ordering, protocol and resource limits.
- [x] Shared text, stable cursors, localized source patches and explicit Apply capture.
- [x] Authoritative semantic commits, source reconciliation and per-user Undo/Redo.
- [x] Durable restart, deduplication, external file/CLI gateway and recovery.
- [x] Dedicated WASM/TypeScript package and provisional authoring session.
- [ ] Opt-in workbench collaboration, separate client contexts and prediction worker.
- [ ] Fault, parity, browser and load qualification; verified replacement preview.
- [ ] Supervising-user acceptance and milestone closure.

The checked items have focused native, actual-WASM and runtime evidence. They do
not substitute for integrated qualification. The coherent development-r4 build
adds the server preview frontend, personal visibility, source suppression and the
dense-text performance repair. Combined checks pass 44 package tests, 63 runtime/
HTTP/domain/preview tests and eight actual-WASM frontend cases; eighteen focused
frontend cases, strict types, demo Clippy and formatting also pass. Browser
performance and fault coverage are running against its exact three WASM modules.
The qualified preview remains unchanged.

## Acceptance

Owner tests cover reorder/duplicate/lost ACK/restart, stale/forged requests, resource limits
and persistence failure. Exercise independent edits, same-value races in both orders,
simultaneous creation, delete/edit and new-dependency races, and per-user Undo/Redo.
Text tests cover text/text, text/canvas/parameter and external saves, invalid syntax, rename,
overlapping literals, Unicode/IME, Apply while typing and canvas under invalid source.
Native/WASM parity retains independent server residual/branch and accepted-authority checks.

Use four real browser editors and a measured eight-editor plus 24-viewer load case. Hold a
solve for ten seconds: navigation must issue zero RPCs and text must synchronize before
release. On the reference host, target p95 local navigation and text ACK below 500 ms;
record bandwidth, queue/memory growth and slow-client recovery rather than infer scalability
from architecture. Focused checks precede the integrated authenticated gate and served-byte
verification. No implementation checkpoint alone establishes milestone completion.

## Foundation implementation checkpoint

The first owner-tested components are on disk; the reference collaborative server and
workbench integration are still pending. No replacement preview or milestone completion
is implied by this checkpoint.

- `geosolve-collaboration::authority` admits immutable requests in server order, binds
  server-issued sessions, deduplicates by user/client/request across journal replay, and
  gives domain workers opaque exact-input tickets. Rejected edits preserve the accepted
  input revision. Uncertain persistence disables further authoring until recovery.
- `journal` is the native append/sync reference. It preserves corrupt/partial bytes and
  requires explicit recovery. `targets` owns monotonic object lifetimes, tombstones,
  bounded restoration and exact dependent-deletion closures. These are components for
  the host transaction; the reference server has not yet composed them into publication.
- `text` synchronizes raw invalid TypeScript with Automerge, explicit UTF-16 indices,
  immutable captures, authenticated incoming actors, stable file identities through
  rename, bounded sync and checked contribution inverses. Dedicated WASM and TypeScript
  wrappers run the same implementation in Node/browser workers.
- The managed compiler's `applyManagedSketchSourceMutation` returns localized preserved
  source and a separate SHA-256/UTF-16 patch. The original three-field Rust compiler
  receipt remains unchanged. Rust `source_patch` checks the same raw patch format and
  uses text-owned character anchors plus complete compiler-owned lexical regions.
  Unrelated invalid code survives; changed ownership returns reconciliation pending.
- `EditableSession` exposes exact managed mutation/captured-source preparation and
  publication, latest-source value batches and checked inverse data. A separate
  `AuthoringPrediction` exposes provisional DTOs without server publication handles.
  It is a semantic prototype; per-frame drag still needs the retained coordinator path.

No equation, solver tolerance, hard/soft priority or implicit branch behavior changes.
Lifecycle/property admission and independent model validation remain mandatory above
raw text convergence. In particular, equal-value writes need contribution ownership
checks beyond value CAS to make per-user Undo safe.

Focused commands run in the pinned Nix shell:

```bash
nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'COMMAND'
```

Passing Rust checks include `cargo fmt --all -- --check`,
`cargo test --locked -p geosolve-collaboration -p geosolve-collaboration-wasm` and
`cargo clippy --locked -p geosolve-collaboration -p geosolve-collaboration-wasm --all-targets -- -D warnings`.
The final owner run covers 41 native cases (12 authority, two journal, 16 text, five
source-patch and six target-lifetime cases), with strict Clippy across both new crates.
Separate engine `managed_authoring` and `editable_session` checks pass seven cases with
strict scoped engine Clippy.

The managed package build/runtime/fixture/bundled-sample/type/managed checks pass 62 runtime
cases with unchanged fixtures; the final allocation guard additionally passes all 24
managed owner cases. The collaboration package builds real WASM and passes seven package
cases, including native-to-WASM-to-native checkpoint exchange and large Unicode paste. Logs remain under
`target/m98/collaboration-*`, `root-collaboration-*` and `localized-source-*`.
These focused checks are development evidence; the integrated gate still needs registration
and complete qualification after server/workbench integration.

Outstanding behavior includes semantic reconciliation of overlapping edited properties,
rebase of captured Apply, durable per-user semantic history, unified external file/CLI
admission, the HTTP/SSE host, worker-driven construction/drag, collaborative CodeMirror and
participant UI, and the four-browser/32-client load gates. The current text wrapper's Undo
stack is bounded and instance-local; it excludes file lifecycle and clears on restart.
That limitation is not the final approved durable collaboration history contract.

Final foundation evidence is `target/m98/collaboration-owner-final-checks-r2.log`; the
locked release WASM build and seven actual-WASM cases pass against the same text owner.
Package dry-run contains nine files; no registry publication occurred. Full-history validation
currently runs on each text admission, so load targets remain unmeasured. Native inverse
tokens are runtime-local and source ownership anchors have a 64 KiB span bound.

## Durable composition and authoring checkpoint

The amendment remains in implementation. Native point replay and its actual-WASM
bindings now retain complete solver-coupled terminal geometry. Shared source has distinct
accepted and working states, durable immutable Apply captures with historical file IDs,
compiler-owned invalid-draft reconciliation and conservative three-way captured Apply
rebase. Checked property contribution history preserves same-value ownership and personal
Undo/Redo across native checkpoint restoration. These do not yet complete structural Undo.

The trusted WASM authority/source adapters expose staged checkpoints: committed reads
stay unchanged until asynchronous filesystem persistence completes. The Node reference
host stores content-addressed model/source/target/history bytes before one fsynced journal
envelope, and restores original operation outcomes after lost acknowledgements. Text
request IDs also deduplicate across restart; reuse with different content or across the
text/model namespaces rejects. Apply admission stores the immutable native capture and
its authenticated old accepted basis. Captured-file compilation cannot read omitted
files from a newer disk mirror.

HTTP commands and SSE now have tested invited roles, bounded concurrent admission,
reconnect cleanup, ordered replay, coalesced disposable presence and bounded subscriber
buffers. A held domain callback allows text persistence and later admission. This proves
queue separation; actual CPU offloading, browser responsiveness and load targets await
the domain worker/workbench integration. Accepted scene transport caches immutable encoded
bytes per model input. No canvas-navigation route was added.

Focused evidence, all through the pinned Nix shell documented above:

- `cargo test --locked -p geosolve-collaboration --test history --test document`:
  14 pass, including corruption refusal and exact Apply recovery. Scoped warnings-denied
  Clippy passes (`engine-authoring-adapter-clippy-final.log`).
- `npm --prefix packages/geosolve-sketch-code run test:runtime`: 83 pass; build,
  fixtures, bundled samples, types and managed checks pass unchanged
  (`captured-apply-rebase-final.log`).
- `cargo test --locked -p geosolve-sketch-engine --test point_gesture --test managed_authoring --test editable_session`:
  17 pass; scoped strict Clippy and WASM check pass (`terminal-publication-final.log`).
- `cargo test --locked -p geosolve-sketch-engine-wasm --test point_gesture --test authoring`:
  seven pass. `node packages/geosolve-engine/scripts/build-wasm.mjs` and
  `node --test packages/geosolve-engine/test/*.test.mjs`: release WASM and 13 package
  cases pass (`point-gesture-actual-wasm.log`).
- Collaboration source/authority adapter checks: nine native adapter cases and 17
  actual-WASM package cases pass with release WASM, strict Clippy and TypeScript builds
  (`source-wasm-owner-final-r2.log`). Semantic ledger binding is the next adapter slice.
- `node --test scripts/collaboration-http.test.mjs scripts/collaboration-host.test.mjs scripts/collaboration-storage.test.mjs`:
  30 pass (`collaboration-text-dedup-checks.log`). These use real native authority and
  filesystem persistence with an explicit fixture domain codec, not geometry evidence.
- `node --test scripts/collaboration-source-integration.test.mjs`: two pass using
  actual native source/authority with filesystem persistence, including held model work,
  invalid source recovery and queued immutable Apply after restart.
- `node --test scripts/workspace-loader.test.mjs`: nine pass, including the complete
  captured-files boundary (`collaboration-captured-loader-checks.log`).

No equations, solver tolerances, hard/soft semantics or implicit branch choices changed.
The independent terminal parity helper preserves writable companions, rectangle semantics,
consumer detachment and computed branch/contact state. Stale gesture bases currently
reject. The existing demo still owns its prior terminal implementation; adoption of the
shared implementation remains integration work. Native preparation and actual WASM are
component evidence, not an end-to-end visible latency claim.

Still required: actual compiler/solver workers in the reference document host, full
semantic/lifecycle history, durable text Undo, external file/CLI mirror reconciliation,
construction prediction, collaborative workbench UI, browser/load/fault qualification,
release-gate registration and integrated clean nomination. The qualified preview remains
unchanged, and supervising-user acceptance remains open.

## Runtime and collaboration client checkpoint

The actual folder runtime now composes native source, target and personal property
history with independent compiler/solver workers and fsynced authority publication.
Cold restart recompiles the captured raw files, authenticates the complete canonical
project/design and restores original operation receipts. Captured Apply, invalid raw
text, disjoint personal Undo and same-value property ownership pass actual HTTP/folder
tests. Point gestures independently replay on the server; their history records exact
changed companion properties, and inverse replay preserves independent later edits.

The reusable HTTP/SSE client persists exact pending request IDs and payloads before
transmission, retains unknown outcomes, retries original requests after reconnect and
keeps per-client text/Apply/later-text admission ordered. Committed text deltas import
into existing native replicas without replacing unsent typing. Authorization stays in
headers; bounded event decoding handles split Unicode, truncation and slow streams.
The reference runtime reserves `geosolve.server.*` for server-owned contribution barriers.

Structural personal history now has one native timeline for properties, creation,
deletion, dependency writes and statement ordering. Delete Undo allocates fresh target
generations. Create Undo refuses later other-user contributions and expanded dependency
closures. Runtime compiler descriptors and structural inverse replay still need integration.
Native/WASM retained construction covers segment, polyline, circle and aligned rectangle,
with independently replayed inference, correction and Step Back. Its browser authoring
worker and server runtime connection remain in progress.

A combined runtime check exposed an intermittent Apply/text-save overlap: disposable
source preparation ran while a native text stage was awaiting fsync. The runtime now
runs short source preparation and cleanup through `host.withCommittedState`; compilation
and solving remain outside that queue. The existing native pending-stage guard is
unchanged. A held-fsync host regression and all eight actual folder cases pass.

Focused commands used the pinned Nix shell already documented above:

- `cargo test --locked -p geosolve-collaboration -p geosolve-collaboration-wasm`:
  65 core and 20 adapter cases pass; warnings-denied Clippy and formatting pass
  (`structural-history-final-native.log`, `structural-history-final-style.log`).
- `npm --prefix packages/geosolve-collaboration run build`, `run build:wasm` and
  `test`: release WASM and 33 package cases pass (`structural-history-wasm-r1.log`).
- `node --test scripts/collaboration-domain-point.test.mjs scripts/collaboration-domain-scene.test.mjs`:
  six pass, including independently constructed local scene handles and canonical
  same-value point ownership (`collaboration-domain-point-scene-final-r2.log`). The
  added actual native tool catalog passes the focused scene case
  (`collaboration-scene-catalog-checks.log`). Original eight domain cases also pass.
- `node --test scripts/collaboration-host.test.mjs scripts/collaboration-runtime.test.mjs`:
  all nine host cases pass. The runtime rerun after correcting the reserved identity
  to the native identifier alphabet passes all eight cases
  (`collaboration-source-preparation-race-checks-r2.log`). The prior illegal `@` prefix
  was correctly rejected by native history; that validation was preserved.
- Native construction: 20 engine, two code-owner and two adapter cases pass, with
  scoped strict Clippy. The release engine WASM package passes all 17 cases
  (`construction-actual-wasm-final.log`). The code-owner regression fixes an existing
  lowering mismatch that discarded authored `regularized: true`; no equations changed.
- Frontend preparation: seven CodeMirror cases and two collaboration-adapter cases
  pass (`collaboration-adapter-checks.log`). The adapter checks use explicit transport
  fixtures and prove queue separation, not actual browser latency. The CodeMirror
  change distinguishes local edits from received source and keeps remote changes out
  of local Undo. The new adapter is not yet mounted in the workbench.

No integrated nomination or preview replacement has occurred. Durable text history,
structural compiler replay, external mirrors/CLI, the complete collaborative UI and
prediction workers, four-browser and 32-client load/fault qualification remain required.
The milestone and supervising-user acceptance remain open.

## Latest-model replay and browser integration checkpoint

The amendment remains in implementation. Trusted admission now captures immutable
historical accepted model/source/target/history checkpoints for gestures. Native
construction authenticates its original trace, independently drafts on latest state,
and permits only fresh declaration allocation with typed-reference renaming. Native
point replay authenticates the old target and uniquely resolves its current semantic
lens. The runtime checks original and latest object lifetimes, including inferred
external operands; an old command cannot borrow a deleted/restored object's token.
Accepted allocation results share the fsynced outcome and survive exact retries and
restart. Earlier exact-basis APIs remain unchanged.

The opt-in browser now connects local navigation, authoring prediction, native source
text and Inspector workers, durable fenced tab outboxes, raw keystroke recovery,
compiler-owned UTF-8 source correspondence and disposable presence. External mirror
work runs in a bounded worker; installed CLI mutations share the same authority.
The qualified preview is unchanged. Remaining work includes exposed authoring actions,
preview fallback, browser/fault qualification and integrated nomination.

Focused evidence uses the pinned Nix shell above:

- `npm --prefix packages/geosolve-collaboration run build && node --test scripts/collaboration-host.test.mjs scripts/collaboration-http.test.mjs scripts/collaboration-runtime.test.mjs scripts/collaboration-cli.test.mjs`:
  45 pass before the added latest-replay integration cases
  (`collaboration-resume-runtime-r1.log`).
- `npm --prefix packages/geosolve-collaboration test`: 43 pass
  (`collaboration-package-resume-r1.log`).
- `node --test --test-name-pattern="concurrent constructions|stale point gestures" scripts/collaboration-runtime.test.mjs`:
  both initially reject stale bases; after integration, both pass in 16.25 s
  (`latest-replay-runtime-r1.log`).
- `node --test --test-name-pattern="queued gesture resumes" scripts/collaboration-runtime.test.mjs`:
  passes in 10.37 s, retaining original queued basis through restart and refusing
  inferred references to a restored lifetime (`latest-replay-queued-r1.log`).
- Mirror worker nine cases and actual runtime external-save case pass
  (`mirror-worker-runtime-final.log`).
- The rebuilt development-r2 Vite bundle passes distribution validation: 22 files,
  three exact WASM modules (`collaboration-ui-build-r2.log`, `collaboration-dist-r2.log`).
  It is a development artifact, not nominated or served as the replacement preview.

The 32-client text case passes; the real slow TCP/SSE reader is retired within the
128 KiB subscriber bound and resumes exact text/model state while healthy clients
continue (`collaboration-browser-slow-sse-r2.log`). Four-browser navigation and dense
manifold editing exposed M98-F019/F020; their unchanged latency and authority assertions
remain required. M98-F021 adds named-parameter replay coverage. See the hardening
ledger for their individual reproduction and qualification status.

## Latest-model replay and extraction integration

Native/WASM latest construction and point replay authenticate the trusted original
accepted checkpoint, re-evaluate on the current accepted model and return exact
external lifetime requirements. The host persists the original checkpoint with the
operation and checks lifetimes again before publication. Simultaneous construction,
later point writes, disjoint parameter edits, deleted/restored operands, durable
queued restart and caller-owned allocation results pass focused runtime coverage.
Five actual-WASM replay cases pass, including named parameters whose public symbols
differ from their lexical variables.

Parameter extraction allocates monotonically on the server, retains current source
ownership and publishes through unchanged native/compiler validation. Concurrent
extraction, checked personal Undo/Redo and later foreign same-value ownership pass.
Source reconstruction applies consumer CAS and structural changes before compiling
the complete candidate. Metadata options produce identical compiler receipts
regardless of JSON key order. F021–F024 in the hardening ledger record reproductions
and repairs. No equation, tolerance, branch default or golden expectation changed.

Focused evidence (pinned Nix shell as above):
- `cargo test --locked -p geosolve-sketch-code --test m98_parameter_values`:1PASS;
  strict scoped Clippy and engine/demo release WASM builds pass.
- `node --test packages/geosolve-engine/test/latest-replay.test.mjs`:5PASS.
- `node --test --test-name-pattern="concurrent constructions|stale point gestures|queued gesture resumes" scripts/collaboration-runtime.test.mjs`:3PASS.
- `node --test --test-name-pattern="concurrent parameter extraction" scripts/collaboration-runtime.test.mjs`:1PASS.
- `node --test packages/geosolve-sketch-code/dist/test/managed-clean.test.js`:46PASS;
  `npm --prefix packages/geosolve-sketch-code run test:fixtures`:unchanged.
- `node --test scripts/collaboration-domain-structure.test.mjs scripts/collaboration-domain-properties.test.mjs scripts/collaboration-domain-extraction.test.mjs`:15PASS.

The optional [server authoring preview](M98_AUTHORING_PREVIEW.md) uses the same
retained native semantics in bounded workers. Six actual-WASM owner tests and two
HTTP/runtime cases pass; browser fallback integration remains underway.

Development-r3 contains22files/14JS/1CSS/3WASM and passes artifact validation. Actual
manifold width12→11 and Gridfinity41.5→41 edits publish and retain source/model
invariants. Their dense text ACK and manifold navigation measurements still exceed
the unchanged500ms budget. The four-editor,32-client and stalled-TCP cases passed
the preceding focused run. Native source/history performance work, remaining
workbench routes, browser fault/concurrency proof, clean integrated nomination and
replacement preview verification remain open. No supervising acceptance or milestone
closure is implied.


### Personal visibility and source suppression (implementation)

Explorer hide/show, isolate/restore and construction visibility now use the detached
Rust canvas worker. The server seed supplies accepted declaration ownership for
masking; the same filtered scene drives paint and picking. Each tab retains its
own bounded hidden-row set, isolation baseline and construction policy across
accepted-model replacement. Local Inspector projection applies that state without
changing authored source, accepted geometry authority or history. Ordinary camera
and hover updates reuse already-admitted visibility.

Suppression uses the existing native managed-source mutation, explicit current
semantic target lifetimes, server validation and durable publication. Per-user
history records suppression separately from numeric values, using compiler-resolved
reference paths for generated members; inverse application checks the exact prior
activation. Explicit same-value writes retain ownership. Suppressed native geometry
is excluded from the advertised point-drag targets by its native node state.

The current collaborative construction engine supports Segment, Polyline,
Center-radius Circle and Two-point aligned Rectangle. Their toolbar identities now
match the native catalog. Other catalog tools remain visible with an unavailable
reason. This availability projection does not qualify the remaining construction,
constraint, dimension or modify authoring routes as complete; that coverage remains
an explicit milestone limitation. Source/Inspector value edits remain available. Changing existing selected curve
roles is likewise marked unavailable; selecting the role of new geometry remains local.
