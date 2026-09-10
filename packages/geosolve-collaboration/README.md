<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# @geosolve/collaboration

Browser/Node bindings for GeoSolve's shared Rust source-text owner. Run this synchronous
replica in a worker independent of compilation and solving. The package has no JavaScript
CRDT, network transport or accepted-model publication authority.

```ts
import { createSharedText } from "@geosolve/collaboration";

const draft = await createSharedText({ actor: serverIssuedActor, checkpoint });
const basis = draft.capture();
draft.edit([
  { kind: "splice", path: "main.ts", start_utf16: 14, delete_utf16: 2, insert: "16" },
], basis.revision);
const capturedForApply = draft.capture(); // immutable; later typing can continue
```

The server creates one genesis; other participants load/fork its native checkpoint.
Actors are unique server-issued bytes, not display names. `generateSyncMessage(peer)`
and `receiveSyncMessage(peer, bytes)` exchange bounded Automerge messages. Server
receive uses `receiveSyncMessageFrom(peer, bytes, boundActor)` so a client cannot add
another writer's new contributions. Reset both peers with `forgetPeer` after reconnect
or a dropped generated message. Hosts persist accepted text changes before ACK without
waiting for the solver; `changesSince` provides incremental native history.

Text accepts incomplete TypeScript and Unicode/IME compositions. Every offset counts
UTF-16 code units; surrogate-pair interiors and unpaired JS surrogates are refused.
Rename uses `{ kind: "rename_file", path, new_path }` and preserves `fileId` and stable
cursors. Hosts serialize lifecycle actions. `cursor` deletion bias controls which
surviving neighbour is used after removal, not insertion affinity. `anchorRange` and
`replaceRange` retain source character ownership across unrelated edits; compiler-owned
lexical scope is still required before semantic source writeback.

`undo`/`redo` use this writer's checked contribution inverses and preserve unrelated
remote text. Overlap can make an inverse unavailable. Instance-local stacks hold at
most 64 contributions/4 MiB payload each, exclude file lifecycle, and clear on restart.
Text Undo changes the draft and requires Apply to affect accepted geometry. Captures and
returned structured values are deeply frozen. Call `dispose` when closing a replica.

Build from the repo's pinned Nix shell with `npm run build` and `npm run build:wasm`.
Bundlers can supply `wasmModule` and `wasm` bytes/URL. Node loads packaged WASM from disk;
browsers load it relative to the module. Rust 1.90 is scoped to collaboration crates;
existing workspace baseline remains 1.89. GPL-3.0-or-later.

Tests require the native parity fixture built with
`cargo build --locked -p geosolve-collaboration --example text_fixture` before `npm test`.
Set `GEOSOLVE_COLLABORATION_NATIVE_FIXTURE` if using a custom Cargo target directory.
The suite runs real WASM and checks native/WASM checkpoint exchange in both directions.

Authenticated text ingress refuses file lifecycle changes; the server orders those
separately. Raw same-text splices retain new character identity. Oversized (>64 KiB)
WASM splice contributions remain valid raw edits but clear instance-local Undo history.

## Trusted server authority

The separate `@geosolve/collaboration/host` entrypoint wraps the Rust
`DocumentAuthority`. Keep this handle in the trusted server process. Client request
handlers may submit commands; they must never invoke validated completion or assign
principal roles from an untrusted request body.

```ts
import { createDocumentAuthorityHost } from "@geosolve/collaboration/host";

const host = await createDocumentAuthorityHost({
  configuration: { documentId, documentEpoch, serverEpoch, initialInput },
  records: recoveredJournalRecords,
});
const connection = host.connect(authenticatedPrincipal, clientId, freshSessionId);
const receipt = await host.admit(commandRequest, async ({ recordJson }) => {
  await journal.appendAndSync(recordJson);
});
```

`admit` stages a private cloned core, calls the asynchronous host persistence callback,
and returns the receipt only after explicit native commit. A duplicate returns its
original durable receipt without invoking persistence again. While one write is pending,
`snapshot`, `receipt`, `resume` and `checkpoint` show committed state; mutations reject
with pending backpressure. The independent shared-text handle remains usable.

