<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94 implementation and qualification

Status: **in progress**. [M94_GOALS.md](M94_GOALS.md) owns the approved contract.

Baseline source: `37e39159790094e4b4fa0e8f9dac0a1c636d38bf`. The accepted M92 service and
immutable artifact remain unchanged. Source, browser and image evidence will be recorded under
`target/m94`; no planned test or qualification result is represented as passed.

Parallel ownership: Rust drawing composition; TS/PixiJS renderer; baseline/browser migration;
root bridge/protocol, acceptance documentation and integrated qualification. Cargo/npm writers
are coordinated and the full gate runs only after the integrated candidate stabilizes.

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
