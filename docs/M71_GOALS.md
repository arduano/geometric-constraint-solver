<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 goals — Retained drafting relations

Status: active and amended on 2026-08-14 for M71-F005/M71-F006. Cross-axis point-pair composition
and the tighter default capture envelope are clean-qualified and published as a byte-verified
immutable replacement. Earlier publications remain historical; supervising-human UAT is pending.

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
- One remembered persistent-point/native-midpoint axis may compose with the complementary exact
  Cartesian direction of a new line/polyline span. The candidate owns the exact coordinate
  intersection and both retained relations atomically. Exact axis-aligned remembered directions
  qualify; oblique and same-axis relations remain alternatives, and distinct semantic operands
  remain ambiguous.
- Two distinct remembered stored points may contribute complementary axes to one point-operand
  stage. Horizontal supplies Y, Vertical supplies X, and the canonical H-then-V candidate owns the
  exact intersection, both constraint-backed guides and one atomic two-relation plan. The same
  semantic anchor never composes with itself. Multiple exact pairings remain `Ambiguous`, and a
  competing F004 point-axis-plus-span-direction candidate remains available rather than collapsing
  to a bare direction singleton. Both point-axis latches retain through the exit band, and line and
  polyline stage handoff preserves both positional references.
- The default capture envelope is deliberately tighter: points, semantic centers and native
  midpoints use inclusive 6 px enter / 9 px leave thresholds; curve contacts use 8/12 px; and
  world, remembered and point-tracking directions use 3/5 degrees. Hosts retain the public
  validated policy override; this changes no equation, branch or persistence rule.

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

The original four-definition candidate was withdrawn because it encoded midpoint-axis alignment as
tracking-only. The later F003 candidate was withdrawn because it could not compose a remembered
endpoint axis with a complementary new-span direction. The clean, byte-verified F004 candidate is
now also withdrawn from continued UAT because it predates the F005 cross-axis point-pair and F006
default-capture corrections. All earlier immutable snapshots remain historical evidence. Clean
source `f8a45ae7b355ab9874bf268c9950e369814e8432` and its byte-verified F005/F006 replacement are the
current mechanical UAT authority.

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
