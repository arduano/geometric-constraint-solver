# Changelog

All notable changes to GeoSolve are documented here. The project follows the
versioning and deprecation policy in `docs/API_COMPATIBILITY.md`.

## [Unreleased]

### Changed

- Adopted the M33-M55 production-embedding roadmap for the planar sketch engine,
  including the standard CAD catalog, immutable host inputs, CAD-like desktop demo,
  companion operations/topology APIs and four consolidated human UAT gates.

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
