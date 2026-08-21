<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0039: Sketch lineage and deterministic rematerialization

Status: proposed for M83

## Context

`SketchDocument` is intentionally a flat persistent design graph. It is the correct authority for
solver variables, constraints, dimensions, explicit branch state, accepted geometry and audit, but
it cannot truthfully answer higher-level questions such as:

- which placement recipe created a set of native entities;
- which authored values should change when accepted geometry is directly manipulated;
- which complete object set belongs to one later operation; or
- how a host should delete or regenerate a feature-like action without editing flat objects one by
  one.

Snapshot Undo/Redo and `ReplayAction` solve different problems. They record application state or
replay concrete interaction commands against revision-specific native IDs. They are not a durable,
declarative source program and are not persisted by the ordinary workspace.

ADR 0026 assigns application history, formula graphs, B-rep naming and cross-system transactions
to the host. ADR 0028 similarly leaves feature history and cross-revision production topology with
the host. A future CAD nevertheless needs a reusable GeoSolve-adjacent implementation of local
sketch action lineage so every embedding does not rebuild the same identity, rematerialization and
failure-authority machinery. This must remain optional host-side state rather than entering solver
or canonical sketch equations.

The rejected M82 exploration used ADR number 0038 on its archive branch. This mainline decision
therefore advances to 0039 and does not reuse that historical number.

## Decision

### Separate optional companion

Add `geosolve-sketch-lineage` as a separate pure safe-Rust host companion. M83 may compose public
`geosolve-sketch`, `geosolve-sketch-ops`, `geosolve-sketch-topology` and
`geosolve-geometry` APIs. It does not depend on `geosolve-sketch-features` until a later milestone
admits an explicit feature action. It owns no residual, Jacobian, rank policy, nonlinear solve,
curve equation, hidden accepted state or browser interaction.

The solver and domain crates do not depend on lineage. `geosolve-linkage` remains a separate model.
`geosolve-demo-web` remains a non-authoritative consumer and is not a dependency. Hosts may use the
flat sketch APIs without lineage exactly as before.

This decision narrowly amends ADRs 0026 and 0028: the host may delegate local sketch action lineage
to this optional module. Formula/configuration graphs, units, PDM/B-rep identity, application-wide
transactions and cross-system feature history remain host responsibilities.

### Lineage is source, not event history

`LineageDocument` is a separately versioned ordered dependency DAG of stable `LineageStepId`s.
Each step stores one closed action definition, stable typed output slots, explicit branch/property
intent and reservations for all persistent identities it owns.

Lineage records what the sketch means to rebuild, not every pointer event used to edit it. Direct
manipulation replaces writable fields of an existing owner step. There is no generic durable Move
action. A separate `LineageSession` history records accepted document patches so a rewrite remains
undoable without becoming another lineage feature.

The only generic edits are exact-revision insert, rewrite, delete, suppress, dependency-valid
reorder and explicit rebind. Malformed, stale, wrong-kind and otherwise structurally rejected
patches add no history position and clear no redo position. A structurally valid program edit is
retained and enters history once even if downstream rematerialization fails; the accepted
materialization pointer does not advance, and Undo restores the preceding valid program/result.
There is one user-visible application history at the lineage layer; scratch inner session
histories are never nested as another user-visible Undo stack.

### Honest legacy compatibility

Existing flat sketches cannot be reverse-engineered into truthful recipes or ownership. A
`SnapshotRoot` action contains one supported canonical-v4 legacy `SketchDocument` and exposes a
typed manifest of its existing persistent outputs. New lineage steps may refer to those outputs.
The root does not claim how the imported objects were originally authored.

Canonical lineage v1 never embeds, normalizes or promotes private draft-v5 bytes. Draft-v5,
workspace-v6 and computed-feature sidecars remain host state until a later supported
sketch-schema/workbench migration. Supporting that future schema requires an explicit lineage
migration rather than changing the meaning of v1 roots.

The lineage wire language remains separate from canonical sketch v1-v4, unstable draft-v5,
computed-feature persistence and the demo workspace envelope. Each language retains its own
version, digest, resource limits and migration policy.

