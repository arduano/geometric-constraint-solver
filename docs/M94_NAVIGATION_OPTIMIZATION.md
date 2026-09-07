<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94-F002 — dense-fixture navigation optimization

The user authorized implementation of the measured pan, zoom and hover optimizations.
The [diagnosis](M94_PERFORMANCE_DIAGNOSIS.md) and the original immutable M94-F001 candidate remain
historical evidence. The correction is implementation-qualified and byte-verified at `http://100.94.63.83:18096/`; M94 still awaits supervising-user acceptance.

## Implementation

- Rust classifies strictly transient Select navigation independently of browser event batching.
  These operations return `{version:2,kind:"frame",revision,frame}` with the existing complete
  numeric frame. Unchanged Select hover still authenticates the scene and performs native picking,
  then returns `null`. Selection, authoring, semantic gestures, pending compilation, errors and
  cancellation that can affect document/UI state retain full workbench publication.
- Camera changes retain authenticated scene inputs and use the existing public projection owner.
  The same pixel chord tolerance and CSS-sized annotations/controls are recomputed. Default
  visibility avoids an unused Explorer projection. Frame composition borrows the retained scene;
  serialization borrows its accepted frame. No seal or validation is weakened.
- The existing `wheel` boundary additionally accepts `{version:2,samples:[WheelRequest,...]}` with
  1–256 ordered samples. Rust validates the entire batch before publication and applies each
  camera operation, including its individual anchor and clamp. Invalid tails are atomic.
- The browser retains non-canvas snapshot references and sends authenticated canvas-only updates
  directly to the renderer. Adapter-local monotonic publication stamps prevent delayed React
  commits or input callbacks from painting an older camera. Immutable decoded drawing trees are
  validated/frozen once; mutable renderer callers remain isolated.
- Input RAF coalescing retains the latest idle Select hover and fixed-origin middle-pan movement.
  Wheel batches retain every sample. Primary solver dragging, authoring and terminal events keep
  their sample order. Command/resize boundaries drain navigation; hidden/disposed/replaced hosts
  retire queued transient work. Current synchronous WASM execution remains the browser authority.
- Pixi retains local graphics under exact translations, including text, markers, dashes and
  shadows. Unchanged native paint order performs no child-reordering search. Direct shape/style
  comparisons replace JSON signatures. Changed geometry, zoom-dependent CSS strokes and DPR
  still repaint as required. `repainted` telemetry distinguishes rebuilds from transform updates.

No solver equation, numerical tolerance, branch, persistence format, sample or native SVG/PNG
export behavior changes. The transient v2 Rust/JS consumers ship together.

## Focused evidence

The first two new native regressions failed before repair. Four owning bridge regressions cover
complete persistence, full-frame parity, picking after camera changes, hidden geometry retention,
full selection/error/cancellation publication and exact batch/clamp/invalid-tail behavior.
The complete bridge suite passes **58 tests**, with one pre-existing ignored test. Focused Rustfmt
and warnings-denied Clippy pass. Renderer, adapter and input suites cover immutable boundary
ownership, real paint changes, exact transforms, queue order, stale publications and disposal.
The four existing canvas browser cases pass on the provisional optimized artifact: actual pixels
and idle, aspect/DPR/resize/picking, context loss/recovery and lost capture.

Executed commands and local logs:

```bash
nix-shell shell.nix --run 'CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --locked -p geosolve-demo-web --lib workbench::bridge::tests:: -- --test-threads=2'
nix-shell shell.nix --run 'CARGO_BUILD_JOBS=4 cargo clippy --locked -p geosolve-demo-web --lib --tests -- -D warnings'
npm --prefix crates/geosolve-demo-web/frontend test -- src/lib/canvas-renderer.test.ts src/lib/canvas-renderer-pixi.test.ts
npm --prefix crates/geosolve-demo-web/frontend test -- src/components/canvas-viewport.test.tsx
nix-shell shell.nix --run 'npm --prefix crates/geosolve-demo-web/frontend test -- --no-cache src/lib/wasm-adapter.test.ts'
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome GEOSOLVE_E2E_BASE_URL=http://127.0.0.1:18106 npm --prefix crates/geosolve-demo-web/frontend run test:e2e -- tests/e2e/canvas-renderer.spec.ts --workers=1'
```

