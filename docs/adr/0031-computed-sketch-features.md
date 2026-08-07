<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0031: Computed sketch features and revision-local output topology

Status: accepted

## Context

M27 and M28 model associative Fillets inside the sketch constraint graph. That remains a useful
advanced API: contacts, output-arc coordinates, trim ownership and optional radius dimensions are
solver state, and downstream sketch constraints may refer to the output arc. M66 first routed the
ordinary workbench Fillet tool through that model under ADR 0030. The mechanically qualified but
unapproved endpoint of that approach is commit `1034afc`, preserved at
`origin/archive/m66-associative-fillet-2026-08-07`.

Human UAT exposed a different product need for the ordinary CAD tool. Applying a second Fillet to
the other end of one span conflicts with M28's one-trim-view ownership, selecting several
polyline corners is flattened into the fixed two-parent authoring arity, and deleting a radius
dimension does not turn a solver-owned association into freely editable derived topology. These
are ownership-model mismatches, not missing Fillet equations.

Future operations such as planar Offset have the same shape at a larger scale. A computed result
may split at intersections, remove loops, or otherwise change output cardinality and topology
without a sensible one-to-one chain of sketch constraints. Persisting sampled output as ordinary
sketch entities would make stale geometry authoritative; forcing every derived fragment into the
constraint graph would make topology changes accidental solver state.

## Decision

### Ordinary Fillet is a computed feature

M66 introduces a separate pure-Rust `geosolve-sketch-features` domain. Among workspace crates it
depends only on `geosolve-sketch` and `geosolve-geometry`. The sketch, core, linkage, operations and production-
topology crates do not depend on it. A host coordinator may combine the sketch session, feature
document and current computed snapshot.

The ordinary workbench **Fillet** tool creates `ComputedFeatureDefinition::FilletSet`; it no
longer creates an M28 `CurveCurveFillet`, `DocumentCurveTrimView` or radius dimension. One Apply
creates one persistent `FilletSet` containing every selected corner and one shared positive
radius. A later Apply creates a separate set. The normal UI exposes no Driving/Reference radius
mode and adds no sketch constraint or dimension for computed radius.

ADR 0030 remains the accepted historical contract for the archived solver-owned authoring
candidate and for deliberate advanced consumers. This ADR supersedes ADR 0030 only for ordinary
workbench Fillet routing. It does not remove or narrow:

- the M27/M28 line and all-family associative-Fillet document definitions, equations, trim views,
  validation, persistence or public APIs;
- `SketchOperationRequest::AssociativeFillet` and the M58 operations-companion integration; or
- documents that already contain those definitions.

There is no automatic or inferred migration between solver-owned Fillets and computed
`FilletSet`s. Existing documents retain their exact meaning. Advanced APIs may continue to create
solver-owned Fillets outside the ordinary tool.

### Persistent intent, ephemeral evaluated output

`ComputedFeatureDocument` is a separately versioned sidecar. It owns a stable document identity,
stable feature and corner identities, an allocation high-water mark, labels, suppression state
and closed `ComputedFeatureDefinition` values. Undo, Redo and import never reuse an allocated
identity.

A `FilletSet` persists intent only:

- its shared finite positive radius;
- each source `CurveSpan` pair and picked parameters;
- the source neighborhoods and winding needed to identify the intended local contacts;
- explicit normal sides, retained endpoint on each source, output endpoint order and sweep; and
- stable set/corner identity.

Generated arcs, trimmed source fragments and evaluation-local edge identities are never
serialized as intent. Evaluating one exact accepted sketch snapshot produces a separately stamped
`ComputedFeatureSnapshot`. Every generated `ComputedEdgeId` is valid only within that snapshot.
Stable provenance maps each output edge to its persistent feature/corner identity and exact source
intervals. Consumers must discard output IDs when the sketch identity, feature revision/digest or
evaluator policy changes.

Result containers support zero, one or many generated fragments per feature/corner even though a
valid M66 Fillet corner normally emits one arc. This is the reusable seam for a future Offset whose
self-intersections may split, cut or remove fragments. M66 adds no Offset definition, evaluator,
workbench action, placeholder or UAT claim.

Version-one feature inputs reference only native constrained sketch spans. Computed-on-computed
chaining, Bake/Explode and topological naming across evaluation revisions are deferred.

### Exact evaluation and endpoint-claim composition

Every active feature is evaluated from one immutable, independently accepted sketch snapshot.
The evaluator neither mutates `SketchDocument` nor writes `DocumentCurveTrimView`. It applies
deterministic bounded construction and independently validates finite geometry, positive radius,
contact domains, retained sides, tangency, endpoint order, sweep, branch state and offset
regularity/singularity conditions before publishing output.

M66 keeps the already qualified parent-family scope: affine/affine and affine/non-affine corners
are eligible; two non-affine parents return a typed unsupported feature failure. This is only an
ordinary computed-authoring limitation and does not narrow M28.

