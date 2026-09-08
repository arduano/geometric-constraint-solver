<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 default design dimensions

The supervising user requested important shape-intent measurements in the default
Focused overview after reviewing the qualified M97 preview. Gridfinity now makes
all 20 authored measurements eligible, including its reference plan width and
construction projection dimensions. The manifold retains plate width/height,
reservoir width/height and one representative outlet radius and screw diameter.
Its public 12 mm channel width and 2.4 mm silicone groove width also appear in the
Inspector without selection. These are exact source controls, not relabeled 6 mm
or 1.2 mm generated offsets.

The other catalog samples declare small sets of operating, envelope or interface
measurements. The dense harness exposes its shared public clip and bend radii.
New/imported sketches without catalog curation retain up to six authored defaults.

## Ownership and behavior

`SampleDimensionPresentation` and `SampleDimensionParameter` extend the immutable
sample registry. Optional `dimension_presentation` manifest data names authored
declarations and exact dimensional control paths; the build validates selectors
against compiled IR. Workbench Rust resolves these selectors through accepted
source navigation, ownership and control provenance. Display labels do not grant
visibility or editing authority.

`DimensionPresentationContext::default_priority` and
`SceneDimensionEntry::default_priority` extend the native presentation API.
Focused admits default priorities in addition to six ordinary contextual
candidates. Priorities do not consume that budget when selected and do not consume
user pins. Active inspection, pins, hover and selected measurements retain their
ranking ahead of idle defaults. Collision clearance and retained placement still
apply; crowded priority rows remain accessible in the Inspector. Hidden suppresses
priority callouts, retaining only necessary active feedback. Navigation freezes
membership and positions as before. Explicit Fit and the first measured host
layout can reconsider occluded measurements; existing visible slots reserve
space before hidden candidates receive another bounded search.

The frontend identifies key measurements and consumes native visibility and edit
authority. Public parameter edits use the existing source transaction. No sample
geometry, compiled source artifacts, solver equations, tolerances, residuals,
DOF/rank semantics, hard/soft priorities or branch state change.

## Qualification status

Implementation and integrated qualification pass. The amended preview is
`http://100.94.63.83:18104/`; the prior preview at port 18103 remains preserved.
M97 remains open for supervising-user acceptance. Visual review verifies all six
manifold callouts in the fitted Design view and 17 readable Gridfinity callouts
with all 20 rows retained in the Inspector. The final evidence below supersedes
the intermediate attempts recorded here.

Focused checks passed:

- `env CARGO_BUILD_JOBS=4 nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --lib dimension_presentation::tests::m97_'`: 17 tests, including selected priorities preserving the contextual budget and
  explicit Fit reconsidering occluded measurements.
- `cargo test --locked -p geosolve-sketch-code --test m92_bundled_sample_registry` in the repository Nix shell: 4 tests, including exact authenticated public parameter resolution.
- `npm test -- src/components/dimension-inspector.test.tsx src/components/canvas-viewport.test.tsx`: 40 tests.
- `cargo test --locked -p geosolve-demo-web --lib m97_ -- --nocapture` in the repository Nix shell: 9 tests.
- `cargo clippy --locked -p geosolve-demo-web -p geosolve-sketch-code -p geosolve-constraint-editor --all-targets --all-features -- -D warnings` in the repository Nix shell: passed.
- `npx playwright test --list tests/e2e/m97-dimensions.spec.ts`: the three existing registered workflows remain present.

Known presentation limit: priority means eligible by default, not forced overlap.
Measurements without a readable slot remain in the Inspector. Full channel/groove
widths remain public Inspector parameters; the amendment does not invent a native
canvas dimension for a patch parameter.

The first added selected-priority budget test placed its last fixture rows outside
the viewport and failed its visibility assertion. Centering the complete 14-row
fixture preserves the candidate and visibility assertions; all 16 owner tests pass.
The failed development output is retained in `target/m97/priority-build-r1.log`.

