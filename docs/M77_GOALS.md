<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 — CAD curve handles and implicit-parameter editing

Status: **complete (2026-08-17); the replacement is clean-qualified, immutably frozen,
scope-approved and exact-verified on both Tailscale and GitHub Pages**. M77 makes advanced curve
parameters directly manipulable in the polished demo while preserving the existing document
model, equations and explicit branch state.

## Product contract

A selected editable curve publishes a small, family-specific control cage. Stored design points
remain ordinary point owners and use the existing projected point-drag path. Values that have no
stored point publish transient typed curve handles owned by the curve. A handle is not a selectable,
constrainable or serialized sketch point, and it does not change the curve's topology.

| Curve family | Spatial controls in M77 |
| --- | --- |
| Line and polyline | Existing endpoint/vertex points only |
| Circle | Existing centre point plus a radius handle |
| Circular arc | Existing centre point, derived Start/End trim handles and a radius handle |
| Ellipse | Existing centre/major-axis points plus a minor-axis-ratio handle |
| Elliptical arc | Existing centre/major-axis points, derived Start/End trim handles and a minor-axis-ratio handle |
| Rational quadratic conic | Existing endpoint points plus ordinary `P1` control/cage for `w != 0`, or an explicit projective `Qh` vector for `w == 0`; weight remains numeric |
| Parabola segment | Existing vertex/focus points plus derived Start/End trim handles |
| Hyperbola segment | Existing centre/transverse-axis points, derived Start/End trim handles and a semi-conjugate handle |
| Quadratic/cubic Bezier, B-spline and NURBS | Existing stored control points and control polygon; NURBS weights remain numeric |

Arc construction uses the same spatial trim language as later editing. A circular arc is authored
as Centre, Start, End. An elliptical arc is authored as Centre, Major axis, Start, End; the two trim
clicks are radially inverse-projected in normalized ellipse space and the explicit sweep option is
retained. Incomplete elliptical-arc stages render a headless-evaluated support ellipse rather than
asking the browser to reconstruct conic equations.

Active computed Fillet output arcs do not expose generic arc handles while their Fillet owner is
authoritative. Inactive, protected, external or otherwise non-editable owners expose truthful
read-only state rather than a handle that promises a mutation.

## Headless scene and interaction authority

The constraint editor publishes finite handle identity, owning curve, role, anchor, optional guide
or rail, control-cage geometry, editability, cursor/accessibility description and exact paint/hit
geometry from one accepted scene. The browser only renders and forwards that state; it does not
reconstruct an endpoint, parameter, priority or edit from SVG geometry.

Handles appear only for selected curves in Select mode and disappear when their owner, tool,
accepted scene, camera or input ownership becomes ineligible. They participate in the shared M75
hover/pointer resolver: a stored draggable point retains its ordinary owner, a visible selected-
curve handle outranks its underlying annotation/curve paint, and active Fillet affordances retain
their existing authority. Exact ties are stable and rendering order is irrelevant.

Pointer-down captures the pointer and preserves the cursor-to-handle grab offset. Movement begins
only after the existing 3 px threshold. Before threshold, the action is selection-only. Hover and
pointer-down identify the same curve and handle under the same scene/viewport input, and Escape,
capture loss, tool/camera change or stale owner cancellation restores the pre-gesture scene.
Prepared candidate scenes keep their truthful candidate design, accepted revision and computed
provenance. A private gesture-local seal separately authenticates the exact pointer-down origin and
accepted preview request; an older sealed candidate cannot sample or release a newer unseen patch.

## Typed edits and preview lifecycle

