<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0037: Native topology-preserving Profile Offset

Status: accepted and implemented for M80; broad pre-nomination qualification passes, while clean
candidate nomination, human UAT and publication are pending.

## Context

ADR 0020 provides two useful line-to-line dimensions: supporting-line offset and exact translated-
segment offset. They describe one source/target segment pair, but they do not express the ordinary
CAD action of offsetting a complete face boundary or connected open chain with one shared distance.

ADR 0031 deliberately reserves a different operation shape for a future computed Offset. That
operation may trim at intersections, split or merge fragments, remove loops and otherwise change
output cardinality. Its output belongs in revision-local computed topology rather than the native
constraint graph. M80 is not that operation. Its purpose is a constraint-friendly, Onshape-like
association only where every source edge has exactly one same-family native target edge and the
selected operand topology can remain unchanged.

Treating both meanings as one permissive Offset would be unsafe. A solver dimension cannot
truthfully create or delete variables when an offset crosses a collapse, self-intersection or
contour-contact barrier. Conversely, routing every regular line/circular offset through computed
topology would prevent the target from behaving like ordinary editable sketch geometry and would
discard the useful one-to-one association.

## Decision

### One grouped driving dimension

M80 adds one driving-only `ProfileOffset` dimension. One positive finite length scalar controls all
source/target pairs in the selected operand. The scalar cannot be shared with another dimension,
and reference mode is rejected. UI input may be signed for convenience: a negative value flips the
stored `Outward`/`Inward` or `Left`/`Right` direction and stores its absolute positive magnitude;
zero and non-finite values remain invalid.

The public runtime `DimensionKind` remains `Copy`. A stable `ProfileOffsetId` indexes a sketch-
local arena, and the runtime dimension carries only:

```rust
DimensionKind::ProfileOffset {
    profile: ProfileOffsetId,
    target: f64,
}
```

One persistent document dimension lowers to one runtime/core source and may emit many sparse
residual blocks. Existing `SketchSourceMapping::residual_ids` remains the complete ordered mapping
from that one source to its rows; diagnostics name the grouped high-level dimension once rather
than pretending that each edge is an independent dimension.

### Closed face and open-chain operands

One Apply accepts exactly one of these operands:

1. one authenticated bounded face, including its outer loop and every hole; or
2. one manually collected, ordered open chain.

A face uses only complete eligible native Profile edges from one exact accepted production-
topology snapshot. Its loops are normalized material-left: the outer loop is counter-clockwise and
holes are clockwise. `Outward` moves every loop to material-right, so the outer boundary expands
while holes shrink; `Inward` reverses both effects. A one-edge circular face is valid and has no
junction record.

An open chain uses the explicit collection order and traversal stored at authoring time. `Left` and
`Right` are relative to that traversal. Canonicalization may sort enclosing document objects, but
must never reverse a collected chain or reinterpret its side. A single line segment or circular
arc is a valid one-edge chain. A full circle is closed and is therefore face-only.

Chain continuity is scoped to the explicitly selected spans. Each selected internal junction has
exactly one selected predecessor and successor, and the selected set must admit one ordered,
non-branching traversal. Additional unselected Profile edges may meet the same authenticated
junction: they remain ordinary unrelated geometry, do not veto the selected path and are never
implicitly absorbed into it. A selected set that itself branches, disconnects or closes remains
ineligible as an open chain.

Every multi-edge source loop/chain is connected by authenticated persistent provenance, never by
coordinate coincidence. Each junction records either a shared persistent point or the active
ordinary endpoint-contact constraint that owns the connection. The generated target uses ordinary
shared points or ordinary endpoint-contact constraints, and their identities are recorded as the
target junction provenance. Deleting or suppressing the `ProfileOffset` dimension therefore
removes only the offset association: target curves and their ordinary connectivity remain.

