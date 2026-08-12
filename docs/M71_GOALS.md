<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 goals — Retained drafting relations

Status: active, formally scoped and authorized for implementation planning on 2026-08-12. No M71
production implementation or release candidate exists yet.

M70 proved reusable auto-constraint interaction using only constraint definitions already owned by
the ordinary retained document/editor workflow. M71 closes the highest-value gap exposed by that
cut: four relations whose runtime mathematics exists but whose ordinary retained lifecycle is
missing.

## Active scope

M71 promotes exactly these relation definitions into `DocumentConstraintDefinition`:

```rust
HorizontalPoints {
    first: DesignPointId,
    second: DesignPointId,
}
VerticalPoints {
    first: DesignPointId,
    second: DesignPointId,
}
Concentric {
    first: DocumentCenterRef,
    second: DocumentCenterRef,
}
Collinear {
    first: DocumentLineSupportRef,
    second: DocumentLineSupportRef,
}
```

These are ordinary retained constraints, not a second source language. Each gains the same source
identity, ordering, validation, lowering, accepted/rejected state, suppression, deletion,
dependency closure, diagnostics, prepared-work, history, persistence, reproduction and editor
publication behavior as existing ordinary constraints.

The mathematical lowering reuses existing sketch operations:

- point-pair Horizontal/Vertical call `Sketch::add_horizontal_points` and
  `Sketch::add_vertical_points`, producing one row each;
- Concentric resolves the two stored semantic centers and calls `Sketch::add_coincident`, producing
  two coordinate rows; and
- Collinear resolves the two directed affine supports and calls `Sketch::add_collinear`, producing
  the existing two line-support rows.

No new residual, equation, Jacobian or solver priority is required. The current independent hard
validation, finite-state, rank/DOF, redundancy/conflict and branch rules remain authoritative.

### Deliberately narrow operands

Point-pair Horizontal/Vertical accepts only stored `DesignPointId` operands in M71. It does not
accept broad `DocumentPointRef` values. A derived midpoint, endpoint projection, center or other
semantic anchor can require extra incidence equations, causing one user relation to lower into
several runtime sources and weakening the ordinary one-source diagnostics/lifecycle contract.

Consequently, alignment with a remembered persistent point may become durable HorizontalPoints or
VerticalPoints. Alignment with a midpoint or another derived anchor remains explicitly
tracking-only in M71. This is a semantic boundary, not an implementation shortcut.

### Contextual authoring and drafting inference

- Existing Horizontal and Vertical contextual intents accept either one affine span or two stored
  points, in either point selection order.
- Concentric and Collinear receive explicit contextual intents. Coincident and Parallel are not
  overloaded to mean these different durable relationships.
- Center-to-center inference may propose Concentric without inventing shared point identity.
- An affine extension may propose Collinear only from exact accepted native line-support
  projection/direction evidence. A sampled crossing, generic intersection, near-parallel guess or
  bounded-span escape is not certified collinearity.
- Construction commit plans gain the prospective curve slots needed for a retained relation to
  reference a curve allocated by the same construction.

All M70 rules remain in force: bounded candidate generation, deterministic ranking, hysteresis,
semantic suppression, exact ambiguity, authenticated accepted-scene authority, one atomic
construction-plus-relation transaction, redundancy rejection and one-step Undo/Redo.

## Persistence decision

Canonical sketch v4 is frozen. Its current implementation incorrectly reuses the evolving
in-memory `DocumentConstraint` type directly as the v4 wire type, so M71 first separates a private
frozen v4 constraint DTO before adding the new in-memory variants.

- Canonical-v4 export continues writing the exact frozen v4 language and rejects any M71 relation
  with typed `DocumentError::UnsupportedM71State`.
- The explicitly unsupported draft-v5 envelope gains an optional, dedicated retained-planar-
  constraint side section. `#[serde(default)]` preserves decode of every existing draft-v5 value.
- The embedded graph retains the complete `source_order`. Restore merges the side-section
  constraints into the in-memory graph before final validation, so source order, IDs and ownership
  remain exact.
- Application workspace remains version 5 and continues labelling these document bytes `DraftV5`;
  no workspace-version bump is needed.
- Frozen v1-v4 readers and bytes remain unchanged and strictly reject M71 syntax. M71 does not
  declare or freeze canonical sketch v5.

## Architectural ownership

M71 does not attach `DocumentSemanticSourceCatalog` or `DocumentSemanticCatalogSession` to
`RetainedSketchDocumentSession`. That M36/M37 catalog remains a separate compatibility/domain
surface. Reusing its runtime math is correct; combining two lifecycle, history and persistence
authorities is not.

ADR 0035 owns this retained-relation and persistence decision. `PLAN.md` owns execution order,
`ACCEPTANCE.md` owns objective completion, and `docs/M71_UAT.md` owns the eventual human scorecard.
`docs/M71_IMPLEMENTATION.md` is created only after implementation begins and can record evidence
that actually exists.

## Deferred backlog

The rest of the former M71 candidate notes remain useful, but are not part of this milestone:

- broad derived `DocumentPointRef` operands for point-pair Horizontal/Vertical;
- integration or retirement of the separate M36/M37 semantic catalog/session;
- certified generic intersection points, multiple-root enumeration and bounded-span extension
  contacts;
- circular/elliptic quadrant anchors;
- nonlinear tangent or normal inference and automatic branch selection;
- equality, curvature-equality or symmetry inference;
- host/workplane axes, grids and linear/angular increments;
- canonical sketch v5 or supported draft-v5 status;
- persistent inference wake/reference state;
- browser-owned geometric policy, browser E2E, mobile behavior or any legacy UI.

Future milestones may select these independently. They must not be smuggled into M71 as fixed
coordinates, zero dimensions, hidden construction geometry, coordinate proximity or implicit
branch choices.
