<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# A custom generator website

A small Gridfinity-style footprint tool using **ordinary TypeScript and the headless
GeoSolve engine**. The page owns its form, SVG preview, measurements, status and download.
It imports no React, workbench, editor or demo WASM. The same generator also opens as a
local generator project through its `geosolve.json` manifest.

The generator creates a 42 mm grid, an outside clearance, optional 3.75 mm corner fillets
and four 6 mm magnet or 3 mm screw bores per cell. These are 2D profiles; height, pocket
depths and manufacturing fits are outside this example. Its dimensions are a practical
Gridfinity-style study, not a complete bin/baseplate specification.

From a prepared repository with built `@geosolve/sketch-code` and `@geosolve/engine`:

```bash
node examples/generator-website/scripts/build.mjs
node examples/generator-website/scripts/serve.mjs
# Open http://127.0.0.1:4188/
```

The build reuses this checkout's existing esbuild installation when the example has no
local `node_modules`. Engine WASM must already be built; the example never starts Cargo.
`PORT=4190 node examples/generator-website/scripts/serve.mjs` selects another loopback port.
Deploy the generated `dist/` directory with any static server serving `.wasm` as
`application/wasm`; there is no server API or account setup. All assets are local.

For a separate website project, install the authoring and engine package archives, add
esbuild, and copy this example's source/build files. Its product code imports only
`@geosolve/sketch-code` and `@geosolve/engine`; the build resolves installed packages first.
Replace the `file:` dependencies in this example's `package.json` with your local archive
locations. Keep one authoring SDK instance in a bundle so recorded reference identities
belong to the engine's recorder.

## Authoring and host responsibilities

- [`src/generator.ts`](src/generator.ts) exports `footprint`, a `defineGenerator` function.
  The source owns defaults, labels, help, units, numeric limits and choice values. Nested
  loops create stable per-cell bore IDs; a branch adds real computed corner fillets.
  The source can equally be an ordinary function when a custom host supplies all inputs.
- [`src/main.ts`](src/main.ts) builds controls from `footprint.inputs` and draws the
  **accepted exported polygons**, with two overall measurements. Dashed cell lines are
  presentation guides. The preview never uses unsolved generator coordinates as geometry.
- [`src/worker.ts`](src/worker.ts) loads the engine, evaluates the generator and exports
  the complete region containing the named `/outline` output, preserving its holes.
  Its 0.08 mm chord bound is model-space profile sampling, not screen tessellation.
- A new input immediately terminates superseded work. A 220 ms debounce avoids compiling
  every keystroke; a 20 second limit terminates a stalled worker. Rejection, cancellation
  and timeout preserve the last accepted preview and download. The export records the
  accepted inputs even while newer controls are invalid or still pending.

Worker termination provides preemption for trusted generator code. It is not a security
sandbox. A generator used inline with `engine.evaluate` runs in the host JavaScript realm;
the engine cannot interrupt an already-running synchronous callback there.

The UI does not expose source edits or dragging: generator mode regenerates from inputs.
The manifold folder example demonstrates reversible editable mode. `isKeyParameter` and
`isKeyConstraint` describe overview intent; they do not determine generator inputs.

## Focused checks

```bash
node examples/generator-website/scripts/check-types.mjs
node --test examples/generator-website/scripts/generator.test.mjs
node --test examples/generator-website/scripts/browser.test.mjs
```

Node checks exercise the real source loader, input validation, changing declarations,
actual engine acceptance, complete named profiles and the source-only manifold folder.
The Chromium workflow verifies accepted dimensions/holes, edits, invalid input retention,
accepted export identity, cancellation, timeout recovery and a narrow viewport. Its
spinning-worker fault injection tests preemption without fabricating native geometry.
Screenshots are written under ignored `test-output/`.
