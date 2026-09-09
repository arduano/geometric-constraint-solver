<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 source-native authoring implementation

The source-native amendment is implemented and qualified on
`e26270cb89e5849092145b329d0cf95821a81b27`. The verified preview is
`http://100.94.63.83:18105/`; port 18104 preserves the prior default-priority
product. All 244 integrated obligations pass in `20260908T235146-b387d273`.
The supervising user accepted this product and requested closure on 2026-09-09.
[M97_CLOSURE.md](M97_CLOSURE.md) records the accepted milestone and continuation.

## Authoring and presentation

The public API lives in `packages/geosolve-sketch-code/src/authoring.ts`,
`presentation.ts` and `managed.ts`. Native authentication, control projection and
prepared edits live in `crates/geosolve-sketch-code/src/managed.rs`,
`managed_control.rs` and `prepared_mutation.rs`. The new
`crates/geosolve-demo-web/src/workbench/bridge/authoring_metadata.rs` and frontend
`src/components/authoring-metadata.tsx` connect those transactions to the Inspector.
The package and native crate READMEs document authoring; all 16 bundled sample
sources and their compiled projections use the new API.

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
  also pass (`authoring-native-final-r1.log`). Final clean-source evidence follows below.

Development logs, exact failed attempts and immutable browser artifacts are under
`target/m97/authoring-*`. A diagnostic unoptimized native manifold selection probe
was stopped after the browser/native response localized the defect to the frontend;
that interrupted probe supplies no passing evidence.

## Qualification attempts and repairs

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

## Final integrated qualification

Clean candidate `e26270cb89e5849092145b329d0cf95821a81b27`, tree
`de47931a170ce13d2dfd7feb2d65ddf6ba2ded81`, passes **244/244 obligations** in
`20260908T235146-b387d273`, with complete coverage and unchanged source at exit.
The command was:

```bash
CARGO_BUILD_JOBS=4 nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260908T224238-9270c86f --since d80bf22'
```

Wall time is 2249.9 seconds (37m29.9s). Nine stages execute freshly and 235 passing
results are authenticated unchanged-input reuse: 59 originate in R1, 97 in R2 and
79 in R3. Those attempts remain failed; only their independently successful stage
receipts supply evidence. Fresh stages are inventory, metadata/format, frontend,
catalog, browser preparation, production transport, licenses, browser and performance.
The complete inventory includes strict Clippy, native workspace/headless, actual-WASM,
documentation, packaging and parity coverage. The extra obligation relative to the
prior 243-stage product is the new native `m97_authoring_metadata` test executable.

Browser prefix coverage passes 17/17 in 107.2 seconds. The full batch passes 48/48
in 1237.6 seconds, with zero skipped, failed, retried or flaky workflows. Its inventory
contains 49 distinct obligations: the fresh catalog check plus those 48 full workflows;
all 16 sample prefixes run freshly and no browser leaves are reused. All four M97
workflows pass, including source metadata/parameter editing (31.2 seconds). The dense
fixture's full two-edit/history/reload workflow passes in 226.0 seconds with its original
six-minute limit. Browser stage wall time, including preparation of its evidence, is
1358.8 seconds. The isolated 256-body sparse crossover passes independent validation
in 137.73 seconds of test execution (145.3-second stage).

The three actual-WASM lifecycle cases pass through authenticated receipts. The
271-case golden remains clean and byte-identical, SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
The signed qualification and stage records are under
`target/release-gate/runs/20260908T235146-b387d273/`; the integrated log is
`target/m97/authoring-integrated-r4.log`.

## Frozen preview and acceptance

The qualified `prepare.browser` receipt authenticates the production manifest and
its exact directory. They were copied without rebuilding into the read-only
`target/m97/preview-20260908T235146-b387d273/` and served on
`http://100.94.63.83:18105/`. The following commands pass:

```bash
python3 target/m97/freeze-authoring-preview.py 20260908T235146-b387d273
GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npm run verify:artifact -- --manifest /home/arduano/programming/geometric-constraint-solver/target/m97/preview-20260908T235146-b387d273/production.json --directory /home/arduano/programming/geometric-constraint-solver/target/m97/preview-20260908T235146-b387d273/geosolve-production --url http://100.94.63.83:18105/ --receipt /home/arduano/programming/geometric-constraint-solver/target/m97/authoring-preview-artifact-verification.json'
python3 target/m97/audit-authoring-preview-artifact.py 20260908T235146-b387d273
```

All 12 files (28,761,396 bytes) and `/` pass HTTP status, byte, MIME and base-path
verification. Production manifest SHA-256 is
`89c3612719ed45b894ccd68833f58157b4f3f528bff9d80e6d84237f99a7aaea`;
file aggregate SHA-256 is
`ffea0a9f5f22a6e7c6fc33e070ef7c9706c1c6e1820f6df57557d0acbf4ecf93`.
Actual Chromium 151.0.7922.173 opens the manifold, reports 182 geometry entries and
a ready WebGL2 renderer with no runtime errors. Served WASM SHA-256 is
`8d7a7dfedb5baa73ef6747ad8fd2cc371faae595f212dbe4b86d4c3cb5e34a44`.
The transport and authenticated binding records are
`target/m97/authoring-preview-artifact-verification.json` and
`target/m97/authoring-preview-artifact-binding.json`. The detached server uses
`target/m97/serve-authoring-preview.mjs`; its log and supervisor PID record are
`target/m97/authoring-preview-server.log` and `authoring-preview-server-pid.json`.

Mechanical acceptance passes for explicit flag/default precedence, source-only
imports, shared and distinct parameter identities, exact source transactions,
stale/tampered rejection, GUI creation/extraction, labels/help/title, Undo/Redo/reload,
old-source compatibility, manifold/Gridfinity intent and retained dimension navigation.
The final manifold overview screenshot shows all six callouts, full 12/2.4 mm parameter
values and their source-editing controls. Mathematical behavior remains unchanged.

Generated per-instance overview overrides remain deferred, overview eligibility
remains subject to collision handling, and old unmarked source has no implicit
first-six priorities. All measurements remain discoverable. Dense-workflow timing
varies: this successful qualification does not erase the recorded R3 timeout or claim
a performance optimization. Supervising-user acceptance and M97 closure are complete
under the recorded scope; no further M97 implementation is pending.

Documentation-only continuation is checked with
`nix-shell shell.nix --run './scripts/release-gate.sh --docs-only --since e26270c'`.
It preserves the qualified product identity above and does not rebuild preview bytes.