Contact ownership is exact, not geometric. A contact-owned endpoint must use the complete bounded
`[0, 1]` domain, winding zero, the matching Start/End neighborhood and a bit-exact `0.0`/`1.0`
parameter scalar. A supporting-line or interior contact at the same coordinate cannot impersonate
an endpoint owner.

The persistent conceptual shape is:

```rust
ProfileOffset {
    target: DesignScalarId,
    operand: DocumentProfileOffsetOperand,
}

enum DocumentProfileOffsetOperand {
    Face {
        direction: Outward | Inward,
        outer: ClosedLoop,
        holes: Vec<ClosedLoop>,
    },
    OpenChain {
        side: Left | Right,
        chain: OpenChain,
    },
}
```

Loops/chains contain ordered source-target edge pairs with explicit source and target traversal.
A closed loop has one junction per edge except for a one-circle loop; an open chain has `N - 1`
junctions and explicit normal-translation policies at both terminals. Every internal junction
stores its source and target connectivity provenance plus one explicit branch:

- `Miter { turn: Left | Right }`; or
- `Tangent`.

The branch is captured during authoring and remains durable discrete state. It is not silently
reclassified after a source edit. A miter cannot cross its persisted turn barrier, and a tangent
join cannot become a cusp, reversal or miter while still reporting success.

### Exact admitted geometry

M80 supports exact same-family pairs only:

- bounded line segments and individual polyline spans map to standalone native line segments in
  the same exact linear family;
- circles map to circles; and
- circular arcs map to circular arcs with explicit matching traversal and sweep branch.

Eligible operands contain only whole native Profile edges. Construction geometry, external or
computed-feature boundaries, arrangement-derived partial fragments and any ellipse, elliptical
arc, rational conic, Bezier, B-spline or NURBS edge are rejected before target allocation. M80 does
not approximate an unsupported curve with lines or arcs.

### Residuals, anchors and branch validation

For each line pair, M80 reuses ADR 0020's supporting-line equations. With source unit tangent `u`,
left normal `n`, traversal-aligned target tangent `v`, source point `p`, target point `q`, positive
distance `d` and left/right sign `s`, the pair contributes:

```text
cross(u, v) = 0
dot(q - p, n) / model_scale - s*d/model_scale = 0
```

Independent validation requires finite nonzero supports, `dot(u, v) > 0`, the persisted side and
same-direction traversal. For face loops, `s = -1` for `Outward` and `s = +1` for `Inward`.

Each circle/circular-arc pair contributes three rows: equal center X/Y plus one signed radius-
difference row. Let `w` be `+1` for counter-clockwise traversal and `-1` for clockwise traversal.
For an open chain, `s = +1` for `Left` and `-1` for `Right`:

```text
r_target - r_source - (-s*w)*d = 0
```

For a face, let `D = +1` for `Outward` and `-1` for `Inward`:

```text
r_target - r_source - (D*w)*d = 0
```

Both source and target circular-arc endpoint angles remain active so hard endpoint drivers on
either side can propagate through the association. The two common endpoint-angle modes would
otherwise be gauges, so Profile Offset adds explicit `Preference`-priority Start/End rows that
retain the source arc's previous angles. These rows are deterministic secondary objectives under
the solver's documented hard/temporary/preference hierarchy; they are not weighted hard rows and
cannot trade away any Profile Offset or user-authored hard equation.

Every open terminal adds one tangential anchor. For corresponding source/target endpoints `p` and
`q`:

```text
dot(q - p, u) / model_scale = 0
```

The edge-support row supplies normal separation. Independent validation additionally recomputes
`q - p = s*d*n` and requires aligned source/target terminal tangents. Thus a one-line chain is the
exact translated-segment relation, while a one-arc chain fixes center, radius, Start and End and
rejects the antipodal endpoint root.

A tangent internal junction receives the same tangential anchor and requires aligned incoming and
outgoing source and target tangents. A miter receives no anchor: the two retained offset supports
and ordinary target connectivity determine their intersection. Independent validation checks its
persisted nonzero turn and the selected intersection cell. Every row has a structured audit
descriptor and analytic/local-AD Jacobian checked by central finite differences.

