<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0039: Sketch lineage and deterministic rematerialization

Status: accepted for M83 implementation

## Context

`SketchDocument` is intentionally a flat persistent design graph. It is the correct solver-domain
input and accepted result for points, curves, constraints, dimensions and explicit numerical
branches, but it cannot truthfully answer higher-level authoring questions:

- which recipe owns a multi-entity shape and its intrinsic relations;
- which authored parameters must change when accepted geometry is directly manipulated;
- how identity flows through split, trim, mirror, Fillet, pattern or Offset operations;
- which complete object set belongs to a native operation or computed feature; or
- how the application can deterministically rebuild the same design after editing an earlier
  action.

Snapshot Undo/Redo and `ReplayAction` solve different problems. They preserve application state or
replay concrete interaction commands against revision-specific native IDs. `ComputedFeatureDocument`
owns computed `FilletSet` intent, but only as a sidecar beside the flat sketch. None is a complete,
durable declarative source for the current workbench.

ADR 0026 assigns formula/configuration graphs, B-rep naming and cross-system transactions to the
host. ADR 0028 assigns production topology/history to the host. Those boundaries do not require
each CAD embedding to reimplement local sketch action identity, dependency evaluation, owner
rewrite, failure authority and workbench migration. GeoSolve needs a reusable lineage layer while
keeping solver equations in their existing owning domains.

The first M83 plan in commit `56d1eda` proposed a narrow line/Horizontal/rectangle/Profile Offset
architecture slice, an opaque flat-document root, and left computed features and the ordinary
workbench outside its scope. That proof would leave two writable authorities and postpone the hard
catalog and persistence integration. M83 now requires the complete current workbench to cross the
boundary at once. The original commit remains historical design evidence but is superseded by this
decision.

The rejected M82 exploration used ADR number 0038 on its archive branch. This decision therefore
remains ADR 0039 and does not reuse that historical number or reactivate M82 behavior.

## Decision

### Lineage is the workbench authority

`LineageDocument` is the authoritative retained design source for the GeoSolve demo workbench.
Flat `SketchDocument`, derived `ComputedFeatureDocument` and evaluated computed geometry are
deterministic materializations. The workbench does not edit those flat products as a second
authority; every accepted persistent mutation first changes lineage and then publishes only an
independently validated materialization.

`LineageSession` retains the current lineage, latest evaluation attempt, last accepted lineage and
reproducible accepted materialization, plus one user-visible Undo/Redo history. A structurally valid
program edit may be retained when downstream evaluation fails, while the previous complete
accepted lineage/materialization remains visible. Scratch flat sessions and feature evaluators do
not expose nested application histories.

The public flat sketch and feature APIs remain supported for lower-level embedders. Making lineage
authoritative for the product workbench does not put feature history into `geosolve-core` or force
`geosolve-linkage` to share a sketch model.

### Separate orchestration layer and downward dependencies

`geosolve-sketch-lineage` is a separate pure safe-Rust crate. It may compose public
`geosolve-sketch`, `geosolve-sketch-ops`, `geosolve-sketch-topology`,
`geosolve-sketch-features` and `geosolve-geometry` APIs. It owns no solver residual, Jacobian,
rank/priority policy, curve equation, branch search or success shortcut. The solver and domain
crates do not depend on lineage.

The headless editor and demo workbench depend on lineage, never the reverse. Stable geometry-recipe
identity and atomic semantic recipe lowering currently located in `geosolve-constraint-editor`
must move behind or be exposed through the smallest dependency-safe lower seam. Browser event
state, selection/picking, SVG, panels and renderer state do not enter canonical lineage.

The workbench may retain non-authoritative drafting and annotation presentation state. A completed
gesture becomes a typed lineage transaction; it never authoritatively mutates a private flat copy.

### Lineage is an editable program, not an event log

Canonical `LineageDocument` wire version 1 is a document-bound ordered dependency DAG of stable
`LineageStepId`s. Each step stores:

- a durable host key distinct from a mutable label;
- one closed action definition and explicit suppression state;
- typed input references and stable typed output-port manifests;
- all authored continuous parameters and explicit discrete branches;
- complete output ownership and identity-flow evidence; and
- typed materialized-ID reservations/high-water effects.

The mutation vocabulary is exact-revision insert, atomic multi-step rewrite, delete,
suppress/unsuppress, dependency-valid reorder and explicit cascade/rebind. Direct manipulation
rewrites existing owners; there is no generic durable Move step. A malformed or stale transaction
records no history. A structurally valid explicit program edit records one history position even
if its downstream evaluation fails.

### Complete current action surface

M83 lineage covers all 25 current `GeometryToolVariant`s, including variable-cardinality
Polyline/NURBS recipes, point reuse, intrinsic relations, modifiers and explicit recipe branches.
It covers every current persistent constraint and dimension definition, curve control/property,
Profile/Construction role, source/element activation state and explicit branch exposed by the
current workbench.

It also covers every current `SketchOperationKind`:

`Split`, `Break`, `Trim`, `Extend`, `Mirror`, `Chamfer`, `AssociativeFillet`, `Rectangle`,
`RegularPolygon`, `Slot`, `LinearPattern` and `ProfileOffset`.

Native Fillet and Profile Offset publication are native operation/materialization paths. Computed
`FilletSet` is a lineage feature action with stable feature and corner ports. Its evaluated trimmed
fragments and generated arcs remain authenticated revision-local results under ADR 0031; the
lineage layer does not invent persistent native identities for computed fragments.

Catalog mappings are exhaustive. A new geometry variant, constraint, dimension, operation,
feature kind, role or branch family requires an explicit lineage representation and test rather
than an unknown/fallback flat mutation.

### Stable typed ports and explicit identity flow

Downstream steps refer to typed `(LineageStepId, LineageOutputPortId, kind)` identities, never a
revision-local vector index or whichever object is nearest. Stable child-port IDs cover repeated
outputs. Document-bound intrinsic origin/X/Y axes are immutable typed action parameters rather
than step-output `LineageInputBinding`s or deletable step outputs; no owner step exists for an
intrinsic datum.

Ownership and lifecycle are recorded separately:

- **owned** ports identify objects/intent controlled and retired by the step;
- **aliased** ports reuse an exact earlier output without taking its ownership;
- **created** ports reserve fresh typed materialized identity;
- **continued** ports name the exact predecessor that survives an operation; and
- **retired** ports are tombstones that remain unavailable for implicit reuse or rebind.

Every replacement/topology operation supplies complete old-to-new evidence. Missing, ambiguous or
retired outputs explicitly block dependents until a typed cascade/rebind transaction resolves
them. Identity is never inferred from coordinates, proximity, tessellation order, insertion order
or a collision-prone hash.

Three identity layers remain visible:

1. stable lineage step/port identity;
2. persistent native sketch/source/feature identity for a materialization; and
3. accepted lineage/materialization revision identity used for stale-publication checks.

`LineageMaterializationMap` is revision-stamped and bidirectional. It maps logical ports to current
materialized identities, writable materialized leaves back to exact owner-step fields, and each
step to its complete materialization set. Computed feature/corner ports are distinguished from
revision-local generated-fragment evidence.

### Explicit reservation and atomic materialization

`LineageDocument` owns the target sketch namespace and monotonic typed identity high-waters.
Reservations retained by live, suppressed, failed, deleted or history-retained steps are never
reused. Inserting or deleting an earlier step therefore cannot renumber an unrelated later output;
Undo restores the same reservations.

Materialization inserts reserved identities only through a narrow atomic validated batch or an
equivalent operation-owned allocation context. Before mutation it checks namespace/base CAS,
identity kind/count/order, duplicates, reference closure, semantic-source ownership,
curve-local spline span cursors and merged high-water. Operations cannot escape their step-scoped
reservations by drawing from a scratch document's ambient allocator.

Wrong namespace, wrong kind/count/order, stale base, exhaustion or invalid mapping rejects before
publication. General unchecked explicit-ID insertion is not added to the public sketch API.

### Strict and dependency-local evaluation

Two policies implement one semantic evaluator contract:

- `StrictChronological` cold-evaluates every step in canonical stored order and is the correctness
  oracle. Every topology-sensitive step consumes a complete independently accepted upstream
  prefix.
- `DependencyLocal` evaluates the exact dirty dependency closure and may reuse authenticated
  unaffected materialization/checkpoint state.

For identical lineage and immutable external inputs, the policies must return identical retained
failure/accepted authority, canonical sketch and feature digests, logical/materialized identities,
ownership, explicit branches and independent validity. Only work counts and policy telemetry may
differ. Differential comparison to a cold strict rebuild gates every optimized edit class; a
mismatch rejects the optimized result.

Each attempt carries lineage revision/digest, immutable host/external-input stamps, deterministic
work limits and cancellation. Exact compare-and-swap publishes only a complete current result that
has passed the ordinary owning-domain validation. Stale, cancelled, exhausted, structurally
invalid, unsolved or independently rejected work publishes no partial scene.

### Direct manipulation atomically rewrites every owner

Projected editing is available only when retained lineage exactly matches the accepted reverse
ownership map. It collects every changed accepted point/scalar/branch leaf, resolves an
action-specific inverse for each owner and emits one expected-revision `RewriteSteps` transaction
containing every affected owner.

No successful subset may publish. No inverse rule may infer side, sweep, winding, neighborhood,
traversal or other discrete branch state from coordinates. Derived handles rewrite their declared
source/parameter owner; sampled derived geometry is not stored as authored placement.

The proposed lineage is cold-rematerialized and independently checked to reproduce the accepted
projection before the rewrite/history entry publishes. Any missing inverse, partial owner update,
reproduction mismatch or downstream failure rejects the entire projected edit and preserves the
previous retained/accepted authority.

Deleting a step retires its complete owned output set. Live dependents return a typed dependency
result unless the caller submits an explicit atomic cascade/rebind transaction.

### Honest `ImportedBaseline` migration and workspace v7

