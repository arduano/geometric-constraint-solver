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