Derived arc, parabola and hyperbola endpoints use
`SketchDocument::project_curve_trim_endpoint` and the returned durable `SetScalarValue` edit.
Projection unwraps near the current scalar where applicable and retains Start/End identity. For
`w != 0`, the rational middle handle presents `P1 = Qh / w`; a spatial edit uses the accepted
host-effective weight to write `Qh = w·P1` through durable `SetConicWeightedMiddle` while
preserving the stored fallback weight. At `w == 0`, the same weighted-middle edit uses explicit
projective `Qh` vector state, never division by zero. Explicit numeric weight/mode edits retain
atomic `SetRationalConicControl`, and exact nonzero weight changes preserve `P1`. New nonzero-weight
canvas construction interprets its middle click as `P1`, matching later editing. Radius,
minor-axis-ratio and semi-conjugate handles project onto deterministic family-specific rails and
publish `SetScalarValue` for their existing owned scalar.

Each pointer sample prepares a candidate from an exact accepted-session clone. Only a finite,
independently accepted candidate becomes the visible preview. A later invalid or out-of-domain
sample retains the last valid preview and reports the non-committing state without replacing
accepted geometry. Release publishes that exact prepared candidate through the existing
compare-and-swap path as one history step; stale work fails closed. A gesture with no valid changed
candidate makes no history entry.

The edit retains every existing scalar domain and ownership rule. Radius and semi-conjugate size
remain positive; minor-axis ratio remains positive and at most one; rational weight retains its
existing finite `w > -1` domain, including valid negative and zero cases. Endpoint identity,
directed trim order, circular/elliptical sweep and the selected hyperbola branch do not auto-swap
or flip when the pointer crosses an invalid boundary. M77 adds no temporary residual and does not
generalize `DocumentDragTarget` to derived points.

## Persistence, inspector and accessibility

Accepted handle edits are ordinary existing document edits, so Undo/Redo, save/reload, workspace
copy/restore and reproduction payloads retain the resulting scalar or weighted-middle value
without a new schema. The handle layer and control cage are recomputed from accepted geometry and
selection and are never persisted.

Tooltip, accessible name and inspector copy distinguish `Start endpoint`, `End endpoint`, `Radius`,
`Minor axis`, `Middle control P1`, `Projective middle Qh` and `Conjugate size`, and identify the
owning curve. Exact numeric controls are the keyboard-accessible fallback. Rational middle weight
and NURBS weights stay numeric because a second spatial weight rail would be ambiguous and visually
noisy; the selected NURBS gauge stays read-only and an explicit “Make gauge” action uses the
existing geometry-invariant normalization. Knot, degree and topology editing are outside this cut.

## Qualification and closeout

Native and WASM coverage must freeze every family/role, selected-only visibility, exact paint/hit
parity, hover/click agreement, grab offset, threshold, valid preview, domain rejection, last-valid
retention, cancellation, staleness, branch preservation, one-step Undo/Redo and save/reload. Thin
demo tests own only event mapping, capture and headless rendering. Existing golden authoring/scene
coverage must remain clean unless a reviewed M77 row is deliberately added.

The initial source `51a3b95d04f27216c164febf0808a180b6775537` and its immutable snapshot are
superseded historical evidence after replacement UAT findings. Exact replacement source
`cc99b11071dc62732e02b630ba7a1381d754b04c`, tree
`3315a2bdd0137f59657ea2500962ef971a23ea15`, passes the complete clean release gate and is frozen
without rebuilding at `/tmp/geosolve-m77-uat.ARrQFw`; its seven served files byte-match aggregate
`abfa7ef6b75f127fa6d93ff6ad6960c7f5df7d4c799a578c785e1192c2b7ee94`. The supervising caller
explicitly approved the current replacement and requested closure on 2026-08-17; U1-U6 pass under
that scoped disposition. Publication descendant `66a89b7e3e0c39d50407f2a540517e6a7facdc77`
passes GitHub Pages run `32012819635`; artifact `9283439225` and deployment `5942438795` succeed,
and root plus all seven public files byte-match ordered-manifest aggregate
`872719a0f4323f978bf31a4e567646b61a8bd607a2dbc384e47b676054979f15`. M77 is closed.

## Non-goals

M77 adds no curve family, solver equation, residual, constraint, dimension, generalized derived-
point constraint target, automatic branch/sweep change, knot/degree/topology editor, spatial
rational/NURBS weight rail, persistence version, mobile layout or browser-owned hit testing.
