# ADR 0018: Spatial assembly document persistence

Status: accepted

## Context

M20 established runtime spatial bodies, concrete feature types, joints, mates,
coordinates, drivers and explicit mode monitors. M23 added accepted branch
hysteresis, mode changes, continuation and velocity fields. Runtime IDs carry an
assembly namespace and local ordinal; serializing them would make identity depend
on one process instance and would not preserve accepted gauge or hysteresis state.

The persistence boundary must also reject a syntactically valid document unless
its ordinary spatial equations can be solved and independently validated. Private
gauges, compiled variable IDs, sparse caches and continuation predictor state are
not domain state.

## Decision

`SpatialAssemblyDocument` is a versioned envelope with one 128-bit document ID and
globally unique document-local 128-bit IDs for every body, feature, source,
coordinate and monitor. Lowering always allocates a fresh runtime namespace and
returns a complete bidirectional `SpatialAssemblyRuntimeMap`; runtime IDs are
never serialized.

Topology stores model scale, labels, local feature geometry, every source in its
semantic insertion order, topology-only coordinates and explicit mode monitors.
Accepted state separately stores revision, one accepted `Pose3` per body, current
driver targets and every branch-boundary hysteresis latch. Gauge policy references
persistent body IDs. Physical ground targets remain topology while accepted body
poses remain accepted state; their local difference must satisfy the normalized
`1e-9` acceptance tolerance, and lowering retains the exact serialized target.

JSON uses fixed lowercase hexadecimal IDs, tagged closed enums and
`deny_unknown_fields`. Import is size bounded, rejects unsupported versions,
duplicate or zero IDs, incomplete state, invalid source order, stale/wrong-kind
references, non-finite geometry and invalid domain values. Canonical output sorts
identity-keyed records, explicit gauge references and boundary latches while
preserving semantic source order.

Lowering creates bodies and concrete features first, then sources in persisted
order. A driver causes its referenced coordinate to be lowered immediately after
that coordinate's already-existing parent source; remaining row-free coordinates
follow deterministically. Monitors and gauges lower last. This preserves source
equation order without inventing serialized runtime keys.

`SpatialAssemblyDocumentSession` either captures an already accepted spatial
session or lowers, solves and independently validates an imported document before
publication. Serialized boundary latches are restored only after their complete
topology-implied boundary set matches the accepted solve. `replace_json` builds a
complete replacement and clone-swaps it, so any parse, lowering, solve or
validation failure retains every prior accepted view.

Version 1 has no implicit migration. M29 owns future public compatibility policy;
unknown versions reject until an explicit migration exists.

## Consequences

- Persistent identities survive deterministic JSON and fresh runtime remapping.
- Bodies, every concrete feature/source/coordinate family, gauges, driver targets,
  windings, parity/side/orientation modes and boundary hysteresis round-trip.
- Malformed or physically invalid imports cannot publish a success-like session.
- Private gauges, core equations, rank caches and continuation scratch state do
  not leak into persistence.
- The schema remains pure safe Rust and adds no physics semantics.
