<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0030: Headless sketch-operation authoring

Status: accepted

## Context

M58 established `geosolve-sketch-ops` as the deterministic, equation-free owner of prepared
split, trim, mirror, fillet and related transaction proposals. M62 established
`geosolve-constraint-editor` as the presentation-independent owner of CAD constraint and
dimension authoring. The surviving workbench can invoke some prebuilt M58 operations, but it has
no reusable interaction contract for collecting geometric operation operands, choosing explicit
branches, previewing an independently accepted result or committing that exact result.

Fillets, offsets and mirrors are especially unsafe to infer in a presentation adapter. A local
fillet needs curve parameters, retained portions, normal sides, trim endpoints, winding,
endpoint order and arc sweep. A line offset needs a selected semantic span, explicit side,
distance, orientation and one of two truthfully different dimension definitions. A mirror needs a
supported source family and an exact line axis. Initial coordinates may seed a proposed branch,
but cannot become unrecorded branch authority.

M66 needs CAD-like helper authoring without adding equations, approximating unsupported families,
creating a second accepted-state path or moving gesture policy back into the browser.

## Decision

### Dependency amendment

This ADR extends ADR 0029's allowed dependency graph:

- `geosolve-constraint-editor` may depend directly on `geosolve-sketch-ops` in addition to
  `geosolve-sketch` and `geosolve-geometry`;
- `geosolve-sketch-ops` remains unaware of the editor, web consumer, topology companion and
  linkage domain; and
- `geosolve-sketch`, `geosolve-geometry`, `geosolve-core` and `geosolve-linkage` remain unable to
  depend on the editor or operations companion.

The editor consumes only the operations companion's public immutable snapshot, request, proposal,
application and typed outcome APIs. This dependency adds no private solver access.

### Separate operation-authoring state

The editor owns a separate `OperationAuthoringState`; it does not overload M62's fixed-arity
constraint/dimension `AuthoringState`. Its closed M66 tool set is:

- associative 2D fillet;
- line offset (associative single-span or one-shot joined-chain); and
- exact supported-family mirror.

The state publishes typed tools, finite model-space picks, expected next operands, pending stages,
options, warnings, preview status and terminal outcomes. A pick carries persistent selection
identity, exact curve parameter where applicable and finite model position. Application selection
may seed a compatible operation once; an empty selection enters persistent repeated mode.

The first Escape clears a staged candidate and the second exits the tool. Apply or Enter commits a
complete accepted preview. Pan and zoom are presentation navigation and remain available while an
operation tool is active. Terminal success, retained rejection, unsupported input or coordinator
failure clears the completed candidate and re-arms repeated mode without silently changing tool
options.

### Preview and publication ownership

`RetainedEditorCoordinator` owns the complete operation lifecycle:

1. capture one immutable `SketchOperationSnapshot` from the current retained session;
2. synthesize one fully explicit public operation request from headless state;
3. execute the request against scratch state with deterministic operation control;
4. apply the proposal only to a scratch retained session;
5. expose preview geometry only when that scratch result is independently accepted for the exact
   input; and
6. on Apply, publish through the proposal's ordinary exact-input compare-and-swap transaction and
   add one normal coordinator history checkpoint.

Cancelled, unsupported, incomplete, stale, exhausted or retained-rejected work cannot carry an
accepted preview and cannot mutate live design, accepted state, selection or history. A successful
commit selects the primary created curve identified by the operation result. Undo, Redo, workspace
persistence and later ordinary constraint/dimension editing use the existing coordinator paths.

The web workbench forwards normalized events and renders these DTOs. It does not reconstruct
applicability, locate a fillet root, choose a side, derive an offset equation, reflect controls or
apply a proposal directly.

Operation canvas acquisition reuses the editor's ordinary presentation-neutral hit policy. Curves
have an inclusive 12-pixel screen-space acquisition radius; nearest distance wins and an exact tie
uses persistent identity. When the scene includes accepted scratch preview geometry, the best
visible preview-only item blocks click-through to a source item underneath but is not forwarded as
a live-document operand. The resulting empty operand event remains available to the headless state
for fillet-radius or offset-side placement.

### Fillet policy

A fillet normally collects two distinct visible curve-span picks near the portions the user
intends to retain. One unambiguous interior-point pick on an open polyline is a deliberate
shortcut: the headless editor atomically expands it to the two ordered adjacent spans and records
the first span's `End` and second span's `Start` as trim ownership. Polyline endpoints, ambiguous
corners and every other same-support pair reject without mutation. The picked parameters are local
seeds. The headless editor performs a deterministic, bounded local branch synthesis only; it does
not enumerate global roots.

