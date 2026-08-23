<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 — authoritative sketch lineage and deterministic rematerialization

Status: **editable-lineage/predictive-drag amendment mechanically qualified, independently
reviewed and frozen for focused human UAT; not accepted or published**. M83 is a full-workbench
architecture and migration milestone. `LineageDocument`
becomes the authoritative editable source for the GeoSolve demo workbench; flat `SketchDocument`,
`ComputedFeatureDocument` and evaluated feature geometry are derived materializations. ADR 0039
is accepted for M83 implementation.

This document supersedes the narrower M83 proof planned in commit `56d1eda`. That commit remains
in Git history as the original design record, but its line/Horizontal/rectangle/Profile Offset
slice, host-side sidecar posture, opaque flat-document root, and exclusion of the ordinary
workbench and computed features are not the active M83 contract. The expanded architecture
implementation passed its prior read-only Lineage-panel qualification, but the approved editable
history and predictive-drag amendment supersedes and withdraws that candidate. No criterion below
is accepted product behavior until the replacement frozen candidate passes explicit human UAT.

M81 remains the accepted product baseline while M83 is in progress. M82 remains a closed,
archived exploration; M83 does not restore its computed arbitrary-curve Offset prototype.

## Product outcome

The complete current workbench can be created, edited, saved, restored, undone, redone and
rematerialized from a versioned program of stable sketch actions. The action program records
authored intent and exact identity flow rather than pointer events or a flat before/after scene.

The intended ownership chain is:

1. `LineageDocument` owns retained design intent, stable action/output identity, explicit branches
   and all authoring parameters;
2. `LineageSession` owns the retained document, the latest evaluation attempt, the last accepted
   lineage state/materialization and one user-visible history;
3. the evaluator derives ordinary sketch state, computed-feature intent/evaluation and exact
   logical-to-materialized identity evidence; and
4. the editor and renderer consume only current accepted materialization plus non-authoritative
   draft/presentation state.

Low-level embedders may continue to use the public flat sketch APIs directly. The GeoSolve demo
workbench, however, must not maintain a second writable flat authority beside lineage. A workbench
mutation succeeds only by changing lineage and accepting the corresponding independently
validated materialization.

This makes the complete current workflow deterministic:

- authoring any geometry variant creates one semantic recipe step, plus any explicitly accepted
  companion relation steps in the same lineage transaction;
- adding a relation, dimension, operation or computed feature creates a typed later step whose
  inputs refer to stable logical ports;
- changing a curve property or explicit branch rewrites the step that owns that field;
- direct manipulation atomically rewrites every affected owner step and never appends a generic
  Move step;
- deleting or suppressing a step rematerializes without precisely the identities that step owns;
  and
- cold, cached and dependency-local evaluation produce the same accepted authority and identities.

## Authority and failure model

`LineageSession` retains three distinct states:

- the current retained `LineageDocument`, even when its latest evaluation is invalid;
- structured evidence for the latest complete, failed, cancelled, stale or exhausted evaluation;
  and
- the last complete independently accepted lineage revision and its reproducible materialization.

An explicit, structurally valid lineage edit enters history once even when downstream evaluation
fails. The retained program advances while the prior accepted lineage/materialization remains the
visible authority. Undo restores the preceding retained program and its exact stable reservations.
A malformed, stale, non-finite, cancelled or work-exhausted request changes neither retained state
nor history.

Projected direct-edit reconciliation is deliberately stricter. It publishes the complete owner
rewrite and one history position only after cold reproduction proves that the rewritten lineage
produces the accepted projection. Any missing owner, partial inverse mapping or downstream failure
rejects that complete reconciliation.

No flat sketch session, scratch evaluator or computed-feature coordinator exposes a nested
user-visible Undo stack. Draft state and failed prefixes are diagnostic only and cannot replace
the last accepted scene.

## Crate and dependency boundaries

`geosolve-sketch-lineage` is a separate pure safe-Rust orchestration crate. It may depend on the
public APIs of `geosolve-sketch`, `geosolve-sketch-ops`, `geosolve-sketch-topology`,
`geosolve-sketch-features` and `geosolve-geometry`. It owns no residual, Jacobian, rank/priority
policy, nonlinear solver, curve equation or independent-validation shortcut. `geosolve-core`,
`geosolve-sketch` and `geosolve-linkage` do not depend on it.