`beginNext` yields a descriptor backed by a real native prepared ticket. Pass that exact
object to `complete` only after independently evaluating the command against its exact
accepted input. For an accepted completion, the callback must durably commit associated
source/design/model snapshots with the terminal record in one recoverable transaction.
This library neither runs the solver nor establishes geometric acceptance from JSON.

Low-level `stageAdmission`, `stageValidatedCompletion`, `commitStage` and `failStage`
are available for host transaction coordinators. A stage's `recordJson` is the exact
pending journal record, not an ACK. Commit the same stage object only after persistence;
any uncertain append calls `failStage`, permanently requiring reconstruction. Restore
under a fresh process epoch from the actual disk journal, then independently rebuild
accepted geometry before serving edits. Old connections and worker tickets cannot resume
publication; original operation receipts survive restart and reconnect.

The adapter caps serialized checkpoints at 128 MiB and configured core ledger bytes at
64 MiB. It retains one pending full ledger clone during asynchronous persistence. This is
a correctness boundary; it does not establish large-ledger latency or load qualification.
Production identity, filesystem transactions and HTTP/SSE transport remain host-owned.

## Independent working and accepted source

The trusted `/host` entrypoint also exports `createTrustedSourceHost`. Construct it with
`{ configuration: { documentEpoch, serverEpoch, initialInput, files, limits? }, actor }`,
or provide its durable `checkpointJson` when restarting. The host independently validates
initial/reconstructed accepted geometry; raw working TypeScript may be incomplete.

`captureApply()` returns a runtime handle, immutable working source and accepted basis,
and exact `captureJson` for durable admission. Later typing continues. After restart,
`restoreApplyCapture(captureJson, authenticatedAcceptedBasis)` verifies the recorded
basis against the trusted journal input and reconstructs old files/identities from exact
CRDT heads. The returned new handle belongs to the restored host. Captures and preparation
tickets have bounded storage; call `release` when no longer needed.

Prepare compiler-owned canvas patches with `prepareCanvasUpdate`, or compiler-rebased
captured files with `prepareApplyUpdate`. These tickets do not block raw text work while
an external model job solves. `stageValidatedPublication` takes a genuine retained ticket,
independently validated model input and reconciliation prepared against the latest working
heads. It preserves invalid draft text with pending notices where ownership is unavailable.

`stageTextSync`/`stageTextChanges` bind incoming typing to the host-authenticated editor
actor; `receiveText`/`receiveTextChanges` await asynchronous envelope persistence before
text ACK. `stageHostEdits`/`editWorking` serve already ordered external/file edits. One
pending source write blocks mutation; committed accepted/working snapshots remain readable.
Use `textCheckpoint` to seed client CRDT replicas and reset disposable peer handshakes
on reconnect. This adapter does not establish roles or lifecycle admission order.

For accepted publication, the transaction coordinator persists the source stage's exact
checkpoint with model/design and the authority stage's terminal record before synchronously
committing both. `failStage` poisons uncertain source writes until reconstruction from disk.
Durable personal draft Undo uses `stageUserTextChanges(changes,actor,operation)` and
`stageUserFileEdits(edits,operation,expectedRevision)`. Principal and operation identity must
come from authenticated host admission. `stageUserUndo(operation)` / `stageUserRedo(operation)`
stage raw source only; persist the exact source stage before commit and require explicit
Apply for geometry. `userHistory(userId)` reports counts and current checked availability.
Native event replay reconstructs inverse ownership across restart, preserving disjoint text,
Unicode, same-value foreign ownership and stable rename identity. File restoration creates
a fresh object ID; stale typing into its old identity remains rejected. Same-path renames
are recorded as contributions, and path swaps/deletion replacements stage atomically.
`stageUserWorkingEdits(edits,operation,expectedRevision)` accepts trusted ordered mixed
splices and file lifecycle as one atomic personal contribution. CLI and external mirror
adapters must call it through the authenticated durable host gateway. The file-only API
remains strict. `SharedText.resolveRange(anchor)` returns native checked current path and
UTF-16 bounds without mutation; lost ownership or invalid cursor positions reject.