Workspace version 7 persists authoritative retained and last-accepted lineage, revisions/digests,
stable identity reservations/high-waters, one lineage history/cursor, required external provenance
and valid annotation-layout state.

Versions 1 through 6 are decoded by their existing strict version-specific decoders and migrated
through an `ImportedBaseline` root. The root owns exact decoded legacy retained intent and exposes
typed ports for its persistent identities. If legacy accepted authority differs from retained
intent, migration preserves a separate accepted baseline checkpoint. Existing sketch identity and
allocator state, computed `FilletSet` intent and feature/corner/evaluation high-waters, external
state and annotations are retained whenever present in the source version.

`ImportedBaseline` states only that these objects and feature intents were imported. It does not
synthesize rectangle/Fillet/operation recipes, pointer events or pre-import history. A field edit
of imported data rewrites the baseline owner atomically. Deleting an imported persistent entity
instead appends a later explicit `Retired` lifecycle action, leaving the immutable root payload and
port manifest intact. New semantic steps may refer to live imported ports normally.

Workspace v7 may include a disposable flat cache containing sketch/feature materialization and the
identity/ownership map. The cache is accepted only after lineage revision/digest, accepted lineage,
external inputs, namespace/high-waters, identity map, feature provenance and independent validity
all verify. Missing, corrupt, stale or swapped cache data is discarded and rematerialized cold.
The cache never becomes authority or repairs lineage.

### Stateful DOM-free RPC

The external JavaScript boundary is `geosolve.lineage.rpc.v0`, a stateful session protocol with
versioned request/response envelopes. Requests carry request/session identity, exact expected
revision/digest, one closed method and payload. Responses echo correlation identity and contain a
closed result or deterministic structured error. All IDs, revisions and high-waters cross
JavaScript as opaque strings.

The protocol covers create/load/import, exact lineage transaction, evaluation, atomic exact-CAS
structural owner rewrite, Undo/Redo, inspection and canonical export. That structural rewrite is
not the coordinator's projected direct-manipulation transaction, which derives every owner and
cold-reproduces accepted geometry. Rust retains the session between calls and performs all
geometry, dependency, identity and acceptance validation.

The adapter has no DOM, renderer, browser storage, `web-sys`, JavaScript callback during solve or
`#[wasm_bindgen(start)]`. A TypeScript client may provide data-only branded handles/builders but is
not geometry authority. Editor-compiled actions with authenticated private materialization intent
are cold-executable; a generic caller-authored structural action is retained but cannot invent that
intent and reports `workbench_materialization_unsupported` on evaluation. Standalone RPC load
strips caller-certified current and historical accepted authority pending a fresh cold evaluation;
workspace v7 instead cold-restores every current/historical accepted authority from its exact
persisted host inputs and replaces serialized latest-attempt metadata with fresh owning-domain
evidence. Native and WASM RPC transcripts must produce the same canonical results and final
session state.

## M83 acceptance allocation

M83 implements and qualifies this decision only after proving:

- exhaustive lineage coverage of all 25 geometry recipes, all current constraints/dimensions,
  curve controls/properties, roles, explicit branches and all 12 operation kinds;
- native Fillet/Profile Offset and computed `FilletSet` ownership without false computed-fragment
  identity;
- stable typed owned/aliased/created/continued/retired ports and reserved materialized identities
  across cold rebuild, insert/delete, suppression and Undo/Redo;
- atomic multi-owner direct-edit rewrite with no Move step or partial owner update;
- `DependencyLocal` equivalence to the `StrictChronological` oracle for every supported edit class;
- workspace-v7 authority, strict v1-v6 `ImportedBaseline` migration and disposable-cache recovery;
- complete workbench mutation routing through lineage with no writable flat side path; and
- native/DOM-free-WASM `geosolve.lineage.rpc.v0` transcript parity.

M83 remains in progress until automated qualification, focused architecture/API review,
immutable-candidate human UAT and standard exact GitHub Pages publication all pass. Acceptance of
this ADR authorizes implementation; it is not milestone/product acceptance.

## Consequences

- The workbench gains one auditable declarative source instead of coordinating flat sketch,
  computed-feature sidecar, replay commands and history as peer authorities.
- Complete catalog migration is larger than the earlier proof, but it avoids making the first
  lineage schema and workspace migration knowingly incomplete.
- Stable ports and explicit identity deltas make direct editing, deletion, operation replacement
  and host references deterministic at the cost of action-specific lowering/write-back code.
- Strict evaluation provides a simple oracle; dependency-local evaluation may recover performance
  only while remaining observably equivalent.
- Workspace v7 can discard corrupted/stale derived state and recover from lineage, while legacy
  files remain truthful through opaque imported ownership rather than invented recipes.
- Computed feature/corner intent becomes part of the same program without claiming stable identity
  for revision-local generated geometry.
- Embedders that need only flat sketch APIs retain them; the demo workbench and RPC expose the
  higher-level lineage product.
- B-rep/PDM naming, arbitrary TypeScript source rewriting, collaboration/merge, formula/unit/config
  systems and new geometry/Offset capabilities remain separate host/product decisions.
