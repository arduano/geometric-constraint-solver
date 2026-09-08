<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98-B accepted baked-profile export handoff

Owner scope: the [coordinated v1 contract](M98_BAKE_CONTRACT.md), 2026-09-08.
Status: **PROTOTYPE_READY_FOR_UAT**. M98 remains a prototype; no release qualification or
milestone acceptance is claimed.
This follow-up starts at `692b13d4f807b491cfec46e8914e1149bf444c2a` on
`m98/file-workspace`, worktree
`/home/arduano/programming/geometric-constraint-solver-worktrees/m98-file-workspace`.
The original parallel base is `d80bf22264f74b60870f2e99feb8cc6ccb9d0133`.
Primary M97, MiniCAD, other worktrees and services remain untouched; no merge or push.

Local product commits: `18371de` (Rust sampler and native regressions), `3863059`
(WASM/CLI transport, v1 contract, fixtures and real exporter regressions). This document and
the minimal roadmap note are a separate documentation closeout; no existing milestone is accepted.

## Run

Run from this worktree root. Node dependencies are already installed privately here.
For a fresh checkout, use the dependency setup in [M98_HANDOFF.md](M98_HANDOFF.md).

```sh
env CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 nix-shell shell.nix --run \
  'npm run wasm:release --prefix crates/geosolve-demo-web/frontend'
node crates/geosolve-demo-web/frontend/scripts/build-workspace.mjs
mkdir -p target/m98/bake
node scripts/file-workspace.mjs bake examples/file-workspace-bake/pi-footprint \
  --out target/m98/bake/pi-footprint.json --chord-error-mm 0.02
node scripts/file-workspace.mjs bake examples/file-workspace-bake/circle-arc \
  --out target/m98/bake/circle-arc.json --chord-error-mm 0.02
```

No running UI, token or browser is needed. `--out` and `--chord-error-mm` are required.
Output parent must exist and output must be outside the source project. Successful export
atomically replaces a regular output file; failed export leaves prior output intact.
Stdout is JSON with paths, exact source SHA-256, process-local accepted/workbench revisions
and region/vertex counts. Stderr supplies diagnostics and nonzero exit on failure.

## Authority, math and integration seams

- `geosolve-sketch-topology::TopologyProductionProfile::sample_polygons` accepts the exact
  retained session, validates the production stamp before/after, and evaluates fragment source
  parameters through `SketchDocument::evaluate_curve_jet`. Output has CCW outer/CW hole loops,
  implicit closure and file-local region IDs. Lines/polyline spans need no curved subdivision.
- Circle/arc segment count uses the accepted radius and evaluator parameter speed, with
  `delta <= min(sqrt(error / radius), pi/2)`. Thus the ideal sagitta
  `r * (1 - cos(delta/2)) <= r * delta²/8 <= error/8`. Endpoint joins must fit `error/8`;
  a conservative scale/angle-dependent floating-point allowance must also fit `error/8`.
  Remaining margin avoids claiming the entire budget for ideal math. Precision/work-limit
  failures reject output. Polygon finiteness, winding, intersections, holes and joins are
  checked independently of render tessellation. No solver residual/equation changes.
- `WorkbenchHandle.bakeProfile(number)` authenticates clean managed code authority and current
  accepted intent/materialization, runs existing bounded production topology with self-intersection
  rejection, and invokes the Rust sampler. Computed features are refused to prevent incomplete
  native-only output. The WASM DTO omits file provenance because Rust does not own disk paths.
- `WasmWorkbenchAdapter.bakeProfile` transports the JSON result. `bakeProject`/CLI uses the
  existing cold compiler/receipt path with recovery disabled, attaches SHA-256 of exact UTF-8
  file bytes (including BOM), stages complete output, rechecks source and manifest, then publishes.
  Node contains no curve sampling, region construction or solver equations. The hash identifies
  the accepted disk snapshot checked at publication; subsequent edits require a new bake.
- New direct demo dependency on topology, one WASM export/build-contract entry, one TS adapter
  method, and the CLI subcommand are the integration seams. No new HTTP write endpoint or UI flow.
  `GEOSOLVE_DIST` optionally selects the server's static artifact for private browser verification.

## Scope and next consumer checks

