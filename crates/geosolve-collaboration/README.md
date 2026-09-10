<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# geosolve-collaboration

Pure Rust collaboration ownership, separate from sketch equations and accepted-scene
publication. `text` owns raw files using pinned Automerge 0.11 with explicit UTF-16
indexing on native and WASM. `authority` orders durable semantic admissions;
`protocol`, `source_patch` and the native `journal` provide the reference host seams.
The host authenticates users and independently validates accepted geometry.

This crate requires Rust **1.90**, Automerge's minimum. The existing workspace's
baseline remains 1.89. GPL-3.0-or-later; no solver FFI or unsafe code is added.

Create one shared genesis with `SharedTextDocument::new`, then `fork` or `load` it
with server-issued unique actors. A raw file can contain any valid Unicode, including
incomplete or invalid TypeScript. `capture` returns an immutable file map and exact
causal heads; an Apply job captures that value while later typing continues.
`apply_edits` rejects stale offsets. Compiler UTF-8 spans must pass through explicit
conversion helpers; surrogate interiors are refused.

Files have a stable text object identity separate from their mutable path. Ordered
renames preserve cursors; removal/recreation invalidates them. Hosts serialize file
lifecycle operations. Concurrent ambiguous file objects or paths refuse admission.
`anchor_range` records exact source bytes and every original character's identity;
`replace_range` preserves unrelated edits and refuses overlaps. An anchor authenticates
characters, not a semantic declaration: source writeback also needs trusted compiler
lexical ownership, as implemented by `source_patch`.

Native changes and checkpoints share the same Rust decoder and schema validation.
The binary sync protocol retains bounded per-peer handshakes. Reset both ends after
reconnect or a dropped generated message. Server ingress uses actor-bound
`receive_sync_message_from` or `apply_changes_from`; trusted `merge`, `apply_changes`
and `receive_sync_message` are for server history and local composition. Actor metadata
is not cryptographic identity: the host binds each session to its issued actor and role.

Admission stages complete history before publication and rejects missing dependencies,
malformed/trailing messages, schema conflicts and resource bounds atomically. Defaults:
512 files, 4 MiB/file, 16 MiB visible text, 64 MiB uncompressed history/checkpoint,
100,000 changes, two million operations, 8 MiB/message and 64 peers. Changes currently
validate the full retained history and checkpoint, so this is a bounded correctness
boundary, not a demonstrated large-document latency guarantee. Hosts must isolate
untrusted decoding CPU/transient memory and rate-limit ingress. Text persistence and
ACK must be independent of compiler/solver workers.

Draft Undo uses contribution-local checked inverses, never document snapshots.
`edit_undoable` supports splice batches; `apply_inverse` returns a checked Redo token.
It preserves unrelated remote text and refuses overlapping lost ownership. Those direct
local tokens remain runtime-local. `SourceDocument::apply_user_text_changes` and
`apply_user_file_edits` add durable, host-principal personal text/file history instead.
`apply_user_text_inverse` stages Undo/Redo against current character and file ownership;
accepted geometry still requires explicit Apply. History stores authenticated historical
heads and operation provenance, deriving its inverse spans/file descriptions again during
restore. Fresh character/file identities are linked only by native checked restoration.
Same-value foreign replacements and explicit same-path renames retain ownership.

`edit_from_revision` handles existing-file typing prepared against a historical displayed
frontier: fork native heads, reject unseen same-actor writes, apply scalar-aligned UTF-16
operations, then merge remote state. It returns both the merged `snapshot` and local-only
`local_revision` for queued keystrokes before the UI installs incoming remote text.

Durable personal history retains at most 512 contributions, 4096 events and 8 MiB accounted
history, with 65,536 scalars per contiguous contribution span. Capacity advances an explicit
Undo horizon instead of rejecting valid raw typing. `user_text_history` reports the horizon's
generation, discarded event count and oldest retained revision; source checkpoints persist
its frontier. Pruning keeps a replayable recent suffix. An inverse whose original contribution
falls before the horizon moves that horizon past the inverse. One contribution too large to
retain clears older personal Undo behind a new boundary, preserving the raw edit. Native
character/file history remains intact; this does not compact Automerge or remove its admission
limits. The host journal owns operation deduplication beyond the retained history. The older
direct local token span limit remains 64 KiB.

Focused verification (inside the pinned development shell):

```sh
cargo test --locked -p geosolve-collaboration -p geosolve-collaboration-wasm
cargo clippy --locked -p geosolve-collaboration -p geosolve-collaboration-wasm --all-targets -- -D warnings
cargo build --locked -p geosolve-collaboration --example text_fixture
node packages/geosolve-collaboration/scripts/build.mjs
node packages/geosolve-collaboration/scripts/build-wasm.mjs
npm --prefix packages/geosolve-collaboration test
```

The dedicated package's tests exercise actual WASM and exchange native checkpoints in
both directions. These focused checks do not establish integrated milestone acceptance.

Authenticated text ingress refuses file lifecycle changes; the server orders those
separately. Raw same-text splices retain new character identity. Oversized (>64 KiB)
WASM splice contributions remain valid raw edits but clear instance-local Undo history.

The `authority` module supplies ordered durable request admission and opaque exact-input
worker tickets; `journal` is the native append/sync reference. `targets` preserves object
lifetimes and exact dependent-deletion intent. `source_patch` applies authenticated raw
source splices and retains unfinished drafts when lexical ownership changes. These
components do not themselves validate or publish sketch geometry. Host composition and
collaborative workbench integration are in progress.

## Semantic personal history

`ContributionHistory::record_transaction` records trusted per-object lifecycle descriptions,
properties, dependency replacements and stable-neighbor reorders in one personal timeline.
It replays the exact before/after `TargetLedger` pair and rejects missing observations.
`prepare_undo`/`prepare_redo` return opaque plans with ordinary changes and typed structural
intent. Hosts independently compile and validate before calling
`commit_inverse_transaction` on staged history/target clones and persisting both.

Deletion restoration authenticates the exact tombstone and allocates fresh generations.
Only an explicit exact-generation remapping updates internal contribution addresses and
stable structural references; old client handles never become valid. Create removal and
delete replay preserve later effective contributions, including same-value writes, and
refuse expanded dependent closures. Typed dependency and position ownership share the
ordinary property mechanism while remaining separate from compiler value edits. No solver
equations, model checkpoint Undo, or geometry acceptance are implemented in this ledger.
