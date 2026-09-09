<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 takeover review — 2026-09-09

The supervising user accepted M97 and requested closure, then review and takeover of
this existing M98 worktree. M97 is now closed in the primary checkout by `3152f33`,
with accepted product `e26270cb89e5849092145b329d0cf95821a81b27`, qualified run
`20260908T235146-b387d273` and unchanged preview `http://100.94.63.83:18105/`.
The primary `docs/M97_CLOSURE.md` owns that sign-off.

This branch was clean at `cb581b50ac10b1e5edeb4da4c86761405577cd7e` when inspected.
Its six commits start from `d80bf22264f74b60870f2e99feb8cc6ccb9d0133`, before M97's
source-native implementation. This review adds prose only; it does not integrate
M97, change prototype behavior, or accept/qualify M98. Historical instructions and
M97 status in this branch describe its pinned base; do not redo M97 here.

## Assessment

The approach is worth continuing. Plain `sketch.ts` remains durable design authority;
Node handles filesystem/HTTP transport while the existing Rust/WASM workbench owns
managed-source transactions, solving and scene publication. The existing UI is reused.
The baked-profile follow-up samples accepted Rust production topology, independently
of canvas tessellation. These are useful boundaries to retain.

This is a working prototype, not just a plan. Inspected logs confirm seven actual
bake tests, 13 browser subtests plus their parent (Node reports 14 tests), and 20
follow-up frontend adapter/receipt tests. Native sampler evidence records three tests;
the initial frontend slice records 67. Full integrated qualification was intentionally
deferred. [Folder handoff](M98_HANDOFF.md) and [bake handoff](M98_BAKE_HANDOFF.md) retain
the exact original commands, artifacts and limitations.

## Takeover priorities

1. Integrate the accepted M97 source/compiler, metadata and Inspector changes, retaining
   the separate folder adapter. Check new names/help, overview toggles, document defaults,
   named parameters, extraction and Undo/Redo through disk writeback and external changes.
   Give the starter radius explicit overview intent now that M97 removes first-six defaults.
2. Replace broad DOM input capture and command-suffix guesses with explicit authoring edit
   lifecycle hooks. Current capture handles `HTMLInputElement`, while M97 descriptions use
   textareas; it can also capture unrelated search input. Current `edit|set` suffix routing
   is not a complete contract for M97 metadata mutations. These are static integration
   risks, not independently reproduced defects in the pinned prototype.
3. Make folder-mode capabilities visible at the affected controls. New/import/sample actions
   currently remain available until refused by the bridge; native point/grip dragging is
   refused after a held-button move. Keep supported source authoring discoverable and explain
   the deferred operations before the user starts them.
4. Review writeback recovery without discarding conflict protection. Displaced plaintext plus
   exclusive publication protects concurrent rename writes, but there is a briefly missing
   entry and recovery files accumulate. Existing tests do not inject interruption between
   displacement and publication. Define a bounded recovery workflow before everyday use.
5. Measure a larger supported sketch before changing the transport. Ordinary pointer requests
   currently capture full persistence for rollback, and full snapshots cross HTTP. The small
   circle evidence does not establish manifold/dense navigation costs. Treat this as a
   measurement priority, not a proven performance defect.

The single-entry/SDK-only scope excludes custom patch files, so the current manifold
cannot simply be opened as a folder project. Multi-file dependency support is the most
useful subsequent expansion after the merged happy path is verified. Bake additionally
refuses computed features and supports native lines, circles and arcs only, so it cannot
export that manifold either. Keep those two limitations distinct.

Bake exports all bounded faces, including hole interiors; consumers must explicitly
select the board region. IDs are local to an export. GeoSolve's output evidence does not
prove the separately owned MiniCAD/STL integration. No other repository was reviewed or
changed during this takeover.

## Fresh takeover observations

These commands ran against this worktree's existing built artifacts, without rebuilding:

```bash
node scripts/file-workspace.mjs status target/m98/demo
node scripts/file-workspace.mjs bake examples/file-workspace-bake/pi-footprint --out target/m98/bake/takeover-pi-footprint.json --chord-error-mm 0.02
```

`status` exits 1 with `TypeError: fetch failed`; a direct request to
`http://127.0.0.1:37135/` receives connection refused. The saved session points to that
endpoint. Earlier statements that the preview is still running are historical and
do not establish current availability. No service was restarted or replaced here.

The real bake exits 0, with accepted revision 1/workbench revision 3. Parsed JSON equals
the preserved `target/m98/bake/pi-footprint.json` exactly. Independent bounds give
`[0,0]` to `[85,56]` for `region-4`, with four holes. The recorded source SHA-256 equals
the current fixture's exact bytes:
`40bef69c119320ac446237dc22cf6491580d69db599594fb59c76bfed1327384`.
This is a fresh smoke check of existing artifacts, not clean-build or full release evidence.

The next implementation step is M97 integration and focused folder-mode verification.
The original task's prototype-first scope remains appropriate; avoid turning this takeover
into a solver or general filesystem framework rewrite. Both worktrees and their historical
artifacts remain intact.