The first browser run retained passing edit/Undo/Redo/reload evidence, but its
pin probe found no readable ordinary selected callout and its Gridfinity probe
opened in Split view. The probe now explicitly inspects a crowded measurement
before pinning and opens the full Design view. Visual review also motivated
explicit Fit reconsideration: expanding from Split view must allow newly readable
key dimensions without moving existing visible labels. Ordinary zoom remains
non-promoting. The initial browser logs and screenshots remain in
`target/m97/priority-browser-r1/`.

Optimized WASM and immutable harness/production preparation pass via
`node crates/geosolve-demo-web/frontend/scripts/build-release-artifacts.mjs --out target/m97/priority-dev-artifacts-r2`
in the Nix shell with `CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_INCREMENTAL=true
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`. The 15-file harness and 12-file production
artifacts are retained.

The browser runs use `GEOSOLVE_E2E_ARTIFACT_MANIFEST` pointing to the corresponding
`target/m97/priority-dev-artifacts-rN/harness.json`, `GEOSOLVE_E2E_PORT=18102` and
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome` in the Nix shell:

- `npx playwright test tests/e2e/m97-dimensions.spec.ts --workers=1 --output=/home/arduano/programming/geometric-constraint-solver/target/m97/priority-browser-r1`: the managed edit/history/reload case passed; two overview probes failed as described above.
- `npx playwright test tests/e2e/m97-dimensions.spec.ts --grep-invert "contextual dimension edits" --workers=1 --output=/home/arduano/programming/geometric-constraint-solver/target/m97/priority-browser-r2`: Gridfinity/zoom/Hidden passed; manifold waited for the wrong Explorer name, `Middle outlet bore`, instead of the accepted declaration `middleOutlet`. No timing or visibility assertion was relaxed to resolve this selector error.
- `npx playwright test tests/e2e/m97-dimensions.spec.ts --grep "manifold focus limits" --workers=1 --output=/home/arduano/programming/geometric-constraint-solver/target/m97/priority-browser-r3`: passed in 55 seconds, covering six default rows, truthful widths, explicit inspection, nonpriority pin/reload/clear, hover/pan/transit and unchanged source/history.

The first integrated amendment run, `20260908T155245-5f0fbeb8`, stopped at
`workspace.geosolve-sketch-code::m93_retained_sample::normal`. Its historical
Bondtech comparison did not account for the intentionally added live-catalog
presentation metadata. The test now asserts exactly `{"all_authored": true}` on
the live manifest and absence of that field in the archive, then compares all
remaining metadata exactly. Historical source, project, witnesses, notices and
edit fixtures remain unchanged. This is a harness expectation correction; no
product behavior or golden rows were changed.

`env CARGO_BUILD_JOBS=4 nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch-code --test m93_retained_sample && cargo fmt --all -- --check'`
passes both retained-sample tests and formatting. Output is retained in
`target/m97/priority-retained-fixture.log`. Replacement integrated qualification
will authenticate reuse of independently completed stages from the failed run;
the failed attempt remains failed.

## M97-F002: exact identity for unchanged annotation viewports

Replacement run `20260908T160816-fcd4a3b4` reproduced a native presentation defect
at source `e87a7ba270c7440184bda359b9a915bf79868ea7`. The existing actual-WASM
all-sample frame comparison failed on the manifold's 5 mm screw diameter. All
364 drawing items and their identities matched; five annotation items differed
only through approximately `1e-14`-pixel rounding in the same radial geometry.
The scale/history and transition-matrix WASM cases passed.

Repeated unchanged snapshots reprojected retained annotations through
screen-to-model-to-screen conversion despite identical viewports. The diameter
edge's x coordinate changed from `111.07604117731624` to `111.07604117731626`.
An isolated native regression with the same circle, fitted viewport and radial
placement independently reproduced the drift through `DimensionPresentationState`.
The helper now treats identical viewports as exact geometry identities while
still refreshing label bounds after callers restore retained geometry. That
refresh preserves collision and picking authority. Actual viewport changes keep
their existing mapping; the complete WASM frame comparison remains exact.

The first minimal circle fixture retained a default radial direction that did
not expose the rounding; it passed before repair. Reproducing the observed radial
direction with an explicit placement made the exact annotation assertion fail.
The regression compares geometry, label bounds and visible metadata over four
unchanged applications. No golden rows, solver equations or sample geometry were
changed. Development occurred in an isolated checkout while independent browser
checks on the failed candidate finished.

