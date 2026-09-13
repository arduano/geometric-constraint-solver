<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 local folder prototype handoff

This records the first working single-file prototype, based on `d80bf22`.
Its limitations were subsequently addressed by the
[M98 implementation plan](M98_IMPLEMENTATION_PLAN.md) and
[M99 shared-engine cleanup](M99_CLEANUP.md). For current setup, use
[Getting started](GETTING_STARTED.md) and the [CLI README](../packages/geosolve-cli/README.md).

## Delivered implementation and APIs

The initial Node bridge connected watched `sketch.ts` source to the existing Rust
workbench. External saves and GUI changes used authenticated compiler transactions,
native materialization and independent validation. A separate source revision and
content hash prevented stale browser edits from overwriting newer disk source.
Current hosting lives in `packages/geosolve-cli/runtime`; the prototype's
`scripts/file-workspace.mjs` and demo-owned execution bundle were replaced in M99.

No primitive, residual, rank, priority or branch policy changed. The browser reused
the existing canvas and Inspector; compiled geometry was never durable folder authority.

## Commands and evidence

Historical focused checks passed optimized WASM, UI build/typecheck, Rust formatting,
strict native Clippy and JavaScript syntax validation. The frontend slice passed
67/67 tests. The real Chromium workflow passed 13/13 subtests plus its parent
(14 reported tests) in 10.80 seconds.

The browser witnessed external atomic-rename radius editing from 10 to 12 mm,
Inspector writeback from 12 to 14 mm, exact plaintext and one publication without
a watcher recompile. Invalid source preserved its text and last accepted geometry.
Stale RPCs, stale browser source and active Inspector drafts refused overwrite.
Failed writes retained pending source; restart, authorization/path rejection,
manual project download and ordinary demo startup passed.

A small-sketch external-save-to-Inspector observation was 835 ms, including polling,
debounce, compilation, validation and browser update. It was not a general latency
claim. Prototype logs and captures were recorded under `target/m98/logs` and
`target/m98/tests`. The full integrated gate was outside this first prototype;
[final qualification](M98_QUALIFICATION.md) supplies later release evidence.

## Retained review preview

The original preview was a loopback-only session. Its ephemeral process and session
URL are not setup instructions. Start a fresh folder server with the current CLI
and open the exact URL it prints; restarting rotates the ordinary folder token.

## Integration seams and limitations

The original scope was one browser, one entry file and SDK imports only. It lacked
custom local patches, multi-editor collaboration and durable point-drag overlays.
Source dimensions were reversible; camera, selection, pins and history were
session-local. A derived `last-good.ts` kept accepted geometry available when disk
source was invalid, while disk source remained authoritative.

Early filesystem publication retained displaced source and rejected concurrent
rename overwrites, but interruption could leave recovery work. The later
[storage contract](M98_WORKSPACE_STORAGE.md) owns journaled publication and recovery.
Independent writers retaining old descriptors remain a documented Linux limitation.

## Next manual UAT

The original smoke sequence exercised external saves, Inspector writeback, invalid
source, stale draft retention, project download and restart. The expanded
[M98 UAT](M98_UAT.md) supersedes that sequence. U02 remains **Fail pending human
recheck**; other unperformed rows remain **Not run**.
