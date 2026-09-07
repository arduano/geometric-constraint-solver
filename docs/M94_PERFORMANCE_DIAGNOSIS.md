<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Dense canvas fixture performance diagnosis — 2026-09-07

Both performance fixtures have substantial optimization opportunities. Navigation is dominated
by repeated workbench/scene preparation and UI churn before GPU submission. Parameter edits
also have a separate expensive native publication path. Changing renderer language or reducing
visual quality should not be the first intervention.

This is diagnosis only: no production code, mathematical behavior, tests or served bytes changed.
Source inspected: `a242f89`; qualified product: `2f1711b`; immutable artifact aggregate
`16fb8cad31e55bde243607a8db06969287a3b3d04b19fa34586a97a8b4f439ae`.
The ordinary endpoint is `http://100.94.63.83:18096/`; measurements used its identical localhost
listener in fresh private browser contexts.

## Measurements

Chromium 151, desktop 1440×900, CSS canvas 871.921875×820, DPR 1 with the renderer's 2×
supersampling. Actual hardware capture reports NVIDIA RTX 3090 / ANGLE Vulkan / WebGL2.
Twelve alternating wheel inputs and twelve empty-space hover moves were sampled per fixture.
These are observations on this host, not FPS measurements or a cross-device performance claim.

| Measurement | Perforated fixture field | Robotic harness backplane |
| --- | ---: | ---: |
| Canvas drawing items | 875 | 550 |
| Entire page DOM elements in Design mode | 41,131 | 12,493 |
| Complete interaction snapshot, million UTF-16 code units | 2.900 | 1.069 |
| Explorer section alone, million code units | 1.935 | 0.546 |
| Drawing section alone, million code units | 0.838 | 0.452 |
| Profiled wheel bridge call, median ms | 199.6 | 60.1 |
| Profiled renderer submission CPU, p50 / p95 ms | 42.9 / 56.5 | 18.2 / 20.7 |
| Native parameter-publication call, seconds | 12.25 | 5.03 |
| Explicit persistence encoding after edit, seconds | 0.92 | 0.44 |
| Persistence response after edit, million code units | 11.81 | 5.29 |

The representative edits are `northernCells.pilotRadius → 2.7 mm` and `bendRadius → 4.5 mm`.
Both produce accepted state and enabled Undo, finish with a ready renderer and no page errors.
There are no persistence calls during the measured wheel or empty-hover phases, and both idle
windows produce zero additional frames. Autosaving every pointer event is therefore not the
cause of the navigation slowdown.

A separate direct release-WASM probe excluded React updates, Pixi rendering, JSON instrumentation
and CPU profiling. Each operation had one warm-up plus eight measured calls on an independent
handle. It used the same WASM and camera extents; the default browser GPU choice is irrelevant
because these calls do not draw:

| Direct operation, median ms | Perforated fixture field | Robotic harness backplane |
| --- | ---: | ---: |
| Unchanged `snapshot()` | 142.9 | 37.7 |
| Empty-space hover | 158.0 | 38.4 |
| Camera wheel | 252.7 | 56.3 |

Even requesting an unchanged snapshot accounts for nearly all the direct empty-hover cost.
This isolates a large optimization opportunity before renderer work. The independently measured
passes vary with host conditions and instrumentation; their columns must not be added together.

The profiled input-to-submission observations were approximately 377/164 ms p50, but include
Playwright transport, observers and instrumentation. Whole phase wall times also include profile
collection/transfer and are **not user edit latency**. In particular, field/backplane edit phases
reported 31.45/14.37 seconds wall time while the actual CPU-profile windows were 17.61/7.28 seconds.
Use the directly wrapped native calls above to attribute the publication bottleneck. The sampled
field managed-compiler bundle contributed about 0.23 seconds self time; this does not support
blaming TypeScript compilation for the bulk of that edit's measured CPU time.

## Confirmed causes and recommended order

1. **Keep unchanged document/UI projections out of camera and hover updates.**
   `WorkbenchBridge::snapshot` reconstructs Explorer visibility, frame, source, Explorer,
   selection, parameters and problems. Explorer is projected once during visibility reconciliation
   and again for the returned snapshot; additional presentation projections are used by selection
   and problems. `pointer_json` returns this complete snapshot even after unchanged empty hover.
   `wheel_json` clears the retained scene and returns the complete snapshot too. Cache immutable
   sections by their exact authority inputs and publish small frame/hover/camera updates. Return
   no update when interaction presentation is unchanged. This is the highest-impact first target.

