<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 Linux workspace publication foundation

`scripts/workspace-storage.mjs` owns filesystem publication only. It does not compile,
evaluate geometry, grant editor leases or install accepted UI state. The root integration
must bind those authorities before invoking it.

```js
import { acquireWorkspaceLock, createWorkspaceStorage } from "./workspace-storage.mjs";

const lock = acquireWorkspaceLock(projectFolder);
try {
  const storage = createWorkspaceStorage(projectFolder, { lock, historyLimit: 32 });
  const recovery = storage.reconcile(); // startup: present unresolved records
  const result = storage.publish({
    operationId: "client-epoch-command-42", // caller-generated stable retry identity
    expectedInputs: [{ path: "patches/channel.ts", expectedHash: patchHash }],
    files: [
      { path: "sketch.ts", expectedHash: installedSourceHash, bytes: candidateText },
      { path: ".geosolve/design.json", expectedHash: oldSidecarHash, bytes: sidecar },
    ],
  });
  // Require result.state === "published" or "acknowledged". A retry of an
  // interrupted/conflicted operation returns its recorded outcome, not a new write.
  storage.acknowledge(result.operationId);
  // Deliver the durable acknowledgment; lost responses are answered by outcome(id).
} finally {
  lock.release();
}
```

All methods are synchronous. `bytes` accepts string, Uint8Array, or `null` for deletion.
Every expected hash is an explicit SHA-256 or `null` for absence. Parent directories must
already exist and be local non-symlink directories. Up to 256 files, each at most 16 MiB
and totaling at most 64 MiB of candidate bytes, may participate. `expectedInputs` checks unchanged imported dependencies as well as
edited files; it does not discover the project graph for the caller.

`read(path)` returns `{bytes, hash, size, mode, inode}` or null. `inspect(id)` / `outcome(id)`
return the journal and current/retained hashes with inspectable paths. `list()` retains
malformed records as `unreadable` diagnostics. No corruption causes competing bytes to
be silently discarded. Inspection currently requires the bridge lock; a live CLI should
ask its bridge to inspect rather than acquiring a second owner.

`resolve({operationId, expectedHashes, action: "keep-current"})` records explicit acceptance
of the current disk choices. To restore a staged candidate or displaced original, read
the chosen inspection path and submit a **new** revision-checked publication. The original
operation and its alternatives remain available. A conflict error carries `operationId`
and a `recovery` inspection. Do not treat a recorded partial publication as accepted geometry.

The lock uses Linux's util-linux `flock` executable with an inherited descriptor. The Node
process retains the open file description and kernel lock after the helper exits. There is
no resident helper and no timer-based stale lock stealing. Same-process aliases are refused;
process death releases the kernel lock. Advisory owner JSON records boot ID, PID and process
start ticks, so PID reuse does not resurrect a stale owner. Never delete `bridge.lock` while
running; its stable inode owns exclusivity. No other process is signaled.

Each operation has a durable manifest under `.geosolve/operations/ID/`, independent candidate
and initial-byte copies, and the original displaced inode. Candidates are published through
exclusive links from a separate publication inode, so an external in-place save cannot
destroy the intended candidate copy. File and directory syncs surround staging, displacement,
publication and acknowledgment. Crash recovery fills only missing live paths with exclusive
links to retained originals, infers a fully published result when all recorded identities
match, and preserves partial/newer live files for explicit resolution. This is recoverable
multi-file publication, **not** an atomic multi-file transaction or atomic CAS against
uncooperative editors retaining writable file descriptors.

`prune()` bounds acknowledged/resolved payload history to `historyLimit` when old retained
bytes still match and there are no observed same-user writable descriptors. Unresolved,
modified or open-descriptor records remain. The safe default also retains data if this
host restricts same-user `/proc` inspection; this workstation does restrict some GUI
processes. Explicit cleanup may pass `{allowUnverifiedDescriptors: true}` after the user has
finished competing edits. This opts out of incomplete descriptor inspection only; observed
open descriptors and modified bytes still prevent pruning. Privileged/cross-user writers
and descriptors opened during the scan are outside that retention check. No finite
retention bound can guarantee arbitrary future writes to an old inode. Small operation-ID
receipts remain after payload pruning so a delayed retry cannot execute twice.

Focused development evidence on 2026-09-09:

- `node --test scripts/workspace-storage.test.mjs`: **28/28 pass**, no skips (4.999 s).
- `node --check scripts/workspace-storage.mjs`: pass.
- `node --check scripts/workspace-storage.test.mjs`: pass.
- `git diff --check`: pass.

The suite covers crash fault points, actual SIGKILL after displacement, external rename/in-place writes, missing paths,
multi-file partial publication, sidecar create/delete, stale input, durable idempotency,
permission retention, descriptor-aware cleanup, malformed recovery, path restrictions,
canonical aliases, real process death, stale process identities and restart during pruning. This foundation does
not by itself qualify the integrated M98 milestone.


Current folder-v2 recovery first tries authored files. An unreadable graph has no current
complete revision (`currentHash:null`), while any accepted identity remains separate.
On restart, an optional previous-source cache can supply candidate dependency bytes; the
loader rehashes them and the managed compiler/native engine independently reconstruct their
project and semantic design before presenting previous geometry. Invalid or mismatched
candidates are ignored. Source repair remains explicit and never publishes the cached files
back over rejected disk bytes. The bounded derived checkpoint does not provide authoring
or file-digest authority.
