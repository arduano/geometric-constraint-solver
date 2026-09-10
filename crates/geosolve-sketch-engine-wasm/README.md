<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# `GeoSolve` sketch engine WASM

Dedicated Node/browser WASM binding for `geosolve-sketch-engine`, independent of the
demo crate, React and browser storage. `SketchEngine` exports `compileProjectJson`,
`evaluateGenerated`, `evaluateManaged`, `lastAccepted`, `exportProfiles`,
`exportProfilesForOutput` and `releaseResult`. Evaluations
return JSON snapshots; errors preserve the last accepted result. Accepted results
are retained for explicit export until `releaseResult` or engine disposal. The adapter
limits retained distinct results to 64 and refuses further new results until released.

`openEditableSession` opens an authenticated project and optional semantic design.
`applyEditableProject`, `applyEditableOverlay`, `undoEditable` and `redoEditable`
take a target session plus its exact expected state token. `editableSessionState`,
`exportEditableDesign` and `closeEditableSession` inspect, persist semantic inputs,
and close sessions. Up to eight sessions may be open. Closing a session preserves
separately retained results for export until their explicit release.

Retained point gestures use explicit semantic targets and the shared native
continuation. Begin, ordered advance, detached scene, finish and cancel are separate
from accepted session publication. Each preview frame reports the owning native work
receipt. The client exports a semantic command; a server independently replays it
through `prepareEditablePointCommit`, producing unpublished project/design/digest
and result DTOs. After durable persistence, `applyEditablePointCommit` consumes the
server's exact session-bound ticket. Caller-provided scene or validation claims are
never publication authority. Stale source/design commands reject explicitly.

Prediction gestures and staged commits have separate tables, capped at eight each,
with a combined 64 MiB semantic-input/output budget and a 1 MiB request limit. Each
gesture reserves its bounded trace budget in advance; the native owner caps traces
at 4,096 samples. These are logical input bounds, not a measurement of native solver
heap size. Closing a session or freeing the engine drops all its retained and staged
handles. Browser prediction should run in its own authoring worker/engine instance
so long native work cannot block accepted-scene navigation.

Construction bindings retain native drafting through begin/advance/scene/finish/cancel.
`prepareEditableConstruction` independently replays the client's semantic command and
returns an exact compiler ticket. `resolveEditableConstruction` authenticates that receipt
and stages a complete project/design/digest/result candidate; it consumes the compiler
preparation only on success. The trusted host durably persists it before synchronous
`applyEditableConstructionCommit`. A separate table caps 24 construction handles and
64 MiB of logical semantic inputs/outputs, reserving the bounded gesture trace in advance.
Replacing a compiler ticket with its validated candidate works even when the table is full.
Session disposal drops prediction and both preparation stages. These handles are trusted
process ownership, not a substitute for the host's user/document/lifecycle admission.

Build and package with `node packages/geosolve-engine/scripts/build-wasm.mjs` in the
workspace Nix environment. The script uses the pinned `wasm-bindgen` version from
the workspace lockfile.