### Stable logical output references

Downstream actions refer to `(LineageStepId, LineageOutputSlotId, kind)`, never directly to a
revision's materialized `DocumentElementId`. Fixed-cardinality steps attach semantic roles such as
`start`, `end`, `span` or `edge:right`. A host-facing durable key, separate from the mutable label,
supports complete-program reconciliation from TypeScript.

Three identity layers remain explicit:

1. stable lineage step/output identity;
2. persistent materialized sketch/source identity for the current build; and
3. accepted materialization revision identity for exact stale-work checks.

`LineageMaterializationMap` is revision-stamped and bidirectional. It maps logical outputs to
materialized identities, materialized writable leaves back to owning action fields, and each step
to every native/source object it generated.

### Explicit materialized identity reservation

M83 must not accept unrelated persistent-ID renumbering during a cold rebuild. Creation of a step
reserves typed materialized IDs from the target sketch high-water context and persists those
reservations with the step. Rebuilding inserts the reserved IDs through a narrow atomic
`SketchMaterializationBatch`, not public per-entity unchecked insertion methods.

`LineageDocument` owns the target sketch namespace and a materialized-identity high-water above
every live, suppressed, failed, deleted or history-retained reservation. Retaining a structurally
valid step after a completed but invalid rebuild advances this lineage high-water; those IDs cannot
be reused by another step. A stale, cancelled or exhausted attempt retains neither the patch nor
its staged reservations. Every later accepted flat materialization carries a native allocator
high-water at least that large, including when the reserving step is suppressed or deleted.

The batch carries the document namespace, expected base high-water, resulting merged high-water,
typed reservations, semantic-source catalog ownership and curve-local spline span cursors. Before
inserting anything it checks a foreign namespace, stale base, wrong identity kind, duplicates
against live or batched state, complete reference closure, semantic-source catalog reservations,
monotonic span cursors and high-water regression. Deleted and failed-step IDs retire without reuse;
Undo restores the same reservation. Identity is not derived from coordinates, tessellation, vector
position or a hash with collision fallback.

An operation step executes with a disposable step-scoped allocation context backed by its exact
reservation set. The resulting proposal must consume the expected identity kinds, count and order;
any mismatch rejects before scratch publication. Operations cannot escape the lineage allocator by
silently drawing from the rebuilt flat document's ambient `next_id`.

This explicit-ID seam is a blocking M83 proof. If it cannot preserve both stable identities and
the existing validation/monotonic-allocation contract, implementation stops for a revised ADR.
Revision-local renumbering is not an implicit fallback.

### Sequential transactional rebuild

Lineage materializes on scratch state in deterministic topological order. Every topology-sensitive
step consumes the freshly independently accepted upstream prefix and exact input stamp. Prefix
checkpoints may be cached by digest, but caches are disposable and cold/warm builds must agree.

One attempt carries immutable host input stamps, lineage revision/digest, cancellation and
deterministic work limits. Exact compare-and-swap publishes only a complete current rebuild whose
flat sketch results have passed their ordinary independent validation. Cancelled,
exhausted, stale or structurally invalid work consumes no live identities and publishes no partial
authoritative scene. A completed invalid program edit may retain only its staged lineage
reservations as described above; it cannot advance the accepted flat allocator or scene.

`LineageSession` exposes retained lineage intent, the latest per-step build attempt and the last
complete independently accepted materialization. A failed later action may expose typed failure or
non-authoritative prefix evidence, but the previous complete accepted materialization remains
authoritative.

### Direct manipulation rewrites owner inputs

An accepted projected edit is reconciled through the exact materialization map only when retained
lineage revision/digest matches the lineage that produced the accepted materialization. All
continuous point/scalar leaves required for deterministic replay are written back through
action-specific inverse mappings to their owner fields. Later constraints, dimensions and
operation definitions remain unchanged.

One projection may change fields owned by several placement steps. Reconciliation therefore emits
one expected-revision atomic `RewriteSteps` transaction containing every affected owner; it never
publishes a successful subset or appends independent Move events.

