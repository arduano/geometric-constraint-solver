<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0035: Retained drafting relation lifecycle and persistence

Status: accepted, implemented and explicitly approved for completed M71 on 2026-08-14. Cross-axis
point-pair composition and the tighter default capture envelope are clean-qualified, published as
a byte-verified immutable replacement and accepted under the scoped human close decision. Earlier
candidates remain historical evidence.

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

### Promote six ordinary retained constraints

M71 adds point-pair Horizontal, point-pair Vertical, point-to-native-span-midpoint Horizontal and
Vertical, Concentric and Collinear directly to
`DocumentConstraintDefinition`. Each owns one ordinary persistent constraint ID and source ID and
participates in source ordering, validation, lowering, audit/diagnostics, activation,
suppression/reactivation, deletion, dependency closure, prepared operations, accepted/rejected
publication, Undo/Redo, reload and reproduction like every existing retained constraint.

Point-pair Horizontal/Vertical uses stored `DesignPointId` operands only. The two midpoint-axis
definitions use an explicit stored point plus certified native line/polyline `CurveSpan`; each
constrains one coordinate to the live average of the span endpoints. Concentric uses
`DocumentCenterRef`; Collinear uses directed `DocumentLineSupportRef`. Repeated operands, missing or
unsupported semantic features and degenerate affine supports reject transactionally.

The ordinary lowering calls the existing sketch mathematics:

- `Sketch::add_horizontal_points` / `Sketch::add_vertical_points` — one row;
- `Sketch::add_horizontal_point_to_midpoint` /
  `Sketch::add_vertical_point_to_midpoint` — one row each;
- semantic-center resolution followed by `Sketch::add_coincident` — two rows; and
- directed-support resolution followed by `Sketch::add_collinear` — two rows.

The midpoint-axis rows use `P[c] - (A[c] + B[c]) / 2`, with Horizontal constraining Y and Vertical
constraining X. They are model-scale normalized, independently recomputed before acceptance and
have central finite-difference Jacobian coverage. Source-level audit descriptors still group every
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
validated. Duplicate, missing, unrepresented, foreign or malformed identities reject atomically;
arbitrary otherwise-valid source ordering is preserved. Existing draft-v5 bytes decode unchanged
because the side section uses `#[serde(default)]` and is omitted when empty.

The application workspace remains v5 with `WorkspaceDocumentEncoding::DraftV5`. This is not a
canonical sketch-v5 declaration and does not change the supported persistence table.

### Extend the headless owner, not the browser

`geosolve-constraint-editor` owns contextual applicability, operand collection, center/support
certification, inferred-candidate ranking, hysteresis, suppression, ambiguity and atomic commit.
Horizontal and Vertical may resolve one line or two persistent points. Concentric and Collinear
are explicit intents because Coincident and Parallel have different semantics.

M70-style construction plans gain prospective curve operands so Concentric or certified Collinear
can refer to geometry created by the same operation. Midpoint-axis inference is native-only:
remembered midpoints of accepted native line/polyline spans can create one durable midpoint-axis
relation per aligned coordinate, and both axes may coexist on one constructed point.
`FilletDiscarded` midpoint occurrences and nonlinear curve-parameter midpoints remain
tracking-only. A durable point/native-midpoint axis may compose with the complementary exact
Cartesian direction of a new line/polyline span: one candidate owns the exact coordinate
intersection and one atomic plan retains both relations. Exact axis-aligned remembered directions
qualify from their original source provenance; oblique and same-axis directions remain
alternatives and semantic ties remain ambiguous.

M71-F005 additionally permits two distinct remembered stored-point operands to own complementary
Cartesian coordinates of one constructed point. Horizontal supplies the remembered Y coordinate,
Vertical supplies the remembered X coordinate, and one H-then-V candidate publishes the exact
intersection, both constraint-backed guides and one atomic two-relation plan. One semantic anchor
cannot be paired with itself: persistent-point identity remains structural intent rather than two
redundant axis relations. Competing pairings remain `Ambiguous`; the pair and an F004
point-axis-plus-span-direction bundle remain explicit alternatives when both are exact. Line and
polyline stage handoff retains both positional references without leaking direction-only memory.

M71-F006 narrows only the default headless capture envelope: stored points, semantic centers and
native midpoints enter/leave at 6/9 screen pixels; curve contacts enter/leave at 8/12 pixels; and
world, remembered and point-tracking directions enter/leave at 3/5 degrees. Comparisons remain
inclusive and explicit host policies remain authoritative. Sampled intersections and generic
nonlinear roots remain deferred. The workbench renders returned DTOs and labels only.

## Consequences

- A host sees one coherent retained constraint lifecycle rather than coordinating two catalogs.
- M70 point tracking can become durable for two persistent points and for one persistent point
  aligned to a certified native line/polyline span midpoint.
- Center equality and supporting-line collinearity are explicit intent rather than aliases based on
  coordinates.
- Two distinct remembered stored points can define one exact Cartesian endpoint intersection
  without persistent-ID tie breaking or loss of the earlier endpoint-axis/span-direction bundle.
- The default capture envelope is less eager while retaining an explicit hysteresis band; hosts may
  continue to supply a validated non-default policy.
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
- **Represent the span midpoint as a hidden point:** rejected because it invents geometry and a
  second lifecycle identity for an exact live endpoint average.
- **Encode point alignment with `FixedCoordinate` or a zero dimension:** rejected because it
  changes semantics and ties intent to a coordinate frame.
- **Treat Concentric as Coincident points or Collinear as Parallel:** rejected because the
  persistent operand identity and mathematical relation differ.
- **Expand sketch v4 or declare canonical v5 now:** rejected because v4 is frozen and M71 does not
  provide the full schema-freeze/release decision required for supported v5.
