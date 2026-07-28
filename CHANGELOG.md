# Changelog

All notable changes to GeoSolve are documented here. The project follows the
versioning and deprecation policy in `docs/API_COMPATIBILITY.md`.

## [Unreleased]

### Added

- Typed retained-design, solve-attempt and accepted-state identities/views for
  repairable unsolved sketch intent, optional finite candidate geometry and separate
  v1-v4 design/accepted persistence with host-owned revision high-water metadata.
- Cooperative cancellation and deterministic work-budget APIs with typed non-success
  outcomes and publication-time cancellation checks.
- Closed semantic relation, dimension and persistent-measurement catalogs through
  M36-M38, including bounded path-length work and independent evidence validation.
- The pure-Rust `geosolve-constraint-editor` state machine for normalized input,
  persistent selection/drafting state, accepted-scene projection and typed effects.
- Persistent construction/activity roles, revisioned typed host-parameter bindings and
  immutable external 2D snapshot/rebind contracts.
- A disposable, non-persisted M52 host-semantics UAT sidecar over the sole workbench,
  with deterministic typed evidence and direct owning-layer regressions.
- Structured current-attempt problem metadata at the headless-editor seam, including persistent
  element targets and an explicit global fallback for unattributable failures.

### Changed

- Rebased the post-M44 roadmap: M45 preserves cleanup evidence without human approval;
  M46-M53 replace and purge legacy browser E2E/playground infrastructure, consolidate one
  directly tested workbench and perform post-cleanup host-semantics UAT. M53 received explicit
  supervising-human approval. The later functional/release sequence is M54-M64, with a dedicated
  M55 alpha constraint/dimension/branch-action parity gate inserted before concurrency and scale.
- Completed the M46 ownership freeze: every old M14/M40/M44 browser/static assertion and
  legacy inline test has a named direct-test owner or reviewed retirement, while no old
  fixture, E2E script or playground code was deleted early.
- Completed M47 with five direct host-semantics fixture groups and deterministic typed
  finding capture, then removed the broad M44 host fixture, fixture-only controls and
  `e2e/m44.mjs` browser qualification infrastructure.
- Completed M48 direct editor/workbench qualification and removed the M40 browser E2E,
  serving script, static scans and browser-only delivery checks.
- Completed M49 legacy semantic extraction with direct owning-layer coverage or explicit
  retirement for every M14 browser group and legacy inline test.
- Completed M50 by deleting the final M14 E2E/CDP/server stack, legacy playground route and
  runtime, hidden DOM/CSS, stale serving glue and release-gate browser invocation. One directly
  qualified workbench remains with pruned dependencies and WASM features.
- Completed M51 by removing the survivor's design-only storage migration, duplicate M40
  report/evidence fixtures and stale M32 distribution copy; one workspace snapshot and directly
  tested presentation, persistence, effect and typed-evidence transformations remain.
- Replaced the M52 candidate's one-off bottom launcher and overlay for M53 review with a
  reusable typed scenario catalog, a top nested **Scenarios** selector and a contextual guide
  sidebar. The original six scenarios preserve the same ten objective points, deterministic
  reset/evidence behavior and ordinary workspace isolation, with no browser-owned domain semantics.
  Nested groups now open as right-expanding hover/focus flyouts, with an inline narrow-screen
  fallback, instead of requiring a separate disclosure toggle at each level.
- Extended the M53 catalog to eight scenarios with attributed-conflict and global-input-error
  recovery examples. The accepted canvas now presents separate current-error highlights and
  accessible non-mutating markers while the Problems panel remains canonical.
- Consolidated the WASM consumer to one directly tested workbench and removed the legacy
  playground, routes, browser E2E, serving/download glue and browser-owned qualification path.

## [0.2.0] - 2026-07-22

Post-expansion sketch preview and release hardening.

### Added

- Interactive public-API UAT for associative offsets, mirrors, directed angles,
  generic fillets and advanced NURBS editing.
- Certified read-only visual-profile analysis across all 15 built-in planar curve
  families, including self-intersections, bounded curved area and containment.
- Deterministic `GEOSOLVE_SCENE_V1` diagnostic capsules with canonical sketch JSON,
  exact profile budgets, checksum and atomic retained-state import failures.
- M32 command/profile mutation coverage and native/browser performance/resource
  envelopes.

### Changed

- Explicit accepted-contact topology and root-isolation retries harden movable
  fillet closure and NURBS self-intersection behavior.
- Cycle-area integration apportions the unchanged uncertainty target across directed
  fragments before independently validating the complete cycle.
- The release gate now includes M32 mutation/performance suites and an unfiltered,
  no-retry desktop browser run.

## [0.1.0] - 2026-07-21

Initial supported preview release.

### Added

- Pure-Rust normalized nonlinear solving with independent residual validation,
  component-local rank and mobility, strict hard/temporary/preference priority,
  sparse hard steps, bounds, diagnostics and persistent sessions.
- Persistent 2D sketch documents covering analytic curves, Beziers, conics,
  B-splines, NURBS, generic contact/tangency, differential constraints,
  associative constructions, visual profiles, generic fillets and trim views.
- Persistent planar and spatial linkage/assembly models with explicit modes,
  gauge-separated mobility, continuation and validated velocity queries.
- Canonical sketch JSON v4 with v1-v4 input migration, planar linkage JSON v1
  and spatial assembly JSON v1.
- Separate WASM diagnostic playground consuming the public domain APIs.

### Compatibility

- Rust `1.89` is the minimum supported Rust version.
- This is a `0.x` preview. Domain workflows and persisted schemas follow the
  compatibility policy; low-level compiler/runtime inspection remains unstable.

[Unreleased]: docs/API_COMPATIBILITY.md
[0.2.0]: docs/API_COMPATIBILITY.md
[0.1.0]: docs/API_COMPATIBILITY.md
