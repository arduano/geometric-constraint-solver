<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0035: Retained drafting relation lifecycle and persistence

Status: accepted for M71 planning; implementation and human UAT are pending

## Context

The ordinary `SketchDocument` and `RetainedSketchDocumentSession` own persistent constraints,
source order, accepted/rejected state, activation, diagnostics, prepared publication, history and
application-workspace restore. A separate M36/M37 `DocumentSemanticSourceCatalog` already exposes
runtime mathematics for point-pair Horizontal/Vertical, Concentric and Collinear, but that catalog
does not participate in the ordinary retained lifecycle or editor transaction model.

M70 therefore kept bare-point alignment tracking-only and deferred richer relation inference. It
would be tempting either to attach the separate catalog to the retained session or to represent
these relationships as existing but semantically different constraints. Both choices create two
source/history authorities or misleading document intent.

Persistence also needs an explicit decision. Canonical sketch v4 is frozen, yet the private
`SketchDocumentV4` wire struct currently serializes the evolving in-memory `DocumentConstraint`
directly. Adding an in-memory relation variant without first separating that wire type would
silently expand the released v4 language.

## Decision

### Promote four ordinary retained constraints

M71 adds point-pair Horizontal, point-pair Vertical, Concentric and Collinear directly to
`DocumentConstraintDefinition`. Each owns one ordinary persistent constraint ID and source ID and
participates in source ordering, validation, lowering, audit/diagnostics, activation,
suppression/reactivation, deletion, dependency closure, prepared operations, accepted/rejected
publication, Undo/Redo, reload and reproduction like every existing retained constraint.

Point-pair Horizontal/Vertical uses stored `DesignPointId` operands only. Concentric uses
`DocumentCenterRef`; Collinear uses directed `DocumentLineSupportRef`. Repeated operands, missing or
unsupported semantic features and degenerate affine supports reject transactionally.

The ordinary lowering calls the existing sketch mathematics:

- `Sketch::add_horizontal_points` / `Sketch::add_vertical_points` — one row;
- semantic-center resolution followed by `Sketch::add_coincident` — two rows; and
- directed-support resolution followed by `Sketch::add_collinear` — two rows.

No residual or Jacobian implementation is added. Source-level audit descriptors still group every
runtime row under the one persistent relation source.

### Keep the M37 catalog separate

`DocumentSemanticSourceCatalog` and `DocumentSemanticCatalogSession` remain separate. M71 does not
embed either in `RetainedSketchDocumentSession`, does not migrate catalog sources automatically and
does not create aliases between catalog and ordinary source identities. Their possible
consolidation or deprecation requires a later compatibility milestone.

### Preserve frozen v4 and extend only unsupported draft v5

Before the in-memory enum grows, M71 introduces private frozen-v4 constraint wire DTOs and explicit
bidirectional conversion for every v4-supported variant. Canonical-v4 export rejects a document
containing an M71 relation with `DocumentError::UnsupportedM71State`; its reader remains strict and
cannot parse M71 tags.

The private unsupported draft-v5 envelope adds a default-empty side section containing only M71
retained constraints. The embedded frozen-v4 graph carries all other graph data and the complete
`source_order`; side constraints are merged by persistent identity before the restored document is
validated. Duplicate, missing, unordered, foreign or malformed identities reject atomically.
Existing draft-v5 bytes decode unchanged because the side section uses `#[serde(default)]` and is
omitted when empty.

The application workspace remains v5 with `WorkspaceDocumentEncoding::DraftV5`. This is not a
canonical sketch-v5 declaration and does not change the supported persistence table.

### Extend the headless owner, not the browser

`geosolve-constraint-editor` owns contextual applicability, operand collection, center/support
certification, inferred-candidate ranking, hysteresis, suppression, ambiguity and atomic commit.
Horizontal and Vertical may resolve one line or two persistent points. Concentric and Collinear
are explicit intents because Coincident and Parallel have different semantics.

M70-style construction plans gain prospective curve operands so Concentric or certified Collinear
can refer to geometry created by the same operation. Only accepted native semantic evidence can
create a durable proposal. Derived midpoint alignment remains tracking-only; sampled intersections
and generic nonlinear roots are deferred. The workbench renders returned DTOs and labels only.

## Consequences

- A host sees one coherent retained constraint lifecycle rather than coordinating two catalogs.
- M70 point tracking can become durable when, and only when, both operands are persistent points.
- Center equality and supporting-line collinearity are explicit intent rather than aliases based on
  coordinates.
- Canonical v4 remains byte-compatible, at the cost of maintaining an explicit frozen wire DTO and
  rejecting v4 export for M71 documents.
- Unsupported draft v5 can round-trip the active workbench state without prematurely freezing a
  supported sketch-v5 schema.
- The first implementation step is persistence isolation, before adding in-memory variants.

## Rejected alternatives

- **Attach the M37 catalog to the retained session:** rejected because it creates overlapping
  source order, diagnostics, history, prepared-input and persistence authorities.
- **Broaden point-pair operands to `DocumentPointRef`:** deferred because derived operands can add
  incidence rows and turn one ordinary relation into several runtime sources.
- **Encode point alignment with `FixedCoordinate` or a zero dimension:** rejected because it
  changes semantics and ties intent to a coordinate frame.
- **Treat Concentric as Coincident points or Collinear as Parallel:** rejected because the
  persistent operand identity and mathematical relation differ.
- **Expand sketch v4 or declare canonical v5 now:** rejected because v4 is frozen and M71 does not
  provide the full schema-freeze/release decision required for supported v5.
