# ADR 0008: Sketch design graph, persistent IDs and closed curve definitions

Status: accepted

## Context

The baseline sketch model has stable runtime keys and pair-specific compiler paths for points, segments, circles and arcs. Adding Bezier, conic, B-spline and NURBS families through entity-pair matches would multiply lifecycle, persistence and branch-state code and would expose runtime keys as accidental document identity.

## Decision

M13 introduces a versioned sketch design graph with persistent identity independent from runtime storage.

Persistent IDs are opaque 128-bit document values, never reused within a document and serialized as fixed lowercase hexadecimal. Creation/import APIs reject duplicates. Runtime stores continue to use generational slot-map keys; loading sorts/remaps persistent IDs deterministically and records the mapping used by compiler, audit and commit paths. Runtime keys are never serialized.

The graph owns:

- `DesignPoint` values and typed `DesignScalar` values with units and validated domains;
- one `CurveId` store containing a closed `CurveDefinition` enum;
- semantic `FeatureRef` values for endpoints, centers, axes, controls and fixed curve locations;
- stable contact slots with parameter domain, selected span, periodic winding, neighborhood and orientation state;
- constraints, driving/reference measurements and explicit branch state by persistent ID.

“Closed curve definition” means an exhaustive, versioned, serializable built-in enum. It does not mean that every curve is geometrically closed. The enum starts with migrated segment/circle/arc definitions and is extended by the ordered curve milestones. Unknown future variants are rejected or migrated by document-version policy rather than interpreted through an unversioned plugin payload.

Curve evaluation and generic contact/tangency residual construction use private or crate-private adapters. No public generic curve trait is exposed before all built-in families prove the seam. Each compiled source owns dependency collection, residual construction, validators, candidate-to-design commit mapping and audit metadata.

Editable controls, weights and latent contact coordinates are explicit dependencies and variables. Span, winding, contact neighborhood, tangent orientation and branch selection are persistent discrete state outside AD. Bounds are delegated to the M10 active-set contract rather than silently extending a bounded curve.

Documents use a versioned envelope containing topology, accepted continuous geometry, constraints/dimensions and all discrete state. M13 establishes migration/remapping; M20 completes persistence for every Deliverable 1 family.

The implementation is pure safe Rust with no `unsafe` code. It does not move sketch variants into `geosolve-core` and does not add third-party curve plugins.

## Consequences

- Generic equation templates replace geometry-pair lifecycle fan-out without erasing domain semantics.
- Audit and diagnostics can map runtime rows back to stable document sources.
- Deleted runtime keys can be reused internally without corrupting persisted references.
- Malformed references, duplicate IDs, invalid domains and unknown variants reject before solving.
- S1-S3 and the complete M5/M7 corpus remain migration regression tests.
