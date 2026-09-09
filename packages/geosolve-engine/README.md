<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# GeoSolve headless engine

Create validated 2D sketches in Node or a browser without the workbench, React, a DOM,
or a local preview server. Your application owns its controls and rendering.

```ts
import { createEngine } from "@geosolve/engine";
import { sketch, mm } from "@geosolve/sketch-code";

const engine = await createEngine();
const result = await engine.evaluate({
  definition: ({ radius }: { radius: number }) => sketch(($) => {
    const ring = $.geometry.centerRadiusCircle("ring", { center: [0, 0], radius: mm(radius) });
    return { ring };
  }),
  parameters: { radius: 12 },
});
if (result.status === "accepted") {
  const profile = await engine.exportProfiles(result, { chordErrorMm: 0.02 });
  // Use profile.regions and result.geometry in your own renderer or manufacturing pipeline.
  engine.release(result);
}
engine.dispose();
```

Use the authoring SDK's `defineGenerator` for optional discoverable input defaults and
validation. Ordinary functions, loops, helpers and imported modules also work. Generator
results carry no reverse source editing capability. `evaluateEditable` admits the existing
strict compiled CodeProject wire; it never fabricates managed lexical receipts.

Results are immutable and belong to their engine. A rejected, cancelled or superseded
request leaves `lastAccepted` intact. Retain a result while using its export authority and
release it afterward; dispose the engine when finished. Equivalent results may share an
internal result ID, but callers must pass an actual retained result object to export it.

Inline callbacks execute synchronously in the host JavaScript realm. AbortSignal cancels
queued work; it cannot preempt a callback already executing. Use a terminable worker for
unbounded source, as the maintained folder loader and generator website do. Worker execution
is not a security sandbox. Bundlers may supply `wasmModule` and `wasm` explicitly; default
initialization loads the packaged WASM through filesystem bytes in Node or a URL in browsers.

Editable source uses `engine.openEditableSession(project, { design? })`, where `project`
is a strict admitted CodeProject. Capture `session.token` before each mutation:

```ts
const session = engine.openEditableSession(project);
const expected = session.token;
const state = await session.applyProject(nextProject, { expected });
// session.applyOverlay, undo and redo use the same expected-token contract.
const design = session.exportDesign();
session.dispose();
```

Stale or cross-session authority is rejected without changing accepted geometry/history.
The inspectable design sidecar stores keyed reconciliation and semantic overrides, without
solved coordinates or a workbench checkpoint; source plus design can reopen in a new session.
Previously retained accepted export handles remain valid until engine release/dispose.
Generator results have no reverse source-editing authority. See the shipped TypeScript
interfaces for exact method/result types.

M98 has focused Node/browser, session and offline archive checks. Final integrated
qualification and supervising-user acceptance remain outstanding.
