# ADR 0008: Sketch document, commands, persistent IDs and closed curve definitions

Status: accepted

Implementation status: M11 complete for the line/polyline/circle/circular-arc
baseline. The reusable Rust implementation provides canonical version-one JSON,
strict typed validation, deterministic runtime remapping, accepted-state projection,
typed commands, atomic branch/contact edits and accepted-only undo/redo. M12 remains
responsible for Bezier variants and geometry-generic contact/tangency residuals.

## Context

The baseline sketch model has stable runtime keys and pair-specific compiler paths for points, segments, circles and arcs. Adding Bezier, conic, B-spline and NURBS families through entity-pair matches would multiply lifecycle, persistence and branch-state code and would expose runtime keys as accidental document identity. The 2D Sketch Playground Alpha also needs one reusable command/history/JSON model so browser interaction cannot become authoritative domain state.

## Decision

M11 introduces `SketchDocument`, a versioned sketch design graph with persistent identity independent from runtime storage, plus typed commands and accepted-command history. M12 extends the same graph with editable quadratic/cubic Bezier and generic curve contact/tangency.

Persistent IDs are opaque 128-bit document values, never reused within a document and serialized as fixed lowercase hexadecimal. Creation/import APIs reject duplicates. Runtime stores continue to use generational slot-map keys; loading sorts/remaps persistent IDs deterministically and records the mapping used by compiler, audit and commit paths. Runtime keys are never serialized.

The document owns:

- `DesignPoint` values and typed `DesignScalar` values with units and validated domains;
- one `CurveId` store containing a closed `CurveDefinition` enum;
- semantic `FeatureRef` values for endpoints, centers, axes, controls and fixed curve locations;
- stable contact slots with parameter domain, selected span, periodic winding, neighborhood and orientation state;
- constraints, driving/reference measurements and explicit branch state by persistent ID.

The M10-M14 alpha geometry variants are point, line/polyline, circle, circular arc and quadratic/cubic Bezier. Rectangle is a command macro that creates ordinary entities and fixed/coincident/horizontal/vertical relationships; it is not a stored solver primitive. The alpha constraint/dimension variants are fixed, coincident, horizontal, vertical, point-on-curve, parallel, perpendicular, equal length/radius, midpoint, symmetry, distance, length, radius, diameter, oriented angle, generic line-curve and curve-curve contact/tangency, and driving/reference dimensions.

“Closed curve definition” means an exhaustive, versioned, serializable built-in enum. It does not mean that every curve is geometrically closed. The enum starts with migrated line/polyline-segment/circle/arc definitions in M11, adds quadratic/cubic Bezier in M12 and is extended by the ordered production curve milestones. Unknown future variants are rejected or migrated by document-version policy rather than interpreted through an unversioned plugin payload.

Curve evaluation and generic contact/tangency residual construction use private or crate-private adapters. No public generic curve trait is exposed before all built-in families prove the seam. Each compiled source owns dependency collection, residual construction, validators, candidate-to-design commit mapping and audit metadata.

Editable controls, weights and latent contact coordinates are explicit dependencies and variables. Span, winding, contact neighborhood, tangent orientation and branch selection are persistent discrete state outside AD. Bounds are delegated to the M10 active-set contract rather than silently extending a bounded curve.

Commands address persistent IDs and cover create, edit, delete, suppress/unsuppress and driving/reference dimension changes. A command becomes part of reusable `CommandHistory` only after `SketchSession` independently validates and atomically accepts it. Failed commands change neither document nor history. Undo/redo restores accepted document states through the same session boundary and preserves persistent IDs and discrete state.

Documents use canonical versioned JSON containing topology, accepted continuous geometry, constraints/dimensions and all discrete state. Import validates version, duplicate IDs, references, numeric domains and finiteness before replacing an accepted document. M11 establishes alpha migration/remapping; M22 completes persistence for every Deliverable 1 family.

Selection, hit testing, tool state, rendering and browser `localStorage` are not document fields and remain web-only under ADR 0010. Curve evaluation, constraints, serialization and command validation are reusable Rust behavior; the web crate contains no equations.

The implementation is pure safe Rust with no `unsafe` code. It does not move sketch variants into `geosolve-core` and does not add third-party curve plugins.

## Consequences

- Generic equation templates replace geometry-pair lifecycle fan-out without erasing domain semantics.
- Audit and diagnostics can map runtime rows back to stable document sources.
- Deleted runtime keys can be reused internally without corrupting persisted references.
- Malformed references, duplicate IDs, invalid domains and unknown variants reject before solving.
- Undo/redo and import failures retain the previous accepted document, history position and branch state.
- S1-S3 and the complete M5/M7 corpus remain migration regression tests.
