<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0036: Headless geometry variants and atomic construction recipes

Status: accepted for active M78 on 2026-08-17; hardened product implementation is committed through
`4845df7`; clean release nomination, human UAT, publication and closeout remain pending.

## Context

ADRs 0029, 0034 and 0035 place deterministic tool progression, inference and retained relation
publication in `geosolve-constraint-editor`. The surviving workbench nevertheless exposes one flat
button per coarse `EditorTool`, and several public construction proposals encode one historical
recipe rather than an exact CAD-style variant. Rectangles, circle/arc alternatives, midpoint lines,
periodic splines and an endpoint Tangent Arc need different stage meaning and intrinsic relations
even when they create the same underlying curve family.

Adding one `EditorTool` case for every UI menu item would conflate legacy compatibility, geometry
kind and authoring recipe. Implementing variants in the browser would be worse: stage count,
circumcircle validity, tangent-arc branch choice, Shift squares, contact allocation and intrinsic-
versus-inferred precedence would cease to be reusable or natively testable.

The existing atomic construction path is inference-oriented. Some historical geometry-only paths
can apply a proposal separately, while draft state keeps parallel point and coordinate arrays whose
meaning depends on the active coarse tool. M78 needs one exact transaction for multi-curve recipes,
derived coordinate samples, created-curve incidence and ordinary intrinsic relations, with failure
retaining the terminal draft for correction.

## Decision

### Separate family, exact variant and legacy projection

M78 adds non-exhaustive `GeometryToolFamily` and `GeometryToolVariant` public enums. Exact variants
have stable string keys, one family, deterministic family ordering, a family default and a coarse
legacy `EditorTool` projection. `ConstraintEditor::activate_geometry_tool` and
`ConstraintEditor::geometry_tool_variant` are the exact activation/inspection seam. Existing
`EditorTool` activation remains compatible and selects the corresponding default variant; M78 does
not multiply the legacy enum into 25 public cases.

The admitted catalog is exactly the nine families and 25 variants in `docs/M78_GOALS.md`. Adding a
later variant is an explicit public catalog change, not a browser-only menu edit.

### Publish semantic draft state and modifier intent

Each exact variant owns an exhaustive semantic stage table. Public `GeometryDraftStatus` reports
variant, named stage, completed/required progress, finishability, explicit branch state and typed
live measurements. Variable-length Polyline/NURBS status is not inferred by the web adapter from a
coordinate count.

`DraftAuthoringInput` contains existing `DraftInferenceInput` plus independent `regularized`
recipe intent. Ctrl/Cmd suppression enters the inference field; Shift square regularization enters
`regularized`. Therefore Shift+Ctrl/Cmd suppresses ambient inference but still commits an intrinsic
square. Compatibility pointer wrappers use `regularized = false`.

Tab candidate cycling, complementary arc-sweep flip, unfinished-stage step-back and finish/cancel
are typed editor actions. Escape cancels one shape while preserving the exact active variant; a
second Escape activates Select. The browser maps platform keys to these actions but owns none of
their eligibility or state transitions.

### Use typed draft operands

Private draft state replaces parallel point/position meaning with typed operands that distinguish:

- a stored/reused `ConstructionPoint`;
- a coordinate-only recipe sample;
- a sample on a prospective created curve, optionally backed by an existing persistent point; and
- an existing curve contact with explicit span/domain/neighbourhood data.

This distinction prevents a diameter, three-point circle or arc sample from inventing a persistent
rim point while still allowing a snapped existing point to receive ordinary created-curve
`PointOnCurve`. It also makes Tangent Arc contact and midpoint-line centre intent explicit before
any persistent identity is allocated.

### Make every recipe one authenticated construction plan

Every interactive construction, including geometry-only output, lowers to
`CommitConstructionPlan`. The plan owns prospective point/curve identities, explicit rectangle
loops and open/closed polylines, sweep-explicit arcs, per-curve Profile/Construction roles and
ordered `ConstructionRelationDefinition` values. Existing proposal entry points remain for
compatibility but do not define M78 interactive semantics.

Each relation records one provenance:

1. `RecipeIntrinsic` for relations that define the selected variant;
2. `RecipeRegularization` for Shift square `EqualLength`; or
3. `AutoInference` for ambient M70/M71 inference.

The trial applies intrinsic sources first, regularization second and compatible inference in stage
order. An ambient source that conflicts with or is fully/partially implied by a recipe cannot make
the recipe fail; recipe intent takes precedence and the redundant inference is not persisted. An
ambient source that adds a compatible orientation remains ordinary durable intent. Controlled plans
charge validation and deterministic proposal-specific lowering work before candidate cloning or
allocation.

Every accepted plan allocates on one cloned retained session, solves once, independently validates
finite geometry/domains/hard residuals and publishes once through exact compare-and-swap as one
history entry. The coordinator records successful publication of the matching expected input/plan;
only that evidence lets a positive acknowledgement consume the terminal draft. Rejection preserves
document, accepted scene, history, allocator high-water, preview and terminal draft. After an
intervening retained edit, typed point/contact/reference operands and Tangent Arc endpoint jets must
reauthenticate from the next exact accepted scene before the prefix can be reused. A missing
dependency stays a local recoverable draft issue.

### Express recipes through ordinary durable geometry and relations

Rectangles contain four explicit shared-corner line curves. Aligned forms use ordinary H/V intent;
oriented forms use ordinary perpendicular/parallel intent. Center forms add a visible Construction
diagonal and one centre Midpoint relation. Shift adds ordinary EqualLength. No rectangle adds a
lock or dimension.

