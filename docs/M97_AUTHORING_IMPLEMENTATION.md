<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 source-native authoring implementation

The source-native amendment is implemented locally. M97 remains open, and the
qualified preview at `http://100.94.63.83:18104/` remains unchanged until replacement
qualification and served-byte verification finish. This document records focused
development evidence, not a completed release claim.

## Authoring and presentation

`sketch(options, callback)` owns title, description and
`dimensions.areKeyConstraintsByDefault`. Existing callback-only source remains valid.
Dimension options accept `isKeyConstraint`; named `$.parameter(id, value, options)`
and dimensional patch schemas accept `isKeyParameter`. Labels and descriptions stay
beside their owners. Named parameters retain their ordinary numeric/unit values,
stable explicit identities and all actual consumers. Equal-valued parameters remain
distinct; unused parameters remain editable without fabricated solver consumers.

Inspector controls write these same properties using Rust-prepared source mutations.
Reset removes an override. Extraction wraps a scalar binding or replaces an exact
inline value, preserving units, comments and other declarations. Accepted source,
geometry, display projections and history publish together. Dirty drafts, stale
requests and invalid receipts cannot grant edit authority. Keyboard focus returns
after controls have been re-enabled by the completed React update.

All 16 live samples now own their presentation in source. The manifold retains six
authored overview dimensions, a shared 12 mm channel width and a 2.4 mm seal width;
Gridfinity retains all 20 authored measurements. Catalog ordering, categories,
provenance, legal records and independent expectations remain separate. The catalog
build, browser inventory and release checker derive display metadata from compiled
source and compare it with the independent catalog contract. Archived source and
the 271-case golden are unchanged.

The managed compiler uses V4 envelopes with bounded V3 restoration and authenticated
transactional upgrades. Loading old source does not silently rewrite its text.
Personal modes, pins, camera, hiding and placement remain outside authoring source.
Generated measurements still have their existing contextual visibility and exact
edit restrictions; per-instance generated overview overrides remain deferred.

## Mathematical behavior and defects

No primitive, residual, Jacobian, solver priority or branch policy changed.
Source metadata never becomes a mathematical input or grants a value-edit route.
Native tests independently verify accepted hard residuals and exact retained
points, scalars, curves and evidence across source-property edits.

M97-F003 reproduced a label mutation rejected by incremental composition:
`RenameNode` correctly returns `OrganizationOnly`, but the composer accepted only
`Accepted`. The unchanged-host composer now admits the organization disposition
and independently revalidates retained native authority before returning it.
`m97_f003_source_relabel_preserves_independently_accepted_native_authority` and the
bridge lifecycle test cover exact geometry, evidence, original-state retention and
Undo/Redo/reload.

Compiler/native integration probes also froze and corrected final-property reset
cleanup, extraction property order, empty invocation presentation, numeric/unit
digest parity, scalar identity collisions and legitimate patch inputs named `label`.
Exact source validation rejects resealed metadata that disagrees with source bytes.

The actual browser selected the correct manifold output, but its frontend selection
decoder rejected the new `metadata` field. The existing selection-delta regression
was extended to reproduce that refusal. It now accepts validated metadata while
retaining source/presentation references and rejecting malformed replacements.

## Focused evidence

Commands ran in the pinned Nix environment where applicable:

- Package `npm run build`, `npm run test:runtime`, `npm run test:fixtures`,
  `npm run test:bundled-samples`, `npm run test:types`, `npm run test:managed` pass;
  runtime coverage contains 44 cases including Node/Deno parity.
- `cargo test --locked -p geosolve-sketch-code --lib managed` passes 20 cases;
  the `prepared_mutation::` filter passes 35; `managed_suppression` passes 5 and
  `managed_typescript_interop` passes 9. All integration targets compile.
- `cargo test --locked -p geosolve-sketch-code --test m97_authoring_metadata`
  passes; `--test m92_bundled_sample_registry --test m93_retained_sample` passes
  4 registry and 2 archived-fixture cases.
- `cargo test --locked -p geosolve-demo-web --lib m97_ -- --nocapture` initially
  passed 10 existing cases and exposed the label transaction defect. The focused
  `m97_source_properties_publish` rerun passes after repair. The adapter test also
  checks explicit false/default, shared/equal-valued parameters, stale requests,
  exact source history, extraction, labels/title and source-only restoration.
- Focused all-targets Clippy passes for sketch-code and demo-web. Formatting and
  `git diff --check` pass at the recorded checkpoints.
- Focused frontend metadata/App/adapter tests pass, including 90 initial cases
  and the 49-case App suite with the focus regression.
- `python3 -m unittest scripts.tests.test_release_gate -q` passes 29 cases.
  Provisional `GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh --preflight` passes
  all five stages in `20260908T215337-03eb4a9d`; it is not clean-source qualification.
- Optimized WASM and immutable harness/production builds pass via
  `node crates/geosolve-demo-web/frontend/scripts/build-release-artifacts.mjs`.
  Focused Playwright runs pass source authoring/extraction/history/reload (31.8 s),
  navigation placement (32.3 s), and dimension numeric edit/history/reload (1.2 min).
  The final four-workflow run against `authoring-dev-artifacts-r5` passes all four
  cases in 3.2 minutes (`authoring-browser-r15.log`), including the repaired manifold
  selection, stable placement, numeric history and source metadata/extraction.
  Final demo-web all-targets Clippy and the native source-property lifecycle test
  also pass (`authoring-native-final-r1.log`). The clean-source gate remains pending.

