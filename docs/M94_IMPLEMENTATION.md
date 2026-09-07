<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94 implementation and qualification

Status: **initial implementation qualified on 2026-09-07; M94-F001 aspect-ratio correction in progress**. [M94_GOALS.md](M94_GOALS.md) owns the approved contract. Supervising-user acceptance remains pending.

Baseline source: `37e39159790094e4b4fa0e8f9dac0a1c636d38bf`. The accepted M92 service and
immutable artifact remain unchanged. Source, browser and image evidence will be recorded under
`target/m94`; no planned test or qualification result is represented as passed.

Parallel ownership: Rust drawing composition; TS/PixiJS renderer; baseline/browser migration;
root bridge/protocol, acceptance documentation and integrated qualification. Cargo/npm writers
are coordinated and the full gate runs only after the integrated candidate stabilizes.

## M94-F001 — canvas aspect ratio and mouse coordinates

User report against the initial Tailscale candidate: the canvas retains a fixed aspect ratio
instead of adapting to its available dimensions, as did the preceding SVG implementation.
Reproduced at source `9dfc0b216036f49803f7035dd1321ec9872fe42d`, with qualified product
`85c57f1e0ac125195aaac69c3f6db5e2f7f2c46d` served at `http://100.94.63.83:18096/`.
No project payload was supplied. The existing real-WASM DPR/resize browser case independently
observes a 1000 px drawing width inside an 871.921875 px CSS canvas. Its new full-size assertion
fails against the frozen original artifact in 7.1 seconds.

Classification: `DEFECT` in presentation camera/bridge ownership, with no solver-equation change.
`CanvasCamera::viewport()` fixes the scene at 1000×700; `resize_json` updates input host extents
but never the camera or drawing. Pixi and pointer normalization therefore consistently letterbox
the same obsolete plane. The correction must share actual CSS dimensions between camera, fit,
paint and pointer routing; preserve uniform geometric scale, accepted authority and history;
and keep DPR confined to rasterization. Focused camera/bridge regressions and the existing
browser lifecycle case own this correction. The 271-row mathematical oracle remains unchanged.

Reproduction command (expected failure):
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18096/ npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/canvas-renderer.spec.ts --grep 'M94 canvas aligns DPR' --workers=1`.
The repair gives `CanvasCamera` a validated live extent and a `resize` method; reset, fit and
retained transforms preserve/use it. The bridge publishes a reprojected frame on CSS resize,
rolls back any captured provisional gesture, and preserves accepted documents, history and
revision. Restored content fits the first measured host once; subsequent resizes preserve
centre/zoom. Import adopts the existing host and refits; DPR-only changes remain frame-free.
Static SVG/PNG exports retain their canonical default dimensions.

Focused commands through `nix-shell shell.nix --run` pass:

- `cargo test --locked -p geosolve-demo-web --lib workbench::bridge::tests::resize -- --nocapture`
  — 3/3 (full-frame/state retention, former-margin picking/zoom, captured-drag cancellation).
- `cargo test --locked -p geosolve-demo-web --lib restored_canvas_fits_first_measured_host_and_then_retains_zoom -- --nocapture`
  — 1/1; the existing `successful_project_import_retains_live_viewport_and_pointer_alignment`
  regression also passes.
- `cargo test --locked -p geosolve-sketch-render camera_ -- --nocapture` — 7/7.
- `cargo fmt --all` and focused renderer/demo Clippy with
  `--all-targets --all-features -- -D warnings` — pass.
- `npm --prefix crates/geosolve-demo-web/frontend run check:types` — pass.

Logs are in `target/m94/aspect/{focused-tests,bridge-integration-tests,clippy}.log`.
An initial fixture picked a nearby higher-priority Fillet grip at low zoom; the isolated point
fixture now separates those legitimate owners. An overly broad `canvas` test selector was stopped
after it selected all bundled samples; the exact owner filters above replace that incomplete
development run.

The development release build (`npm --prefix crates/geosolve-demo-web/frontend run
build:release-artifacts -- --out /tmp/geosolve-m94-f001-development`) passes with
`CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_INCREMENTAL=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
inside the Nix shell. The same compiler-harness and production build scripts remain in use.