The current geometry recipe catalog and parts of atomic recipe lowering live in
`geosolve-constraint-editor`. M83 moves or exposes the minimum reusable semantic recipe seam below
the editor/lineage dependency boundary. Canonical lineage must not depend upward on browser event
handling, hit testing, SVG, panels or mutable draft interaction state. The editor remains the
owner of interaction and presentation, consumes lineage as its retained authority, and emits
typed lineage transactions after an authoring gesture is complete.

`geosolve-demo-web` integrates the lineage session and workspace-v7 codec. It is still a consumer,
not an owner of geometry or solver truth. A DOM-free adapter exposes the same stateful engine to
JavaScript through `geosolve.lineage.rpc.v0`; the adapter has no renderer, browser storage or start
hook.

## Complete M83 authoring surface

M83 covers the complete product surface present at the frozen M81 baseline. Coverage is closed and
inventory-driven: adding a new catalog variant without an explicit lineage mapping must fail an
exhaustiveness test rather than silently fall back to flat mutation.

### Geometry recipes

All 25 current `GeometryToolVariant`s retain their exact recipe identity, semantic draft inputs,
intrinsic relations, modifiers and stable output roles:

- point: `SketchPoint`;
- lines: `Segment`, `Polyline`, `MidpointLine`;
- rectangles: `TwoPointAlignedRectangle`, `ThreePointCornerRectangle`, `CenterRectangle`,
  `ThreePointCenterRectangle`;
- circles: `CenterRadiusCircle`, `TwoPointDiameterCircle`, `ThreePointCircle`;
- arcs: `CenterArc`, `ThreePointArc`, `TangentArc`;
- ellipses: `CenterAxesEllipse`, `AxisEndpointsEllipse`, `CenterAxesEllipticalArc`,
  `AxisEndpointsEllipticalArc`;
- Beziers: `QuadraticBezier`, `CubicBezier`;
- conics: `RationalQuadraticConic`, `Parabola`, `Hyperbola`; and
- splines: `OpenControlNurbs`, `PeriodicControlNurbs`.

Repeated/variable-cardinality recipes such as Polyline and NURBS allocate stable child ports in
authored order. Reused/snapped points are explicit aliases to earlier ports; fresh points,
scalars, curves, contacts and intrinsic relations are explicit owned outputs. Shift regularization,
tangent-arc source attachment, sweep/branch choice and construction/profile role are stored intent,
not reconstructed from final coordinates.

### Constraints and dimensions

Every current persistent `DocumentConstraintDefinition` is a lineage action or an explicitly owned
intrinsic recipe output:

- fixed/reference relations: `FixedPoint`, `FixedCoordinate`, `CoincidentWithOrigin`,
  `PointOnDatumAxis`, `ExternalPointCoincident`, `ExternalLineCollinear` and
  `CollinearWithDatumAxis`;
- incidence/alignment relations: `Coincident`, `Horizontal`, `Vertical`, `HorizontalPoints`,
  `VerticalPoints`, `HorizontalPointToMidpoint`, `VerticalPointToMidpoint`, `PointOnCurve`,
  `Midpoint`, `Concentric` and `Collinear`;
- pair relations: `Parallel`, `Perpendicular`, `EqualLength`, `EqualRadius`,
  `SymmetricAboutLine` and `SymmetricAboutDatumAxis`;
- contact/differential relations: `LineCircleTangency`, `CircleCircleTangency`,
  `CircleArcTangency`, `LineCurveTangency`, `CurveCurveContact`, `CurveCurveTangency`,
  `CurveDirection`, `EqualCurvature` and `EndpointContinuity`; and
- native fillet relations: `LineLineFillet` and `CurveCurveFillet`.

Every current `DocumentDimensionDefinition` is covered in both Driving and Reference modes:
`PointDistance`, `CurveLength`, `Radius`, `Diameter`, `OrientedAngle`, `SupportingLineOffset`,
`ExactTranslatedSegmentOffset` and `ProfileOffset`.

Constraint/dimension source identity, scalar targets, suppression, annotation keys and explicit
orientation/side/branch fields remain stable across rematerialization. Intrinsic origin and datum
axes are document-bound immutable logical inputs: concretely, they are typed parameters within an
action payload rather than step-output `LineageInputBinding`s, because no step owns them. They are
referenceable but never owned, moved, unconstrained, suppressed or deleted by a lineage step.

### Curve controls, properties and persistent state