2. **Retain geometry across camera changes and remove redundant renderer work.**
   Screen coordinates are baked into all draw items, so wheel/pan changes their signatures and
   clears/rebuilds most Pixi Graphics. Separate the authoritative camera transform from retained
   geometry, with correct treatment of screen-sized strokes, points and annotations. Keep native
   picking on the same camera. Independently, `setChildIndex` is called for every item every draw;
   pinned Pixi performs two linear `children.indexOf` searches even for an unchanged index.
   Avoiding unchanged reorder calls removes concrete O(N²) work, although its isolated contribution
   has not yet been benchmarked. Prefer pan as the first retained-transform implementation.

3. **Preserve immutable frame/UI identity and bound native UI work.**
   Drawings are validated at WASM decode, again during pending-mutation resolution, and again
   before renderer acceptance. They are then cloned, deeply frozen, whole-frame stringified and
   individually stringified for renderer signatures. Validate/freeze once at a trusted boundary
   and use explicit immutable identities/deltas afterward. The complete fresh snapshot also
   rerenders the unvirtualized Explorer; in Split mode fresh `source.files` identity recreates
   the language project and forces diagnostics even when source is unchanged. Stable references,
   memoized surfaces and virtualized declaration/parameter lists avoid much of this churn.

4. **Coalesce navigation and unchanged hover before entering WASM.**
   Rendering is already RAF-coalesced, but every incoming pointer/wheel event synchronously
   performs bridge work first. Accumulate camera deltas and retain the latest hover request per
   frame. Treat drag samples separately: terminal events, bounded continuation and explicit
   branch state must retain their interaction semantics.

5. **Optimize editing in a separate pass at the native publication owner.**
   `CodeProjectWorkbench::resolve_canvas_managed_mutation` and `resolve_managed_source_apply`
   serialize the entire workbench and reconstruct it from persistence to obtain an isolated
   candidate before applying the compilation. Preserve that isolation using an internal fork
   with shared immutable data, then reuse unaffected materialization/dependency work where its
   inputs prove identical. Keep cold/native residual validation and atomic history publication.
   A symbolized native profile should separate restoration, expansion, validation and solving
   before changing numerical algorithms. Moving long work into a worker can improve UI
   responsiveness, but does not by itself reduce its computation time.

Relevant owners:
[bridge](../crates/geosolve-demo-web/src/workbench/bridge.rs),
[publication](../crates/geosolve-demo-web/src/workbench/code_projects.rs),
[renderer](../crates/geosolve-demo-web/frontend/src/lib/canvas-renderer-pixi.ts),
[frame acceptance](../crates/geosolve-demo-web/frontend/src/lib/canvas-renderer.ts),
[frame validation](../crates/geosolve-demo-web/frontend/src/lib/canvas-scene.ts),
[App](../crates/geosolve-demo-web/frontend/src/App.tsx),
[side panels](../crates/geosolve-demo-web/frontend/src/components/side-panels.tsx).

No measured speedup is claimed yet. GPU timings, continuous drag/solve performance, Split-mode
cost, edited-history reload latency and exact native subroutine attribution remain unmeasured in
this diagnosis. Lowering raster resolution, removing geometry or weakening validation would
change the contract and is unnecessary as a first approach. The next implementation should use
focused owner regressions and these operation probes, followed by one integrated qualification
with authenticated reuse once the candidate stabilizes.

## Evidence and executed commands

Temporary diagnostic scripts and CPU profiles are preserved under `target/m94`, outside the
product build. No full gate, Cargo build or test suite was run for this diagnosis.

```bash
nix-shell shell.nix --run 'timeout 600 node /tmp/m94-dense-profile.mjs'
nix-shell shell.nix --run 'DENSE_OUTPUT=/home/arduano/programming/geometric-constraint-solver/target/m94/dense-performance-r3 DENSE_SAMPLES=robotic-harness-backplane timeout 180 node /tmp/m94-dense-profile.mjs'
nix-shell shell.nix --run 'timeout 90 node /tmp/m94-dense-boundary.mjs'
```

The first profiled attempt retained navigation data but stopped at the default five-second
Playwright assertion budget during the field edit. The second attempt completed every field
phase and backplane navigation but used an incorrect `bendRadius · value` locator. The corrected
backplane-only third attempt passes all phases; the existing sample witness correctly labels
this scalar `bendRadius`. These are diagnostic harness failures, not new product defects.

Final field evidence: `target/m94/dense-performance-r2/report.json` and adjacent CPU profiles.
Final backplane evidence: `target/m94/dense-performance-r3/report.json` and adjacent CPU profiles.
Unprofiled boundary evidence: `target/m94/dense-performance/direct-boundary.json`.
Logs: `target/m94/dense-performance/{profile,profile-r2,profile-r3,direct-boundary}.log`.
The corrected reusable scripts are `target/m94/dense-performance/{profile,direct-boundary}.mjs`.
The original failed runs remain recorded; they do not supply successful edit evidence.