All four canvas lifecycle cases pass against its production server at `http://127.0.0.1:18097/`:
the initial focused run passes idle/pixels and lost capture; the replacement context-loss case
passes; the expanded DPR/aspect case passes in 27.1 seconds. Exact frontend command:
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18097/ npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/canvas-renderer.spec.ts --workers=1`;
replacement filters are `--grep 'aligns DPR|context loss'` and `--grep 'aligns DPR'`.
Logs/traces remain under `target/m94/aspect/green-browser*`; earlier failing batches remain failed.
The new mouse fixture exposes the former margins by hiding Explorer, accounting for retained
pane proportions after a narrow window. The context-loss fixture zooms out so its unchanged
complete-marker pixel assertion does not sample naturally clipped geometry after zooming in.
No pixel or coordinate tolerance was loosened.

Six full-page screenshots of Jansen and the manifold at 1440×900, 2200×800 and 1280×1200 were
captured and visually inspected under `target/m94/aspect/visual`. Their recorded logical drawing
extents equal the measured CSS canvas extents, including fractional widths; geometry retains
uniform scale and the fitted scenes use the available proportions. The expanded browser case
also checks exact centred positions, fixed point radii, persisted-state retention, real segment
creation, picking, free-point drag/Undo, cursor-anchored zoom and exact pan displacement in both
former-margin orientations at DPR 2. Integrated qualification and replacement delivery remain pending.

The first integrated repair run, `20260907T120422-361fd1ed` on `309f401`, was intentionally
interrupted after an additional visual check exposed another fixed-size camera assumption.
At a 771.109375×1120 CSS canvas, fitting either 360 mm scale sample required less than the
canonical 2 px/unit minimum. Fit rejected and reset to 50 px/unit, displaying a close-up.
The native reproduction `camera_fits_scale_samples_in_narrow_canvas_and_zoom_never_reverses_direction`
failed, matching the screenshots in `target/m94/aspect/dense-visual`.

The minimum camera scale now follows the usable extent inside the fit margins, preserving the
canonical maximum visible model span and the exact default static-export bounds. A retained
zoom below a newly enlarged viewport's floor never snaps inward during zoom-out. The owner
regression covers both scale-sample bounds at fractional narrow, split-pane and small extents;
all 36 renderer tests pass, including unchanged native SVG/PNG and uncontainable-export checks.
`cargo test --locked -p geosolve-sketch-render` and focused warnings-denied Clippy run through
the Nix shell; logs are `target/m94/aspect/{red-narrow-fit,narrow-fit-checks,narrow-fit-clippy}.log`.
The interrupted gate is incomplete; only authenticated unaffected passing stages may be reused.

## Implementation checkpoint

The live bridge now emits transient protocol v2 with finite numeric `DrawFrame` primitives;
its composer does not construct SVG. PixiJS 8.20.1 consumes this presentation output through one
persistent WebGL2 canvas, retaining native pointer routing, popovers and controls. Existing native
SVG/PNG APIs and persisted protocol versions are unchanged. Browser witnesses use prefix v2 and
leaf v3, complete saved state/source/drawing hashes and actual canvas screenshots/point pixels.
Only stable surface/GPU diagnostics enter initial-state identity; counters/timings remain telemetry.

The immutable accepted baseline contains 16 samples and 35 states, with all ten artifact and HTTP
files authenticated. Evidence: `target/m94/baseline/provenance.json`. Chromium 151 defaults to
SwiftShader; a separate bounded probe acquired the NVIDIA RTX 3090 through ANGLE Vulkan. Hardware
performance will be distinguished from the default software qualification environment.

Focused development checks completed (not a release nomination):

- `nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch-render'` — 33 tests pass;
  focused all-targets Clippy with `-D warnings` and renderer rustfmt also pass.
- `nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --lib frame'` — 9 pass,
  including rejected-source/cached-frame retention and resize authority.
- Frontend `vitest run --no-cache src/lib/wasm-adapter.test.ts src/lib/pending-managed-mutation.test.ts`
  — 13 pass; `src/App.test.tsx` — 41 pass. Renderer owner's focused 15 tests passed; a subsequent
  hidden-viewport regression and viewport rerun passed 14 tests across those two files.
- `python -m unittest scripts.tests.test_release_browser_reuse -q` — 17 pass, including rejection
  of SVG/blank/lost-context witnesses and invalidation on changed pixels or GPU identity.