One fixed `geosolve.json` plus `sketch.ts`, managed SDK-only source; no local dependency graph.
Profile-role native lines/polyline spans, circles/circular arcs only. Construction geometry is
excluded. Empty/open/ambiguous/incomplete topology, unsupported curves, computed features,
stale state and invalid disk source refuse. Limits: 65,536 sampled vertices across emitted loops
and 2,000,000 polygon edge-pair tests; these are bounded prototype limits, not performance promises.
The error is source-curve polygonization error only, not downstream Boolean/STL/manufacturing error.

Every bounded production face is exported, including interior disks inside circular contours.
Consumers must select the board-with-four-holes region explicitly, not extrude all Pi regions.
Region IDs are local to this export; no persistent topological naming is promised.
The Pi source is only a nominal 85 × 56 mm reference with four diameter-2.7 mm holes. Thickness,
component envelopes, MiniCAD extrusions/STLs and independent solid validation belong to the
coordinator's separate consumer task. No case design or hardware-fit claim is made here.

M97 source-native metadata integration remains deferred. Its live changes were never read/copied;
later integration must reconcile the bridge/module additions and accepted source/metadata authority,
the fixed single-file manifest and any future metadata-dependent export provenance. Isolation
prevents overwrites, not later merge conflicts. Existing ordinary UI/manual exports stay available.

Broad integrated release gate, full golden inventory, all-workspace test suite and production UAT
are deliberately unrun for this bounded prototype follow-up. Focused evidence is recorded below.

## Actual bake evidence

These are real accepted Rust/WASM exports, not contract stubs. Paths below are relative to this
worktree. The two direct `bake` commands above ran successfully, with stdout retained as
`target/m98/bake/pi-footprint.stdout.json` and `circle-arc.stdout.json`.
Both cold runs report `acceptedRevision: 1`, `workbenchRevision: 3`; revisions are process-local,
so SHA-256 is the cross-run source identity.

| Artifact | Observed contents |
| --- | --- |
| `target/m98/bake/pi-footprint.json` | **Select `region-4` for the board**: four outer vertices, 85 × 56 mm, four CW holes with 52 vertices each. Regions `0`–`3` are the four interior disks. Hole centers/radii match the nominal fixture. |
| `target/m98/bake/circle-arc.json` | `region-0`: quarter-arc/chord segment, 26 vertices. `region-1`: accepted radius-12 disk, 154 vertices, despite radius-10 source seed. |
| `target/m98/bake/pi-footprint-width-90.json` | Private source changed only `width: mm(85)` to `width: mm(90)`; accepted outer width measured 90 mm, same four holes. |
| `target/m98/bake/dimension-change.json` | Exact private source path and before/after hashes; reproducer source retained beneath `target/m98/bake/regression-g1OadR/pi-width-90/`. |
| `target/m98/bake/sampling-measurement.json` | Maximum measured circular-chord midpoint sagitta **0.002496861729769151 mm** for requested 0.02 mm; includes interior arc samples. |
| `target/m98/bake/regression-invocations.json` | Exact exporter argument arrays, exit statuses, stdout and refusal diagnostics from focused tests. |

Exact `sketch.ts` SHA-256:

```text
Pi 85: 40bef69c119320ac446237dc22cf6491580d69db599594fb59c76bfed1327384
Pi 90: 9a57b9c385fa7a53c0267b317060dde43585f900a9f95de2132cdeaaaf3016a6
Curves: 669bcdfc600f2411558f1129f600c19355c63a1c3c8f5e44878e035241a31da6
```

The initial arc/chord source compiled but production topology refused `IntersectionAmbiguous`.
Diagnosis through the existing profile owner showed that production closure needs owned endpoint
contacts. The final fixture declares explicit start/end `pointOnCurve` contacts and passes
unchanged topology. No proximity welding, tolerance relaxation or topology/solver fix was made.
The rejected probes and exact diagnostics remain under `target/m98/bake/arc-probes/`; this was a
fixture-authoring correction, not a new accepted solver-defect claim. Rectangle source likewise
uses the existing required `role: "profile"` field.

## Exact focused checks and outcomes

Commands ran from the worktree root unless a directory is specified. Cargo concurrency was two.

