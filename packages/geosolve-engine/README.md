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

Managed point prediction uses `pointGestureTargets()` and `beginPointGesture(target,
{ expected, gestureId, viewport })`. Keep one retained gesture in a dedicated
authoring worker, pass ordered model-space samples to `advance`, and render its
detached `sceneJSON()` as provisional output. `finish()` returns the semantic command;
`cancel()` discards the fork. Neither publishes source, design or session history.

The trusted server independently calls `preparePointGestureCommit(command,
{ expected })` on its own session. Persist that candidate's complete `project`,
`design` and `source_design_digest`, then call synchronous
`applyPointGestureCommit(prepared)`. The candidate result has no accepted export
authority until installation. Dropping/releasing a candidate preserves the live
session. Foreign, consumed and stale preparations cannot publish. Exact source/design
bases must match; latest-state gesture rebase is not yet supported.

Construction uses `beginConstruction(tool, { expected, gestureId, viewport, role? })`
for `segment`, `polyline`, `center_radius_circle` and `two_point_aligned_rectangle`.
The returned prediction accepts ordered `move`, `click`, `complete` and `step_back`
events. Each pointer event independently controls inference suppression and recipe
regularization. Render the shared native preview and inference guides from each frame
together with `sceneJSON()`. A correction-ready diagnostic remains local to the draft.

`finish()` returns a semantic command, including the resolved source references and
branch choices. The trusted server calls `prepareConstruction`, runs its exact compiler
request, then calls `resolveConstruction` with that receipt. This returns an unpublished
project/design/digest candidate; persist it before `applyConstructionCommit`. Compiler
preparations and durable candidates are separately owned and one use. Neither a client
scene nor a copied candidate object can become accepted server authority. Construction
uses the default shared inference cohort; explicit candidate cycling and advanced tool
variants remain outside this API.

`exportProject()`, `exportDesign()` and `sourceDesignDigest()` provide complete durable
accepted inputs. Reopening those inputs independently revalidates geometry and preserves
the digest. Individual evaluation IDs include session state and are not recovery IDs.

M98 has focused Node/browser, session and offline archive checks. Final integrated
qualification and supervising-user acceptance remain outstanding.