- `node crates/geosolve-demo-web/frontend/scripts/check-runtime-licenses.mjs` — 83 packages pass;
  GPL-compatible BSD-3-Clause notices and pinned Pixi dependency notices reviewed and included.
- `nix-shell shell.nix --run 'node crates/geosolve-demo-web/frontend/scripts/build-wasm.mjs --release'`
  — optimized WASM/bindings pass. This is a development artifact, pending integrated qualification.

Visual review, real-browser lifecycle/sample checks, latency measurement and clean integrated gate
remain pending at this checkpoint. Existing mathematical golden bytes are unmodified.

## Browser integration and visual review

The first production browser run found an initialization assumption missed by unit doubles:
`manageImports:false` intentionally leaves Pixi's optional event system absent. The renderer now
checks its presence before detaching it. Selection glows additionally require an explicit
`pixi.js/filters` import when automatic imports are disabled. These are browser renderer setup
failures; no domain/accepted-scene defect or golden expansion was identified. The initial four
lifecycle failures remain in `target/m94/lifecycle-results`; the independent corrected run passes
all four in 26.1 seconds (`/tmp/m94-lifecycle-tests-r3.log`). A brief development-server attempt
used the repository's default mock adapter and is harness-only evidence, never WASM qualification.

`m94-capture-canvas.mjs` captured all 16 samples and 35 states. Side-by-side review of
`target/m94/comparison-{0,1,2,3}.png` found the same mechanical geometry, dimension positions,
features, construction marks and fitted layout. The initial raster showed rougher thin diagonals
and heavier small text; low-DPR canvases now supersample at a minimum 2× resolution, preserving
CSS pointer coordinates. The background matches the existing UI (`#151619`). A fresh Jansen
capture in `target/m94/canvas-raster2` confirms the smoother diagonals and matching text. Text
textures remain cached when only the native-authored position/rotation changes. The actual raster
resolution is exposed alongside CSS DPR in stable diagnostic evidence.

Five focused real-WASM workbench cases pass (58.8 seconds): rejected source retains its complete
frame; scissor-lift constrained dragging; Parallel authoring; computed Fillet authoring; normal
pointer release plus Circle radius editing. Command: frontend `playwright test
 tests/e2e/workbench.spec.ts --grep 'normal pointer capture release|invalid source retains|computed
 Fillet authoring|canonical scissor|non-axis Parallel' --workers 1`, with the candidate URL and
pinned Chromium environment. The release gate still owns the complete two-edit/history sample
inventory and remaining suites.

## Reviewed release input boundaries

M94 retains the M93 scheduling policy. The explicit program audit covers the entire changed
source map, including tests, lockfiles, renderer, adapter, browser witness and new capture tools:

- New Rust drawing composition is pure presentation over public scene projections, with no
  solver equations, filesystem operations, runtime fixture lookup or shared scratch writer.
  Existing SVG helpers only gain crate visibility; native export behavior stays intact.
- Bridge/WASM edits change transient transport version and presentation assertions. Existing
  persistence, compiler and test-runtime ownership remain; no new concurrent writer is added.
- New Pixi resources are browser-local. Browser helpers observe the last submitted frame and
  send real pointer events; screenshots/attachments use existing private case/run outputs.
  Standalone M94 capture/performance scripts run only when explicitly invoked and are not
  imported by the release test inventory, package builds or native tests.
- Browser prefix v2/leaf v3 rejects old SVG evidence and retains complete workspace/source/frame
  identity plus actual pixels and stable surface/GPU identity. Volatile telemetry is explicitly
  separate. All existing 38 browser obligations remain; four canvas lifecycle rows are added.
- Golden adapters still own their exported fixtures and never select catalog samples. The
  mathematical oracle is unchanged. Static frontend/catalog ownership remains unchanged.

These observations justify refreshing the existing golden/browser/static and native/frontend
build-overlap program pins after the final source audit. They do not authorize reuse across
changed inputs; changed lockfiles and global witness contracts conservatively invalidate prior
results. Native preparation remains immutable and Cargo/npm writers remain serialized under
M93's documented scheduling boundaries.

## Hardware timing observation before qualification

`M94_HARDWARE=1 node scripts/m94-performance.mjs` was run separately against the immutable SVG
baseline and the final canvas development build, using Chromium 151 / NVIDIA RTX 3090 / ANGLE
Vulkan. Each case uses 24 alternating camera-wheel inputs, with browser trace paint tasks and
renderer submission CPU recorded separately. End-to-end observation includes input transport and
two RAF boundaries; it is not an FPS benchmark or isolated GPU execution time.