Each valid corner claims one endpoint interval on each source span. Opposite endpoints of a shared
span may be claimed by different `FilletSet`s, so sequential adjacent sets compose to the same
visible geometry as one batch when their intent agrees. Duplicate endpoint claims, crossed claims
or intervals that consume a source span fail every participating set deterministically. One
invalid corner makes its complete set unavailable; unrelated sets may remain current.

Computed output is all-or-nothing per set and never stale. A failed or unsupported set publishes
attributed issues and no arcs/fragments for that set. The last accepted sketch remains fully
editable, and a valid sketch edit is accepted even when it invalidates a feature. Moving a source
may recover the same persistent set; deleting a source produces a repairable missing-source
failure, and Undo may recover it with the same feature identities.

### Authoring and interaction ownership

`geosolve-constraint-editor` owns reusable feature authoring rather than a fixed two-pick
operation collector. Preselected interior polyline points remain grouped corner targets instead
of flattening into `2N` curve picks. Repeated corner or curve-pair collection accumulates a batch.
Preview uses the remembered radius, or `0.1 * model_scale` when no valid remembered value exists.
Numeric editing and a preview arc/radius grip edit the one shared radius. Apply/Enter commits the
set; there is no final canvas radius-confirmation click.

Each corner retains explicit branch controls while radius belongs to the whole set. Generated arcs
select their stable corner/set provenance. Dragging an arc or radius grip edits only the feature
radius and never moves sketch coordinates. Deleting a generated arc removes that corner; deleting
the final corner removes the set. Suppression applies to the whole set. Computed arcs are not
sketch constraint operands, while every native source point and span remains normally selectable
and draggable.

The workbench adds a **Features** tree section and presents feature/corner/source-attributed
failures on the canvas and in Problems. A failure that cannot be attributed safely remains global.
Invalid computed output is withheld rather than drawn as a stale ghost.

### Coordinator and persistence

The retained editor coordinator owns one sketch session, one feature document and the latest
computed snapshot. Feature evaluation and publication use exact compare-and-swap evidence over
the complete sketch input/accepted identity, feature revision/digest and evaluator policy.
Cancellation, work exhaustion and stale results cannot replace current feature intent or output.

Restore checkpoints include feature intent. The application workspace envelope advances from
version 3 to version 4 and stores the separately versioned feature document beside the unchanged
canonical-v4/draft-v5 sketch payload. Workspace versions 1 through 3 migrate to an empty feature
document bound to the restored sketch. They do not reinterpret existing M28 Fillets. Undo, Redo
and reload preserve feature/corner IDs and intent, then evaluate fresh output IDs.

M66 does not feed computed output into `geosolve-sketch-topology`, visual-profile faces, fills or
other production/profile consumers. When active computed geometry would make base-sketch-only
profile or fill presentation misleading, the workbench withholds that presentation and exposes a
typed “computed geometry not yet included” status. Production consumption is a later milestone.

## Verification

Direct feature-domain tests must own persistence, canonicalization, evaluation, validation,
provenance, claim composition, conflict recovery and revision-local IDs. Direct editor/coordinator
tests must own grouped multi-corner authoring, shared-radius editing, stale CAS, source editing and
history. Direct workbench tests must own Features-tree presentation, source editability, generated-
arc interaction, error attribution and base-profile withholding.

The M66 regression matrix includes:

- a four-point/three-span polyline whose two corners generate two arcs and leave the middle source
  interval bounded at both ends;
- reverse-selection canonicalization and sequential adjacent sets matching batch visible geometry;
- atomic conflicting-radius failure and recovery;
- shared-radius edits that change no sketch identity, accepted coordinate, residual, rank or DOF;
- motion of every source point, source-deletion failure and Undo recovery;
- deleting or suppressing either adjacent set while preserving the other;
- Undo/Redo/reload, stale CAS, cancellation, exhaustion and allocator non-reuse; and
- evaluation-local output-ID invalidation and a variable-output-count fixture.

Existing M27/M28/M30 and M58 compatibility suites remain mandatory. The normal workbench route
must additionally prove it creates no M28 association, trim view, sketch radius scalar, constraint
or dimension.

## Consequences

- Sketch equations and DOF remain unchanged by an ordinary Fillet feature.
- Multiple corners and adjacent sequential Fillets compose through explicit source endpoint
  claims rather than competing persistent trim views.
- Invalid derived topology cannot lock source geometry or masquerade as accepted sketch state.
- Feature intent and stable provenance survive history and reload, while generated topology is
  truthfully revision-local.
- The variable-cardinality result seam can support future topology-changing operations without
  committing M66 to an Offset implementation.
- Production/profile consumers must remain fail-closed until a later milestone explicitly admits
  computed geometry.
- The separate feature document, coordinator and UI add architectural surface, but prevent
  derived topology from becoming either hidden solver state or stale persisted geometry.