History retains at most 512 contributions, 4096 events and 8 MiB accounted history, with
65,536 scalars per contiguous changed span. Capacity shortens personal Undo to a recent
replayable suffix. `userHistory(userId).horizon` reports `{generation,discardedEvents,oldestRevision}`
and the source checkpoint persists the boundary. Valid raw typing continues; one contribution
too large for Undo clears crossing personal history behind an explicit new horizon. Retained
same-value ownership and file lineage remain checked through native restart replay. This does
not compact the underlying Automerge document; its separate admission limits still apply.
The outer host journal must deduplicate requests older than retained history.
Direct replica local Undo remains
instance-local. `editFromRevision(edits,displayedRevision)` returns `{snapshot,localRevision}`;
use localRevision for subsequent queued local offsets until installing the merged snapshot.
It clears the direct local-only Undo stack; collaborative UI must call server personal Undo.

Historical Apply capture restoration is
supported; pending model preparation tickets are re-created after restart.

## Semantic target and contribution authority

The trusted `/host` entrypoint exports `createTrustedSemanticHost` over the actual Rust
`TargetLedger` and `ContributionHistory`. Initialize with
`{ configuration: { documentEpoch, serverEpoch, objects: [{ object, dependencies }] } }`.
The compiler supplies canonical object names and dependency names; forward references
work because Rust allocates all names before installing dependencies. `current` returns
the server-allocated generation, and `planDelete`/`authenticateDelete` retain the exact
reviewed dependent closure. Never derive these identities from canvas labels.

`stageValidatedRecord` records independently validated property before/after values.
`stageValidatedTransaction` stages creation, exact deletion and dependency changes with
optional property recording in one transaction. Existing target references include their
generation; `{ kind: "created", object }` resolves only a name created by this transaction.
The caller supplies `basisRevision` and its exact successor `revision`; stale work rejects.
Same-value writes acquire contribution ownership, so Undo cannot overwrite another user's
later same-value contribution. Property keys and observations come from the trusted engine.

`prepareUndo`/`prepareRedo` return descriptive changes backed by opaque native inverse
plans. Independently validate their effect on the model, then call `stageValidatedInverse`
with the exact retained object. Intervening committed edits invalidate preparation tickets.
Use `release` for unused plans. The adapter caps retained plans at 32 and their serialized
changes at 64 MiB; target limits can be lowered below core defaults, never increased.

`userHistory(userId)` returns counts, `canUndo`/`canRedo`, unavailable reasons, committed
`revision`, `hasPendingStage` and `needsRecovery`. This read-only observation retains no
preparation handle. Pending persistence or uncertain storage disables the commands with a
specific reason while counts continue to describe committed history.

Persist a stage's exact `targetsJson` and `historyJson` with model/source and the authority
terminal record in one recoverable envelope, then call `commitStage`. The async `record`,
`transact` and `inverse` helpers await a persistence callback before committing. Reads stay
committed during a pending write; mutation rejects. `failStage` requires reconstruction.
`checkpoint()` returns `{ documentEpoch, revision, targetsJson, historyJson }`; supply that
checkpoint to the factory when restarting under a fresh server epoch. The host authenticates
this pair against its journal and independently rebuilds the model before serving edits.

Property and structural contributions share one per-user timeline and survive restart,
including checked Redo. For undoable lifecycle transactions, supply `record.structural`:
`created` observations carry `{object,payload,position}` and `deleted` observations carry
`{target,payload,position}`. Payloads are bounded trusted per-object compiler inverse
descriptors, not accepted model snapshots. Position is `{previous,next}` with stable
neighbors; created neighbors may use `{kind:"created",object}` references. Dependencies
come from the exact committed/candidate native ledgers; explicit dependency replacements
and `structural.reorders` acquire ownership even when their values do not change.

Prepared inverses include `structural` recreation/deletion/dependency/reorder intent and
an explicit old-to-fresh `restored` mapping. The host must independently compile/validate
that intent before staging. Restore allocates fresh native generations privately; existing
handles remain stale. Later effective foreign properties, dependencies or order edits block
creation removal, and deletion replay refuses changed ownership or an expanded closure.
Same-name reuse, even followed by deletion, invalidates an old tombstone restoration.

`discardUnpersistedStage` is available only for known pre-append failures. It preserves the
committed state and prepared ticket, and retries receive fresh stage IDs. Uncertain writes
still require `failStage` and recovery. Omitting `record.structural` retains the old trusted
lifecycle-only inventory route; such a route does not make lifecycle edits undoable.
This adapter does not establish solver validity, server load capacity or client responsiveness.