Midpoint Line stores its centre and commits ordinary Midpoint. Diameter and three-point circle/arc
recipes derive centre/radius analytically, reject scale-aware collinearity and create no synthetic
rim point. Existing snapped rim points receive created-curve PointOnCurve in the same plan. Arc and
elliptical-arc sweeps remain explicit durable branch state.

Representable midpoint/reflection and circle projection use finite algebra that avoids overflowing
endpoint sums, doubled centres or radius ratios. Circumcircle derivation prefers translated
normalized chords and falls back to normalized absolute coordinates when finite chord subtraction or
length is not representable; the rounded result must still pass local point-to-centre incidence.
Existing sketch midpoint/symmetry equations retain their Jacobians but use the same overflow-safe
midpoint evaluation, while segment/conic validation uses overflow-safe Euclidean norms. This adds no
residual or relaxed validity threshold.

Tangent Arc is limited to finite jets at endpoints of native open curves. For source endpoint `S`,
target `E` and the selected outgoing unit normal `n`, it computes

```text
center = S + n * |E-S|² / (2 * dot(E-S, n))
```

The implementation evaluates the equivalent scale-safe product in a translated or absolute-
normalized frame and validates both requested endpoints against the rounded centre/radius. It
rejects zero chord, zero/invalid jet, zero denominator, non-finite radius, failed endpoint incidence
or vanishing sweep. It persists the existing generic curve-tangency definition with explicit source
contact, orientation, endpoint neighbourhood and created-arc sweep. No new tangency residual or
browser branch heuristic is introduced.

### Keep the workbench a thin family presenter

`geosolve-demo-web` groups the exact returned catalog into persistent bottom-left family overlays,
remembers session-local variant/options, renders published stages/previews/status and forwards
platform actions. It does not reconstruct a rectangle, circumcircle, ellipse projection, tangent
arc, point/contact identity, relation order or failure policy. Invalid family-local inputs cannot
block a different active variant. Headless status omits any nonrepresentable derived measurement;
the adapter cannot manufacture NaN/Inf copy or promote a local draft issue to global Problems state.

## Consequences

- Hosts can offer rich CAD-style recipes without duplicating geometric intelligence or growing a
  coarse compatibility enum for every menu item.
- All multi-curve geometry and intrinsic/ambient relations share one independently validated
  retained transaction and one Undo/Redo step.
- Recipe precedence is reviewable provenance rather than an accident of source insertion order.
- Operation budgets and positive acknowledgement are authenticated before mutation/draft
  consumption rather than inferred from a host boolean.
- Correction-ready semantic operands survive intervening edits only after scene reauthentication;
  deleted dependencies fail locally.
- Coordinate-only samples no longer masquerade as stored points; snapped existing points remain
  associative through ordinary incidence.
- Scale-safe arithmetic retains representable finite geometry and still fails closed when the
  rounded derived result cannot satisfy local incidence.
- Explicit stage/branch/status DTOs improve prompts, accessibility, native/WASM parity and invalid-
  terminal recovery.
- The private draft and construction-plan refactor is larger than adding web buttons, but it
  removes parallel authoring authorities and supports later variants without browser equations.

## Rejected alternatives

- **Add 25 `EditorTool` cases:** rejected because it breaks the useful coarse compatibility seam
  and confuses geometry kind with authoring recipe.
- **Implement variants in JavaScript/DOM state:** rejected because recipe geometry, branches,
  inference precedence and atomic failure would become presentation-owned.
- **Commit geometry and add relations afterward:** rejected because failure could publish partial
  shapes, split history and consume persistent identities.
- **Represent every sampled rim/trim as a point:** rejected because most recipe samples are
  coordinates; synthetic points create false selection, persistence and constraint semantics.
- **Encode squares or centre rectangles with locks/dimensions:** rejected because those sources
  overconstrain and misrepresent the selected construction intent.
- **Add a special Tangent Arc residual:** rejected because the existing generic tangency relation
  already owns contact/orientation mathematics and audit semantics.
- **Choose ambiguous branches from coordinates after commit:** rejected because sweep, orientation,
  topology and contact neighbourhood must remain explicit draft/document state.

## Scope boundary

M78 does not include two-/three-tangent or tangent-tangent-radius circles, interior/periodic Tangent
Arc, curve/curve intersection Point inference, fit-point splines, polygons, slots or duplicate
radius/diameter center-circle tools. It adds no solver equation, curve family, canonical persistence
version, weighted-priority substitute, hidden construction geometry, mobile UI or B-rep operation.

## Verification

Direct `geosolve-constraint-editor` and retained-coordinator tests own catalog exhaustiveness,
semantic stages, modifier separation, relation provenance/order, every recipe's typed operands,
invalid/redundant/stale rollback, branch/contact identity, history and native/WASM parity. Thin demo
tests own only event mapping, overlay lifecycle, accessibility and headless rendering. The stable
golden authoring/scene matrix expands only for a reviewed systemic catalog or lifecycle dimension;
isolated findings receive focused owner regressions first. M78-F001 through M78-F010 are recorded in
`docs/M78_IMPLEMENTATION.md` and `docs/SCENARIOS.md`. At product commit `4845df7`, focused evidence
is 362/362 editor library tests, 32/32 geometry-variant cases, 7/7 editor extreme-finite cases and
1/1 sketch extreme-finite case, plus passing editor warnings-denied Clippy. The unchanged 270-case
golden survey/check/clean sequence also passes. Complete clean workspace/WASM/Rustdoc/Trunk/release
qualification, immutable Tailscale review, explicit human UAT, accepted-source publication and
hosted-byte verification remain required before M78 closes.
