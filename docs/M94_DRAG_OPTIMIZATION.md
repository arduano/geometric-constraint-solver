<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94-F003 — dense backplane dragging

Status: bounded repair implemented and focused checks pass; final qualification pending. M94 remains open for supervising-user acceptance.

The supervising user reported dragging in the dense robotic harness taking about two seconds per
move and authorized a correction if it fits M94. The reproduced delay is principally the release
of an ordinary point drag. Continuous point previews also exceed a 60 Hz frame budget.

## Reproduction and attribution

Source `0d8c274`, qualified product `93fdcbf`, immutable artifact aggregate
`cd6b351b29fd6e802b7b5ba0f8e1b783f6c105d274b924f93e4528b2e6766d5b`.
Open **Dense robotic harness-routing backplane**, choose Design and Fit, then drag either the
upper inner-east mounting-hole centre or the power-bus source endpoint through six ordered
`(+2, -1)` CSS-pixel moves and release. The first sample is below the movement threshold; the
remaining five advance the geometry. The complete `(+12, -6)` pixel terminal remains exact.

Chromium 151.0.7922.173, NVIDIA RTX 3090 / ANGLE Vulkan / WebGL2, viewport 1440×900, canvas
871.921875×820, DPR 1 and unchanged 2× supersampling:

| Phase, milliseconds | Mount centre | Power-bus source |
| --- | ---: | ---: |
| Accepted pointer move, median WASM | 55.3 | 55.9 |
| Renderer submission, median CPU | 3.5 | 3.9 |
| Pointer-up WASM publication | 1386.9 | 1394.1 |
| Following persistence encoding | 371.3 | 371.5 |
| Release browser-action wall time | 1834.2 | 1835.2 |

Separate direct WASM handles reproduce 1325–1467 ms release, 50–63 ms move medians and 24–28 ms
unchanged snapshots without React, rendering or autosave. Each move returns approximately
1.069 million UTF-16 characters. Both successful gestures preserve source and exact terminal
geometry. These phases are separate observations, not an FPS claim.

A public native editor probe on the exact sample isolates 5.8–8.7 ms point preview and about
7.1 ms scene composition. Each accepted movement performs one bounded native preview, no Intent
materialization and no history publication. Scene construction performs 65 computed evaluations
(one complete snapshot plus the 64 existing radius rails).

A temporary native bridge regression reproduces the complete delegated release and passes exact
terminal, independent validation, source/compiler retention, persistence restoration and Undo.
Its instrumented native publication costs are:

| Phase | Milliseconds |
| --- | ---: |
| Incremental native materialization | 134.6 |
| Exact native/computed parity | 2.7 |
| Editor checkpoint encoding | 233.7 |
| Fork accepted authority | 36.8 |
| Prepare code overlay | 86.9 |
| Apply code transaction | 181.0 |
| Total, including small staging/copy costs | 676.0 |

The checkpoint path repeatedly constructs, decodes and validates the same immutable Intent and
workspace data. Code publication validates a private prepared snapshot again after preparation
has already validated it. Complete session serialization also clones its entire history into an
owned wire value. These are bounded publication/transport concerns; no solver equation or new
numerical algorithm is required for this repair.

## Repair boundaries

Preserve independent native residual and computed-feature validation, exact terminal parity,
private prepared-input compare-and-swap, history/allocator consistency, wire-size limits and
strict imported-byte validation. Serialized formats and canonical bytes remain unchanged.
Keep every ordered semantic drag sample. Point previews may omit unchanged surrounding UI only
when the native route and identities establish that it is unchanged; failures, recovery,
selection, release and cancellation still publish the complete required UI.

## Evidence and limits

Raw evidence: `target/m94/drag/{baseline-summary.json,baseline/,baseline-more/,native-baseline.log,
native-terminal-baseline.log,native-terminal-instrumentation.patch}`. The diagnostic native
instrumentation was removed after its run. The reusable browser probes are
`target/m94/drag/{probe.mjs,direct-probe.mjs}` and the standalone public native source is
`target/m94/drag/native-probe.rs`.

Executed temporary native regression:

```bash
nix-shell shell.nix --run 'cargo test --locked --release -p geosolve-demo-web --lib m94_backplane_drag_profile -- --nocapture'
```