Lineage covers all current curve definitions, including Line, Polyline, Circle, Circular Arc,
quadratic/cubic Bezier, Ellipse, Elliptical Arc, Rational Quadratic Conic, Parabola, Hyperbola,
B-spline and NURBS state.

The owner/write-back map covers every `DocumentCurveControlKind`: `Center`, `StartPoint`,
`EndPoint`, family-local `ControlPoint`, `Radius`, `TrimStart`, `TrimEnd`, `MajorAxisPoint`,
`MinorAxis`, `RationalMiddle`, `Vertex`, `Focus`, `TransverseAxisPoint` and `ConjugateAxis`.
The selected-curve property surface covers every `CurveNumericPropertyKind`: `Radius`,
`MinorAxisRatio`, `TrimStart`, `TrimEnd`, `SemiConjugate`, `RationalWeight` and each ordinal
`NurbsWeight`. It also preserves family/degree metadata, rational Euclidean/projective mode,
arc/elliptical-arc sweep, hyperbola branch, NURBS gauge and geometry role.

Every current explicit branch remains authored state. This includes line/polyline branch
directions; circular and elliptical sweeps; hyperbola branch; contact parameter, winding,
neighborhood, normal side, retained endpoint and periodic anchor; tangency mode/direction;
Fillet sides, trim endpoints, endpoint order and sweep; angle orientation; Profile Offset
direction, traversal, junction and terminal policy; spline/NURBS form, span/contact transition and
gauge; and source/element suppression or host activation. No evaluator may rediscover these from
coordinates or whichever nonlinear branch happens to converge.

Points, scalars, curves, contacts, trim views, sources, parameters, parameter bindings/outputs,
external bindings/snapshots and Profile/Construction roles are retained wherever the current
workbench can persist or manipulate them. Valid annotation-layout state remains keyed to stable
semantic owners and is preserved as its existing disposable presentation state.

### Native operations and computed features

All current `SketchOperationKind`s have closed lineage definitions with typed inputs, outputs,
parameters and identity deltas:

- `Split`, `Break`, `Trim`, `Extend`, `Mirror`, `Chamfer`, `AssociativeFillet`, `Rectangle`,
  `RegularPolygon`, `Slot`, `LinearPattern` and `ProfileOffset`.

This includes both current native workbench Fillet/Profile Offset publication and computed
`FilletSet` intent. A computed Fillet step publishes stable feature and corner lineage ports, but
its evaluated trimmed fragments and generated arcs remain authenticated revision-local derived
materialization under ADR 0031. M83 does not give those fragments false persistent native identity.

Current operations that change or replace flat topology must persist exact identity evidence for
their outputs. They may not depend on the transient vector order of an operation proposal after
the proposal has been accepted.

## Typed ports, ownership and identity flow

Each `LineageStep` has a monotonic `LineageStepId`, a durable host key distinct from its label, one
closed action definition, typed input ports, stable typed output ports, explicit dependencies and
the identity reservations needed by its materialization.

Ports distinguish identity kind and semantic role: point, scalar, curve/span, contact, trim view,
constraint, dimension, source, parameter, feature, feature corner and other current persistent
kinds cannot be interchanged. Variable output collections use stable child-port IDs rather than
array position as durable identity.

Ownership and lifecycle are orthogonal and explicit:

- an **owned** output may be rewritten, suppressed or retired only through its owner step;
- an **aliased** port refers to an exact earlier logical output without taking ownership;
- a **created** output reserves a fresh typed materialized identity;
- a **continued** output names the exact predecessor identity that survives an operation; and
- a **retired** output is a persistent tombstone that cannot be rebound or reused implicitly.

Split/replacement operations provide complete old-to-new evidence, including continued, created
and retired ports. A dependent that names a retired or ambiguous output becomes explicitly blocked
until the caller supplies a typed rebind/cascade edit. Coordinate equality, proximity,
tessellation order, insertion order and hashes with collision fallback are never identity rules.

`LineageMaterializationMap` is revision-stamped and bidirectional. It maps every logical port to
its current materialized identity, every writable materialized leaf to its exact owner-step field,
and each step to its complete owned materialization set. It also records feature/corner lineage
separately from computed generated-fragment evidence.

