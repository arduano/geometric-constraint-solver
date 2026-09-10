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
