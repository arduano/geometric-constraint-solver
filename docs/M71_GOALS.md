<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 goals — Retained drafting relations

Status: active and amended on 2026-08-13 for M71-F003. The midpoint-axis correction passes the
complete dirty-tree development gate; replacement clean qualification, publication and
supervising-human UAT are pending.

M70 proved reusable auto-constraint interaction using only constraint definitions already owned by
the ordinary retained document/editor workflow. M71 closes the highest-value gap exposed by that
cut: four relation families whose runtime mathematics existed but whose ordinary retained
lifecycle was missing, plus the narrowly authorized M71-F003 native-span midpoint-axis family.

## Active scope

M71 promotes exactly these six relation definitions into `DocumentConstraintDefinition`:

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
HorizontalPointToMidpoint {
    point: DesignPointId,
    line: CurveSpan,
}
VerticalPointToMidpoint {
    point: DesignPointId,
    line: CurveSpan,
}
```

These are ordinary retained constraints, not a second source language. Each gains the same source
identity, ordering, validation, lowering, accepted/rejected state, suppression, deletion,
dependency closure, diagnostics, prepared-work, history, persistence, reproduction and editor
publication behavior as existing ordinary constraints.

The mathematical lowering uses these sketch operations:

- point-pair Horizontal/Vertical call `Sketch::add_horizontal_points` and
  `Sketch::add_vertical_points`, producing one row each;
- point-to-midpoint Horizontal/Vertical call `Sketch::add_horizontal_point_to_midpoint` and
  `Sketch::add_vertical_point_to_midpoint`, producing one row each;
- Concentric resolves the two stored semantic centers and calls `Sketch::add_coincident`, producing
  two coordinate rows; and
- Collinear resolves the two directed affine supports and calls `Sketch::add_collinear`, producing
  the existing two line-support rows.

The midpoint-axis relation adds one linear residual, `P[c] - (A[c] + B[c]) / 2`, where Horizontal
constrains Y and Vertical constrains X. Its analytic Jacobian is `[+1, -1/2, -1/2]`, its audit row
is model-scale normalized, and acceptance independently recomputes the hard residual. No solver
priority or implicit branch rule is added. The current finite-state, rank/DOF,
redundancy/conflict and branch rules remain authoritative.

### Deliberately narrow operands

Point-pair Horizontal/Vertical accepts only stored `DesignPointId` operands in M71. It does not
accept broad `DocumentPointRef` values. M71-F003 authorizes one deliberately narrower derived
case: an explicit stored point plus a certified native line/polyline `CurveSpan` midpoint. That
midpoint is the live average of two existing endpoints, so each axis remains one source and one
hard row without a hidden point or extra incidence source.

Alignment with a remembered persistent point may become durable HorizontalPoints or
VerticalPoints. Alignment with a remembered native line/polyline midpoint may become durable
HorizontalPointToMidpoint or VerticalPointToMidpoint, and both may coexist. Other derived anchors,
including fillet-discarded midpoint occurrences and nonlinear curve-parameter midpoints, remain
tracking-only. This is a semantic boundary, not an implementation shortcut.

### Contextual authoring and drafting inference

- Existing Horizontal and Vertical contextual intents accept either one affine span or two stored
  points, in either point selection order.
- Concentric and Collinear receive explicit contextual intents. Coincident and Parallel are not
  overloaded to mean these different durable relationships.
- Center-to-center inference may propose Concentric without inventing shared point identity.
- An affine extension may propose Collinear only from exact accepted native supporting-line
  projection/direction evidence. Bounded-span escape alone, a sampled crossing, generic
  intersection or near-parallel guess is insufficient; exact native supporting-line evidence may
  remain valid beyond a finite endpoint.
- Construction commit plans gain the prospective curve slots needed for a retained relation to
  reference a curve allocated by the same construction.
- Remembered accepted native line/polyline midpoints may propose one constraint-backed axis at a
  time; an atomic construction plan may carry both axes to retain exact live centering.
  Midpoint-axis inference is native-only: `FilletDiscarded` midpoint occurrences and nonlinear
  curve-parameter midpoints remain tracking-only.

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

The previously published four-definition candidate is withdrawn because it encoded midpoint-axis
alignment as tracking-only. The correction passes provisional development qualification, but no
corrected clean candidate has completed release qualification or publication.

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

- broad derived `DocumentPointRef` operands for point-pair Horizontal/Vertical beyond the two
  explicit native-span midpoint-axis definitions;
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
