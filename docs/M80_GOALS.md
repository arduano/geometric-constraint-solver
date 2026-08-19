<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 — native topology-preserving Profile Offset

Status: **amended implementation and development qualification complete; the clean replacement
gate, frozen nomination, human UAT, publication and closure remain pending**. The pre-amendment
implementation and qualification remain valid historical evidence, but their frozen candidate is
withdrawn from acceptance and is no longer served. ADR 0037 owns the architecture. M80 adds the
narrow constraint-friendly Offset plus one explicit native line-line Fillet publication path, and
deliberately leaves topology-changing Offset for a later computed-feature milestone. GitHub Pages
remains on accepted M79.

## Product outcome

The ordinary workbench gains **Modify → Offset**. One Apply creates ordinary native target geometry
plus one grouped driving `ProfileOffset` dimension. Editing the shared distance or either side of
the association preserves the exact offset while the source/target topology stays in the same
explicit cell. Deleting or suppressing the association leaves the generated curves and their
ordinary connectivity in the sketch.

One Apply accepts exactly one operand:

- one bounded face, including its outer boundary and all holes; or
- one manually collected, ordered open chain, including a valid one-line or one-circular-arc chain.

M80 also adds an explicit **Apply native profile** alternative for exactly one eligible line-line
Fillet corner. **Apply computed** remains the unchanged default and still creates ADR 0031 computed
feature intent. Apply native profile instead materializes the current exact one-corner preview as
ordinary persistent line-arc-line sketch topology so the existing Offset tool can consume it
without any Fillet-specific Offset path.

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

Computed Fillet arcs and discarded fragments remain unavailable Offset operands. Only the ordinary
native line/circular-arc geometry produced by the explicit Apply native profile action can subsequently
participate in a Profile Offset operand.

These are intentional product boundaries, not partial implementation. Variable-cardinality
topology change belongs to the later ADR 0031 computed Offset.

## Native Fillet authoring amendment

Apply native profile is available only for one current, independently valid line-line Fillet preview whose
parents are two distinct standalone, untrimmed native Profile `Line` curves sharing exactly one
persistent endpoint. The sharp point must have exactly those two direct curve owners and no other
point-based dependent; neither source line may already participate in a Profile Offset or be
claimed by persisted computed-feature intent. A remote endpoint may remain connected to ordinary
geometry, and compatible ordinary line constraints may survive the trial. Eligibility is
authenticated before any persistent identity is allocated; coordinate coincidence cannot
manufacture the corner.

One successful Apply native profile atomically:

- physically shortens both existing line curves to the exact tangent contacts while preserving
  their non-corner ends and line identities where semantically possible;
- inserts one ordinary native `CircularArc` with deterministic persistent identity;
- publishes two exact endpoint `LineCurveTangency` definitions as the ordinary incidence and
  tangency owners;
- publishes one ordinary driving Radius dimension for the inserted arc; and
- authenticates explicit first/second parent order, retained line endpoints and both normal-side
  choices, then materializes them into the shortened endpoints, arc Start/End mapping and sweep,
  and persistent contact tangent orientations. Canonical parent ordering co-permutes computed arc
  contacts and tangent orientations, including for reverse manual picks; no Fillet-specific
  provenance record survives.

The complete proposed document is solved and independently checked for finite geometry, exact
incidence/tangency/radius semantics, explicit branch agreement and normalized hard residual at
most `1e-9` before publication. Publication is one retained transaction and one Undo step. Undo
restores the exact pre-publication corner in one history position while persistent-ID high-water
remains monotonic; Redo restores the same native identities and branch state. Any invalid, stale,
ambiguous or exhausted attempt changes no
document, accepted scene, history, transcript or persistent-ID high-water.

The result is ordinary persistent Profile topology. Its line-arc-line chain enters the existing
M80 Offset authentication, construction, equation and topology-validation path unchanged; Offset
does not read a Fillet feature, generated-fragment ID or Fillet-specific equation. Apply native profile
creates no `FilletSet` and leaves the computed-feature sidecar unchanged.

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

The existing Fillet panel keeps **Apply computed** as its default. When and only when the current
authoring state contains one eligible line-line corner, it also exposes **Apply native profile**
with a headless disabled reason otherwise. The retained coordinator prepares and holds the exact
solved native patch beside that preview, including independent validation and computed-feature
parity; Apply only authenticates and consumes that held patch. The browser only renders and
dispatches the action.

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
change or stale scene restores/cancels without history. Every preview, release and restore is tied
to one monotonic gesture identity, so a delayed effect from an earlier drag cannot consume a newer
capture over the same candidate. Pointer release changes only the candidate distance; Apply stays
unavailable during the captured gesture and is enabled after release only for a complete, finite,
solved and topology-valid candidate.
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
- Native-Fillet owner tests cover exact one-corner eligibility, deterministic line shortening and
  arc identity, two endpoint `LineCurveTangency` owners, one driving Radius, all explicit branch
  fields, independent hard residual/domain validation, atomic rejection, exact one-step Undo/Redo
  and direct consumption of the resulting native line-arc-line chain by unchanged Profile Offset.
  Radius-gesture rollback retains its exact prepared native sketch patch while renewing only
  revision-local computed parity, so availability and Apply agree without evaluation-ID reuse.
  Rejected replacement previews consume no live computed revision, dependent/high-valence corners
  expose one concise identity-free disabled reason, and the complete native preparation trial runs
  under cooperative cancellation/work limits.
- Add one reviewed native-profile Fillet authoring family to the stable golden matrix with its
  deterministic transforms; review every new row rather than blessing changed bytes wholesale.
- Formatting, warnings-denied Clippy/Rustdoc, locked all-feature workspace tests, relevant WASM,
  unchanged historical persistence/golden authority, Trunk and the complete release gate pass.
- The exact gate-produced distribution is copied without rebuilding, frozen read-only and byte-
  verified at the shared Tailscale endpoint. Human UAT in `docs/M80_UAT.md` explicitly accepts that
  replacement candidate before Pages publication and milestone closure. The previously nominated
  `b83dad2` snapshot is pre-amendment evidence and cannot receive final M80 acceptance.

## Explicit deferrals

M80 adds no computed or topology-changing Offset, intersection trimming, loop removal, fragment
generation, Bake/Explode, approximation tolerance, canonical v5 support, computed-on-computed
chaining, mobile layout, B-rep/surface offset or 3D sketch operation. Apply native profile does not cover a
polyline-owned corner, line-circle or other curve pair, multiple/batched corners, dependent or
high-valence corners, or conversion of an already published computed Fillet. It does not broaden
any unsupported analytic or parametric curve family.