Focused correction commands ran in the isolated checkout, in its repository Nix shell:

- `env CARGO_BUILD_JOBS=4 nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --lib dimension_presentation::tests::m97_identical_viewport_reapplication_preserves_exact_radial_annotation'`: the observed radial-placement fixture fails before repair with exact coordinate/bounds differences.
- `env CARGO_BUILD_JOBS=4 nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo test --locked -p geosolve-constraint-editor --lib dimension_presentation::tests::m97_ && cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings'`: formatting, all 18 dimension tests and warnings-denied Clippy pass after repair. The first Clippy attempt caught a test initializer style issue; a direct struct initializer resolves it.

Native before/after logs are preserved under `target/m97/priority-idempotence-*.log`.

`env CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_INCREMENTAL=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 nix-shell shell.nix --run 'CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked --release -p geosolve-demo-web --lib actual_wasm_all_bundled_samples_match_independently_composed_production_frames --target wasm32-unknown-unknown'`
passes the existing exact all-sample drawing comparison after repair (48.0 seconds
of test execution, two other cases filtered). The complete comparison and all
sample assertions remain unchanged. Output is preserved in
`target/m97/priority-idempotence-wasm.log`.

## Package-verification cache correction

Run `20260908T164242-cfcce90c` passes the repaired three-case WASM lifecycle
suite and the unchanged golden, then fails `package.archive`. The packaged
`build.rs` exactly matches the checked-in file (SHA-256
`031627ca30f3ecbd5b3f58e9ef8b3a934895ad3e4d9548aea800e45d66affa13`), but Cargo
reuses a cached build-script executable dated 2026-09-06 that predates
`dimension_presentation`. Its old parser rejects the new manifest field.

Cargo's archive gives source files a normalized historical modification time
(`1153704088`). The verifier previously restored those timestamps into every
new extraction while sharing a Cargo target directory, allowing mtime-based
freshness checks to retain stale package code. Extraction now uses `tar -m` so
the extracted package files receive current timestamps. The archive bytes and
normalized manifest remain intact; the package's own code is checked afresh while
dependency compilation stays reusable. No product behavior or assertion changes.

`env CARGO_BUILD_JOBS=4 nix-shell shell.nix --run 'bash scripts/verify-geosolve-sketch-code-package.sh'`
passes in an isolated checkout against a reflink copy of the stale verification
cache. Cargo rebuilds the extracted package and accepts the current manifests.
Output is retained in `target/m97/priority-package-timestamps.log`.

## Final integrated qualification