Development logs, exact failed attempts and immutable browser artifacts are under
`target/m97/authoring-*`. A diagnostic unoptimized native manifold selection probe
was stopped after the browser/native response localized the defect to the frontend;
that interrupted probe supplies no passing evidence.

## Remaining qualification

The first clean candidate is `3bcd06611f15a20d79f94d5f795d0694db8eaeed`
(implementation `27a844a`, followed by the sample authoring README correction).
`CARGO_BUILD_JOBS=4 nix-shell shell.nix --run './scripts/release-gate.sh --since d80bf22'`
starts run `20260908T221332-021daddc`. Preflight and strict workspace Clippy pass;
the native web-adapter stage reports 335 passes, two failures and one ignored test.
The failures are existing Profile Offset and scale-sample tests looking up old display
labels. The repaired tests retain stable source-ID checks and separately verify authored
labels, with no production changes. Focused exact reruns pass (1.05 s and 42.48 s):

```bash
env CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_DEBUG=line-tables-only nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --lib workbench::bridge::tests::managed_profile_offset_closure_is_nested_and_mutates_as_one_source_block -- --exact --nocapture && cargo test --locked -p geosolve-demo-web --lib workbench::bridge::tests::m92_scale_sample_edited_history_restores_through_the_browser_request_envelope -- --exact --nocapture'
```

The first failed run does not qualify a release. The focused log is retained at
`target/m97/authoring-stale-selectors-exact.log`.

Resumed run `20260908T223007-8fb67de1` on `4a7fb6c` passes all 337 web-adapter
tests (one existing ignored test), then exposes a stale manifest `groups` read in
`m92_fabrication_wave_a`. An adjacent audit finds the same expectation in wave B.
Both now freeze the previously reviewed ordered group names independently of actual
compiled source, preserving ownership, provenance, editing, restoration and residual
assertions. No sample, product or golden changes are needed. Focused validation passes:

```bash
CARGO_BUILD_JOBS=4 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true CARGO_PROFILE_TEST_DEBUG=line-tables-only nix-shell shell.nix --run 'cargo fmt --all && cargo test --locked -p geosolve-sketch-code --test m92_fabrication_wave_a --test m92_fabrication_wave_b'
```

Wave A passes in 5.11 s and wave B in 0.67 s;
`target/m97/authoring-fabrication-groups.log` retains the exact output.

Run `20260908T224238-9270c86f` on `53ff5d9` passes native/headless, all three
actual-WASM lifecycle tests, the unchanged 271-case golden, documentation, package,
license and WASM parity stages. Browser prefixes pass 17/17; the full browser batch
reports 37 passes and 11 failures. Eight failures use source symbols where authored
Explorer/parameter labels are now displayed. One reveals the embedded editor SDK
omitting `presentation.d.ts`; in-memory injection removes all four manifold TypeScript
errors. The declaration generator and virtual filesystem now include that file, with
an owning language-service regression accepting metadata and rejecting invalid flags.
The 11-case language-service suite, `check:language-sdk`, `tsc -b`, and Playwright
discovery pass. Browser test repairs retain source identity, geometry and exact history
assertions while resolving labels independently from source declarations.

Two other browser cases time out: Circle source publication and the dense fixture-field
workflow. The latter's trace shows cumulative time rather than an established Undo
hang. An isolated exact rerun against the same R3 harness passes in 4.2 minutes with
the unchanged six-minute limit (`target/m97/authoring-dense-isolated.log`). No performance
fix or timeout extension is made on that evidence. The updated R6 frontend uses the
same optimized WASM bytes. All ten affected browser workflows pass in 5.7 minutes
(`target/m97/authoring-browser-repairs-r1.log`), including Circle creation/radius editing
(1.3 minutes), robotic-harness two-edit/history/reload (1.4 minutes), and the actual
editor's zero-TypeScript-error check. R3 remains failed; performance qualification
has not yet run.

```bash
nix-shell shell.nix --run 'node crates/geosolve-demo-web/frontend/scripts/build-release-artifacts.mjs --out target/m97/authoring-dev-artifacts-r6 --wasm-package target/release-gate/prepared/7f6a693863d766e3cc4a9a9f2192259c198ab28b529f105a55cc4a56e8ff39ab/wasm'
GEOSOLVE_E2E_ARTIFACT_MANIFEST=/home/arduano/programming/geometric-constraint-solver/target/m97/authoring-dev-artifacts-r6/harness.json GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome GEOSOLVE_E2E_PORT=18112 nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npx playwright test tests/e2e/workbench.spec.ts tests/e2e/m95-navigation.spec.ts tests/e2e/m92-sample-audit.spec.ts --grep "real WASM opens|click-authored|canonical Jansen|non-axis Parallel|computed Fillet|Cubic Bézier|normal pointer capture|M95 canvas and Explorer|M95 explicit|robotic-harness-backplane" --workers=1 --output=/home/arduano/programming/geometric-constraint-solver/target/m97/authoring-browser-repairs-r1'
```

Repair the affected owner checks and resume through the integrated release runner.
Reuse only its authenticated
unchanged-input evidence. Freeze the qualified production artifact without rebuilding,
verify its HTTP bytes and actual WASM readiness, and record the new preview here.
Supervising-user acceptance and M97 closure remain separate outstanding actions.
