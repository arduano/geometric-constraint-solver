<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0030: Headless sketch-operation authoring

Status: accepted; amended 2026-08-02 for the Fillet-only M66 pivot

## Context

M58 established `geosolve-sketch-ops` as the deterministic, equation-free owner of prepared
split, trim, exact supported-family Mirror, Fillet and related transaction proposals. M62
established `geosolve-constraint-editor` as the presentation-independent owner of CAD constraint
and dimension authoring. The surviving workbench needs a reusable interaction contract for
collecting Fillet parents, choosing explicit branches, previewing an independently accepted result
and committing exactly that result.

Fillets are unsafe to infer in a presentation adapter. A local Fillet needs curve parameters,
retained portions, normal sides, trim endpoints, winding, endpoint order, arc sweep and meaningful
contact bounds. Initial coordinates may seed a proposed branch, but cannot become unrecorded branch
authority.

The first M66 candidate applied the same authoring framework to Fillet, Line offset and Mirror.
Human UAT showed that carrying three tools obscured architectural defects in the Fillet path. That
unapproved candidate is preserved at `origin/archive/m66-three-helper-tools-2026-08-02`, commit
`80d4939`. Active M66 is deliberately Fillet-only. It withdraws only the candidate's Offset/Mirror
authoring state, UI, samples and tests, plus its M66-only single-span/joined-chain line-offset
request APIs. M25's signed Offset constraints and M58's exact supported-family Mirror
operation-companion API remain accepted, unchanged capabilities.

## Decision

### Dependency amendment

This ADR retains ADR 0029's amended dependency graph:

- `geosolve-constraint-editor` may depend directly on `geosolve-sketch-ops` in addition to
  `geosolve-sketch` and `geosolve-geometry`;
- `geosolve-sketch-ops` remains unaware of the editor, web consumer, topology companion and
  linkage domain; and
- `geosolve-sketch`, `geosolve-geometry`, `geosolve-core` and `geosolve-linkage` remain unable to
  depend on the editor or operations companion.

The editor consumes only public immutable snapshots, Fillet requests/proposals, application APIs
and typed outcomes. This adds no private solver access.

### Separate Fillet-authoring state

The editor owns a separate, Fillet-only `OperationAuthoringState`; it does not overload M62's
fixed-arity constraint/dimension `AuthoringState`. The state publishes finite model-space picks,
expected next operands, pending stages, branch/radius options, warnings, preview status and
terminal outcomes. A pick carries persistent selection identity, exact curve parameter where
applicable and finite model position. Compatible selection may seed the operation once; empty
selection enters persistent repeated mode.

The first Escape clears a staged candidate and the second exits the tool. Apply or Enter commits a
complete accepted preview. Pan and zoom remain available while Fillet is active. Confirmed success,
retained rejection, unsupported input or coordinator failure clears the completed candidate and
re-arms repeated mode without changing remembered process-local options.

An unconfirmed hover is different. If pointer-derived radius/branch synthesis becomes invalid, the
state clears only the transient candidate and scratch preview. It retains both parents and remains
in radius-placement mode so later valid pointer motion can recover without reselecting them. This
is the required disposition for `M66-F008`.

### Preview, current-publication eligibility and publication

`RetainedEditorCoordinator` owns the complete Fillet lifecycle:

1. capture one immutable `SketchOperationSnapshot` from the current retained session;
2. synthesize one fully explicit public Fillet request from headless state;
3. execute it against scratch state with deterministic operation control;
4. apply the proposal only to a scratch retained session;
5. expose preview geometry only when scratch publication is independently accepted for the
   eligible current publication; and
6. on Apply, publish through the proposal's ordinary exact-input compare-and-swap transaction and
   add one normal coordinator history checkpoint.

Geometry-dependent authoring must not use literal request equality as a proxy for current accepted
geometry. A successful point-position attempt may contain a transient one-shot
`candidate_request`, while its retained prepared input correctly omits that drag. The sketch domain
therefore owns a publication-compatible comparison that ignores only `candidate_request` and still
matches design, publication request, solver policy, activation, parameters, external snapshots,
accepted identity and originating/latest attempt. Exact proposal compare-and-swap is unchanged.
This is `M66-F010`; it is not permission to accept a stale or rejected state.

Cancelled, unsupported, incomplete, stale, exhausted or retained-rejected work cannot carry an
accepted preview and cannot mutate live design, accepted state, selection or history. A successful
commit selects the created arc. Undo, Redo, workspace persistence and later ordinary
constraint/dimension editing use the existing coordinator paths.

### Shared acquisition and preview barriers

Operation hover and click consume the same editor-owned, presentation-neutral hit-test result.
Curves have an inclusive 12-pixel screen-space acquisition radius; nearest distance wins and an
exact tie uses persistent identity. The exact boundary is accepted for both hover and click.

When accepted scratch preview overlaps source geometry, the best preview-only foreground item
blocks click-through to the source but is neither hoverable nor forwarded as a live-document
operand. The resulting no-operand pointer event remains available to radius placement. This
preserves the relevant `M66-F004`/`M66-F005` behavior and closes hover/click divergence as
`M66-F009`.

### Fillet policy

