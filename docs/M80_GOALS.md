<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 — native topology-preserving Profile Offset

Status: **implementation and broad pre-nomination qualification complete; clean nomination,
human UAT, publication and closure pending**. ADR 0037 owns the architecture. M80 adds the narrow
constraint-friendly Offset and deliberately leaves topology-changing Offset for a later computed-
feature milestone.

## Product outcome

The ordinary workbench gains **Modify → Offset**. One Apply creates ordinary native target geometry
plus one grouped driving `ProfileOffset` dimension. Editing the shared distance or either side of
the association preserves the exact offset while the source/target topology stays in the same
explicit cell. Deleting or suppressing the association leaves the generated curves and their
ordinary connectivity in the sketch.

One Apply accepts exactly one operand:

- one bounded face, including its outer boundary and all holes; or
- one manually collected, ordered open chain, including a valid one-line or one-circular-arc chain.

Face `Outward` expands the outer loop and shrinks holes; `Inward` reverses that behavior. Open-chain
`Left`/`Right` is relative to the stored collection traversal. Flip changes only this explicit
direction. A signed UI distance is convenient input only: a negative value flips direction and is
stored as a positive magnitude.

## Exact admitted scope

Each source edge has exactly one target edge of the same native family:

| Source | Target | Operand availability |
| --- | --- | --- |
| Line segment or polyline span | Standalone native line segment in the same exact linear family | Face or open chain |
| Circle | Circle | Face only |
| Circular arc | Circular arc with explicit traversal/sweep | Face or open chain |

Operands must contain complete native Profile edges joined through persistent shared points or
active ordinary endpoint-contact constraints. A face is authenticated against one exact accepted
production-topology snapshot; a chain is authenticated in its exact manual collection order.
Coordinate coincidence is not connectivity.

Topology preservation is scoped to the selected source/target operand paths and their contours. It
does not freeze or compare unrelated sketch arrangement geometry. A source polyline span remains a
complete semantic edge, but its target is an ordinary standalone native line rather than a rebuilt
polyline container.

The following are unavailable and reject before target allocation:

- Construction, external and computed-feature boundaries;
- arrangement-derived partial fragments;
- ellipses and elliptical arcs, rational conics, Beziers, B-splines and NURBS;
- approximation or sampled conversion of any unsupported curve;
- unbounded/open faces, closed full-circle chain collection and selected span sets that are
  disconnected, closed or internally branching;
- any offset that would collapse an edge or loop, self-intersect, split, merge, contact another
  contour, lose or invert a hole, change nesting or cross a persisted miter/tangent branch barrier.

These are intentional product boundaries, not partial implementation. Variable-cardinality
topology change belongs to the later ADR 0031 computed Offset.

## Persistent design intent

One positive `DesignScalarId` drives the whole association and cannot be shared with another
dimension. `ProfileOffset` is driving-only. Both source and target circular-arc endpoint angles
remain active. Explicit `Preference`-priority rows retain the source Start/End angles only as the
deterministic shared-angle gauge, so a hard target endpoint driver propagates through the offset;
this is not a weighted substitute for any hard equation. The operand persists:

- face `Outward`/`Inward` or chain `Left`/`Right`;
- every ordered source-target edge pair and both traversals;
- exact source and target junction provenance;
- an explicit `Miter { Left | Right }` or `Tangent` branch at every internal join; and
- normal-translation terminal policy for an open chain.

The compiler emits one high-level source with multiple sparse residual blocks. Line pairs reuse the
ADR 0020 supporting-line rows; circle/arc pairs use equal-center and signed-radius rows; open
terminals and tangent joins add tangential anchors. Independent validation rechecks all equations,
orientation, side, terminal, branch and topology predicates before any success-like publication.
Every new row has a structured audit descriptor and central finite-difference Jacobian coverage.

Generated target curves are ordinary native geometry. Their shared points or endpoint-contact
constraints are not hidden inside `ProfileOffset`, so they remain connected after association
deletion/suppression. M80 does not delete the target as an implicit side effect of removing only the
dimension.

## Authoring and presentation

`geosolve-sketch-ops` owns deterministic Profile Offset construction/proposal generation over an
authenticated topology index. `geosolve-constraint-editor` owns a separate offset-authoring state,
exact face/chain ownership, selection/traversal, distance/direction, branch capture, preview
invalidation and atomic retained commit. The browser owns only the established bottom-left panel,
platform events and rendering.