Owner details: `/tmp/m94-bridge-result.md`, `/tmp/m94-bridge-tests.log`,
`/tmp/m94-input-done.txt`. Browser log: `target/m94/navigation/canvas-focused-r2.log`.
Two intermediate TypeScript checks found test-double typing errors, subsequently repaired.
The first provisional artifact build stopped at that check; its successful WASM compilation
was not a successful browser build. The second provisional build passes.

## Preliminary measured comparison

Chromium 151, actual NVIDIA RTX 3090 / ANGLE Vulkan / WebGL2, 1440×900, CSS canvas
871.921875×820, DPR 1 with unchanged 2× supersampling. Sixteen wheel and sixteen pan movements
per fixture use the same probe on the old immutable candidate and provisional optimized build.
Numbers are median milliseconds; bridge and GPU-submission CPU phases are measured separately,
not a promise of FPS or cross-device latency. The bridge measurement is the actual synchronous
WASM call; renderer time excludes that call and browser decode/scheduling.

| Operation | Field before → optimized | Backplane before → optimized |
| --- | ---: | ---: |
| Wheel bridge | 207.5 → 18.1 | 62.1 → 9.2 |
| Pan bridge | 219.7 → 18.0 | 58.0 → 6.5 |
| Empty-hover bridge | 139.2 → 6.4 | 40.6 → 2.6 |
| Wheel renderer submission | 45.7 → 20.6 | 19.4 → 9.4 |
| Pan renderer submission | 42.8 → 13.8 | 19.0 → 8.2 |

Independent unprofiled direct WASM handles preserve exact complete persistence and return four
characters (`null`) for unchanged hover. Both actual UI runs have zero page errors and zero idle
redraws. Before/after captures were visually reviewed for both fixtures. The additional removal
of a draw-composition scene clone follows this preliminary measurement; no extra gain is claimed
for it until final measurement.

The probe and raw reports are `target/m94/navigation/probe.mjs`, `baseline/report.json` and
`optimized-r2/report.json`; adjacent PNGs and logs retain both runs. A supplemental remaining-pan
CPU profile is diagnostic only, with its own observer overhead.

## Remaining limits

The largest field still exceeds a 16.7 ms frame budget. Native reprojection, JSON transport and
changed polyline stroke buffers remain costs. Exact local-coordinate equality conservatively
rebuilds polylines when camera roundoff changes their relative coordinates. Changing zoom can
change adaptive tessellation and must retain CSS stroke sizes. No drawing precision, raster
quality or validation was reduced to improve these numbers.

A pre-existing owner mismatch rejects retained scenes after point rows are hidden, so those
camera operations safely rebuild their filtered scene. Hidden items remain absent from drawing
and picking. This pass does not weaken that owner check. Native parameter publication remains a
separate expensive operation, as recorded in the diagnosis. A future genuinely asynchronous
adapter would need command-wide serialization; input queue ordering alone is not that contract.
M94 remains open for supervising-user acceptance.

## Final qualification and delivery — 2026-09-07

Product source **`93fdcbf843ed88b80cb2a7813c5b3f66a8ce5d81`**, tree
`c0b6ba21fcbeb132260b04e298abcf1862937c85`, passes the clean integrated run
**`20260907T143802-fee2e639`** in **21m57s**. All **241 stages** are accounted for:
**23 fresh passes and 218 authenticated reused passes**. This includes format, workspace
warnings-denied Clippy, affected native tests, actual WASM lifecycle, builds/artifact checks,
unchanged **271/271** golden rows and browser qualification. The browser run has **17 successful
catalog/open checks** and **41 full cases**, including all **16 sample edit/history workflows**,
with no skipped, flaky or unexpected results. The performance stage retains its authenticated
unchanged-owner evidence; the new navigation measurements below are separate fresh observations.
The final focused frontend run passes **37/37**; the last scene-borrow refactor passes its five
navigation-filtered native tests. Full-gate source and receipts are unchanged and authenticated.

