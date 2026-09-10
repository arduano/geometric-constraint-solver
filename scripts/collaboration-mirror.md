<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Explicit collaborative file mirror

`createCollaborationMirror` connects an authored folder to committed shared raw source.
The caller holds the existing exclusive workspace lock, supplies a trusted principal and
serializes text admission through its durable host journal. The mirror imports invalid
source syntax without compiling or solving. Model publication still requires Apply.

```js
const mirror = await createCollaborationMirror({
  folder, documentId, documentEpoch, clientId, userId,
  readCommitted: async () => ({
    checkpoint: source.textCheckpoint(),
    snapshot: source.snapshot(),
  }),
  admitWorkingEdits: async ({ operation, expectedRevision, edits }) => {
    // Authenticate the configured mirror principal; use the ordinary durable
    // host.writeText gateway with exact-operation deduplication.
    // It stages source.stageUserWorkingEdits(edits, operation, expectedRevision).
    // Return only after durable commit/refusal, or throw for uncertain storage.
    return { status: "committed" }; // Or {status:"rejected",reason}.
  },
});
await mirror.reconcile();
const status = await mirror.status();
```

`readCommitted` must return matching native checkpoint, `snapshot.working` and
`snapshot.fileIds`. `admitWorkingEdits` receives an exact current `expectedRevision`,
ordered mixed `TextEdit[]`, and `{userId,clientId,requestId}`. A retry must receive the
original durable result for the same ID and identical payload. This callback must never
return success before the text/source/history envelope has been durably journaled.
The optional `onPhase` fault-injection hook is for filesystem recovery tests.

`reconcile` serializes calls but starts no timer. Hosts may schedule it after source or
filesystem changes and explicitly expose it through a CLI. Status contains `status`
(`synchronized`, `reconciliation_pending`, `pending`, `uninitialized`), `generation`,
`pendingPhase`, `notices` and `manifestPath`. Pending notices include affected paths,
reasons and protected external blob references when applicable. Root UI/runtime owns
how to display these notices and when to retry.

The durable last-export checkpoint binds exact native file identities and text to the
exported disk image. A bounded scalar diff finds intentional external changes against
that image. Native `resolveRange` authenticates every changed character span and resolves
it through concurrent insertions and shared renames. Edits then enter the exact-revision
mixed gateway as one contribution. Overlapping ownership, competing lifecycle changes,
ambiguous renames or excessive batches remain pending with external bytes preserved.
Same-value shared replacements remain ownership barriers. Unique inode correspondence
identifies actual disk renames; unchanged path handles editor atomic saves; unique equal
bytes provide a conservative fallback when a disappeared file reappears elsewhere.

The manifest is stored under `.geosolve/collaboration-mirror/`. A checksum and native
checkpoint/blob hashes detect corruption; this is not a substitute for trusted storage
or authentication. Before admission, the exact operation and payload are fsynced. Before
export, a durable plan records observed and desired images. Existing external files are
moved into protected storage and verified before installing new fsynced inodes with an
exclusive hard link. Directory fsyncs protect each step. The final exported frontier is
recorded only after a complete matching disk scan. This is recoverable publication across
multiple files; readers can observe intermediate files during an export.

A lost admission ACK replays the exact request. An interrupted export resumes its stored
plan without treating partially exported bytes as a new external contribution. Bytes that
change again during publication remain pending and preserved. To resolve a text overlap,
edit external source to an explicit merged choice that passes native ownership, or choose
the current shared bytes. For an interrupted export with competing newer bytes, preserve
that version, then choose the expected/desired planned image to complete the export before
submitting further edits. Initial attachment only establishes a frontier if the folder
already matches shared source; differing initial bytes remain pending for explicit recovery.

Limits match bounded authored source: 512 files, 4 MiB/file, 16 MiB total, at most 32
directory levels and 8192 scanned entries, only `geosolve.json` and `.[cm]?[jt]s` files.
Metadata, Git, dependencies and build directories are excluded. Traversal, symlinks,
nonregular files and invalid UTF-8 reject. Mixed gateway batches contain at most 256 edits;
broader external batches need explicit subdivision. Preserved text variants cap at 256 MiB
and 8192 blobs; exhaustion requires explicit archival and cannot silently discard competing
bytes. Obsolete native checkpoint copies, completed move copies and staging files are
collected after durable frontier publication. The underlying CRDT history limits still apply.

Native processing and full filesystem scans have bounded but potentially substantial CPU
and I/O cost. Run reconciliation outside latency-sensitive presentation workers. This
reference relies on a canonical folder and the caller's exclusive workspace lock; it does
not sandbox a malicious process concurrently replacing ancestor directories. No multi-editor
throughput, disk-full chaos, network filesystem or power-loss hardware qualification is
claimed by the focused process-crash tests.