| Sample | SVG end-to-end p50 ms | Canvas end-to-end p50 / p95 ms | Canvas submission CPU p50 / p95 ms |
| --- | ---: | ---: | ---: |
| Jansen | 48.2 | 55.9 / 60.5 | 4.7 / 9.2 |
| Manifold | 209.7 | 212.7 / 240.6 | 10.3 / 17.6 |
| Perforated field | 332.5 | 292.6 / 392.6 | 42.7 / 49.0 |
| Harness backplane | 83.3 | 85.0 / 111.7 | 17.3 / 22.9 |

The raster-quality refinement preserves useful ordinary-sample submission times; dense samples
and the existing end-to-end pipeline do not sustain 60 Hz in this observation. Jansen's observed
end-to-end median rose 7.7 ms while submission CPU improved; this single tracing run cannot isolate
scheduler/compositor timing from GPU workload, so it is not presented as a uniform speedup. The
first canvas build measured 37.8 ms for Jansen before the quality refinement. These limits remain
explicit for review; this milestone does not silently broaden into solver or camera-authority
optimization. Raw system details, all samples and paint/task totals are in
`target/m94/performance-{svg-hardware,canvas-hardware,canvas-hardware-final}.json`.

## Qualification attempt and presentation parity

Clean source `c3431f0` ran `nix-shell shell.nix --run './scripts/release-gate.sh'`; run
`20260907T093227-241945f1` passed inventory, format/metadata, managed and frontend stages, then
stopped in catalog preflight on an obsolete `38 tests in 4 files` assertion. This is a harness
inventory mismatch. The repaired check reads the reviewed release inventory and compares every
shared `(file, title, project)` identity, retaining exact sample membership and memory grouping.
Its focused `node --test --test-name-pattern='prepared Playwright discovery'
scripts/test-release-artifact.mjs` run passes. The repaired source map is re-reviewed for the same
pins: only that read-only preflight checker changed; no concurrent writer or test body was added.

An independent offline baseline comparison checks 2,072 painted native/computed curve and point
items, persistent owners and 26,876 numeric coordinates/radii across all 16 fitted samples.
Maximum difference is `0.0005` logical pixels, exactly the previous SVG's three-decimal rounding.
The first comparison included the SVG's 70 invisible `.wb-computed-hit` DOM paths; those are
intentionally absent from the new presentation because native Rust still owns picking. Excluding
only those unpainted duplicates yields exact item/coordinate-shape and owner correspondence for
every sample. Receipt: `target/m94/baseline-canvas-geometry-comparison.json`.

## Qualification harness corrections

Run `20260907T093757-164f743d` on `3bcb0d7` resumed the initial inventory repair.
Format, warnings-denied Clippy, preparations, completed native suites, documentation, benchmark
compilation and production transport passed. The runner stopped scheduling after WASM lifecycle
failed, preserving independent running browser evidence: 17/17 catalog/sample prefixes and 36/41
full browser cases passed; five sample rows failed. This attempt remains failed and incomplete.

The three failing WASM assertions still expected SVG class tokens. The independent numeric
owner-frame comparison already passed; the repair asserts typed accepted provenance and the
exact persistent point's selected stroke plus selection-only halo at pointer-down and release.
It also checks that selection preserves the point's identity and geometry. Source, authority,
finite validation, rank/DOF, deselection and complete history assertions remain intact.

All five terminal browser failures were exact string comparisons of numeric fitted geometry:
Jansen Undo, scissor isolation restoration, Gridfinity Redo, and vacuum/Voron reload. Independent
trace comparison checked all 1,513 numbers with identical structure and nonnumeric values; maximum
difference was `2.2737367544323206e-13` logical pixels. SVG previously rounded these coordinates
to three decimal places. A test-only comparator now requires exact structure/order/kinds and uses
an absolute `1e-9` logical-pixel tolerance, with exact radian rotations and malformed/nonfinite
rejection. The same comparison guards inequality, preventing rounding noise from qualifying a
no-op edit. Raw captures, complete workspace/source hashes and canonical drawing/pixel witnesses
remain unrounded and unchanged. These are harness migration corrections, not mathematical defects
or changes to golden expectations.

