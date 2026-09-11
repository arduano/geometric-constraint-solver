<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 shared toolbar parity

The supervising user authorized implementing the remaining native toolbar routes after
reviewing why shared editing exposed only four construction tools. This amendment is
in progress; the qualified dragging product and live documents remain the baseline.

| Existing native surface | Shared route required |
| --- | --- |
| 25 geometry variants | Native construction, staged guides, options, explicit branches, inference cycling, Finish/Step Back |
| 13 constraint tools | Native operand collection, preselection, applicability, options, source insertion |
| Five dimension tools | Native operand collection, value/branch controls, source-owned overview metadata |
| Fillet and Profile Offset | Exact native curve occurrences, preview, options, validated source transactions |
| Profile/Construction roles | Personal new-geometry default and source-authoritative selected-geometry changes |
| Existing tool/property actions | Same native applicability and source authority as the ordinary workbench |

Geometry construction extends its existing ordered replay contract. Other tools use a
separate native operation continuation over the accepted editor. Pointer motion does not
invoke the source compiler. Browser code transports events and displays native results;
it does not reconstruct geometry equations or tool applicability. Detached scene bindings
translate personal selection into the engine's namespace, including picked curve occurrences.

Terminal commands retain the original accepted source/design basis, bounded ordered samples,
semantic operands and resolved branch/declaration witnesses. The server restores the original
accepted checkpoint, independently replays the command, authenticates original dependency
lifetimes and prepares its effect on the latest accepted state. Genuine compiler receipts,
native parity and independent residual validation precede durable publication. Provisional
geometry never grants authority. Server prediction uses the same native continuation.

Accepted edits contribute to personal Undo/Redo. Existing source drafts, invitations, journals,
target lifetimes and other editors' changes remain subject to the existing collaboration
contract. Navigation and provisional reprojection remain local while tools or commits run.

- [x] Native construction inventory, options/branches and staged rendering pass focused checks.
- [x] Constraint/dimension/modify/role operations pass native and actual-WASM checks.
- [x] Historical/latest replay rejects replaced operands and preserves concurrent disjoint edits.
- [x] Personal Undo/Redo covers new geometry and relations, modifications and generated points.
- [x] All existing native toolbar routes and contextual controls are reachable for editors.
- [x] Client/server prediction preserves held-navigation responsiveness and terminal ownership.
- [ ] Integrated clean-source qualification and exact preview replacement preserve live state.

No new solver primitive, residual equation, tolerance, or golden expectation is authorized by
this integration. M98 acceptance and closure remain open.

The parity inventory is the current workbench's geometry/relation/dimension/modify
catalog and its source-authoritative controls. The older command manifest also
contains a native-profile Fillet output action, which the current ordinary React
bridge does not expose; restoring that historical action is separate work. Generated
role edits require an actual writable source role path, as other generated property
edits do. Read-only generated output does not acquire authoring authority by selection.


Focused development qualification on the in-progress worktree passes:

- `cargo test --locked -p geosolve-sketch-engine --test construction_parity --test
  tool_operations --test tool_presentation --test tool_selection`: 26 tests pass;
  two explicit genuine-compiler fixture writers are ignored. All 25 construction
  and 21 operation/role lifecycle recipes run inside those owner tests.
- `cargo test --locked -p geosolve-demo-web --lib
  operation_prediction_paint_preserves_native_pending_and_offset_guides_after_navigation`:
  passes pending styling, ordered chain cues, current-camera reprojection, active
  dimension disclosure in Hidden mode and noninteractive paint.
- `cargo clippy --locked -p geosolve-sketch-engine -p geosolve-sketch-engine-wasm
  -p geosolve-demo-web --all-targets -- -D warnings` and `cargo fmt --all`: pass.
- `node packages/geosolve-engine/scripts/build-wasm.mjs`, engine TypeScript build,
  and `node crates/geosolve-demo-web/frontend/scripts/build-wasm.mjs --release`:
  pass from the final native implementation (65 s engine, 31.73 s demo).
- `node --test scripts/collaboration-preview.test.mjs
  packages/geosolve-engine/test/construction.test.mjs`: 41 pass, no skips; includes
  all 25 actual-WASM construction recipes and sequenced camera/reset semantics.
- `node --test --test-name-pattern='shared native|shared tool replay|shared selected
  geometry|server tool activation' scripts/collaboration-runtime.test.mjs`: six
  pass, no skips, including genuine Fillet/Offset publication after a peer edit,
  durable restart/dedup and personal Undo/Redo.

These commands run through the pinned repository Nix shell. Native tests use
`CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only`.
The subsequent actual-WASM operation and browser results below complete focused
development checks. Integrated clean-source nomination is pending; existing qualified
live services are unchanged.

`node --test packages/geosolve-engine/test/tool-operations.test.mjs` also passes
25/25 actual-WASM tests: all 21 actions independently compile, validate, publish,
reconstruct cold and Undo/Redo. Empty reset and rejected relation recovery continue
through genuine compiler publication; cumulative trace exhaustion preserves draft
and permits a smaller correction at the same sequence. No product failure remains
in that matrix.

All five focused collaboration browser workflows pass against the final isolated
production build: full toolbar availability/advanced conic/reference dimension with
peer history, native Parallel preselection across zoom, Fillet options and selected
roles, and client/server Offset with Explorer input and held publication. Two initial
failures were test locator errors (`getByLabel` selected the toolbar button or missed
the nested select); role-based combobox locators corrected the harness. Native traces
confirmed the Fillet candidate was complete throughout; product behavior did not change.
Held Offset navigation records client wheel/pan/resize at 98/66/346 ms and server
prediction at 87/71/266 ms, with zero navigation RPCs. These are focused observations,
not a 60 Hz or dense-model performance claim. Full integrated nomination follows.

The first integrated product attempt `20260911T130620-4b8e58fc` passed native,
headless and optimized-WASM lifecycle checks but stopped during browser preparation:
the expanded engine module was 21,842,414 bytes, exceeding its existing 20 MiB
ceiling. The engine release script had omitted the `wasm-opt -Oz` step already
used by the demo release script. Applying that optimizer inside the private staging
directory reduces the focused engine artifact to 14,068,507 bytes (13.42 MiB),
without changing the ceiling or solver source. All 66 actual-WASM construction, operation and server-preview tests pass on the
optimized binary (`node --test packages/geosolve-engine/test/construction.test.mjs
packages/geosolve-engine/test/tool-operations.test.mjs scripts/collaboration-preview.test.mjs`).
A new clean integrated nomination must pass before delivery; the failed run remains failed.

The second nomination `20260911T133551-a403dc7b` passes all 49 ordinary
browser workflows, engine/folder/runtime/package/frontend checks and optimized
artifact preparation, but fails four collaboration browser assertions. A bounded
Fillet paint wait fixes one harness race; collaboration latency measurements now
require exclusive gate execution. Isolated and previous-artifact comparisons
expose the existing synchronous GPU stall recorded as [M98-F039](M98_HARDENING.md#m98-f039--synchronous-gpu-completion-stalls-canvas-input).
The asynchronous fence repair retains exact validated frame evidence and error
checks; 39 focused renderer tests, dragging and all four actual-browser
pixel/resize/context-recovery workflows pass. Renewed integrated qualification
and preserved live delivery remain pending.