```sh
env CARGO_BUILD_JOBS=2 BINARYEN_CORES=2 nix-shell shell.nix --run \
  'cargo fmt --all && cargo clippy --locked -p geosolve-demo-web --lib -- -D warnings && cargo clippy --locked -p geosolve-sketch-topology --test m98_sampling -- -D warnings && cargo test --locked -p geosolve-sketch-topology --test m98_sampling && npm run wasm:release --prefix crates/geosolve-demo-web/frontend'
node crates/geosolve-demo-web/frontend/scripts/build-workspace.mjs
env GEOSOLVE_DIST=$PWD/target/geosolve-m98-bake-ui npm run build:ui --prefix crates/geosolve-demo-web/frontend
npm run test --prefix crates/geosolve-demo-web/frontend -- src/lib/wasm-adapter.test.ts src/lib/pending-managed-mutation.test.ts --maxWorkers=1
node --test --test-concurrency=1 scripts/file-workspace-bake.test.mjs
env GEOSOLVE_DIST=$PWD/target/geosolve-m98-bake-ui node --test --test-reporter=tap --test-concurrency=1 scripts/file-workspace.test.mjs
env CARGO_BUILD_JOBS=2 nix-shell shell.nix --run 'cargo fmt --all -- --check'
node --check scripts/file-workspace.mjs
node --check scripts/file-workspace-bake.test.mjs
git diff --check
```

- Native sampler **3/3**; native clippy passed. Strict line rectangle, circle/hole sagitta,
  refinement, stale stamp and invalid/unrepresentable targets.
- Exporter **7/7**, with actual compiler/accepted geometry and CLI subprocesses: exact contract,
  winding/holes/hash, solved radius/arc samples, deterministic repeat, private width change,
  invalid disk despite recovery, concurrent source change, open/unsupported refusal, target/path
  guards and exact BOM-byte hash. Existing output and source preservation are asserted.
- Actual Chromium file-sync suite **14/14**, using the **new** WASM/runtime and separate new UI
  dist. Witnessed both live directions, restart, save-by-rename, invalid-source last-good view,
  stale/failed-write preservation, no watch loop, manual sketch export and ordinary demo mode.
  Small-sketch save-to-Inspector observation: **900 ms** under concurrent build activity;
  not a general latency claim. Screenshots, export and reproduction evidence: `target/m98/tests/`.
  Actual downloaded canonical sketch: `target/m98/tests/manual-project-export.json`.
- Frontend adapter/receipt tests **20/20**; TypeScript check, language declarations, Vite build
  and optimized WASM/binding contract passed. Build contract also passed via
  `node scripts/test-build-contract.mjs` from `crates/geosolve-demo-web/frontend`.
- Logs: `target/m98/logs/bake-build.log`, `bake-runtime.log`, `bake-ui-build.log`,
  `bake-frontend-tests.log`, `bake-regressions.log`, `bake-browser-regressions.log`,
  `bake-format.log`.

Additional WASM-target clippy was attempted:

```sh
env CARGO_BUILD_JOBS=2 nix-shell shell.nix --run \
  'cargo clippy --locked -p geosolve-demo-web --target wasm32-unknown-unknown --lib -- -D warnings -A dead_code'
```

It failed on **five pre-existing findings**: wildcard imports in bridge navigation/dimensions and
code-project navigation, plus `tool_catalog` unused self and `start` unnecessary result in the
WASM module. Those lines are unchanged from `692b13d`. The normal WASM build also reports existing
dead-code warnings. No source lint policy or tests were weakened. The explicitly limited follow-up
command **passed**, allowing those existing categories; its separate log is
`bake-wasm-clippy-focused.log`. This is not a strict WASM clippy pass:

```sh
env CARGO_BUILD_JOBS=2 nix-shell shell.nix --run \
  'cargo clippy --locked -p geosolve-demo-web --target wasm32-unknown-unknown --lib -- -D warnings -A dead_code -A clippy::wildcard_imports -A clippy::unused_self -A clippy::unnecessary_wraps'
```

## Preserved preview and next manual steps

The original M98 preview on **127.0.0.1:37135** remains running, with its static dist unchanged
(compared against the preserved copy). Prior WASM/runtime/dist and browser evidence were copied
to `target/m98/prototype-692b13d/`; no restart or port change was made. Original launch URL and
token remain in `target/m98/demo/.geosolve/session.json`, with logs in `target/m98/logs/preview.log`.
It is loopback-only: use `ssh -L 37135:127.0.0.1:37135 <this-host>`, then the saved local URL.
New test servers used fresh private ports and were closed after the tests.

The coordinator can now run the real exporter on a private input copy into consumer-owned output,
select Pi `region-4`, and perform the separately owned MiniCAD extrusion/STL/reload checks.
Confirm the source hash, change one source dimension in that private copy, rebake and verify the
changed consumer result. This handoff proves the GeoSolve half and does **not** claim MiniCAD/STL
integration was run here. Keep IDs explicitly selected and compare hashes after every source edit.
