# ADR 0019: Sketch extension identity and typed attributes

Status: accepted

## Context

M11 established fixed persistent sketch identities, strict version-1 JSON and
deterministic lowering to fresh runtime IDs. M19-M22 broadened the closed curve,
constraint and dimension schemas substantially. M25-M28 will add offsets, entity
mirrors, visual profile analysis and fillet/trim state. Those additions need a
version boundary and an application join seam before introducing more persisted
variants.

Embedding CAD applications also need arbitrary layer, display, provenance,
selection-group and product-data attributes. Making `SketchDocument`, every
record, `SketchDocumentSession`, commands and results generic would propagate host
types through the solver API, complicate serde bounds and make application
metadata part of equation-state compatibility. Runtime slot-map and core source
IDs are unsuitable because lowering remaps them on every import.

## Decision

### Persistent element and source views

`DocumentElementId` is the additive typed identity seam for one document, point,
scalar, curve, contact, constraint, dimension or audit source. It exposes the
underlying fixed `PersistentId` for external joins while retaining the requested
semantic kind. `SketchDocument::element` resolves a raw persistent identity back
to its unique typed element, and `contains_element` checks current liveness.

`DocumentSourceRef` maps each `DocumentSourceId` to a
`DocumentSourceOwner::{Constraint,Dimension}`, label and suppression state.
`SketchDocument::sources` follows semantic source order. Embedders therefore do
not need runtime mappings or core source IDs to decorate audit results.

### Generic application sidecar

`SketchAttributes<T>` is bound to exactly one `DocumentId` and stores a
`BTreeMap<DocumentElementId, T>`. Insertion validates document identity, target
liveness and semantic kind. Raw IDs reused as another typed kind reject rather
than aliasing that element.

Values remain available when targets become dormant after deletion. Undo makes
the same persistent target live again; redo makes it dormant again. Applications
may inspect orphaned targets and must call `retain_live` explicitly to destroy
dormant values. This supports host history without coupling arbitrary `T` to
sketch command snapshots.

The sidecar has no serde implementation or public metadata trait. Applications
own attribute codecs, deterministic ordering guarantees, schema versions,
migrations and any combined workspace transaction/history envelope. Span-local
and visual detected-boundary attributes remain out of scope because those are not
top-level persistent document elements.

### Frozen version-one wire boundary

The public in-memory `SketchDocument` no longer doubles as its serializer. A
private `SketchDocumentV1` DTO freezes the exact version-1 fields and order.
Canonical export validates and sorts the in-memory document, then serializes that
DTO. Import reads only a version header, explicitly dispatches to the version-1
DTO, converts to the current model, validates and canonicalizes. Unknown versions
and fields remain strict errors. M25 may add a version-2 DTO and explicit v1-to-v2
migration without changing the frozen v1 type.

## Consequences

- Existing canonical version-1 JSON remains byte-for-byte unchanged.
- Application metadata cannot dirty solver components, alter rank, equations,
  accepted geometry, audit or canonical sketch JSON.
- `SketchDocument`, sessions and solver results remain non-generic and lightweight
  for native and WASM consumers.
- Attribute values can be non-cloneable or non-serializable unless the embedding
  application chooses otherwise.
- Persistent source/audit decoration no longer requires accidental compiler/core
  integration seams.
- Geometry deletion remains authoritative; sidecar liveness is always checked
  against the currently accepted document rather than inferred from command
  effects.
