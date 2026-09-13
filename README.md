<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GeoSolve

**An embeddable Rust sketch constraint solver, with a headless interaction layer
and bidirectional TypeScript authoring.**

[Try the browser demo](https://arduano.github.io/geometric-constraint-solver/) ·
[Documentation](docs/README.md) · [Local server](packages/geosolve-cli/README.md) ·
[Architecture](ARCHITECTURE.md)

GeoSolve is building the foundation for CAD applications where you can draw a
sketch, edit its code, and work alongside an AI agent on the same design. The
solver and interaction logic are reusable libraries; the included web workbench
and local server demonstrate how to put them together.

## What it provides

**A Rust-native, WASM-friendly solver.** Define geometry and constraints, solve
them, inspect degrees of freedom and diagnostics, and persist the result. The
same Rust implementation runs natively or as WebAssembly. Accepted geometry must
pass independent residual validation, and rejected edits preserve the last valid
result.

**A headless UI adapter.** Reuse selection, picking, dragging, construction tools,
dimensions and accepted-scene projections without adopting the demo's renderer or
React components. Your application owns its presentation and storage.

**Code and canvas authoring together.** The TypeScript authoring system gives
geometry, parameters and constraints stable names. Supported UI edits update
their source, and source edits compile back into the sketch. You and an AI agent
can work through the same authored model, with validation and history around
each accepted change.

**Working integration examples.** The [web demo](https://arduano.github.io/geometric-constraint-solver/)
combines a canvas, code editor, Explorer and Inspector. The local server watches
project files and writes supported UI edits back to them, so a person using the
canvas and an agent editing files can collaborate. A shared server mode also
demonstrates multiple editors with server-authoritative changes and local canvas
navigation.

## A sketch is also source code

```ts
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const ring = $.geometry.centerRadiusCircle("ring", {
    center: [0, 0],
    radius: mm(10),
  });
  const radius = $.dimension.radius("radius", {
    curve: ring.curve,
    value: mm(10),
  });
  return { ring, radius };
});
```

Change the driving radius in code or through the Inspector. Both routes update
the same sketch. Parameters and reusable patches extend this to designs such as
the [water manifold](examples/file-workspace-manifold/README.md).

Reversible sketches use the supported TypeScript authoring vocabulary. General
generator programs can expose typed controls and geometry too; arbitrary generated
geometry does not automatically have an editable source representation.

## Try it

The [GitHub Pages demo](https://arduano.github.io/geometric-constraint-solver/)
requires no installation. It hosts a published snapshot; local source and server
features may be newer.

With the matching CLI packages installed:

```bash
./node_modules/.bin/geosolve init my-design
./node_modules/.bin/geosolve serve my-design
```

Open the printed session URL, then edit `my-design/sketch.ts` in your editor or
with an AI coding agent. The server and workbench keep source and supported UI
edits connected. The CLI's `check my-design` command validates without opening a browser;
`geosolve bake` exports planar profiles for another application to consume.

See [installation and server usage](packages/geosolve-cli/README.md) for the four
matching offline packages, platform requirements and revision-checked agent commands.
See [development](docs/DEVELOPMENT.md) to build from source.

## Embed the parts you need

| Layer | Entry point |
| --- | --- |
| Sketch documents, constraints and solving | [`geosolve-sketch`](crates/geosolve-sketch) over `geosolve-core` |
| Headless input, tools and scene interaction | [`geosolve-constraint-editor`](crates/geosolve-constraint-editor) |
| Authored source and editable engine sessions | [`geosolve-sketch-engine`](crates/geosolve-sketch-engine/README.md) |
| TypeScript and WebAssembly embedding | [`@geosolve/engine`](packages/geosolve-engine/README.md), [`@geosolve/sketch-code`](packages/geosolve-sketch-code/README.md) |
| Server authority and shared editing | [`@geosolve/collaboration`](packages/geosolve-collaboration/README.md) |
| Browser-free inspection and rendering | [`geosolve-headless`](crates/geosolve-headless) |

The workspace also includes a separate planar/spatial linkage model, curve and
profile operations, and static rendering. Sketches and mechanisms share numerical
machinery while keeping distinct domain models. The [architecture guide](ARCHITECTURE.md)
maps these boundaries.

## Project status

GeoSolve is an actively developed, pre-1.0 project. The implemented sketch surface
includes lines, circles, arcs, conics, Bézier curves, B-splines and NURBS, together
with constraints, dimensions, explicit branches, fillets and profile offsets.
The demo is intended for desktop browsers. This is a sketch/kinematics foundation;
solid modelling and a complete production CAD application are outside its scope.

The [compatibility policy](docs/API_COMPATIBILITY.md) distinguishes supported
persistence from experimental APIs. [Qualification](docs/RELEASE_QUALIFICATION.md)
covers native/WASM behavior, a reviewed authoring/scene corpus, installed packages,
browser interaction and bounded performance. Dense sketches can still take seconds
to initialize or solve; [known limits](ACCEPTANCE.md#current-limits-and-review-status)
and the [roadmap](PLAN.md) describe the current scope.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE) and
[third-party notices](THIRD_PARTY_LICENSES.md). The solver uses pure Rust numerical
code; no C/C++/Fortran solver FFI is required.
