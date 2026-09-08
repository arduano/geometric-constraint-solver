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

Nominate the completed implementation as clean committed source to the integrated
release runner. Reuse only its authenticated
unchanged-input evidence. Freeze the qualified production artifact without rebuilding,
verify its HTTP bytes and actual WASM readiness, and record the new preview here.
Supervising-user acceptance and M97 closure remain separate outstanding actions.
