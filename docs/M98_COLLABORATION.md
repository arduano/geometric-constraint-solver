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

- [ ] Rust document authority, operation ordering, protocol and resource limits.
- [ ] Shared text, stable cursors, localized source patches and explicit Apply capture.
- [ ] Authoritative semantic commits, source reconciliation and per-user Undo/Redo.
- [ ] Durable restart, deduplication, external file/CLI gateway and recovery.
- [ ] Dedicated WASM/TypeScript package and provisional authoring session.
- [ ] Opt-in workbench collaboration, separate client contexts and prediction worker.
- [ ] Fault, parity, browser and load qualification; verified replacement preview.
- [ ] Supervising-user acceptance and milestone closure.

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