Typed persistent IDs are reserved from a lineage-owned document namespace and monotonic high-water.
Live, suppressed, failed, deleted and history-retained reservations are never handed to another
logical output. Materialization uses an atomic validated batch or equivalently narrow owning-domain
seam; it never exposes general unchecked explicit-ID insertion. Wrong namespace, stale base,
wrong kind/count/order, duplicate, dangling reference, invalid source ownership, spline-span cursor
regression or allocator regression rejects before publication.

## Deterministic evaluation policies

M83 exposes two deterministic policies over the same evaluator contract:

- `StrictChronological` evaluates every retained step in canonical stored order on fresh scratch
  state. Each topology-sensitive step consumes the complete independently accepted upstream
  prefix. This is the correctness oracle.
- `DependencyLocal` computes the exact dirty dependency closure and may reuse authenticated
  unaffected step materializations/checkpoints. It may change work performed and diagnostic
  telemetry only.

For the same lineage revision and immutable external inputs, both policies must produce identical
accepted/failed authority, canonical flat sketch and feature intent/evaluation digests, stable
logical/materialized identities, ownership map, explicit branches and independent validity
evidence. Differential tests compare DependencyLocal to a cold StrictChronological rebuild after
every supported edit class. Any disagreement rejects the optimized result and is a defect; there
is no policy-dependent accepted scene.

Every attempt carries lineage revision/digest, exact host/external-input stamps, deterministic
resource limits and cancellation state. Publication uses exact compare-and-swap. Cancelled,
exhausted, stale, structurally invalid, unsolved or independently rejected work publishes no
partial flat scene or feature result.

## Direct editing and owner rewrite

Direct manipulation starts only from an accepted materialization whose reverse map exactly matches
the retained lineage revision/digest. On release it:

1. collects every accepted point/scalar/branch value changed by the projection;
2. resolves each value through the reverse map to a writable recipe, relation, dimension,
   operation, feature or `ImportedBaseline` field;
3. requires a complete action-specific inverse for every affected owner;
4. emits one expected-revision `RewriteSteps` transaction containing all owners;
5. preserves unrelated action bytes and every explicit discrete branch; and
6. cold-rematerializes and independently proves reproduction before publication.

A projection may move fields owned by several steps. Updating one owner while leaving another
stale is never accepted. Derived handles such as native Profile Offset distance or computed Fillet
radius rewrite the declared parameter owner; sampled derived geometry is never stored as authored
placement. Read-only/gauge/host-owned controls remain unavailable under their current ownership
rules.

Deletion, suppression, reorder and rebind use the same dependency graph and identity evidence.
Deleting a step removes its complete owned set. Live dependents require an explicit typed cascade
or rebind transaction; the engine does not guess.

## Workspace v7 and honest migration

M83 introduces workbench workspace version 7. Its durable authority contains:

- retained and last-accepted lineage state with revisions/digests;
- stable step/port IDs, reservations, namespace and allocator high-waters;
- lineage history entries, cursor and lifecycle high-waters needed for exact Undo/Redo;
- external-input provenance and current retained failure/attempt evidence needed for recovery; and
- valid annotation-layout state keyed to stable semantic owners.

Workspace v7 may also contain a flat materialization cache: sketch document, derived
computed-feature document/evaluation, materialization/ownership map and their input stamps. The
cache is disposable. It is accepted only after its lineage revision/digest, accepted lineage
identity, external inputs, document namespace, allocator high-waters, logical/materialized map,
feature provenance and independent validity all verify. Missing, corrupt, stale or mismatched
cache data is discarded and rebuilt cold; it never repairs or changes lineage.

Workspace versions 1 through 6 migrate through their existing strict decoders. Migration creates
an honest `ImportedBaseline` root containing the exact decoded retained workbench intent. When the
legacy accepted scene differs from retained intent, migration also retains an explicit accepted
baseline checkpoint so the old accepted-versus-retained authority split survives. Existing sketch
IDs/high-waters, computed `FilletSet` intent and feature/corner high-waters, computed-evaluation
high-water, external state and valid annotation layout are preserved whenever that source version
contains them.

`ImportedBaseline` publishes typed logical ports for exact imported persistent identities and can
be field-rewritten as one honest baseline owner. It does not claim whether an object was authored
as a rectangle, individual lines, a Fillet or any other recipe. Migration creates one initial
history position and no fictional pre-import actions, pointer events or recipe ownership. New
lineage steps may depend on imported ports normally.

Round-trip and corruption tests cover every v1-v6 route into v7, including retained-invalid with
older accepted authority, feature/high-water preservation and cache discard followed by exact cold
recovery.