The rewritten lineage is cold-rematerialized and independently checked to reproduce the accepted
projection before publication. No inverse mapping may infer or alter side, sweep, winding,
neighborhood, traversal or other explicit branch state.

Projected reconciliation is all-or-nothing: a write-back, reproduction or downstream failure
retains neither the rewritten program nor a history entry. This differs from an explicit numeric
program edit, which may be structurally retained in history with a failed downstream rebuild and
the previous accepted materialization. While retained lineage is ahead/invalid, the old accepted
reverse map is unavailable until Undo or explicit repair produces a matching accepted build; M83
does not implicitly rebase it.

Derived Profile Offset coordinates are not authored placement data. A source edit rewrites the
source placement and reevaluates the unchanged Offset action. A later derived-handle design must
explicitly select a source or Offset-parameter owner rather than storing sampled target geometry.

### Deletion follows ownership

Deleting a step rebuilds without its complete ownership set. An Offset step therefore removes all
of its target geometry, scalars, connectivity, constraints and dimensions in one lineage edit; it
does not call flat object-by-object delete as the semantic operation.

Deletion with live dependent steps is not guessed. It returns a typed dependency result unless the
caller submits an explicit cascade/rebind transaction. Coordinate proximity never repairs a
missing reference.

### Topological naming boundary

M83 names fixed-cardinality placement outputs and one-to-one native Profile Offset outputs. The
latter use source-logical-output-keyed target slots. Operation identity evidence must explicitly
describe retained, replaced, proposed, retired or split results.

Variable-cardinality future actions require an explicit old-to-new/retired/split mapping and fresh
stable child slots. Ambiguous or missing mapping blocks dependents. M83 does not solve stable
computed-fragment identity, arbitrary trimming/Offset topology, B-rep naming or PDM replacement.
Generated computed-feature geometry remains revision-local under ADR 0031.

### DOM-free TypeScript/WASM boundary

Add a separate `geosolve-sketch-lineage-wasm` adapter with no start hook, DOM, renderer or storage.
Its narrow JSON-string ABI accepts/returns versioned discriminated DTOs for create/load,
complete-program reconcile, exact-revision patch, canonical export and evaluation.

A data-only TypeScript builder supplies explicit durable step keys and branded typed handles. It
may assemble DTOs but owns no geometry, constraint, operation, branch or acceptance logic. IDs and
revisions cross JavaScript as opaque strings. Rust never calls JavaScript during lowering, solving
or validation.

The engine does not rewrite arbitrary user TypeScript source. A host either treats lineage as
authoritative and generates code, or treats code as authoritative and applies returned typed field
patches to its own parameter/AST/override model before regeneration.

## M83 allocation

M83 accepts the architecture only after proving:

- stable reserved materialized IDs across cold rebuild, insertion, deletion and Undo/Redo;
- line placement, Horizontal constraint and direct-edit owner rewrite;
- the M78 2-Point Aligned Rectangle, native Profile Offset on its closed face, source rewrite and
  late Offset-step deletion;
- retained complete authority on a failed downstream rebuild;
- honest `SnapshotRoot` migration;
- canonical persistence, dependency/resource/stale/cancellation safety; and
- native/DOM-free-WASM/TypeScript-shape parity.

The full authoring catalog, complete workbench migration, computed-feature conversion, arbitrary
topology naming and npm publication are later milestones.

## Consequences

- Embedders can choose a CAD-like editable sketch program without changing solver equations or
  forcing all flat-API hosts to adopt feature history.
- Direct edits remain compact semantic rewrites while still participating in ordinary Undo/Redo.
- Deleting a feature-like action removes exactly what it owns and preserves unrelated stable
  outputs.
- Rebuild cost increases because topology-sensitive steps are sequential; safe digest-keyed prefix
  caches can reduce work without becoming authority.
- Action-specific write-back and stable identity reservations add API surface, but make ownership
  and failure behavior auditable instead of heuristic.
- TypeScript gains an OpenSCAD-like construction surface without moving geometry or solver truth
  into JavaScript.
- Host B-rep topological naming, script source rewriting and broad recipe migration remain
  explicitly unsolved rather than being approximated by coordinate matching.
