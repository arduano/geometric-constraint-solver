<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 implementation and development evidence

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

M97 implements the approved [focused dimension contract](M97_GOALS.md). Clean-source
qualification and the verified preview pass; maintainer acceptance remains
pending and M96 is the accepted product. [Final qualification](M97_QUALIFICATION.md)
records the candidate identity. This record retains the development checks and
failed/interrupted attempts separately from that passing nomination.

## Native presentation and bridge

`dimension_presentation.rs` in `geosolve-constraint-editor` adds
`DimensionDisplayMode`, `DimensionPresentationState`, `DimensionPresentationContext`
and complete `SceneDimensionEntry` metadata. Applying the policy resolves the same
scene consumed by numeric drawing, SVG and native picking. Standalone callers keep
existing defaults. Focused selects at most six measurements, prioritizes inspection
and at most four pins, expands direct curve endpoints, and leaves omitted rows in
the Inspector. Hidden explicitly excludes ordinary callouts from every paint/pick
override. All deliberately shows every drawable dimension.

Automatic annotation slots remain separate from manual layout. The cache retains
positions across selection-only rebuilding and camera movement, bounds automatic
search to 96 CSS pixels, and applies 6/12 px collision clearance/hysteresis. Navigation
preserves membership and slots and does not restore suppressed candidates merely
because zoom changed. Accepted geometry and hidden-item changes invalidate the
relevant geometry-derived base. Same-revision preview changes are checked too.

The bridge caches accepted producer/source metadata and source editing authority,
persists exact mode/pin identities outside design history, and exposes contextual
Inspector rows. Managed source names replace internal native identities; public
patch controls remain separate from generated offsets. Ordinary dimension targets
appear once. Exact accepted values supply editing fields; compact canvas text is
never parsed back into an edit. `DimensionTargetMetadata::storage_value_for_display`
reuses the existing native acute-angle conversion, preserving quadrant and full turns.
Native edits use Inspector history and bridge revision publication; managed edits
prepare the existing compiler transaction before publishing geometry.

The frontend adds Focused/All/Hidden, the Dimensions Inspector, four-pin controls,
collapsed generated rows and inline edits. Idle hover is serialized after 250 ms;
wheel input ends its navigation boundary after 180 ms without another sample.
Hover and intermediate navigation responses contain only a frame. A terminal
wheel/pan response also publishes current dimension metadata; the adapter updates
the Inspector only when that metadata changed, without changing the autosave
revision. Late hover-clear/settle commands preserve a pending compiler ticket
without publishing a frame against the unsettled snapshot. Explicit selection
clears transient inspection focus. Stable row identities keep an input focused
across accepted edits while commands retain their separate revision-bound tokens.

## Reproduction and focused checks

M97-F001 reproduced at M96 source `41ad6c7`: four -90 wheel samples followed by an
empty-canvas click moved 45 of 82 manifold labels, by up to 628 CSS pixels. The
minimal native legacy fixture independently moves 11 labels, by up to 596 pixels.
Its retained-policy regression passes. Actual-WASM browser testing of the initial
M97 candidate retains all 82 label positions within 0.01 CSS pixels across the
same zoom/cold-selection sequence.

Executed under `shell.nix` unless otherwise noted:

- `cargo test --locked -p geosolve-constraint-editor --lib m97_ -- --nocapture`:
  11 passed, including all eight families, reference values, hidden picking,
  hover transit, manual layout, same-revision geometry changes, exact values,
  angle quadrants and first-filtered-scene restoration.
- `cargo test --locked -p geosolve-sketch-render --lib m97_ -- --nocapture`:
  one passed, numeric/SVG/static/contextual visibility parity.
- `cargo test --locked -p geosolve-demo-web --lib m97_ -- --nocapture`:
  seven passed after the final interaction review, including manifold public
  width, pin/reload, stale requests, precise native edits through history/reload,
  managed preparation, deferred cleanup and terminal camera metadata publication.
  The native-edit case also passes a subsequent check of the stable `rowKey`
  transport field; the original snake-case spelling failed the browser focus check.
- `npm test -- --no-cache` in the frontend: 215 passed after the edit-focus
  regression; `npx vitest run src/lib/wasm-adapter.test.ts --no-cache` then passed
  all nine adapter cases, including the added cleanup regression.
- `cargo fmt --all` and `cargo clippy --locked -p geosolve-demo-web -p geosolve-sketch-render --all-targets --all-features -- -D warnings`: passed.
- `node crates/geosolve-demo-web/frontend/scripts/build-release-artifacts.mjs --out target/m97/dev-artifacts-r5`:
  optimized WASM plus 15-file harness and 12-file production distributions passed.