## Stateful DOM-free RPC

The JavaScript boundary is the versioned protocol `geosolve.lineage.rpc.v0`. It is a stateful,
DOM-free session protocol rather than a collection of unrelated JSON helper calls.

Every request/response uses a versioned envelope with a request ID, retained session ID, method,
opaque string revisions/identities and a closed payload/result or deterministic structured error.
The protocol covers session create/load/import, exact-revision lineage transactions, evaluation,
atomic exact-CAS structural owner rewrite, Undo/Redo, inspection and canonical workspace/lineage
export. That RPC rewrite is distinct from coordinator-projected direct manipulation, which derives
all affected owners from accepted geometry and cold-reproduces the projection before publication. Stale
session/revision, wrong-kind ports, invalid state transitions, resource exhaustion and domain
failures have stable discriminated results.

The Rust engine retains session state between calls. JavaScript/TypeScript owns neither geometry
equations nor hidden authoritative mirrors. A data-only TypeScript client may provide branded
handles and ergonomic structural builders, but Rust repeats all validation. Only editor-compiled
actions containing authenticated workbench materialization intent are cold-executable in M83;
generic caller-authored actions remain useful structural lineage but report
`workbench_materialization_unsupported` on evaluation. Standalone RPC load strips caller-certified
current and historical acceptance until an ordinary cold evaluation succeeds, whereas workspace
v7 cold-restores accepted authority from exact persisted host inputs. The adapter has no DOM,
`web-sys`, storage access, renderer, callback into JavaScript during evaluation or
`#[wasm_bindgen(start)]` hook, and it must initialize in a worker or Node-like host.

Native and WASM RPC transcripts must agree on canonical responses and final session authority.
Arbitrary TypeScript source rewriting and a workbench script editor remain outside M83.

## Implementation sequence

1. Adopt this supersession and ADR 0039 while freezing the M81 product/golden baseline and keeping
   M82 archived.
2. Complete the stable typed step/port, identity-delta, reservation and atomic materialization
   seams, including honest `ImportedBaseline` ownership.
3. Lower all 25 geometry recipes and the complete current constraint, dimension, curve-property,
   role and branch surfaces without introducing an editor-to-lineage dependency cycle.
4. Add all 12 current operation actions, native Fillet/Profile Offset ownership and computed
   `FilletSet` feature/corner lineage.
5. Implement `StrictChronological` as the oracle, then `DependencyLocal` with differential
   equivalence gates and retained/accepted failure authority.
6. Route every accepted workbench mutation and direct manipulation through atomic owner rewrites;
   remove writable flat workbench authority and nested history.
7. Add workspace v7, strict v1-v6 `ImportedBaseline` migration and authenticated disposable-cache
   cold recovery.
8. Add `geosolve.lineage.rpc.v0`, native/WASM transcript parity and the data-only TypeScript client.
9. Render the current retained program in a selectable Lineage panel beside Sketch Tree. Add an
   authority-derived Inspector, exact-CAS reorder controls and a collapsed compatible raw-action
   editor without adding browser persistence or a second program authority. Add predictive
   presentation for expensive captured point/control drags while preserving an exact terminal
   publication. Then run focused catalog, identity, migration, policy, RPC and workbench tests;
   run the unchanged baseline and complete release gate; nominate one immutable Tailscale build
   for human UAT.
10. Close only after the approved UAT scorecard, frozen-artifact evidence and standard exact
    GitHub Pages publication are recorded.

Steps 1–9 pass, including committed-source aggregate qualification and immutable replacement
nomination. Exact product source `d8137543fecf4a09433471e16383db5069de0d41`, tree
`031a8c95ddc99f1bf52847c0485ac857b0af1079`, passes the complete clean gate and focused
architecture/API/interaction and predictive-terminal reviews. Its exact no-rebuild seven-file
snapshot `/tmp/geosolve-m83-amendment-uat.AVC9ce` is frozen read-only with ordered-manifest
aggregate `260445142878a35c4e5cfed3934438c58c1e47b09a462cec4030e34f68da11e2` and is byte-verified at
`http://100.94.63.83:8080/` under PID `1733554`; temporary and final eight-request ledgers are
byte-identical at SHA-256
`7f7f1f3fa813dca1ee99e5a9fe4f6d622babbf65bfb342a6a1a37470874b34ee`. Step 10 remains blocked
only on M83-U1–U12, explicit supervising-human acceptance and exact Pages publication.