Clean candidate `fc3fdcb71b4d910815a51cf030128c42317ff2ed`, tree
`300a72c8e54bec0522531ccd6a8234ad36671958`, passes **243/243 obligations** in
`20260908T172035-7255b491` (376.0 seconds). The product implementation is the
unchanged descendant of `c8584ce`; the final commit corrects package verification.

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
python3 target/m97/audit-priority-qualification.py 20260908T172035-7255b491
```

The runner freshly executes 13 stages and authenticates 230 unchanged-input
successes. Fresh coverage includes package contents/archive, licenses, transport,
seven WASM interaction-parity suites, inventory and isolated performance. Reused
native, optimized-WASM, golden and browser receipts preserve their original run
identities; those successes do not turn earlier failed overall runs into passes.
The audit authenticates all 243 signed stage receipts and **9,903 evidence files**.
Logs and audit: `target/m97/priority-gate-r4.log` and
`target/m97/priority-qualification-audit.json`.

The authenticated coverage includes 217 frontend tests, 459 editor tests and 335
workbench tests (one existing ignored test), all headless sample/edit/history
checks, warnings-denied Clippy, formatting, documentation, optimized WASM,
packaging and licenses. All three optimized-WASM lifecycle cases pass, including
exact complete drawing equality for every sample. All 271 golden cases pass,
with the unchanged checklist SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
The final performance stage passes in 172.9 seconds; its 256-moving-body case
passes in 128.5 seconds.

Browser evidence comes from the unchanged prepared artifact in
`20260908T164242-cfcce90c`: all 17 catalog/sample-open checks and 47 full workflows
pass, with no failures, skips, retries or reused leaves. The browser stage takes
1254.6 seconds. M97 focus/pins, stable placement, and editing/history workflows
pass in 53.8, 31.7 and 68.6 seconds. Native zoom, Hidden, inspection, default
priorities, contextual budget, explicit Fit, source controls and reload criteria
remain covered. The prior attempt `20260908T160816-fcd4a3b4` also completed its
independent browser stage successfully; both failed overall attempts retain their
original failed qualification status.

## Frozen preview

The authenticated production artifact is copied without rebuilding to
`target/m97/preview-20260908T172035-7255b491/`, made read-only and served by
`target/m97/serve-priority-preview.mjs` at **http://100.94.63.83:18104/**.
It contains 12 files and 28,404,410 bytes, with files SHA-256
`53c300834fa3d8a2c65256148852d9cafab982eadc6cb232b3c422020b176ee6`
and manifest SHA-256
`6c90674ef5fdc7692b0d79ba472ff2b7961a6695f8f4c117a00dacab9fac9f5a`.

```bash
python3 target/m97/freeze-priority-preview.py 20260908T172035-7255b491
```

The first transport invocation raced the detached server startup and failed before
any route or WASM execution. Its receipt is preserved as
`target/m97/priority-preview-artifact-verification-startup-failure.json`.
The server then responded successfully; the same full verification was repeated.

The following repeated verification and binding audit pass:

```bash
env GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npm run verify:artifact -- --manifest /home/arduano/programming/geometric-constraint-solver/target/m97/preview-20260908T172035-7255b491/production.json --directory /home/arduano/programming/geometric-constraint-solver/target/m97/preview-20260908T172035-7255b491/geosolve-production --url http://100.94.63.83:18104/ --receipt /home/arduano/programming/geometric-constraint-solver/target/m97/priority-preview-artifact-verification.json'
python3 target/m97/audit-priority-preview-artifact.py 20260908T172035-7255b491
```

All 13 routes (12 files and `/`) match exact bytes, MIME and base-path requirements;
actual WASM readiness opens the manifold with accepted rendered geometry and no
browser errors. `target/m97/priority-preview-artifact-binding.json` binds endpoint,
manifest and file hashes to the signed preparation and qualification. The prior
preview on port 18103 remains preserved. Both temporary repair worktrees were
removed after integrating their commits and preserving evidence.

## Final navigation observations and acceptance

```bash
env NAV_OUTPUT=target/m97/priority-navigation-final NAV_MANIFEST=target/m97/preview-20260908T172035-7255b491/production.json nix-shell shell.nix --run 'node target/m97/run-navigation-probe.mjs'
```

The same Chromium 151, 1440×900, DPR 1, RTX 3090 probe measures the exact frozen
artifact. All three samples have zero browser errors, zero idle frames and exact
direct-probe saved-source preservation. Median bridge work, milliseconds:

| Sample | Wheel | Hover | Pan |
| --- | ---: | ---: | ---: |
| Dogbone coupon | 4.2 | 3.5 | 4.7 |
| Manifold | 9.3 | 11.0 | 8.8 |
| Dense robotic harness | 5.7 | 4.0 | 5.6 |

These are bounded navigation observations, not a universal speedup or 60 Hz claim.
Full report and screenshots: `target/m97/priority-navigation-final/`. Source
compilation and large managed edits retain their previously recorded costs.
Priority dimensions still require readable canvas slots; all key measurements
remain accessible in the Inspector. Full patch widths are Inspector parameters.

The amendment's mechanical criteria pass: authored sample priorities, all 20
Gridfinity rows, truthful manifold widths, stable navigation/identical snapshots,
contextual access, pins/Hidden, precise edits and persistence. **M97 remains open
for supervising-user acceptance; this report does not close the milestone.**

The prose handoff passes `git diff --check` and
`./scripts/release-gate.sh --docs-only --since fc3fdcb` for five documentation files
and six added links. This preserves the qualified product identity and served
bytes; it does not rebuild or requalify the product.