- The r4 Playwright focus/pin/hover/pan/transit case and the zoom/cold-selection
  case passed. Managed editing changed the accepted source, but its focus assertion
  caught the `rowKey` spelling mismatch described above. The corrected r5 browser
  edit/history/reload check passed in 1.2 minutes, including continued input focus.
  It retains the exact source through Undo/Redo and reload. Earlier pin testing selected a
  collision-hidden row; the corrected test pins an already visible row. Direct
  actual-WASM inspection verifies that such a pin survives restoration and resize.

Browser commands use `GEOSOLVE_E2E_ARTIFACT_MANIFEST` pointing to the corresponding
`target/m97/dev-artifacts-rN/harness.json`, `GEOSOLVE_E2E_PORT=18102` and
`GEOSOLVE_CHROMIUM_PATH=$(command -v google-chrome)` from the Nix shell:

```bash
npx playwright test tests/e2e/m97-dimensions.spec.ts --workers=1 --output=target/m97/browser-r4
npx playwright test tests/e2e/m97-dimensions.spec.ts --grep "contextual dimension edits" --workers=1 --output=target/m97/browser-r5
```

Earlier provisional failures are retained under ignored `target/m97/`: a reference
fixture reused scalar ownership and was corrected; `typed-panel` has no dimension
annotations and was replaced with dimensional fixtures; initial bridge compilation
used a private scene field and was corrected to the public native metadata API.
These are development failures, not passing qualification evidence. Golden rows
and solver equations have not changed.

Final review independently reproduced two additional interaction failures. A
hover-only measurement remained visible throughout middle-button pan because the
navigation freeze retained its membership. The native hover/transit regression
now also requires its removal at pan start. An accepted edit changed a command
token used as a React key and remounted the input; a component regression now
requires the same focused DOM input and the replacement command token. Both
regressions failed before their respective corrections. Terminal navigation also
refreshes Inspector visibility, with adapter checks for changed/unchanged metadata,
malformed replacements and deferred cleanup. These refinements do not alter the
accepted six-callout/four-pin policy or any design history.

## Navigation observations

The same Chromium 151 script, 1440×900 viewport and three samples compared the
frozen M96 production artifact with final development source in `dev-artifacts-r5`. Median
bridge times in milliseconds:

| Sample | Wheel before / after | Pan before / after | Idle hover before / after |
|---|---:|---:|---:|
| Dogbone coupon | 3.9 / 3.0 | 3.6 / 2.9 | 1.5 / 2.1 |
| Manifold | 11.6 / 7.5 | 14.1 / 7.8 | 6.1 / 8.4 |
| Dense robotic harness | 6.5 / 5.6 | 7.7 / 5.6 | 2.9 / 3.8 |

Manifold rendering medians improved from 15.2 to 10.4 ms during wheel navigation
and 11.4 to 6.6 ms during pan. Dense rendering during pan improved from 8.1 to
5.8 ms. Idle pointer handling became more expensive; these measurements do not
claim a universal speedup. All direct probes retained exact saved workspace bytes.
Raw timings, actual screenshots and artifact manifests are under `target/m97/`.
All three samples retained exact direct-probe persistence, had no browser errors,
and produced no idle frames. These are small comparative navigation samples,
not a universal performance guarantee. Executed command:

```bash
NAV_OUTPUT=target/m97/optimized-r5 NAV_MANIFEST=target/m97/dev-artifacts-r5/production.json node target/m97/run-navigation-probe.mjs
```

These development measurements precede the integrated nomination. The final
frozen-artifact measurements and qualification are recorded in
[M97_QUALIFICATION.md](M97_QUALIFICATION.md).

## Qualification harness corrections

The initial clean nomination `20260908T114541-82fb60fa` stopped at three
warnings-denied `float_cmp` assertions in the exact-value editor regression.
Commit `d097c24` preserves exact equality by comparing floating-point bits.
The resumed run `20260908T115102-41556b3b` passed workspace Clippy, all native
workspace and separate headless sample/edit/history checks, WASM and browser
preparation, artifact transport, Rust documentation and benchmark preparation.
It remains a failed attempt: two optimized-WASM lifecycle cases and 46 full
browser cases passed, while one lifecycle and one browser case failed.

Both failures are `HARNESS_ERROR`, with their original evidence retained:

- The independent WASM frame composer retained standalone All defaults while the
  production workbench deliberately defaults to Focused. For the first Jansen
  mismatch, all non-dimension drawing items are exactly equal; the expected frame
  alone includes 77 dimension primitives. The composer now explicitly applies
  Focused through the public native presentation owner and asserts the workbench
  mode. Full-frame equality, including provenance and geometry, is preserved.
- The first-shader context-recovery screenshot placed the Y datum glyph beneath
  the wider Dimensions toolbar, leaving exactly two colored pixels. The harness
  now centers the camera through the ordinary control and independently requires
  both glyph sampling rectangles to clear that toolbar. Both original colored
  glyph assertions and the line-interior pixel threshold remain unchanged. The
  focused browser case passes against the same prepared artifact.