The prior read-only form of step 9 passed for source
`bb888cc68c00ad3a3823a9f2215528dfb357f9f9`, tree
`dff5ebebbfe024c00f88ba231362a3ea29d6e0bc`. Its exact no-rebuild seven-file snapshot
`/tmp/geosolve-m83-lineage-uat.1KL8gG` is byte-verified but withdrawn from current UAT; historical
PID `4152505` is retired after the replacement passed temporary verification. The pre-panel form
previously passed for source
`d378f7b31f56b43af787202dc1ebb92b7d199f84`, tree
`25a47e821cc80ff62d1891cfc7095d10fb2ec87f`; that snapshot is withdrawn and no longer served.

## Acceptance scenarios

### M83-W1 — complete geometry catalog

Author and reload each of the 25 `GeometryToolVariant`s, including repeated Polyline/NURBS output,
point reuse, intrinsic relations, modifiers and explicit sweep/branch state. Every recipe retains
its exact step kind and stable semantic ports. An exhaustiveness test fails if the current catalog
and lineage mappings differ.

### M83-W2 — complete relation and dimension catalog

Create, edit, suppress, unsuppress, delete, Undo/Redo and cold-reload every current persistent
constraint and dimension definition in all admitted modes/branches. Logical IDs, materialized
source IDs, target scalars and annotation keys remain stable; wrong-kind inputs reject atomically.

### M83-W3 — controls, properties, roles and branches

Exercise every editable curve control/property family, Profile/Construction role and explicit
branch action. Each edit rewrites its declared owner and round-trips through lineage/workspace v7.
Gauge-, host-, driving-dimension- and derived-feature-owned controls remain read-only for their
existing typed reason. No cold rebuild flips a stored branch.

### M83-W4 — complete native operation catalog

Apply all 12 `SketchOperationKind`s to their admitted operands. Verify typed inputs/outputs,
created/continued/retired mappings, whole-step deletion and dependent blocking/rebind. Insert,
delete or reorder unrelated earlier steps without changing surviving logical or materialized IDs.

### M83-W5 — native and computed Fillet/Offset ownership

Exercise native line-line/curve Fillet publication, native face/open-chain Profile Offset and
computed `FilletSet`. Native outputs retain exact typed native identities; computed feature and
corner ports remain stable while generated fragments stay revision-local. Editing a source or
radius/distance rematerializes the unchanged dependent intent, and deleting the step removes its
complete owned set.

### M83-W6 — atomic multi-owner direct editing

Project accepted drags that alter one owner and several owners across simple and advanced curves,
relations and operation parameters. Exactly the required owner steps are rewritten in one history
entry; no Move step or partial owner update appears. Cold reproduction matches the accepted
projection, and any unavailable inverse rejects the whole edit.

### M83-W7 — strict/local deterministic equivalence

After every supported insert, rewrite, suppress, delete, reorder, branch/property edit, Undo and
Redo class, compare a cold `StrictChronological` rebuild with `DependencyLocal`. Accepted/failure
authority, canonical sketch and feature digests, stable identities, ownership maps, branches and
hard-validity evidence match exactly; only policy telemetry may differ.

### M83-W8 — retained failure and history authority

Retain a structurally valid source rewrite that makes a later operation/feature invalid. The
retained lineage and one history position advance, the exact failing step is reported, and the
previous complete accepted lineage/materialization remains visible. Projected editing is disabled
against the stale reverse map; Undo and Redo restore exact programs/reservations. Stale, cancelled,
exhausted, malformed and non-finite requests record nothing.

### M83-W9 — workspace v7 and v1-v6 migration

Round-trip workspace v7 with retained/accepted lineage, history cursor/high-waters, features and
annotations. Migrate golden workspaces from each v1-v6 decoder through `ImportedBaseline`,
preserving all state available in that version without invented recipe history. Corrupt, omit or
swap the flat cache and prove cold rematerialization recovers the same accepted authority.

### M83-W10 — stateful native/WASM RPC parity

Replay identical multi-request `geosolve.lineage.rpc.v0` transcripts natively and through DOM-free
WASM: create/import, edit, evaluate, failed edit, Undo/Redo, inspect and export. Canonical envelopes,
errors, opaque IDs/revisions and final retained/accepted authority agree. A worker/Node-like smoke
test initializes without DOM or a start hook.