Before preview, the request explicitly materializes both spans, picked parameters, contact
neighborhoods, winding, normal sides, retained trim endpoints, periodic anchors where required,
endpoint order, arc sweep, positive radius and driving/reference mode. Once both parents are
known, the state enters a dedicated pointer-placement stage that derives the proposed radius from
finite model-space pointer input. `0.1 * SketchDocument::model_scale()` is only its finite fallback
seed. The ordinary radius dimension defaults to Reference; Driving requires explicit user intent.
Defaults otherwise prefer the picked retained portions and a minor output arc. Flip-first-side,
flip-second-side and alternate-arc controls are explicit corrections.

A circular arc publishes its stored center as presentation-neutral semantic drag metadata.
Dragging the body of a free associated fillet therefore routes through the ordinary projected
center-drag lifecycle and updates its center, radius, contacts and retained parent intervals as one
accepted edit. Deleting only the radius dimension removes that dimension without deleting the
fillet association or this drag route.

Ambiguous local roots, duplicate supports, already-trimmed parents, parallel or singular offsets,
zero-speed/pole/cusp geometry and escaped parameters remain typed warnings or operation failures.
Same-support fillets other than the explicit adjacent-polyline shortcut and global root search are
deferred.

### Line-offset policy

M66 retains one `geosolve-sketch-ops` request for a single line or polyline span. It atomically
creates two target points, one target line, one positive scalar and one driving public offset
dimension. Exact translated segment is the default. Supporting-line offset is an explicit
alternate mode and retains its truthful axial-slide/length freedoms. Left/right side is explicit;
generated endpoint orientation is explicitly `Same`. The authoring pointer may seed side and
distance, but both are stored as typed state before preview.

A distinct joined-offset request accepts an explicitly clicked ordered chain of at most 32 unique,
endpoint-connected line/polyline spans. It offsets every support by one requested signed distance,
uses intersections of adjacent offset supporting lines for interior miters and atomically emits
one ordinary one-shot polyline. It creates no persistent offset scalar, dimension or association
and performs no recursive path discovery, healing, caps or self-intersection repair. Duplicate,
disconnected, over-budget or unresolved-miter input rejects without partial allocation. The
Exact/Supporting choice remains a single-span contract and cannot masquerade as joined-chain
semantics. General curve and approximated conic/spline/NURBS offsets remain unsupported.

The shared Line offset tool must expose that boundary before commit. With exactly one span its
guidance says the result is associative and later source edits propagate. With two or more spans it
says the joined result is one-shot and source edits do not propagate. Persistent associative
multi-span offsets remain deferred rather than being inferred from path collection.

### Mirror policy

Mirror collects one source curve and then one line axis. One source is committed per transaction;
repeated mode permits rapid consecutive operations. Line, polyline, quadratic/cubic Bezier and
non-rational B-spline sources use the existing exact point-defined construction. Circle, arc,
ellipse, conic, rational and NURBS families remain typed unsupported and are never tessellated or
approximated. Multi-source transactions are deferred.

## Verification

Direct operation tests own request expansion, identity mapping, exact-CAS behavior, both
single-span offset modes/sides, bounded joined-chain expansion and rejection, supported mirror
families and fillet transaction outcomes. Direct editor tests own preselection, repeated
collection, adjacent-polyline corner resolution, exact picked parameters, pointer radius
placement, Reference/Driving intent, option/branch state, preview acceptance, Apply/Escape,
terminal re-arming, stale work, semantic arc-center drag, history and persistence lifecycle.
Direct workbench tests own only palette/icon presentation, event routing, preview rendering and
ordinary editable sample integration.

The existing M25/M27/M28 derivative, all-family and independent-validation corpora remain the
mathematical gate because M66 adds no residual. Native, warnings-denied Clippy, locked all-feature
workspace, WASM and release Trunk qualification must pass before the M66 human UAT begins.

## Consequences

- Fillet/offset/mirror interaction policy becomes reusable across browser, native and future
  sketch-plane hosts.
- Explicit branch state remains inspectable and correctable instead of being hidden in pointer
  coordinates.
- Preview and commit share one independently validated operations/publication seam.
- A free fillet remains an ordinary movable associated construction instead of silently gaining a
  hard radius from its fallback seed.
- The operations companion owns both the associative single-span line-offset transaction and a
  bounded non-associative joined-chain transaction, but no solver or UI state.
- General curve offsets, persistent associative profile offsets, automatic chain discovery,
  approximate mirrors, global fillet search, multi-source mirror and a CAD feature tree remain
  future scope.
