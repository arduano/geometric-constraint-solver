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

Build and package with `node packages/geosolve-engine/scripts/build-wasm.mjs` in the
workspace Nix environment. The script uses the pinned `wasm-bindgen` version from
the workspace lockfile.