### M83-W11 — full-workbench authority

Instrument the sole demo workbench and prove every persistent geometry, relation, dimension,
property, role, branch, operation, Fillet, Offset, delete, direct-edit and history mutation enters
through lineage. Reloading without the flat cache reproduces the scene. No writable flat side path
or second user-visible history remains.

The same workbench presents that authority through a secondary selectable **Lineage** panel beside
Sketch Tree. Each render consumes the coordinator's borrowed current `LineageDocument` plus its
history cursor/availability directly. Chronological rows preserve stable step/schema identity and
Live/Suppressed/Deleted state, while the header distinguishes current program revision from the
Undo/Redo cursor over prior program versions. Selection opens a fresh coordinator-owned Inspector.
Buttons, position selection, keyboard movement and authenticated desktop drag/drop use one
exact-CAS reorder transaction; dependency boundaries clamp directionally, imported/tombstoned
steps are pinned and suppressed steps remain movable. A collapsed debug editor submits only a
compatible `LineageStepRewrite`, never identity or ownership manifests.

Captured flat-action `source_order` snapshots remain compatible serialized ordering state, not
semantic lineage inputs. Reordering independent constraint/dimension owners must replay source
order compositionally: retain currently ordered IDs that still exist, prune removed IDs and append
the sources newly materialized by the current action. The reordered chronology may change, while
stable native source identities and manifests do not.

Browser selection, dirty JSON draft, notices and drag nonce are transient and absent from
workspace v7. Predictive drag likewise owns presentation only: after an expensive exact captured
Point/CurveControl preview, moving frames may show coalesced cursor intent without rebuilding
durable panels, saving or publishing lineage. Context changes revoke it and release still requires
the newest terminal request to be the exact retained preview before one ordinary atomic owner
rewrite. Rejected terminal projection/recomposition consumes the gesture, retains accepted
authority and performs no workspace save. Tests must exercise reorder,
direct manipulation, Undo/Redo and cold reload together so this is a genuine topological-identity
stress rather than cosmetic list sorting.

### M83-W12 — human UAT and publication

An immutable Tailscale candidate passes focused human UAT across simple/advanced authoring,
constraints/dimensions, native/computed Fillet, Profile Offset, direct manipulation, Undo/Redo,
failure recovery and workspace reload/migration. Findings are fixed or explicitly dispositioned,
the frozen candidate passes the complete clean gate, and the accepted bytes are published and
verified on GitHub Pages.

## Qualification and closeout

- Every public lineage/workspace/RPC DTO has a version, deterministic ordering, bounded decoding,
  resource limits and structured errors.
- Catalog exhaustiveness and golden lineage fixtures cover all current variants, constraints,
  dimensions, operations, feature kinds, roles and explicit branch families.
- Identity tests cover owned/aliased/created/continued/retired outputs, high-water monotonicity,
  insertion/deletion stability, dependent rejection and exact Undo/Redo restoration.
- Policy differential tests make `StrictChronological` the oracle for every dependency-local edit
  class and cache path.
- Workspace migration/corruption tests cover v1-v7, retained-versus-accepted authority, feature
  high-waters, annotations and cold recovery from a discarded flat cache.
- Existing sketch, operation, topology, feature, editor, persistence and 271-row authoring/scene
  golden authority remain valid unless a separately reviewed workbench-lineage expansion is
  explicitly recorded. M83 adds no solver residual and therefore no replacement Jacobian rows.
- Formatting/diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature tests,
  native/WASM parity, performance, licence/package checks, Trunk and the complete release gate pass
  from committed source.
- Focused architecture/API/interaction review, editable-Lineage authority/presentation tests,
  predictive terminal-publication tests, replacement clean qualification and immutable
  byte-verified Tailscale nomination pass at the exact replacement source recorded above. Human
  UAT, explicit acceptance and exact GitHub Pages publication are still required before M83 can
  be closed.

## Explicit non-goals

M83 does not add new geometry families, solver equations, arbitrary-curve/computed Offset,
topology-changing Offset construction, computed-on-computed feature chains, B-rep/PDM topological
naming, formula/configuration/unit systems, collaboration/merge semantics, TypeScript source
rewriting, a browser script editor or npm publication.

Those are separate product decisions. They cannot weaken M83's requirement that the complete
current workbench has one authoritative lineage, exact identity flow and deterministic derived
materialization.