Result: 1/1 passed, 4.54 seconds test execution; the diagnostic test name is historical and was
removed with instrumentation. It is not a permanent regression target.

No two-second individual point-preview sample was reproduced. A tight power-bus corner picks its
computed Fillet rail, whose preview costs 180–288 ms in WASM (about 75–85 ms native), and follows
a different materialization path. A much larger source drag previews but rejects at release with
an explicit Problem and retains original authority; it is not counted as accepted movement.

The first headless Vulkan captures omit many line strokes despite populated frame data; those
captures do not establish full visual parity. Follow-up investigation identified M94-F004 below.
The independently timed synchronous WASM publication work remains valid; final drawing and
renderer timings must come from the corrected complete canvas.

## Implemented correction

- `workbench/bridge.rs` reuses the existing frame-only bridge response for successful ordinary
  Point continuations. Before/after native route, pointer owner, code/Intent identity, selection,
  interaction policy and diagnostic guards establish that omitted surrounding UI is unchanged.
  Every native movement and fresh scene remains; press, release, cancellation, error and recovery
  retain complete snapshots.
- `workbench/persistence.rs` and `code_projects.rs` capture a delegated editor checkpoint directly
  from validated live Intent authority, authenticate native evidence against it and encode the
  immutable local result. Four redundant Intent decodes and an ordinary-history serialization
  disappear. Imported bytes and ordinary snapshot encoding retain their strict validation.
- `geosolve-sketch-code/src/session.rs` validates all five private prepared-edit construction paths
  once, then reuses that immutable validation at publication. Live compare-and-swap, allocator and
  history consistency and exact wire-size limits remain. Borrowed canonical serialization avoids
  cloning the complete history; a streaming byte count avoids another allocated wire buffer.

No public API, serialized format, solver equation, numerical tolerance, branch choice or golden
row changes. The exact terminal remains independently validated before publication.

Permanent regressions cover frame/full-snapshot parity for every omitted UI section, unchanged
preview persistence, error/recovery/cancellation, exact dense-backplane terminal geometry and
source/compiler retention, cold restore and Undo. The persistence regression compares canonical
bytes with the former capture path and proves zero redundant Intent decodes. The code-session
regression compares complete legacy wire bytes, escaped Unicode and exact byte count, cold
restore, stale CAS and Undo/Redo, and proves publication adds no redundant snapshot validation.
Both redundant-work assertions were observed failing before their repairs.

## Provisional browser result

Same browser, hardware, dimensions and two ordered gestures as the baseline:

| Phase, milliseconds | Mount centre | Power-bus source |
| --- | ---: | ---: |
| Accepted pointer move, median WASM | 30.5 | 35.4 |
| Pointer-up WASM publication | 894.4 | 928.3 |
| Following persistence encoding | 358.3 | 364.3 |
| Release browser-action wall time | 1333.6 | 1364.4 |

This reduces ordinary point-preview WASM work by about 37–45% and release browser-action wall
by about 26–27%. Frame payloads fall from 1.069 million to 456 thousand UTF-16 characters.
These are small reproducible phase samples, not a sustained frame-rate benchmark. The remaining
release and autosave work is still material; this correction does not establish 60 Hz editing or
optimize the separately measured computed Fillet radius-drag path.

Evidence: `target/m94/drag/optimized-r1/{report.json,direct.json}` and screenshots alongside.
The exact final qualified artifact will receive the same measurement before delivery is recorded.

## Focused qualification

Commands run in the pinned Nix shell (logs in `target/m94/drag/`):

```bash
nix-shell shell.nix --run 'CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --locked -p geosolve-sketch-code --lib session::tests:: -- --test-threads=2'
nix-shell shell.nix --run 'CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --locked -p geosolve-demo-web --lib workbench::persistence::tests:: -- --test-threads=2'
nix-shell shell.nix --run 'cargo fmt --all -- --check && CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --locked -p geosolve-demo-web --lib workbench::bridge::tests:: -- --test-threads=2 && CARGO_BUILD_JOBS=4 cargo clippy --locked -p geosolve-demo-web -p geosolve-sketch-code --lib --tests -- -D warnings'
nix-shell shell.nix --run 'cargo fmt --all -- --check && CARGO_BUILD_JOBS=4 cargo clippy --locked -p geosolve-demo-web -p geosolve-sketch-code --lib --tests -- -D warnings && CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --locked -p geosolve-demo-web --lib point_preview_ -- --test-threads=2 && CARGO_BUILD_JOBS=4 npm --prefix crates/geosolve-demo-web/frontend run build:release-artifacts -- --out /home/arduano/programming/geometric-constraint-solver/target/m94/drag/provisional-r1'
```