The existing global overlap/equivalence pins are deliberately not refreshed for this isolated
test repair. The runner must authenticate unaffected successes and use its conservative build
locks wherever the prior source audit no longer matches; no manual result bypass is authorized.

Focused repair qualification passes:

- `nix-shell shell.nix --run 'cargo fmt --all -- --check'`.
- Frontend `npm test -- --no-cache tests/fitted-geometry.test.ts` — 19/19; the helper also
  accepts every captured terminal pair within the measured bounds above.
- Frontend `npm run test:e2e -- tests/e2e/m92-sample-audit.spec.ts --workers=1 --grep
  'M92 visual workflow: (theo-jansen-leg|five-stage-scissor-lift|gridfinity-bin-section|vacuum-fixture-plate|voron-panel)$'
  --output=/tmp/m94-browser-harness-repair-results` against the existing local production
  candidate — 5/5 in 3.0 minutes.
- `nix-shell shell.nix --run 'CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_INCREMENTAL=true
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
  cargo test --locked --release -p geosolve-demo-web --lib actual_wasm_ --target wasm32-unknown-unknown'`
  — 3/3 in 107.65 seconds, after 43.55-second compilation. The default-feature WASM test build
  reports existing dead-code warnings; the integrated warnings-denied workspace Clippy remains required.

Logs: `/tmp/m94-{fitted-geometry-unit,browser-harness-repair,wasm-lifecycle-repair}.log`.

Run `20260907T100735-2c84bd07` on `8220cf8` passed every native workspace/headless
obligation and optimized WASM preparation, then failed browser preparation with TS6307: the new
helper was outside the composite `tsconfig.node.json` file list. Direct strict TypeScript checks
had not exercised that project-reference boundary. Adding the exact helper file to the existing
include list fixes the build contract; `nix-shell shell.nix --run
'npm --prefix crates/geosolve-demo-web/frontend run check:types'` passes. This configuration-only
repair changes no test assertion or product behavior. Native/WASM successes remain eligible for
authenticated reuse in the replacement integrated run. Log: `/tmp/m94-composite-types-repair.log`.

## Qualified candidate — 2026-09-07

Product source `85c57f1e0ac125195aaac69c3f6db5e2f7f2c46d`, tree
`34632b1f2b73462b1b0fa184d46be12b35e68520`, passes:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260907T100735-2c84bd07'
```

Signed run `20260907T101813-cd18572a` records **241/241 passing stages**, 25 freshly executed
and 216 authenticated reused successes, in **1289.36 seconds (21m29s)**. Its complete, clean-source,
source-unchanged and qualified-release flags are true. The nomination audit also authenticated
every individual passing receipt. Coverage includes format, warnings-denied Clippy, all native
workspace/headless obligations, optimized WASM/build/lifecycle and interaction parity, Rustdoc,
package archives, licences, release ceilings, golden, browser and exclusive performance gates.

The mathematical oracle is **271/271**, with its reviewed TSV unchanged. Browser prefix coverage
is 17/17 (catalog plus all 16 sample opens), followed by 41/41 complete cases: the full reviewed
42-case inventory is covered without retries, flakes or skips. Every sample completes both edits,
Undo/Redo and reload; Jansen additionally completes its numeric constrained-drag history check.
The four canvas lifecycle cases cover real pixels/idle frames, DPR/resize/hidden layouts, context
loss/restoration and lost pointer capture. The final performance stage passes in 136.1 seconds;
its 256-moving-body independent-validation crossover passes in 130.60 seconds.

### Files/API and mathematical behavior

- `geosolve-sketch-render::drawing` adds the finite `DrawFrame`/`DrawItem` presentation schema and
  `compose_draw_frame`; the live adapter emits bridge v2 numeric scene output.
- Frontend `canvas-scene.ts`, `canvas-renderer.ts`, `canvas-renderer-pixi.ts`,
  `canvas-renderer-geometry.ts` and `canvas-viewport.tsx` own validated immutable inputs, retained
  WebGL2 resources, drawing and native input/lifecycle routing.
- Canvas browser observers/witnesses, lifecycle/sample tests, capture/performance tools and licence
  notices support qualification. The geometry comparator is test-only.
- No solver equation, residual, rank/DOF, branch, domain model or hard/soft behavior changed. Rust
  remains authoritative; native SVG/PNG exports and persisted formats retain their prior contracts.

### Exact production visual and runtime evidence

The prepared production payload remains byte-identical across the test-only corrections. Its
12 files were served at `http://127.0.0.1:18096/` during the final run and captured with:

