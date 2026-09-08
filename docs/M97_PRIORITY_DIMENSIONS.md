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

Implementation and focused development verification pass. The prior
qualified preview at `http://100.94.63.83:18103/` remains unchanged. Replacement
integrated qualification is pending; M97 remains open. Visual review verifies
all six manifold callouts in the fitted Design view and readable Gridfinity
measurements with all 20 rows retained in the Inspector.

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