Results: six code-session tests; 37 persistence tests; 61 bridge tests with one existing ignored
case; final two point-preview tests; formatting; focused warnings-denied Clippy; optimized WASM
and harness/production build all pass. The combined bridge command initially reached a Clippy
similar-name warning in the new persistence test after passing all bridge tests. Renaming its
local counter fixed the warning; the final command passes. A preliminary test-only counter used
an unsupported `LocalKey` method and was corrected before these passing runs.

The actual provisional production artifact also passes all four canvas browser tests:

```bash
nix-shell shell.nix --run 'GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18106/ GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/canvas-renderer.spec.ts --workers=1'
```

Result: 4/4 passed in 42.1 seconds, including actual WebGL2 point pixels/idle behavior,
DPR/resize/picking and drag/Undo, context loss, and lost-capture cancellation/next gesture.
This point-pixel suite alone does not resolve the separate polyline-stroke observation above.

## M94-F004 — context loss during first shader compilation

The visual audit independently reproduced a pre-existing renderer recovery defect on both the
F002 baseline and provisional F003 artifacts. On this Chrome/ANGLE run the context is lost during
initial batch-shader compilation. Pixi 8.20.1 generates empty uniform-upload functions from the
failed shader's missing active-uniform metadata. Restoration rebuilds GPU programs but keeps
those empty functions. Batched lines/grid/text consequently use zero GPU transform/color uniforms,
while independently drawn large point/circle outlines remain visible. Rust frame geometry,
tessellated path vertices and native accepted authority are intact.

A diagnostic no-batch path restores strokes. Resetting only Pixi's uniform-upload caches after
context restoration restores strokes while retaining batching. These experiments are browser
response instrumentation, not qualified production artifacts. The correction is confined to
renderer recovery, with a pinned-version compatibility seam and fresh retained presentation
resources after restoration. No native geometry/solver policy is involved.

Evidence: `target/m94/drag/visual-audit/{trace-deep.log,sync.json,sync-calls.json,no-batch.png,
context-cache-clear.png}`. The earlier point-pixel tests passed even with missing strokes; the
restoration regression now also needs real line-interior and label pixels, including loss during
first compilation. Focused correction and qualification outcomes will be recorded below.

The final first-shader regression fails on the F002 artifact with zero line-interior pixels
(`regression-before-r3.log`) and passes against the corrected frontend (`regression-after-r3.log`,
10.6 seconds). It strengthens the existing context-restoration browser case with a separate fresh
context and checks line-interior plus both X/Y label pixels; all prior lifecycle assertions remain.
The production backend resets the two version-checked Pixi 8.20.1 uniform-upload caches on the
first draw following restoration and recreates retained paint/text resources once. Ordinary GPU
batching and translation reuse remain enabled. Unsupported cache structure fails visibly.

```bash
nix-shell shell.nix --run 'npm --prefix crates/geosolve-demo-web/frontend run check:types && npm --prefix crates/geosolve-demo-web/frontend run test -- --no-cache src/lib/canvas-renderer-pixi.test.ts && GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18107/ GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/canvas-renderer.spec.ts --workers 1 --output /tmp/m94-f004-canvas'
```

Typecheck, five renderer resource tests and all four canvas cases pass (browser 48.0 seconds).
`visual-audit/f004-actual/before-wheel.png` also visibly restores the complete backplane, grid and
both datum labels with actual hardware rendering. Early test attempts interrupted Pixi's shader
capability probe and hung its synchronous retry loop; the final injector targets only the first
actual batch shader. A preliminary compiler declaration mismatch and one browser executable
launch failure were corrected before the decisive red/green and final focused checks. Those
failed attempts remain in the evidence directory; none is counted as a passing result.
