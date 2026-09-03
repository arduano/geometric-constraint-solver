<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# GeoSolve workbench frontend

This directory owns the browser DOM and presentation layer. Rust/WASM remains authoritative for
project state, code expansion, solving, history, semantic interaction and accepted SVG frames.
`src/lib/adapter.ts` is the bounded versioned seam between those layers; tests use the isolated
mock in `src/lib/mock-adapter.ts`.

The frontend intentionally supports desktop layouts from `1024 × 720` upward. Its transient UI
contract permits one menu or Open surface at a time, uses light-dismiss without consuming the
destination click, restores the invoker's focus after keyboard dismissal, and gives Escape to the
trace modal, then transient UI, a captured canvas gesture, and finally active authoring.

Useful checks:

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

Set `GEOSOLVE_PUBLIC_BASE=/repository-name/` and `GEOSOLVE_DIST=/tmp/geosolve-pages` to assemble a
repository-prefixed artifact. Every build copies `LICENSE`, `THIRD_PARTY_LICENSES.md` and
`docs/API_COMPATIBILITY.md` from repository authority; the validator checks their exact bytes,
content-hashed JS/CSS/WASM assets, a real optimized WASM module below the 20 MiB release ceiling,
and absence of symlinks or undeclared files.