The frozen production artifact is `/tmp/geosolve-m94-f002-uat.ompe59ry/geosolve-production`,
with manifest `/tmp/geosolve-m94-f002-uat.ompe59ry/production.json`: **12 files, 27,619,409 bytes**,
aggregate **`cd6b351b29fd6e802b7b5ba0f8e1b783f6c105d274b924f93e4528b2e6766d5b`**.
Root plus all 12 served paths byte/MIME-match, and actual WASM startup/manifold rendering passes
without page/console/request errors. Receipt: `target/m94/navigation/tailscale-final.json`.
The old M94-F001 nomination is preserved at `target/m94/navigation/nomination-original.json`;
its frozen files and the accepted M92 service at port 18092 remain unchanged.
The temporary provisional listener at localhost 18106 is retired.

A fresh run against the **exact final served artifact** confirms the improvements on the same
NVIDIA RTX 3090 / Chromium 151 / ANGLE Vulkan / WebGL2 host and unchanged raster resolution:

| Median milliseconds | Field baseline → final | Backplane baseline → final |
| --- | ---: | ---: |
| Wheel bridge | 207.5 → **17.2** | 62.1 → **9.1** |
| Pan bridge | 219.7 → **17.9** | 58.0 → **5.4** |
| Empty-hover bridge | 139.2 → **6.4** | 40.6 → **3.6** |
| Wheel renderer submission | 45.7 → **19.0** | 19.4 → **10.8** |
| Pan renderer submission | 42.8 → **13.2** | 19.0 → **5.7** |

Both final UI runs have zero page errors and zero idle redraws. Independent direct WASM handles
preserve exact complete persistence; their median hover/wheel calls are 3.9/17.2 ms for the field
and 1.8/4.65 ms for the backplane. Those separate probes exclude React/Pixi and are not additive
with the UI measurements. Final before/after screenshots were visually reviewed for both
fixtures. An optional Python pixel-difference attempt could not run because Pillow was absent;
no pixel-byte-equivalence claim is made. Raw final data and captures are under
`target/m94/navigation/final/`. The remaining limits above still apply, particularly the field's
frame budget and the separate native edit-publication path.

Executed final commands:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260907T131046-91e1a335'
python3 /tmp/freeze-m94-f002.py 20260907T143802-fee2e639
nix-shell shell.nix --run 'node /tmp/m94-f002-servers.mjs /tmp/geosolve-m94-f002-uat.ompe59ry/production.json /tmp/geosolve-m94-f002-uat.ompe59ry/geosolve-production'
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome npm --prefix crates/geosolve-demo-web/frontend run verify:artifact -- --manifest /tmp/geosolve-m94-f002-uat.ompe59ry/production.json --directory /tmp/geosolve-m94-f002-uat.ompe59ry/geosolve-production --url http://100.94.63.83:18096/ --receipt /home/arduano/programming/geometric-constraint-solver/target/m94/navigation/tailscale-final.json'
nix-shell shell.nix --run 'NAV_URL=http://127.0.0.1:18096/ NAV_MANIFEST=/tmp/geosolve-m94-f002-uat.ompe59ry/production.json NAV_OUTPUT=/home/arduano/programming/geometric-constraint-solver/target/m94/navigation/final timeout 240 node /tmp/m94-navigation-probe.mjs'
```

Logs and authenticated freeze summary: `target/m94/navigation/{release-gate.log,freeze.json,
tailscale-final.log,final.log}`. Final prose is qualified separately with the documentation-only
path; it does not rebuild or replace these qualified product bytes.
