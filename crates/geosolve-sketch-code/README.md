<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# geosolve-sketch-code

Optional, equation-free code/GUI sketch authoring companion. It validates and rewrites the bounded
managed `sketch.ts` subset, validates caller-built data-only patch artifacts, retains semantic
feature references, reconciles keyed generated members and owns one atomic code/editor history.

The crate never evaluates JavaScript and never defines solver equations. Custom TypeScript is
compiled by an explicitly invoked caller-owned Node process; Rust and WASM consume only canonical
bounded artifacts which expand to ordinary Design Intent declarations handled by the existing
materializer and independently validated solver.

## Code-only sketch

An artifact-free sketch can start from one managed TypeScript file, without first creating a GUI
scene:

```typescript
"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  $.organize("Frame", [frame, diagonal]);
  return $.outputs({ frame, diagonal });
});
```

`frame.corners.lowerLeft` is a lexical, type-checked feature reference—not a serialized ID. Moving
or resizing `frame` therefore keeps the diagonal attached to the same semantic outputs. The exact
directive selects `GeoSolve`'s deterministic managed-v1 subset, which Rust can parse and rewrite
without executing the callback.

A native host admits those source bytes as a validated artifact-free project through the optional
companion API:

```rust
use geosolve_sketch_code::{CodeProject, ProjectKey};

let source = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";
export default sketch(($) => {
  const line = $.geometry.line("line", { start: [0, 0], end: [20, 0] });
  return $.outputs({ line });
});
"#;
let project = CodeProject::managed_only(ProjectKey("example".into()), source)?;
assert!(project.custom_files.is_empty());
# Ok::<(), geosolve_sketch_code::CodeProjectError>(())
```

Parsing alone never publishes geometry. The host still performs keyed expansion, ordinary Intent
materialization, native solving and independent residual validation before the candidate can
replace accepted scene authority.

## Demonstrations

The bundled project catalog contains four complete code-authored examples:

- **Rounded polyline** maps one reusable Fillet patch over every current keyed corner, so insertion
  and removal change cardinality without ordinal retargeting.
- **Typed panel** passes named rectangle corner outputs into a mapped Fillet record and returns the
  same record keys in its result type.
- **Braced frame** combines direct geometry, a reusable cross-brace and an ordinary native relation
  in one GUI/code/GUI-editable scene.
- **Mounting plate** consumes a caller-compiled, pinned custom patch that expands a rounded profile
  and stable keyed holes without evaluating TypeScript in Rust or the browser.

In `geosolve-demo-web`, open a fresh sketch, select **Code**, and choose **Start from code** or one
of those examples. Editing `sketch.ts` and pressing **Apply** validates the complete candidate
atomically; invalid syntax or geometry retains the prior accepted canvas and remains undoable.
