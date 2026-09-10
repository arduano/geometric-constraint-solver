<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# geosolve-collaboration-wasm

Thin Rust/WASM JSON and binary bindings for `geosolve-collaboration`. Native and browser
text use the same pinned Automerge implementation and UTF-16 semantics. Requires Rust
1.90 and wasm-bindgen 0.2.121. No equations, CRDT algorithm or accepted-model authority
are duplicated in JavaScript.

`SharedTextReplica` exposes raw snapshots, revision-checked edits, native checkpoints,
actor-bound binary sync, stable file/cursor/range ownership and checked draft Undo/Redo.
The adapter keeps at most 64 contributions and 4 MiB payload per Undo/Redo stack; history
is local to that instance and clears on restart/fork. Remote overlapping edits can make
an inverse unavailable. File lifecycle Undo belongs to the host. A direct WASM caller
must reject JavaScript unpaired surrogates; `@geosolve/collaboration` does this before
wasm-bindgen converts its strings.

Run text synchronization in a worker independent of compiler/solver jobs. The host owns
transport, actor authentication, durable ACK, identity provider, accepted geometry and
semantic Undo. Build via `packages/geosolve-collaboration/scripts/build-wasm.mjs`.

Authenticated text ingress refuses file lifecycle changes; the server orders those
separately. Raw same-text splices retain new character identity. Oversized (>64 KiB)
WASM splice contributions remain valid raw edits but clear instance-local Undo history.

`TrustedDocumentHost` is the separate server-only authority adapter, exposed to TS through
`@geosolve/collaboration/host`. It stages a cloned `DocumentAuthority`, captures the exact
record, and withholds committed mutation/receipt until explicit `commitStage` after host
async append+sync. One pending stage blocks mutation while committed reads continue.
`failStage` requires fresh journal reconstruction after any uncertain persistence.
Prepared tickets remain real native values behind opaque process-scoped identifiers;
completion JSON is accepted only from the trusted independently validating domain host.
Restore validates ordered input history, not solved geometry. The host must persist the
accepted source/design/model with the terminal journal event in a recoverable transaction.

`TrustedSourceHost` stages the real `SourceDocument` for independent draft text ACKs
and accepted-source publication. Its capture/preparation handles do not lock typing
while the external model worker runs. Publication must reconcile against latest working
heads; failed ownership retains working bytes with explicit pending notices. One pending
durable stage blocks further source mutation; committed snapshots remain readable.

`captureApply` returns an opaque runtime handle plus exact durable `captureJson`.
`restoreApplyCapture` reconstructs that capture only when the host supplies its exact
journal-authenticated accepted basis and historical CRDT heads independently reproduce
all files and identities. It never replaces a capture with later typing. Hosts persist
this capture with admitted Apply intent. Preparation tickets themselves are disposable;
restart re-prepares from authenticated capture and reconstructed accepted source.

The source stage carries `checkpointJson`, accepted source snapshot and working heads.
Persist it with associated source/design/model and the authority terminal record in one
recoverable transaction before committing either wrapper. Text-only stages use their
own durable envelope sequence and do not claim semantic publication. An uncertain write
poisons the source handle until disk reconstruction. Capture/preparation storage is
bounded to 32 handles and 64 MiB accounting; source envelopes to 384 MiB. Durable personal
draft history uses `stageUserTextChanges`, `stageUserFileEdits`, `stageUserUndo` and
`stageUserRedo`, with host-authenticated operation metadata and actor binding. `userHistory`
reports native availability and a durable `horizon` containing `generation`, `discardedEvents`
and `oldestRevision`. Reaching the retained history limits shortens personal Undo instead of
rejecting valid typing; an oversized contribution explicitly clears crossing Undo history.
Historical event replay authenticates retained raw Undo/Redo ownership
on source checkpoint restore; accepted geometry remains unchanged until explicit Apply.
Direct SharedTextReplica local Undo is still instance-local.
`stageUserWorkingEdits` adds trusted ordered mixed text/file batches with the same staged
durability and personal history. `SharedTextReplica.resolveRange` exposes native read-only
owned range mapping for mirror reconciliation; it does not retain a preparation ticket.

`SharedTextReplica.editFromRevision` returns `{snapshot,localRevision}`. The local revision
is the post-edit displayed branch before unseen remote merges; queued next keystrokes must
use it until the editor actually installs the merged snapshot. Unseen same-actor edits,
recreated files and invalid scalar positions reject through the native text owner.

`TrustedSemanticHost` wraps native `TargetLedger` and `ContributionHistory` together.
Initial compiler-owned names are all allocated before forward dependencies are installed.
Validated property/lifecycle updates stage one private clone against the exact accepted
revision; pending persistence leaves committed target/history reads unchanged. Stages
carry separate exact `targetsJson` and `historyJson` strings for atomic publication with
model/source and the authority terminal record. An uncertain write poisons the handle.
`userHistory(userId)` observes counts and checked availability without retaining preparation
tickets. It adds committed `revision`, `hasPendingStage` and `needsRecovery`; pending or
uncertain durability disables Undo/Redo with the corresponding reason while committed counts
remain unchanged.

Undo/Redo descriptors retain actual native `InversePlan` values behind bounded handles
(32 plans, 64 MiB serialized change accounting). Same-value writes retain ownership;
stale deletion cannot absorb new dependents; recreation always allocates a new generation.
Restore checks core state and history revision against the authenticated model revision.
The host must independently reconstruct model state and authenticate the checkpoint pair.
Opt-in `record.structural` compiler observations add create/delete/dependency/reorder
contributions to that same personal timeline. Prepared descriptors include structural
intent and private fresh generation allocations; independent model validation remains
mandatory. Stage commit installs the exact candidate target/history pair. Known pre-append
failure can `discardUnpersistedStage`; uncertain writes still poison through `failStage`.
Dependency cycles follow core `TargetLedger` semantics; this adapter adds no JavaScript graph rules.
