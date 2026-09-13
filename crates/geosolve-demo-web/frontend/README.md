<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# GeoSolve workbench frontend

This directory contains the React workbench: canvas, Explorer, Inspector and TypeScript editor.
It demonstrates the embeddable Rust solver, headless interaction adapter and bidirectional
authoring system. Try the [public demo](https://arduano.github.io/geometric-constraint-solver/)
or follow [Getting started](../../../docs/GETTING_STARTED.md) to run it locally.

Rust owns geometry, validation, source-edit preparation and native interaction. The frontend
renders numeric drawing frames with WebGL and keeps navigation, selection and highlighting
local while compilation and solving run in independent workers or the local server. The
standalone, folder and shared hosts reuse the native engine and browsing services;
`src/lib/adapter.ts` connects their typed messages to the interface.

The frontend intentionally supports desktop layouts from `1024 × 720` upward. Its transient UI
contract permits one menu or Open surface at a time, uses light-dismiss without consuming the
destination click, restores the invoker's focus after keyboard dismissal, and gives Escape to the
trace modal, then transient UI, a captured canvas gesture, and finally active authoring.

Run these commands from this directory after preparing the repository packages:

```text
npm ci --ignore-scripts
npm run check:manifest
npm run check:licenses
npm test
npm run build:ui
npm run test:e2e
```

The complete build additionally compiles and binds the Rust WASM package. It needs the repository
Rust/Nix toolchain and the Cargo-locked `wasm-bindgen` CLI version:

```text
npm run build
npm run validate:dist -- ../dist ./
```

`npm run dev` builds debug WASM once and starts Vite. Use `npm run wasm` after Rust changes; use
`npm run dev:ui` when iterating only on React/CSS against already-generated bindings.

Set `GEOSOLVE_PUBLIC_BASE=/repository-name/` and `GEOSOLVE_DIST=/absolute/path/to/site` to assemble a
repository-prefixed artifact. Every build copies `LICENSE`, `THIRD_PARTY_LICENSES.md` and
`docs/API_COMPATIBILITY.md` from repository authority; the validator checks their exact bytes,
content-hashed JS/CSS/WASM assets, a real optimized WASM module below the 20 MiB release ceiling,
and absence of symlinks or undeclared files.