### Topology is a hard acceptance condition

Solving the equations is necessary but not sufficient. Before preview or publication, M80
independently reconstructs the exact selected source/target loops or chain and proves:

- every edge remains finite, regular, nonzero and in its persisted family/traversal/sweep cell;
- every recorded junction still has its exact source and target connectivity owner;
- every miter/tangent/terminal branch predicate remains true;
- edge count, loop count, hole count, cyclic order and source-target correspondence are unchanged;
- closed loops remain simple, non-contacting and correctly oriented;
- a face keeps one outer loop, the same holes and the same strict nesting; and
- open chains remain non-self-intersecting and do not acquire non-adjacent contacts.

Collapse, self-intersection, split/merge, contour contact, hole loss, nesting reversal, a miter
crossing its tangent barrier or any other topology change rejects transactionally and retains the
last independently accepted scene. No threshold may trim, split, delete or silently choose a
different topology in order to make the dimension succeed.

This certificate concerns the selected source/target operand paths and their mutual face contours;
unrelated sketch arrangement geometry is not part of the native association's topology cell.

### Persistence without promoting the canonical schema

M80 does not promote supported canonical sketch JSON beyond version 4. The v2-v4 dimension wire
syntax is first frozen behind private seven-variant DTOs so older version labels cannot begin
accepting M80 syntax merely because the live enum grew. Canonical v4 export of a document containing
`ProfileOffset` returns typed `UnsupportedM80State`.

Private draft-v5 adds an omitted-when-empty `profile_offset_dimensions` side section containing the
driving dimension identity/source/label/suppression state, positive target scalar, operand kind,
ordered traversals, exact edge pairs, junction provenance, branch state and terminal policies.
Existing v1-v4 bytes and an empty draft-v5 payload remain byte-compatible. Workspace v6 and
`GEOSOLVE_REPRO_V1` continue to carry draft-v5 through their existing strict validation and atomic
restore paths.

### Headless authoring and thin presentation

`geosolve-sketch-ops` owns deterministic same-family construction and an immutable exact-stamped
proposal over the authenticated topology index. `geosolve-constraint-editor` owns a separate
`OffsetAuthoringState`; computed-Fillet authoring is not reused. The state authenticates one face
or an ordered collected chain against the exact accepted scene/topology snapshot, owns direction,
distance, branch capture, preview lifecycle and one atomic construction plan. Hover and pointer-
down use the same headless face/edge owner.

The demo exposes **Modify → Offset** through the established persistent bottom-left canvas panel.
The panel shows operand kind/status, Distance, Flip, Apply and Cancel. A valid remembered distance
is process-local; otherwise the initial value is `0.1 * model_scale`. Blur, canvas clicks and
camera movement do not close it. Explicit close or Cancel returns to Select. A negative Distance
entered before selection supplies transient direction intent for the next face/chain. Unsupported
or dynamically invalid targets preserve typed unavailable hover/click feedback; ordered chains
render traversal arrows and Start/End terminals; pointer, tree and keyboard activation share one
semantic pick. The preview remains visibly provisional and unavailable to ordinary selection, but
its target edges and grouped distance presentation form one explicit authoring-only distance-drag
surface. That gesture uses the shared three-pixel threshold, pointer capture, an exact
pointer-down-stamped source/target rail, absolute sampling, last-valid preview retention and
cancel-to-origin behavior. Pointer release updates only the held authoring candidate; Apply remains
the sole durable transaction. Apply is unavailable until a complete solve plus the independent
topology validation passes. Any scene/history/import/tool change invalidates stale selection,
gesture and preview authority.

Publication creates every target curve, ordinary target connectivity constraint, positive scalar
and one grouped driving dimension in one retained transaction and one Undo step. Invalid, stale,
cancelled or resource-limited work changes no document, accepted scene, history, transcript or
persistent-ID high-water. Repeated tool use remembers only the last valid distance, never source or
target identities.