```bash
nix-shell shell.nix --run 'GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18096/ M94_CAPTURE_OUTPUT=target/m94/production-review node crates/geosolve-demo-web/frontend/scripts/m94-capture-canvas.mjs'
nix-shell shell.nix --run 'GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18096/ M94_CAPTURE_OUTPUT=target/m94/production-hardware M94_CAPTURE_SAMPLE=theo-jansen-leg M94_HARDWARE=1 node crates/geosolve-demo-web/frontend/scripts/m94-capture-canvas.mjs'
```

The first run captures **16 samples/35 states** with Chromium 151/SwiftShader. Fitted comparisons
`target/m94/production-comparison-{0,1,2,3}.png` preserve the accepted geometry, dimension positions,
construction marks, controls and layout; the refined raster matches the SVG background and keeps
thin lines legible. Interactive state pairs are in `production-state-comparison-{0,1,2}.png`.
The independent final geometry audit compares 2,072 painted items and 26,876 scalars: maximum
difference `0.0005` logical pixels, the old SVG rounding, with exact item/owner correspondence.
Receipt: `target/m94/baseline-production-geometry-comparison.json`.

The hardware capture draws four actual production states through **NVIDIA RTX 3090 / ANGLE Vulkan /
WebGL2**. `target/m94/production-gpu-unavailable.json` additionally authenticates all 12 local/HTTP
files and passes native error, code/parameter editing, complete-source canonical export, save and
reload with only WebGL2 context creation forced to return null. No frame is fabricated and no page
error occurs. Executed command: `nix-shell shell.nix --run 'node /tmp/m94-gpu-unavailable.mjs
--endpoint http://127.0.0.1:18096/ --artifact-dir target/release-gate/prepared/d3985da5e2bcaf5781dad3309fe6971e9df88db2401f163402fc202b1eeebe25/browser/geosolve-production
--output target/m94/production-gpu-unavailable.json'`. The exact script is preserved at
`target/m94/gpu-unavailable-check.mjs`; its evidence records the invocation and script SHA-256.

### Frozen delivery and acceptance boundary

The read-only snapshot is `/tmp/geosolve-m94-uat.jkpamgan/geosolve-production`; its original
manifest and signed qualification receipt are preserved alongside it. Nomination record:
`target/m94/nomination.json`. It contains **12 files / 27,573,549 bytes**, files aggregate
`639eea21e62ef1244b18ba51f4938b5baf80d7b31455c0bee2050540f633dc1d`, original manifest SHA-256
`f2e222ae2948762391118723142aaa452529697f596522683d74397abf25e2ea`. No rebuild supplies the
nomination. Exact moved-copy/endpoint verification passes:

```bash
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome npm --prefix crates/geosolve-demo-web/frontend run verify:artifact -- --manifest /tmp/geosolve-m94-uat.jkpamgan/production.json --directory /tmp/geosolve-m94-uat.jkpamgan/geosolve-production --url http://127.0.0.1:18096/ --receipt /home/arduano/programming/geometric-constraint-solver/target/m94/frozen-endpoint.json'
```

Every file and `/` byte/MIME check and bounded real-WASM readiness pass. An earlier standalone
endpoint attempt used Playwright's unpinned cached browser and failed to load system libraries;
its failed receipt remains `production-endpoint.json`. The pinned Chromium replacement and frozen
copy verification pass without any product change.

The local candidate is **ready for supervising-user acceptance**. M94 remains open. The accepted
M92 service and external publications are unchanged. The documented dense-scene/end-to-end
60 Hz limitation remains: latency measurements used the development compiler-harness build,
while final production hardware/pixel checks establish actual acceleration and visual behavior.
No uniform speedup or 60 Hz qualification is claimed. The conservative stale-pin behavior noted
above remains explicit; future audit-pin refresh requires the normal source review.

Documentation-only nomination notes preserve the qualified product identity. Their closeout
checker is `./scripts/release-gate.sh --docs-only --since 85c57f1`; this checks prose diff, links
and input ownership without rebuilding or repeating product qualification.
