<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# geosolve-sketch-code

Optional, equation-free code/GUI sketch authoring companion. It validates and rewrites the bounded
managed `sketch.ts` subset, validates caller-built data-only patch artifacts, retains semantic
feature references, reconciles keyed generated members and owns one atomic code/editor history.

The crate never evaluates JavaScript and never defines solver equations. Custom TypeScript is
compiled by an explicitly invoked caller-owned Node process; Rust and WASM consume only canonical
bounded artifacts which expand to ordinary Design Intent declarations handled by the existing
materializer and independently validated solver.
