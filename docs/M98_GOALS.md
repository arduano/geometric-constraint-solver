<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 local plaintext sketch prototype

Status: **PROTOTYPE_READY_FOR_UAT**; production nomination and acceptance deferred.
Implementation, commands and actual-browser evidence: [M98_HANDOFF.md](M98_HANDOFF.md).
Authorized parallel exception, pinned to `d80bf22264f74b60870f2e99feb8cc6ccb9d0133`.
M97 remains owned by the active primary checkout; its live metadata changes are absent here.

## Happy path and short plan

1. Add `init`, `serve`, and JSON `check/status` commands for a normal local folder.
2. Serve the existing demo with an opt-in remote WorkbenchAdapter. A loopback Node transport
   hosts the existing pure Rust WorkbenchHandle in WASM; all source compilation, prepared
   mutation authentication, independent validation, canvas geometry and Inspector data reuse
   existing APIs. Node owns filesystem/HTTP transport, never solver equations.
3. Exercise actual browser editing in both directions, restart, manual export, invalid source,
   rename saves and stale UI writes. Record focused evidence and leave a runnable preview.

## Authority and revisions

`sketch.ts` is durable design authority. `geosolve.json` selects the fixed entry and format.
Initial scope is one UTF-8 entry, the existing managed authoring subset, SDK imports only;
local source dependencies and custom patches are explicitly deferred. No second language.
Existing writer comment/label retention applies; arbitrary handwritten AST roundtrip is not promised.

The bridge polls entry content, including save-by-rename, and coalesces settled changes.
One serialized workbench performs each external source transaction. Accepted UI source changes
publish complete file bytes only against the browser's expected SHA-256 and a fresh disk comparison.
An exclusive link publishes the staged file after retaining the displaced inode in a plaintext
recovery file, preventing a concurrent rename from being overwritten. There is a brief missing-entry
interval; interrupted publication may require recovery from `.geosolve/before-*.ts`.
Conflict leaves disk intact and returns pending intent for explicit refresh/retry. Invalid disk
text stays intact, with last accepted geometry and clear stale/error status. A derived last-good
plaintext cache can reconstruct that view after restart; it never overrides the current entry.
No browser project/localStorage restore is authoritative in folder mode.

Camera, selection and presentation stay in the running workbench and are not geometry files.
Source-backed dimensions, authoring and source Apply are the happy path. Native point-instance
overlays (dragging) are outside the initial persisted-source scope and must be blocked clearly.
The ordinary demo and existing canonical project/source exports remain available.

## Deferred

Production release gate and milestone acceptance; M97 implementation/closure; multiple browsers;
semantic merges/CRDT; recursive dependency watching; custom patch builds; durable native drag
overlays, camera and history; account/cloud services; deployment; 3D/solid export; Pi case modelling;
new primitives, solver changes, golden expansion and a general filesystem framework.
