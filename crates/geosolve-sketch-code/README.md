<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# geosolve-sketch-code

Optional, equation-free code/GUI sketch-authoring companion. It authenticates compiler-produced
managed `sketch.ts` IR and execution artifacts, validates caller-built data-only patch artifacts,
retains semantic feature references, reconciles keyed generated members, and owns one atomic
code/editor history.

The crate never evaluates JavaScript and never defines solver equations. A browser or Deno compiler
host parses and executes the bounded authoring program, then hands Rust a canonical IR/artifact
envelope. Rust validates that complete envelope before expanding it into ordinary Design Intent
declarations handled by the existing materializer and independently validated solver.

## Code-only sketch

An artifact-free sketch can start from one managed TypeScript file, without first creating a GUI
scene:

```typescript
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.twoPointAlignedRectangle("frame", {
    firstCorner: [0, 0],
    oppositeCorner: [60, 35],
    role: "profile",
  });
  const diagonal = $.geometry.segment("diagonal", {
    start: frame.corners[0],
    end: frame.corners[2],
    role: "construction",
  });
  const width = $.dimension.curveLength("width", {
    curve: frame.spans[0],
    value: mm(60),
  });
  $.group("Frame", [frame, diagonal, width]);
  return { frame, diagonal, width };
});
```

`frame.corners[0]` is a lexical, type-checked feature reference—not a serialized ID. Moving or
resizing `frame` therefore keeps the diagonal attached to the same semantic output. Declarations
use named, domain-shaped inputs and the callback returns an ordinary nested result; transport
tuples and explicit output wrappers are not part of the public authoring API.

A browser or Deno host compiles those source bytes with `compileManagedSource` from
`@geosolve/sketch-code/ir`. (`/compiler` is the separate build-time custom-patch compiler.) A
native host then admits the compiler envelope as an
artifact-free project through the optional companion API:

```rust
use geosolve_sketch_code::{CodeProject, CompiledManagedSource, ProjectKey};

let compiler_envelope = include_str!("../assets/demos/neon-manifold.compiled.json");
let compiled = CompiledManagedSource::from_json(compiler_envelope)?;
let project = CodeProject::managed(ProjectKey("example".into()), compiled)?;
assert!(project.custom_files.is_empty());
# Ok::<(), geosolve_sketch_code::CodeProjectError>(())
```

Compilation alone never publishes geometry. The host still performs keyed expansion, ordinary
Intent materialization, native solving, and independent residual validation before the candidate
can replace accepted scene authority. The authenticated IR remains deep enough to print the whole
normalized sketch again, while the execution artifact records runtime reference flow and callback
output structure.

## Demonstrations

The bundled project catalog contains complete code-authored examples, including:

- **Rounded polyline** maps one reusable Fillet patch over every current keyed corner, so insertion
  and removal change cardinality without ordinal retargeting.
- **Typed panel** passes named rectangle corner outputs into a mapped Fillet record and returns the
  same record keys in its result type.
- **Braced frame** combines direct geometry, a reusable cross-brace and an ordinary native relation
  in one GUI/code/GUI-editable scene.
- **Mounting plate** consumes a caller-compiled, pinned custom patch that expands a rounded profile
  and stable keyed holes without evaluating TypeScript in Rust or the browser.

In `geosolve-demo-web`, open a fresh sketch, select **Code**, and choose **Start from code** or one
of those examples. Editing `sketch.ts` and pressing **Apply** compiles and validates the complete
candidate atomically; invalid syntax or geometry retains the prior accepted canvas and remains
undoable. Compiler/IR and live-control APIs intentionally live on the package's `/compiler`, `/ir`,
and `/control` subpaths rather than the public authoring root.
