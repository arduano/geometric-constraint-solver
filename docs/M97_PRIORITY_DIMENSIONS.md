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