A Fillet normally collects two distinct visible curve-span picks near the portions the user
intends to retain. One unambiguous interior-point pick on an open polyline is a deliberate shortcut:
the headless editor atomically expands it to the two ordered adjacent spans and records the first
span's `End` and second span's `Start` as trim ownership. Polyline endpoints, ambiguous corners and
every other same-support pair reject without mutation. Picked parameters are local seeds. The
editor performs deterministic, bounded local branch synthesis only; it does not enumerate global
roots.

Before preview, the request explicitly materializes both spans, picked parameters, winding,
normal sides, retained trim endpoints, periodic anchors where required, endpoint order, arc sweep,
positive radius and Driving/Reference mode. Once both parents are known, the state enters a
dedicated pointer-placement stage. The finite fallback is only a seed. The ordinary radius
dimension defaults to Reference; Driving requires explicit user intent. Defaults otherwise prefer
the picked retained portions and a minor output arc. Flip-first-side, flip-second-side and
alternate-arc controls are explicit corrections.

M66 authoring uses a closed parent-pair policy. Two affine line/polyline spans persist
`ContactNeighborhood::Interior` on both parents. When exactly one parent is non-affine, the affine
span remains `Interior` and the curved span receives a strict `Local` cell from the public,
non-mutating `SketchDocument::certify_line_curve_fillet_branch_cell` query. The caller supplies the
complete bounded curved span or one explicit unwrapped period. The query reuses the private
outward-rounded all-family interval/curve-piece kernel and certifies that
`cross(curve_tangent(t), fixed_line_direction)` is finite, excludes zero and retains the selected
sign throughout the returned cell. Its safe endpoints conservatively approach the nearest
tangent-parallel barriers without crossing them. This branch certificate, rather than an arbitrary
fraction around the seed, is the narrowed disposition of `M66-F011`.

Two non-affine-parent authoring returns typed `UnsupportedFilletPair` feedback until a bounded
pairwise-continuation contract exists. This is an authoring-abstraction limitation, not a domain
capability removal: M28's all-family generic Fillet request, persistent association, residual and
independent validation remain public and unchanged.

A circular arc publishes its stored center as presentation-neutral semantic drag metadata.
Dragging the body of a free associated Fillet routes through ordinary projected center drag and
updates center, radius, contacts and retained parent intervals as one accepted edit. Deleting only
the radius dimension removes that dimension without deleting the association or this drag route.

Ambiguous local roots, duplicate supports, already-trimmed parents, parallel or singular offsets,
zero-speed/pole/cusp geometry, escaped parameters and two-non-affine authoring remain typed warnings
or operation failures. Same-support Fillets other than the adjacent-open-polyline shortcut,
pairwise curved continuation and global root search are deferred.

### Workbench presentation

The workbench exposes one text-free Fillet action. It forwards normalized events and renders
headless DTOs; it does not reconstruct applicability, locate roots, choose branches or apply a
proposal directly. Fillet options render as a viewport-clamped overlay over the canvas, outside the
scrolling palette's overflow context. Placement/clamping is covered by pure tests for all viewport
edges and resize (`M66-F007`). The **2D fillet workshop** is an ordinary editable save-like sample
with no guide, protected entity, scripted action or alternate coordinator.

There is no active M66 Offset or Mirror action, options panel or sample. This presentation choice
does not deprecate or remove the older M25/M58 domain APIs.

## Verification

Direct operation tests own Fillet request expansion, identity mapping, exact proposal CAS,
polyline-corner ownership and local branch synthesis. Direct sketch branch-cell tests own the
outward-rounded symmetric-cubic and explicit-period certificates plus typed/non-mutating failures;
direct sketch lifecycle tests own current-publication compatibility after successful point edits
and rejection of genuinely stale/different input. Direct editor/coordinator tests own affine
`Interior` persistence, line/curved certified-cell integration, typed two-curved-parent refusal,
preselection, repeated
collection, exact picked parameters, pointer radius placement, Reference/Driving intent,
option/branch state, preview acceptance, invalid-hover recovery, Apply/Escape, stale work,
semantic arc-center drag, shared hover/click acquisition, history and persistence. Direct
workbench tests own only icon/palette presentation, overlay placement, event routing, preview
rendering and ordinary editable sample integration.

The existing M27/M28 derivative, all-family and independent-validation corpora remain the
mathematical gate because M66 adds no residual. M25 Offset and M58 Mirror regressions must also
remain green to prove the pivot did not remove their pre-M66 APIs. Formatting, warnings-denied
Clippy, locked all-feature workspace tests, WASM and release Trunk qualification must pass on one
post-pivot source before M66 human UAT begins.

## Consequences

- Fillet interaction policy is reusable across browser, native and future sketch-plane hosts.
- Explicit branch state remains inspectable and correctable instead of being hidden in pointer
  coordinates.
- Invalid exploratory hover is recoverable without discarding expensive parent selection.
- Hover, click, preview barriers and exact acquisition tolerances cannot diverge by renderer CSS.
- Preview eligibility follows current accepted publication semantics without weakening exact
  proposal publication.
- A free affine Fillet retains full parent interiors. A free line/curved Fillet remains an ordinary
  movable associated construction inside its interval-certified tangent branch, not a hidden hard
  radius or arbitrary seed-centred cage.
- M25 Offset constraints and the M58 Mirror operation companion remain intact, but their M66
  authoring experiments are archived rather than maintained in active UI scope.
- Pairwise continuation for two non-affine parents, global Fillet search, a CAD feature tree,
  Offset/Mirror authoring and new equations remain future scope; M28's existing generic Fillet API
  remains available below the authoring layer.