The failed runs do not qualify a release. The replacement nomination must use
authenticated unchanged-input evidence through the integrated runner and execute
every affected obligation again. No production renderer, geometry or solver
behavior changed in these harness corrections.

Focused correction checks passed:

```bash
cargo fmt --all -- --check
CARGO_BUILD_JOBS=4 CARGO_PROFILE_RELEASE_INCREMENTAL=true CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked --release -p geosolve-demo-web --lib actual_wasm_all_bundled_samples_match_independently_composed_production_frames --target wasm32-unknown-unknown
node_modules/.bin/playwright test tests/e2e/canvas-renderer.spec.ts --grep "context loss retains" --workers=1 --output=target/m97/browser-glyph-focused
```

The WASM command passes its one all-sample case; the browser command passes its
one case in 10.8 seconds. The latter uses the resumed gate's prepared harness
`4814c5b3d8e08cb4b7a696c4f7fc1a889be875e368a092185c8da95fb8f42fef`,
port 18102 and the Nix Chromium path through the same environment variables above.

## Large browser publication and restoration checks

Run `20260908T123427-82cdb8e6` passed all native, WASM and 271 unchanged golden
cases, plus all three M97 browser cases. Its sole browser failure was a `TIMEOUT`
in existing cubic authoring on the manifold. The final click took 17.64 seconds;
the first subsequent IndexedDB evaluation stayed pending beyond the existing
30-second accepted-source deadline. No missed click, explicit rejection or
network failure was captured. The identical production bytes passed that case
in the previous run and in an unchanged isolated check: 57.9 seconds overall,
with a 23.3-second accepted-source wait. This is not evidence of fast editing.

Unchanged-source run `20260908T130518-dc9ad29d` passed cubic authoring but exposed
the same pending-publication timeout in Circle authoring. It also failed the
reproduction import's five-second title check: `setFiles` starts asynchronous
restoration, and the UI query remained blocked for 21.5 seconds; the eventual
failure snapshot already contained the restored manifold. The gate parent later
exited with SIGTERM while its browser child drained. That run remains interrupted
and incomplete, with its two browser failures and traces preserved.

The harness now captures the exact accepted source before export, proves New
sketch removed that source from persistence, and waits for the exact source to
return after import through the existing 30-second completion helper. The
original title, source, geometry and runtime assertions follow this authenticated
transition. No timeout constant changed. The old source cannot satisfy the import
check before restoration.

Cubic, Circle, reproduction import and M97 contextual edits now share the existing
one-worker browser project for expensive compilation/restoration workflows. The
failed cubic interval overlapped reproduction import; the focused unchanged cubic
check passed in isolation. Four project assignments change in the reviewed
inventory, preserving every case and assertion. This is test scheduling and
completion synchronization, not a production performance correction. Large
manifold compilation, restoration and source authoring still take tens of seconds.

Focused checks use the unchanged gate-prepared harness and Nix Chromium with
`GEOSOLVE_E2E_PORT=18102` and the artifact environment described above:

```bash
node_modules/.bin/playwright test tests/e2e/workbench.spec.ts tests/e2e/m97-dimensions.spec.ts --grep "Cubic Bézier authoring|a downloaded reproduction imports|M97 contextual dimension edits" --workers=2 --reporter=json --output=target/m97/browser-publication-focused
node_modules/.bin/playwright test tests/e2e/workbench.spec.ts --grep "normal pointer capture release commits Circle" --workers=2 --reporter=json --output=target/m97/browser-circle-focused
node_modules/.bin/playwright test --list --reporter=json
git diff --check
```

The first three cases pass in 170.1 seconds and Circle passes in 75.6 seconds,
without failures, skips or retries. Actual discovery through
`release_browser.validate_discovery` matches all 48 cases, 16 sample workflows
and one catalog check after all four reviewed project assignments. These focused
results remain development evidence until replacement integrated qualification.

## Final nomination

Clean source `a39f35ade58c3d5a272899dfb4e616d6578a9d14` passes all 243 obligations
in `20260908T133354-cff36f90` (38m19s, 243 fresh stages). All 271 golden cases,
216 frontend tests and the complete 48-case browser inventory pass. There are no
browser failures, skips or retries. The independent audit authenticates all
9,881 referenced evidence files. The frozen 12-file production candidate passes
HTTP bytes/MIME and actual-WASM readiness at the archived preview.

The final navigation probe passes with exact persistence, zero browser errors and
zero idle frames on all three samples. Manifold wheel/pan bridge medians improve
to 7.5/9.4 ms; hover rises to 9.1 ms and dense wheel rises to 7.9 ms. These mixed
results and the remaining expensive source-publication paths are disclosed in
[final qualification](M97_QUALIFICATION.md). Acceptance remains pending; M97 is open.
