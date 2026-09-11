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
session. Foreign, consumed and stale preparations cannot publish. For latest-model replay, use `preparePointGestureReplay` with the independently admitted
original session and authenticate its required target lifetimes against server history.

Construction uses `beginConstruction(tool, { expected, gestureId, viewport, role? })`
for all 25 existing native geometry variants. `initialFrame` exposes native defaults
and capabilities before the first click. Send ordered `move`, `click`, `complete`,
`step_back`, `reset`, `flip_branch`, `cycle_inference`, `conic_options` and
`nurbs_options` events as applicable. Pointer events carry explicit inference
suppression and recipe regularization choices. After navigation, send a `viewport`
event before the next authoring input; this updates pixel picking/snap tolerances
without changing the command's original camera or publishing a model edit.

Constraint, dimension, Fillet and Profile Offset authoring use
`beginToolOperation(tool, { expected, gestureId, viewport, selection, options? })`.
Options apply before preselection, which may complete an ordinary relation or
dimension immediately. Use `toolOperationPresentationJSON(viewport)` and
`toolOperationOperands(nativeSelection)` to resolve exact accepted selection into
source-owned semantic operands. A detached workbench scene has its own namespace;
map its source bindings and full curve-pick occurrences before this conversion.
Selected geometry roles use the same operation API with `toggle_geometry_role`.
Generated outputs require a source-owned writable role path.

Operation events include pointer motion/clicks, explicit `pick`/`pick_selection`,
`authoring_options`, `fillet_options`, `fillet_radius`, `offset_distance`,
`offset_flip`, `complete`, `step_back`, `reset` and `viewport`. The native frame
supplies pending operands, applicability diagnostics, reset/finish capabilities and
exact per-corner Fillet options. `presentationJSON()` combines the detached scene,
source correspondence and native paint-only guides/highlights; reproject it using
the current personal camera. Prediction never publishes source or history.

For either tool family, `finish()` returns replay intent with its original basis,
ordered bounded samples and explicit resolved declarations/branches. The trusted
server independently calls `prepareConstructionReplay` or
`prepareToolOperationReplay` with its admitted original and current sessions. It
must authenticate required declaration lifetimes against its own ordered history.
Run the returned compiler request, resolve its genuine receipt with
`resolveConstruction` or `resolveToolOperation`, and persist the resulting complete
candidate before `applyConstructionCommit` or `applyToolOperationCommit`.
Preparation and commit handles are one use. Neither client paint nor a copied
candidate grants accepted authority. Exact replay rejects changed operand meaning;
server conflict policy and personal contribution history remain host-owned.

`exportProject()`, `exportDesign()` and `sourceDesignDigest()` provide complete durable
accepted inputs. Reopening those inputs independently revalidates geometry and preserves
the digest. Individual evaluation IDs include session state and are not recovery IDs.

M98 has focused Node/browser, session and offline archive checks. Final integrated
qualification and supervising-user acceptance remain outstanding.