The panel contains operand status, Distance, Flip, Apply and Cancel. It persists through blur,
canvas clicks and pan/zoom; explicit close or Cancel returns to Select. The first valid default is
`0.1 * model_scale`, after which the last valid distance is remembered process-locally. A negative
Distance entered before selection retains transient direction intent for the next face or chain;
no prior operand identity is remembered. Hover and click share the same authenticated face/edge
owner, including typed unavailable targets. Ordered chain presentation includes traversal arrows
and Start/End terminals, while pointer, tree and keyboard activation use the same semantic pick.

An open chain is non-branching with respect to its selected span set. It may pass through an
authenticated junction that also has unselected incident geometry: those edges neither veto nor
join the operand. Selecting a set that itself branches, disconnects or closes remains a typed local
rejection.

Preview geometry is visibly provisional and non-selectable by ordinary tools, while its target
edges and grouped distance presentation remain one explicit authoring-only drag surface. Dragging
uses the common three-pixel threshold, pointer capture, frozen pointer-down rail and absolute
sampling. Invalid samples retain the last complete preview; Escape, capture loss, camera/tool
change or stale scene restores/cancels without history. Pointer release changes only the candidate
distance; Apply stays unavailable during the captured gesture and is enabled after release only
for a complete, finite, solved and topology-valid candidate.
Cancel, stale accepted scene/topology, failed solve, invalid target, resource exhaustion or a
topology barrier changes no document, accepted scene, history, transcript or persistent-ID
high-water. A successful Apply creates all target geometry, ordinary target connectivity, the
scalar and grouped dimension in one transaction and one Undo step, then keeps Offset active for
repeated work.

The accepted association has one production-quality movable `ProfileOffset` annotation. Its
placement offset is retained only in M76's disposable scene annotation cache and is safely
recomputed if that cache is missing. Ordinary workspace save/reload retains compatible cache rows;
reproduction capsules omit them on export and ignore them on import.

## Persistence compatibility

M80 does not promote supported canonical JSON. Private v2-v4 wire DTOs freeze the historical seven
dimension variants, and canonical v4 export returns typed `UnsupportedM80State` when the new
dimension is present. Private draft-v5 gains an omitted-when-empty `profile_offset_dimensions`
section with the complete operand and branch contract. Existing v1-v4 and empty draft-v5 bytes do
not change; workspace v6 and `GEOSOLVE_REPRO_V1` continue to use the strict draft-v5 restore path.

## Acceptance gate

- Direct domain tests cover rectangle/polygon, circle, mixed line/arc and holed faces in both
  directions; single line/arc and multi-edge/tangent open chains on both sides; edits from both
  sides; association removal; and explicit traversal/branch retention.
- Topology-barrier tests cover collapse, self-intersection, split/merge, non-adjacent contact, hole
  loss/nesting change, tangent/miter transition, antipodal arc root and unsupported provenance/
  curve families. Every rejection is atomic and preserves the last accepted scene.
- Each new residual passes local-AD/analytic versus central finite-difference comparison at model
  scales `1e-6`, `1` and `1e6`; source mappings, row audits, rank/DOF and deterministic order are
  directly asserted.
- Undo/Redo, deletion/suppression, canonical-v4 rejection, draft-v5/workspace/repro round trips,
  stale preview, cancellation, allocation exhaustion and native/WASM parity pass.
- Thin demo tests own panel persistence, accessible labels, face/edge hover-click parity,
  non-selectable preview, annotation movement/cache recovery and exact headless effect routing.
- Formatting, warnings-denied Clippy/Rustdoc, locked all-feature workspace tests, relevant WASM,
  unchanged historical persistence/golden authority, Trunk and the complete release gate pass.
- The exact gate-produced distribution is copied without rebuilding, frozen read-only and byte-
  verified at the shared Tailscale endpoint. Human UAT in `docs/M80_UAT.md` explicitly accepts that
  candidate before Pages publication and milestone closure.

## Explicit deferrals

M80 adds no computed or topology-changing Offset, intersection trimming, loop removal, fragment
generation, Bake/Explode, approximation tolerance, canonical v5 support, computed-on-computed
chaining, mobile layout, B-rep/surface offset or 3D sketch operation. It does not broaden any
unsupported analytic or parametric curve family.