The accepted dimension publishes one movable `ProfileOffset` annotation. Witness/leader placement
belongs to the disposable annotation cache established by M76; it is recomputable and is not part
of dimension persistence. Ordinary workspace save/reload retains compatible cache rows, while a
reproduction capsule omits them when copied and ignores them when restored.

## Consequences

- Regular face and chain offsets remain native, associative, editable and auditable through one
  ordinary driving dimension.
- One scalar and one high-level source describe the user's intent even though the compiler emits
  several sparse residual blocks.
- Explicit traversal, side/direction, junction provenance and branch state make source edits
  deterministic and fail closed at topology barriers.
- Removing the association leaves useful ordinary target geometry and connectivity rather than
  deleting or baking opaque output.
- The admitted family is intentionally narrow; unsupported curves receive a clear refusal instead
  of an approximate or topology-changing result.
- Draft-v5 can preserve the active demo/workspace feature without silently expanding supported
  canonical v4.
- A later computed Offset remains free to change cardinality under ADR 0031 without weakening this
  milestone's one-to-one constraint semantics.

## Rejected alternatives

- **Use one independent offset dimension per edge:** rejected because distances can diverge,
  diagnostics fragment the user's intent and junction branch ownership becomes accidental.
- **Represent the feature as computed topology:** rejected for M80 because the generated targets
  must be ordinary native geometry coupled bidirectionally through solver equations.
- **Bake geometry and discard association:** rejected because source edits would not preserve the
  requested offset intent.
- **Approximate unsupported curves:** rejected because tolerance-dependent segmentation is neither
  same-family nor stable under editing.
- **Trim or drop loops at a barrier:** rejected because variable cardinality belongs to the future
  ADR 0031 computed Offset milestone.
- **Infer connectivity or branch from current coordinates on every solve:** rejected because
  coordinate coincidence and seed-dependent intersection choice are not persistent design intent.
- **Serialize M80 through the live v4 enum:** rejected because it would mutate the accepted language
  of historical version labels without a schema decision.

## Verification

M80 qualification includes rectangle/polygon, circle, mixed line/arc, hole and open-chain fixtures;
single-line and single-arc exactness; miter and tangent joins; direction/traversal reversal; source
and target edits; deletion/suppression retention; topology barriers; Undo/Redo and draft-v5/
workspace/repro round trips; stale/cancelled/resource-limited atomicity; source mapping, rank/DOF,
structured audits and finite-difference Jacobians at scales `1e-6`, `1` and `1e6`.

`docs/M80_IMPLEMENTATION.md` records the focused counts and the `M80-F001`-`M80-F009` owner
regressions for native-origin authentication, bidirectional arc gauges, exact endpoint-contact
ownership, current accepted-state capture after point edits and the persistent Profile-role
invariant, selected-set branch scope, exact provisional-distance gesture authority and one-span
self-closure rejection, including per-gesture authentication against delayed preview and terminal
effects from an earlier drag over the same provisional proposal.

Focused native/WASM and workbench tests, unchanged historical persistence bytes and the stable
golden gate pass as pre-nomination evidence. The complete clean release gate must pass before the
exact no-rebuild output is frozen read-only and byte-verified on the shared Tailscale endpoint.
`docs/M80_UAT.md` must be run against that exact candidate. GitHub Pages publication and milestone
closure occur only after the supervising human accepts the focused scorecard or explicitly records
another scoped close decision.

## Scope boundary

M80 adds no topology-changing Offset, computed Offset definition, output trimming, loop removal,
arrangement-derived fragments, computed-on-native chaining, Bake/Explode, canonical v5 support,
curve approximation, mobile UI, B-rep offset, surface offset or 3D sketch behavior. Ellipses,
elliptical arcs, rational conics, Beziers, B-splines and NURBS remain unsupported operands even when
a local mathematical offset exists.
