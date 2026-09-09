<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Single-file compatibility example

This checked-in circle fixture uses the original `geosolve-folder-v1` format to preserve
compatibility coverage. New projects created by `geosolve init` use folder-v2, with complete
local imports, custom patches, semantic design sidecars and revision-checked CLI edits.
See the [current CLI](../../packages/geosolve-cli/README.md) and
[complete manifold example](../file-workspace-manifold/README.md).

From a prepared checkout:

```bash
node scripts/geosolve-cli.mjs init target/my-sketch
node scripts/geosolve-cli.mjs serve target/my-sketch
# Open the exact printed session URL, including its token.
node scripts/geosolve-cli.mjs inspect target/my-sketch
node scripts/geosolve-cli.mjs check target/my-sketch
```

Edit the circle's driving `value: mm(10)` in `sketch.ts`. The open canvas and Inspector
update after save. Inspector edits write source; Split shows the plaintext, and source
editor changes use Apply. Rejected files retain their text and previous accepted geometry.
Pending edits keep their original revision. Download pending intent on conflict, or use
Check save status after a disconnect to resolve the saved operation before retrying.

Only one bridge owns a folder and one tab owns editing. Use Take over editing when changing
tabs. Folder mode keeps its own project; the bare server URL opens the ordinary demo.

The v1 compatibility fixture supports source-backed dimensions and source Undo/Redo.
Its single-file format cannot persist native point-drag overlays or complete dependency
history; use a new v2 project for those workflows. Both formats use the current Linux
publication journal in `.geosolve/operations`, with explicit CLI inspection/recovery and
retained competing bytes. Optional v1 `last-good.ts` is a derived cache. See the
[journal contract](../../docs/M98_WORKSPACE_STORAGE.md); no cache provides authored authority.

Install the three offline archives to use the CLI without a Rust checkout. To prepare a
source checkout instead, run these commands from the repository Nix shell:

```bash
npm ci --prefix packages/geosolve-intent
npm run build --prefix packages/geosolve-intent
npm ci --prefix packages/geosolve-sketch-code
npm run build --prefix packages/geosolve-sketch-code
npm ci --prefix crates/geosolve-demo-web/frontend
node packages/geosolve-engine/scripts/build-wasm.mjs
node packages/geosolve-engine/scripts/build.mjs
npm run wasm:release --prefix crates/geosolve-demo-web/frontend
node crates/geosolve-demo-web/frontend/scripts/build-workspace.mjs
npm run build:ui --prefix crates/geosolve-demo-web/frontend
```

The bridge listens on loopback. For a different machine, forward its printed port with SSH
and open the same local URL/token. The documented implementation and focused checks do not
replace final release qualification or supervising-user acceptance.
