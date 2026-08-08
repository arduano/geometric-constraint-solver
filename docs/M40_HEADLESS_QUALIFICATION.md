<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M40 headless-editor qualification record

## Status

Complete and historical as of 2026-07-26. The requirements, proposed APIs, gap tables,
open questions, browser commands and recommendations below preserve the
pre-implementation investigation at its checkpoint; they are not current instructions.
The dated completion result at the end records the implemented M40 boundary. M48 later
replaced durable browser claims with direct Rust/WASM owners and removed the browser
harness. Current qualification and human review are owned by M52-M53.

M67 subsequently retired the evidence-only M40 matrix, JSON transition runner and golden digest
after mapping all fourteen retained behaviors to current direct tests. See
`docs/M67_M40_OWNERSHIP.md`; the file paths and runner commands below remain historical evidence,
not live repository instructions.

## M40.4 lifecycle and action contract

### Requirements

- M40.4 must make `geosolve-constraint-editor` the presentation-independent owner
  of lifecycle projection, edit/action applicability, effects, and replay over
  public `geosolve-sketch` APIs. It must not solve equations, infer an accepted
  result from a core report, or make attempted geometry authoritative.
- The retained-session owner is `RetainedSketchDocumentSession`, not the older
  accepted-only `SketchDocumentSession`: M40.4 must retain structurally valid,
  unsolved design edits while continuing to render the last independently accepted
  state.
- Every mutating editor effect must carry the exact current
  `SketchDesignIdentity`; stale input must be a typed no-mutation result. A
  construction macro must be one `transact` effect, not a sequence of individually
  published edits.
- Presentation DTOs must identify their design/attempt/accepted provenance. An
  attempt, its candidate geometry, mappings, failure, and rejection are evidence;
  accepted geometry, audit, and reference values come only from
  `SketchAcceptedDocumentState`.

### Evidence and source pointers

- `PLAN.md` M40.4 requires retained design/attempt/accepted coordination,
  constraint/dimension/delete/suppression/history/stale transitions, presentation
  DTOs, and deterministic replay; `ACCEPTANCE.md` M40.4 additionally requires
  transactional rollback through public sketch APIs.
- `RetainedSketchDocumentSession::{design_identity,design_document,last_attempt,
  accepted_state,apply,transact,reattempt,import_design_json,export_design_json,
  export_accepted_json,revision_high_water,restore_design,restore_design_with_accepted}`
  is the public lifecycle surface. `apply` and `transact` require a
  `SketchDesignIdentity`, retain a valid candidate even on solve rejection, and
  return `RetainedDocumentTransactionOutcome<T>` with the new design and attempt
  identities plus an optional `published_accepted_identity`.
- `SketchDesignIdentity`, `SketchAttemptIdentity`, and
  `SketchAcceptedStateIdentity` each include document identity and a separate
  revision. `SketchDocumentAttempt` supplies its exact `SketchAttemptInput`, parent
  accepted identity, optional published accepted identity, optional report/candidate
  geometry/mappings, and `SketchAttemptFailure`. `SketchAcceptedDocumentState`
  coherently supplies its identity, solved document, runtime/mappings, and one
  accepted `SketchSolveResult`.
- Valid but unsolved conflicts are deliberate: `tests/m34_lifecycle.rs`
  `conflicting_edit_is_retained_and_an_ordinary_repair_is_accepted` shows design and
  attempt revisions advancing while accepted JSON/identity remain unchanged; source
  suppression repairs that design and publishes the next accepted revision. The
  topology-divergence test proves attempt mappings cannot interpret accepted state.
- `DocumentEdit` is the closed edit vocabulary: `CreateConstraint`,
  `CreateDimension`, `SetDimensionMode`, `SetSourceSuppressed`, and `Delete` cover
  the required ordinary actions; `DocumentCommandEffect` returns persistent affected
  identities. `SketchDocument::{constraints,dimensions,source,source_order,sources}`
  exposes the design graph for editor-owned applicability/selection projection.
- The older `SketchDocumentSession` has revision-checked `DocumentCommand`, atomic
  `apply`/`transact`, and accepted-only `can_undo`, `can_redo`, `undo`, and `redo`.
  Its documentation explicitly says rejected commands do not mutate history.
  `RetainedSketchDocumentSession` exports no undo/redo/history API.
- `DocumentMeasurementCatalog` evaluates persistent M38 measurements against a
  retained session. `DocumentMeasurementValue` includes finite value/unit/work and
  `DocumentMeasurementAudit` (source ID/label/template/unit/provenance,
  independent-evaluation flag, rows). Its provenance is explicitly
  `AcceptedDocument { revision }` or `RetainedDesign { revision }`; M38 tests reject
  stale or foreign provenance without mutating the session.
- M36/M37 also expose separate catalog/session APIs:
  `DocumentSemanticSourceCatalog`, `DocumentSemanticCatalogSession`,
  `DocumentSemanticSolveResult`, and `DocumentPlanarAudit`. They are accepted-only
  catalog solves, not extensions of `RetainedSketchDocumentSession`.
- The current web `DomainAdapter` is contrary evidence, not a target design: it
  locally derives `LifecycleView`, scans `display_audit` for redundancy, formats
  `SolveRejection`, computes dimension values, and maintains snapshot history. Those
  are precisely the duplicated report/history policy M40.4 must replace.

### Decisions / inferred constraints

#### Minimal editor state, API, and effects

Use one `ConstraintEditor` state machine with: retained session; editor selection and
tool/gesture state from M40.2/M40.3; a retained-history sidecar; and a fully derived
`EditorView`. The sidecar stores opaque restore checkpoints, never a second solver
state: `{ design_json, accepted_json: Option<String>,
SketchLifecycleRevisionHighWater }`, cursor, and ordered valid retained editor
transactions (accepted or rejected).
It restores through `restore_design[_with_accepted]`, so revision high-water is not
reused. Preview sessions remain transient and never enter that history.

Proposed minimal public editor boundary (names are proposed, not existing symbols):

```text
EditorInput = pointer/key/widget/replay input | Execute(ActionId, expected_design)
EditorEffect = Apply { expected_design, edit: DocumentEdit }
             | Transact { expected_design, proposal: ConstructionProposal }
             | Reattempt { expected_design, request: DocumentSolveRequest }
             | RestoreHistory { checkpoint, expected_design }
EditorView   = { lifecycle, identities, accepted_scene, selection, actions,
                 problems, measurements, audit, history }
```

`Apply` calls retained `apply`; a multi-object dimension/construction calls retained
`transact`; `Reattempt` calls retained `reattempt`. After each outcome the editor
rebuilds all DTOs from the session rather than preserving a locally edited scene.
The host performs only the returned effect against its owned session and returns the
typed outcome; the editor owns deterministic conversion of that outcome to state.

#### Revision, lifecycle, and history invariants

- A valid retained edit advances design and attempt once, whether it solves or not;
  accepted revision advances only when that exact attempt publishes an independently
  accepted state. Invalid/non-finite/referentially invalid edits and stale identities
  advance none. Reattempt advances attempt (and possibly accepted) but not design.
- Always label/render design from `design_document()`. Render accepted scene, accepted
  reference dimensions, and accepted audit only from `accepted_state()`; its document
  may predate design. Candidate geometry/mappings are explicitly attempt preview or
  problem evidence and never replace accepted scene/audit.
- A stale `DocumentSessionError::StaleDesign` leaves editor state, session, history,
  selection, and preview unchanged except for a typed stale notice; refresh actions
  against the returned current identity. Never retry an old edit automatically.
- A valid retained edit, including a rejected solve, is an undoable design
  transaction: snapshot the *pre-transaction lifecycle*, append the post-transaction
  checkpoint, truncate redo, and clear transient preview. This permits an explicit
  undo of a conflicting dimension without making the rejected attempt accepted.
  Invalid/stale/cancelled operations add no checkpoint. A repair that later accepts is
  another history entry.
  Undo/redo are restore-and-reattempt operations and may publish fresh, never-reused
  lifecycle revisions; they cannot promise historical attempt/accepted revision
  numbers. A failed/cancelled restore must retain the selected checkpoint/session.
- This is necessarily editor-owned until sketch supplies retained-history APIs. Do
  not use `SketchDocumentSession` history as a substitute: it cannot retain unsolved
  design intent and operates in a different revision domain.

#### Available-action rules

- Compute actions from ordered persistent selection plus the *design* graph, not
  screen indexes, candidate geometry, or report classifications. An action is
  enabled only when every selected ID exists in the current design, has the required
  kind/arity/order, required support/span is valid, all operands share the current
  document, and the editor can construct a finite closed `DocumentEdit` or atomic
  transaction. Otherwise publish a deterministic disabled reason.
- Constraint and dimension actions must construct the exact typed
  `DocumentConstraintDefinition`/`DocumentDimensionDefinition` (including required
  explicit side, orientation, span, winding, branch, mode, scalar unit/domain, and
  target). No coordinate-proximity inference or fallback definition is permitted.
  Driving/reference is an explicit `DocumentDimensionMode`; changing it uses
  `SetDimensionMode`.
- Delete is enabled for an extant selected `DocumentObjectId`; its only mutation is
  `DocumentEdit::Delete`. Suppress/unsuppress is enabled only for the selected extant
  `DocumentSourceId` when its current design `source(...).suppressed` differs from
  the requested state, and uses `SetSourceSuppressed`. Effects, not the UI, determine
  dependency cleanup and accepted/rejected lifecycle consequences.
- Undo/redo action availability is the retained-history cursor, not accepted-state
  revision comparison. Reattempt is enabled only with the current design identity;
  action availability must not call or interpret a solve report.

#### DTO provenance rules

- `LifecycleDto` is a tagged identity relationship, not a UI inference from core
  fields: `Accepted` when last attempt published its accepted identity; `DesignUnsolved`
  when no accepted state exists and the last attempt did not publish; `RejectedAttempt`
  when an older accepted state exists and the latest attempt did not publish;
  `SolvedPreview` only for a transient preview with its own attempt identity; and
  `Solving` only while an effect is outstanding. Include all applicable identities.
- `ProblemsDto` carries attempt identity, design identity, optional parent accepted
  identity, and verbatim domain-level `SketchAttemptFailure` or `SolveRejection`
  evidence. It must not classify `core_report`, scan rows for redundancy, or format a
  synthetic “success” from termination/rank fields. A structured accepted redundancy/
  conflict DTO requires a sketch-owned public adapter; until then it is a gap, not a
  browser/editor interpretation task.
- `MeasurementDto` carries the measurement source ID, value, unit, work, and the
  exact `DocumentMeasurementAudit` provenance. Publish accepted measurements only
  when `AcceptedDocument { revision }` matches the accepted identity revision; a
  retained-design measurement must be visibly tagged design and match design revision.
  Reject stale/foreign/missing values rather than borrowing a nearby revision.
- `AuditDto` preserves source/document identity, row descriptors and whether it is
  accepted or attempt evidence. Accepted audit comes exclusively from the accepted
  state's `solve_result`; attempted audit comes only from that attempt's
  `solve_result` plus that attempt's mappings. Do not combine mappings, geometry, or
  rows across views, and do not reinterpret `SolveReport` into UI semantics.

### Native scenario matrix

| Scenario | Required assertions |
| --- | --- |
| Initial conflict and repair | Initial conflicting design has design+attempt but no accepted state; suppressing the conflicting source retains design, creates one new attempt, and then publishes accepted state. |
| Rejected dimension | Add incompatible driving dimension; design contains it and attempt/problem names that attempt, prior accepted scene/audit/measurements and accepted identity remain byte-identical; one undoable retained-design history entry is recorded. |
| Accepted reference/driving dimension | Exact applicable selection enables each action; reference value is accepted-provenance only, mode change is revision-checked, and stale measurement provenance is withheld. |
| Delete and suppression | Delete/suppress effects use persistent IDs; rejected unsuppression remains repairable design while older accepted source stays suppressed; undo/redo restoration preserves IDs and never lets dependency cleanup become UI policy. |
| Retained undo/redo | Accepted edit checkpoints restore design plus optional accepted graph/high-water; redo truncates after a new accepted edit; unsolved design is not silently discarded; restored attempts use fresh revisions. |
| Stale effect | Submit an old `SketchDesignIdentity` after another edit; get `StaleDesign`, with no changes to lifecycle, scene, selection, preview, or history. |
| Preview/release/cancel | Candidate preview has separate attempt identity and never alters accepted DTOs/history; release commits exactly one eligible action; cancel leaves all retained identities unchanged. |
| Reload | Restore design-only and design+accepted snapshots with high-water metadata; no accepted claim without accepted bytes, no revision reuse, and design/accepted divergence remains visible. |
| Accepted redundancy/attempt diagnostics | Accepted redundancy and rejected conflict are displayed only if a domain DTO preserves correct provenance; assert no editor code scans `display_audit` or `core_report`. |

### Open questions

- `RetainedSketchDocumentSession` has no retained undo/redo/history/checkpoint API;
  the proposed editor sidecar needs a concrete snapshot/effect ownership and failure
  contract before implementation.
- No public sketch lifecycle/problems DTO exists. In particular, M40.4 cannot meet
  the required accepted redundancy/conflict presentation without interpreting
  `SketchSolveResult::{core_report,display_audit}`; a sketch-owned stable adapter or
  explicitly opaque pass-through DTO is needed.
- M36/M37 semantic catalog sessions and M38 measurement catalog are separate from
  retained document lifecycle. Their composition, revision bridge, mutation/history
  ownership, and one coherent audit/measurement DTO remain unspecified.
- Public APIs expose closed edits but not an authoritative applicability constructor
  matrix for all M37/M38 actions. The editor can validate IDs/arity/type, but exact
  semantic operand construction and deterministic disabled reasons need a frozen
  editor matrix or a sketch query API.
- `SolveRejection` is public but non-exhaustive and `SketchAttemptFailure` messages
  are strings; stable problem codes/text suitable for presentation are not yet a
  documented sketch DTO contract.

### Out of scope

- Adding editor/sketch APIs, changing session/history implementation, solver/report
  semantics, or modifying the browser adapter.
- M41+ activation, parameters, external snapshots, prepared concurrent jobs, and
  host/application history. M40.4 only documents the currently public M34-M38
  lifecycle boundary.

## M40.5 web interaction-policy migration inventory

Status: implemented and qualified on 2026-07-26. The inventory below records the
pre-migration ownership evidence and the disposition used to review the completed
adapter; references to the old modules describe the removed M39 implementation.

Completion evidence: `workbench/mod.rs` now translates browser events and centrally
dispatches `EditorEffect`s through `RetainedEditorCoordinator`; `scene.rs` renders
`EditorScene` and typed construction proposals; `panels.rs`, `persistence.rs` and
`evidence.rs` consume persistent editor/coordinator DTOs. `app_state.rs`,
`domain_adapter.rs`, `selection.rs` and `tools.rs` are deleted. The locked WASM check,
release Trunk build, source-policy assertions and fresh-profile `e2e/m40.mjs` pass.
Broader generated/model-based and per-scorecard-action parity evidence belongs to M40.6.

### Requirements

- M40.5 replaces the workbench's selection, tools, drafts, gesture thresholds,
  constraint compatibility, history orchestration, and lifecycle inference with
  `geosolve-constraint-editor` inputs, state, and effects.
- The adapter renders editor scene primitives and persistent identities, without
  DOM/CSS-authoritative hit geometry; duplicate workbench policy must be deleted,
  not retained as a fallback.
- Browser-only ownership is event translation, SVG presentation, accessibility,
  storage, files, and evidence capture. Native editor tests remain the policy
  oracle; focused fresh-profile browser tests qualify the adapter.

### Evidence and source pointers

- `PLAN.md` M40.1 assigns normalized input, viewport, scene, hit testing,
  selection, drafting, gestures, applicability, effects, lifecycle, and replay to
  the headless crate. M40.5 names the web removals and retained browser boundary.
- ADR 0029 §§Decision/Verification policy states that the editor owns selection,
  scene tessellation, picking, gestures, drafting, snapping, action applicability,
  lifecycle presentation, and replay; the browser maps platform events, renders
  DTOs, and applies effects.
- `workbench/mod.rs` currently combines DOM event registration/translation with
  policy helpers and the `Workbench { domain, app, scene }` aggregate.
- `workbench/app_state.rs`, `selection.rs`, `tools.rs`, `scene.rs`, and
  `domain_adapter.rs` contain the duplicate interaction state and policy described
  in the inventory below.

### File/symbol inventory

| Current owner | Symbols | Current responsibility | M40.5 disposition |
| --- | --- | --- | --- |
| `app_state.rs` | `AppState`, `PointDrag`, `LifecycleView`, `AppState::record` | Tool, ordered selection, draft/snap/cursor state, click/drag threshold state, lifecycle inference display, transcript | Delete/replace with editor state and replay/lifecycle DTOs; browser may retain presentation notice wiring only if supplied by editor effects. |
| `selection.rs` | `SelectionItem`, `Selection::{set,toggle,clear}` | Persistent selection identity and ordered replace/toggle policy | Delete; render and forward editor selection identities only. |
| `tools.rs` | `Tool::{from_key,key,required_points}` | Tool vocabulary and point-count draft completion policy | Delete/replace with editor tool input/state. DOM tool keys remain adapter mapping data only. |
| `scene.rs` | `RetainedScene::{rebuild,svg_markup}`, `tessellate_span`, `subdivide`, `MAX_DEPTH`, `CHORD_TOLERANCE` | Accepted-scene construction/tessellation and scene cache | Delete/replace with rendered editor scene primitives. |
| `scene.rs` | `.wb-curve-hit` markup and `data-select-*` index mapping | DOM/CSS hit target as selection authority | Delete; DOM targets may route/accessibility-focus events but must not determine geometric selection. |
| `scene.rs` | `project_arc_endpoint`, `draft_markup` | Arc projection and tool/draft preview policy | Delete/replace with editor preview primitives and construction proposals. |
| `domain_adapter.rs` | `DomainAdapter::{create_point,create_line,create_rectangle,create_circle,create_arc}` and helpers `distance`, `direction` | Draft-to-document construction, endpoint reuse, arc branch/sweep choice | Move construction proposals/commit effects to editor; delete web policy wrappers. Domain session invocation follows editor effects. |
| `domain_adapter.rs` | `create_constraint`, `create_dimension` | Selection filtering and constraint/dimension applicability/definition policy | Delete; editor publishes available actions and typed `DocumentEdit` effects. |
| `domain_adapter.rs` | `HistoryEntry`, `history`, `cursor`, `transact`, `apply`, `undo`, `redo`, `restore`, `can_undo`, `can_redo` | Web-owned history orchestration | Delete; editor/session lifecycle-history transitions own it. |
| `domain_adapter.rs` | `drag_preview`, `preview_point_move`, `clear_drag_preview`, `move_point` | Projected drag preview/commit/cancel policy | Delete/replace with editor gesture transitions and effects. |
| `domain_adapter.rs` | `lifecycle`, `problem`, `accepted_dimensions`, `diagnostic_evidence` | Lifecycle inference and UI interpretation of session/audit data | Replace with editor lifecycle/problems/measurement/audit DTOs; browser only renders/captures supplied values. |
| `mod.rs` | `install_canvas`, `install_keyboard`, `draw_point`, `finish_draft`, `snap_point`, `point_distance`, `select_index`, `clear_draft`, `finish_operation` | Pointer/keyboard policy, snapping, drafting, selection, lifecycle transitions | Delete policy helpers; retain listener registration and platform-event normalization/dispatch to editor. |
| `mod.rs` | `install_clicks`, `perform_action` | Mixed DOM dispatch plus tool/action/selection policy | Retain only DOM control decoding and typed editor-input dispatch; remove direct `AppState`/`DomainAdapter` mutations. |
| `mod.rs` | `install`, `render`, `save`, `pointer_model_point`, `input_value`, `select_value`, `required` | Workbench bootstrap, DOM/SVG rendering, browser storage write, DOM value extraction, and client-to-normalized-coordinate measurement | Retain as adapter functions, but make `render` consume editor DTOs and limit `pointer_model_point` to finite platform-coordinate/viewport observation rather than editor-policy decisions. |
| `panels.rs` | `tree_markup`, `row`, `escape` | HTML tree presentation and escaping; currently also paints web-owned selection | Retain presentation/escaping only; render editor selection identities and emit typed editor inputs rather than indexes interpreted in the web crate. |
| `persistence.rs` | `STORAGE_KEY`, `LEGACY_STORAGE_KEY`, `WorkspaceSnapshot`, `WorkspaceRevisions` | Browser storage envelope/versioning | Retain browser-storage concern pending the snapshot interface decision; no lifecycle/history inference may be added here. |
| `evidence.rs` | `capture`, `EvidencePayload`, `EvidenceEnvelope`, `checksum`, `download` | Browser environment metadata, Blob/URL download, scene export, checksum | Retain browser evidence/download mechanics; replace direct `AppState`/`DomainAdapter` diagnostic and transcript reads with editor/session evidence DTOs. |
| `platform.rs`, `routing.rs` | `window`; `Route`, `parse` | Browser window access and URL route presentation | Retain. |

### Decisions / inferred constraints

- **Headless ownership is disjoint:** all deterministic interpretation after
  platform-event normalization—including viewport-aware hit testing and selection
  compatibility—moves to the editor. The browser cannot preserve a parallel
  index-based selection or CSS-hit fallback.
- **Browser retention is disjoint:** `platform::window`, `persistence.rs`
  `WorkspaceSnapshot` encode/decode plus `localStorage` I/O, `evidence.rs`
  download/Blob/URL operations, DOM listener installation, SVG/HTML formatting,
  ARIA/widget state, route parsing, and file/browser APIs remain. Snapshot contents
  must be obtained from editor/session DTOs rather than reconstructed policy.
- `Cargo.toml` presently lacks a `geosolve-constraint-editor` dependency; adding
  it is a prerequisite for the adapter migration (implementation work, not this
  documentation change). Its WASM dependencies identify retained browser surfaces:
  DOM events, storage, file I/O, downloads, location/window, and accessibility
  widgets.
- The current SVG coordinate conversion in `mod.rs::pointer_model_point` is browser
  event normalization/viewport measurement; the resulting normalized input and all
  viewport validity/picking decisions belong to the editor under ADR 0029.

### Migration order

1. Add the editor dependency and construct one editor/session owner in the
   workbench; preserve only a browser adapter aggregate.
2. Translate pointer, keyboard, modifier, widget, and viewport measurements into
   normalized editor inputs; render returned state/effects without direct policy
   mutation.
3. Replace `RetainedScene`, `draft_markup`, SVG hit paths, and indexed
   `data-select-*` authority with editor scene primitives keyed by persistent IDs.
4. Replace `AppState`, `Selection`, `Tool`, snapping/draft helpers, and drag state
   with editor state; remove duplicate gesture thresholds and arc projection.
5. Route editor effects to public session edits, render lifecycle/problems/action
   availability/measurements from editor DTOs, then delete `DomainAdapter` policy
   and its local history/preview/lifecycle interpretation.
6. Reconnect storage, evidence capture, downloads, accessibility, and route/UI
   controls to editor/session DTOs; delete superseded workbench paths before adding
   fresh-profile adapter checks.

### Browser acceptance checks

- Fresh profile: platform pointer/keyboard/modifier and widget events reach the
  editor; rendered persistent IDs and selection state match returned editor DTOs.
- DOM hit targets are non-authoritative: a wide SVG/CSS target cannot select an
  object inconsistent with editor hit resolution, and no adapter geometry pick is
  present.
- SVG correctly renders editor accepted scene, draft/drag previews, lifecycle,
  action enablement, problems, measurements, and retained accepted geometry after a
  rejected effect.
- `localStorage` restore/save preserves supplied design/attempt/accepted identities;
  invalid stored data reports a browser-visible restore error without replacing the
  valid editor/session state.
- Accessibility controls dispatch the same typed inputs as pointer/keyboard paths.
- Evidence capture downloads its checksum-valid JSON and SVG using editor/session
  DTOs, without independently interpreting diagnostics or transcript semantics.
- Focused replacements for the current `e2e/m40.mjs` browser assertions cover shell
  routing, SVG coordinate/event wiring, selection rendering, draft/drag preview
  rendering and cancellation, persisted reload (including rejected retention),
  downloads, and the developer-lab route; its constraint/dimension applicability,
  snapping, projection, click-versus-drag, and lifecycle policy assertions move to
  native editor tests.

### Open questions

- Which exact editor DTO/effect APIs from M40.3/M40.4 will carry snapshots,
  diagnostics/evidence transcript, storage restore input, and download-ready scene
  identity is not yet established by the allowed sources.
- Whether `WorkspaceSnapshot` remains a web schema or is replaced by an
  editor-provided persistence DTO needs an M40.4/M40.5 interface decision.
- The future browser adapter's source-check rule (forbidden symbols/modules or
  behavioral patterns) is not specified; it must be concrete enough to prevent a
  second policy path.

### Out of scope

- Implementing the editor, modifying Rust/WASM dependencies, or changing browser
tests.
- Solver equations, sketch authority, persistence semantics owned by
  `geosolve-sketch`, and non-browser host integrations.

## M40.6 qualification matrix and implementation plan

Status: implemented and qualified on 2026-07-26. The requirements and original gap
table below are preserved as the pre-implementation plan; their `Partial`/`Missing`
labels are historical and are superseded by the executable all-covered artifact in
`docs/M40_QUALIFICATION_MATRIX.json`.

Historical completion evidence: `qualification::run_m40_qualification()` consumes the checked-in
transition corpus and emits one canonical report. Native tests validate that report
against golden bytes and validate every machine-matrix evidence ID; the release WASM
exported the same runner, and the now-retired `e2e/m40.mjs` compared its report
byte-for-byte while qualifying only browser platform boundaries. The seeded bounded model schedules and
executes all creation, snapping/picking/selection, constraint, dimension,
projected-drag, history/persistence, conflict/repair, lifecycle/retention, redundancy
and malformed/boundary classes. `RetainedEditorCoordinator::add_selected_dimension`
owns dimension-family applicability, and accepted redundancy comes verbatim from a
provenance-bearing sketch DTO. The supported release build plus browser suite passes
14/14 without retries; locked workspace Clippy/tests and the locked WASM check pass.

### Requirements

- M40.6 closes only when every objective UAT-C1 scorecard action has deterministic
  native state-machine/model evidence, equivalent behavior exercised in the locked
  WASM consumer, and a focused browser-adapter assertion where a platform boundary is
  involved. A browser E2E or human observation alone is not coverage.
- The gate additionally requires generated/model-based transitions, deterministic
  replay, exact threshold/overlapping-hit/scale-and-viewport/cancellation/malformed
  input/persistence/accepted-retention matrices, and a machine-readable cross-link
  from each action to its evidence (`PLAN.md` M40.6; `ACCEPTANCE.md` M40).
- Browser scope remains event normalization, rendering of editor DTO identities,
  accessibility/widget dispatch, `localStorage`, and evidence downloads. In
  particular, selection, picking, snapping, drafting, applicability, drag projection,
  lifecycle classification, history, and diagnostic policy must remain in
  `geosolve-constraint-editor`/`RetainedEditorCoordinator`, not be reimplemented in
  the adapter.

### Evidence and source pointers

| Objective action / M40.6 requirement | Native editor/coordinator evidence now | WASM parity evidence now | Focused browser-adapter evidence now | Status and smallest exact addition |
| --- | --- | --- | --- | --- |
| Geometry creation; finish/cancel (scorecard, UAT-C1-F1) | `lib.rs` tests `every_core_draft_has_exact_completion_and_cancellation`, `draft_transition_matrix_covers_tools_stages_modifiers_and_interruption`, `degenerate_terminal_candidates_rollback_and_a_valid_retry_completes`, and `nonfinite_inputs_and_modifiers_do_not_disturb_normalized_state` cover core tool stages, cancellation and invalid terminals. | Only `cargo check --target wasm32-unknown-unknown`/Trunk compilation is recorded; no named same-vector WASM result. | `e2e/m40.mjs` checks line preview then Cancel; `mod.rs` routes pointer, double-click and Enter to editor APIs and renders proposal SVG. It does not cover all point/line/polyline/rectangle/circle/arc completion/cancel vectors. | **Partial.** Add a shared serialized transition corpus for every tool/stage/finish mechanism/cancel/interruption and run it natively and from WASM; add one browser wiring case per distinct platform completion route (pointer, Finish button, double-click, Enter, Escape/pointercancel), not geometry-policy assertions. |
| UAT-C1-F1 live preview, polyline guide and endpoint snapping | `preview_and_commit_share_identical_typed_operands`, `snapping_is_identity_ordered_and_exactly_inclusive_at_tolerance`, and `proposal_apply_uses_public_document_construction_and_is_atomic` prove proposal operands/snap identity natively. | No named parity vector. | `e2e/m40.mjs` asserts a line draft path and cancellation only; it does not assert polyline guide/Finish, endpoint reuse, letterbox conversion, or both-line endpoint sharing. | **Partial.** Corpus must include snap-inside/on/outside tolerance, equal-distance ID tie, reused endpoint topology, and letterboxed normalized input. Browser cases assert guide visibility and that emitted/rendered persistent identities show reuse; no browser distance/snap calculation. |
| Canvas/tree/Inspector selection coherence, including UAT-C1-F3 multi-select and line picking | `line_is_selected_from_screen_space_without_dom_hit_targets`, `point_has_priority_at_a_line_endpoint`, `extended_line_selection_exposes_and_builds_parallel_relation`, and click threshold test cover line tolerance, priority, ordered Shift selection and a typed Parallel edit. | No executed parity evidence. | `e2e/m40.mjs` creates/selects two points with Shift and applies Coincident; it asserts persistent tree IDs/no index selection. It does not exercise curve selection, tree-to-canvas coherence, Ctrl/Command, overlap ties, or Inspector. | **Partial.** Add generated hit/selection sequences (replace/toggle, Shift/Ctrl/Command, stale/deleted selection), exact 6.5/7 px and overlap/tie vectors across viewports/scales. Run identical vectors in WASM. Add browser event/render cases for curve selection and tree selection identity reflection only. |
| Constraint application and persistent glyph (fixed, coincident, horizontal, vertical, parallel, perpendicular, equal length) | `extended_line_selection_exposes_and_builds_parallel_relation`; coordinator `action_matrix_dimensions_and_replay_are_deterministic`, stale-effect tests, and persistent-ID delete/suppression tests. `ConstraintEditor` defines all seven action kinds. | No executed parity evidence. | `e2e/m40.mjs` applies only Coincident and sees one glyph. `mod.rs` locally decodes a widget key then calls editor `constraint_edit`; current E2E source policy prevents the removed legacy policy but does not prove all actions. | **Partial.** Add table-driven native applicability/effect/replay vectors for all seven kinds, valid/invalid arity/type/span and retained rejection. Run them in WASM. Browser should only prove widget-key-to-typed-action dispatch and glyph identity for representative point, one-line, and two-line action paths. |
| Driving and reference dimensions; annotation and reload | Coordinator `action_matrix_dimensions_and_replay_are_deterministic`, `dimension_mode_transition_replays_and_undoes_without_stale_mutation`, and `accepted_measurements_withhold_stale_provenance` cover point-distance/segment-length creation, mode transition, replay and accepted provenance. | No executed parity evidence. | `e2e/m40.mjs` has no dimension action/assertion. `scene.rs` renders driving scalar or accepted reference value, but that presentation path has no focused browser check. | **Partial.** Add native vectors for both dimension families × driving/reference, mode edit, stale/foreign measurement withholding, reject/retention and replay. Run same corpus in WASM. Add browser checks for select/widget dispatch, mode/identity/value rendering and reload retention; expected values originate in corpus/editor DTOs, not JS formulas. |
| Constrained projected drag, preview/release/cancel (scorecard and UAT-C1-F2) | `projected_drag_retains_last_valid_preview_and_requires_matching_pointer`, `tool_switch_interrupts_drag_and_clears_only_an_existing_preview`, `foreign_pointer_down_interrupts_drag_without_an_old_release_commit`; coordinator preview provenance tests cover accepted/stale/foreign/rejected preview sessions. | No executed parity evidence. | `e2e/m40.mjs` currently has no pointer-move projected-drag/release/Escape assertion, despite `mod.rs::dispatch_effects` forwarding editor request/result and `render` selecting preview session. | **Partial.** Add native generated request/result ordering, threshold boundary, accepted/rejected last-valid preview, release-one-commit and cancel-no-history vectors, including horizontal projection through public retained sessions. Execute in WASM. Browser asserts pointer wiring and rendered `Solved preview`/accepted identity/cancel restoration, not projected coordinates calculated in JS. |
| Delete, undo, redo and reload retention | Coordinator `delete_selected_uses_domain_dependency_cleanup_and_undo_restores_ids`, `suppression_delete_and_selection_reconciliation_use_persistent_ids`, `stale_edit_is_history_and_selection_neutral_and_new_edit_truncates_redo`, and `reload_uses_checkpoint_bytes_without_reusing_revisions` cover lifecycle ownership. | No executed parity evidence. | `e2e/m40.mjs` checks undo/redo of one glyph and localStorage reload of point count. It does not prove delete, dependent cleanup, IDs, redo truncation, rejected-design retention, or revision high-water. | **Partial.** Add model/replay corpus for create/edit/delete/suppress/reject/repair, undo/redo/reload, IDs and revision non-reuse; execute natively and WASM. Browser checks only click/keyboard dispatch plus snapshot restore/rendered identity/lifecycle, including malformed storage retaining the live coordinator. |
| Accepted redundancy presentation | `RetainedEditorCoordinator` intentionally does **not** inspect `core_report`/`display_audit`; M40.4 completion notes say stable sketch domain DTO is unavailable. | No parity evidence. | No browser assertion; `e2e/m40.mjs` source scan forbids legacy report symbols, correctly preventing browser fabrication. | **Missing / blocked.** Add a sketch-owned provenance-bearing accepted diagnostic DTO before any editor/WASM/browser test. Then add native and WASM accepted-redundancy vectors and a browser rendering-only assertion. Do not satisfy this by scanning audit rows in Rust or JS. |
| Conflict rejection, Problems evidence, and retained accepted scene | Coordinator `rejected_dimension_is_retained_and_undo_restores_with_fresh_revisions`, `stale_identity_precedes_incompatible_selection_without_mutation`, and lifecycle/audit DTO implementation establish retained rejection and provenance. | No executed parity evidence. | `e2e/m40.mjs` does not submit a conflicting edit or assert rejected lifecycle/prior accepted scene. `mod.rs::problem_text` renders coordinator failure/rejection. | **Partial.** Add native/WASM conflict vectors asserting design/attempt advance, accepted bytes/identity unchanged, problem provenance and undo. Add one browser adapter test that renders those supplied DTOs and accepted scene after a rejected effect; no browser diagnostic classification. |
| Lifecycle clarity: Accepted, Design unsolved, Solved preview, Rejected attempt | `LifecycleDto`/`LifecycleStatus` and coordinator tests for preview provenance and rejected retained dimensions cover portions of all states. | No executed parity evidence. | Browser renders the five labels through `lifecycle_presentation`, but E2E does not assert lifecycle transitions. | **Partial.** Add exhaustive native lifecycle transition model including initial unsolved, accepted, solving, preview, rejection, cancellation and restore; compare same trace in WASM. Browser asserts label/data-state rendering for supplied editor states only. |
| Deterministic replay/model coverage for all core tools/actions | `ReplayAction`, `replay`, and targeted coordinator replay tests exist; source calls it “small replay vocabulary” and explicitly defers generated/model qualification. | No executed parity evidence. | Finding evidence serializes a debug transcript; no replay-from-artifact test. | **Partial.** Add seeded/generated bounded transition model and checked-in deterministic corpus covering all rows above, replayed from initial snapshots with final design/accepted/checkpoint/selection/lifecycle assertions; use the identical corpus in native and WASM. |
| Exact boundary, overlapping-hit, scale/viewport, cancellation, malformed-input, persistence, accepted-retention matrices | Individual native coverage exists for invalid viewport, 3 px threshold, snap tolerance, non-finite input, zero sweep, persistence/reload and rejected retention. | Locked WASM compile only. | E2E covers a normal fixed viewport, cancellation of a line preview, reload, download and source-policy scan. | **Partial.** Make each matrix dimension explicit in the shared corpus: threshold just below/equal/above; point/curve/curve ties; finite extreme viewports and model scales; pointer/tool cancellation; malformed events/storage; and accepted bytes after every failed transition. Add WASM execution and only adapter-boundary browser cases. |
| Thin adapter: platform wiring, rendered identity, accessibility, storage and downloadable evidence; no browser policy | `lib.rs` documents headless ownership; coordinator is the effect/lifecycle owner. | Locked WASM build is present, and Chromium E2E runs the built WASM application, but it is not a parity oracle. | `e2e/m40.mjs` source-policy checks removed modules/symbols and editor dependency; it checks readiness, persistent-ID rendering, selection, undo/redo, storage reload, checksummed JSON/SVG downloads and dev route. No accessibility-control equivalence or malformed-storage test. `mod.rs` still contains adapter key decoding, coordinate normalization, DTO rendering and localStorage/download mechanics. | **Partial.** Retain source-policy scan; add browser cases for normalized pointer/keyboard/widget equivalence, ARIA control dispatch, malformed storage error/retention, viewport letterboxing, and evidence identity/checksum. Keep all expected interaction outcomes generated by the shared editor corpus. |

The M40.7 scorecard’s exploratory usability judgment is deliberately absent from this
table. `docs/M40_UAT.md` now preserves the completed approval; `docs/SCENARIOS.md`
UAT-C1 says automation proves numerical facts and human review judges
discoverability/intent/state clarity.

### Decisions / inferred constraints

#### Matrix artifact and corpus shape

Create `docs/M40_QUALIFICATION_MATRIX.json` as the checked-in, machine-readable gate
manifest (documentation artifact; its test IDs point at code rather than duplicating
policy). Use a versioned object with this minimum shape:

```json
{
  "schema_version": 1,
  "milestone": "M40.6",
  "rows": [{
    "id": "m40.uat.drag.preview-release-cancel",
    "uat_sources": ["M40_UAT.md#Targeted-Recheck-UAT-C1-F2", "SCENARIOS.md#UAT-C1"],
    "requirements": ["native", "wasm-parity", "browser-adapter"],
    "native": {"tests": ["..."], "status": "partial"},
    "wasm": {"tests": [], "status": "missing"},
    "browser": {"tests": ["e2e/m40.mjs:..."], "status": "partial"},
    "gaps": ["..."],
    "policy_owner": "geosolve-constraint-editor"
  }]
}
```

Require a native test to parse this manifest and fail for a missing row, unknown test
ID, `partial`/`missing` status, absent required evidence channel, or a
`policy_owner` other than `geosolve-constraint-editor` for deterministic interaction
semantics. Store executable shared vectors separately at
`crates/geosolve-constraint-editor/tests/m40_transition_corpus.json`; each vector
needs `id`, initial design/checkpoint fixture, normalized inputs/actions, expected
effects, and final design/accepted identities, selection, lifecycle and retention
digest. Native and WASM harnesses must consume those bytes unchanged.

#### Historical implementation sequence (completed)

1. Add the corpus runner and manifest validator to the editor crate. Expand the
   existing targeted tests into table/model cases rather than adding web-only
   reimplementations.
2. Add a WASM-callable corpus runner/result encoder in the editor consumer path and a
   locked browser/WASM harness that compares its structured result byte-for-byte (or
   canonical JSON byte-for-byte) with native golden results. A compile-only wasm check
   is not parity evidence.
3. Extend `e2e/m40.mjs` only with adapter observations selected by the manifest:
   platform event/widget delivery, DTO identity/state rendering, storage/file/evidence
   behavior and accessibility. It must invoke the WASM corpus runner rather than
   compute picks, snaps, projections, applicability or lifecycle expectations in JS.
4. Obtain the missing sketch-owned accepted redundancy/conflict presentation DTO (or
   explicitly narrow M40 only with caller-approved acceptance/plan change). Wire it
   through coordinator DTOs before adding its matrix row’s tests.
5. Add a source-policy assertion for any newly introduced adapter module and run the
   manifest validator in the normal native and browser gate. Clear the unrelated
   legacy-playground Clippy warnings called out by `PLAN.md` before claiming closure.

#### Historical validation commands (retired)

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
(cd crates/geosolve-demo-web && trunk build --release)
(cd crates/geosolve-demo-web && node e2e/m40.mjs)
```

The new WASM parity harness must be included in the last command (or be exposed as an
equally explicit checked-in command); `cargo check` alone cannot satisfy the parity
row. The release M40.6 report must record the manifest’s all-covered result and the
exact browser/WASM command outcome without retries.

### Historical open questions (resolved or superseded)

- M40.4 explicitly leaves accepted redundancy unavailable pending a stable
  sketch-owned DTO. Is a provenance-bearing accepted diagnostic DTO in M40 scope, or
  must the acceptance/scorecard be amended before M40.6 can close?
- No allowed source establishes a current WASM test runner or a public WASM export for
  executing native transition vectors. Select and document the minimal harness without
  introducing a second browser policy path.
- The current adapter’s tree click directly calls `set_selection`, and widget key
  decoding selects enum actions. Confirm the ADR-0029 source-policy rule treats these
  as permitted event/widget translation and define forbidden future patterns
  concretely (geometry calculations, action applicability, report interpretation).
- Scope of “all core tools and actions” is larger than the present seven constraints
  and two dimension families if it includes every M37/M38 catalog action. The matrix
  must freeze the M40 core action set or name each additional supported action before
  evidence can be complete.

### Out of scope

- Implementing the corpus, WASM harness, DTOs, source-policy checks, or browser tests.
- Reopening M40.7 human usability review, treating exploratory authoring as an
  automated acceptance substitute, or moving solver/interaction policy into JS.
- Solver equations, sketch accepted-state authority, and non-browser host integrations.

## M40.7 preview-consistency second-pass investigation

### Requirements

- Trace every point, polyline, rectangle, circle, and counterclockwise-arc draft
  stage from normalized browser input through headless preview/effect and browser
  rendering to the atomic construction commit.
- A complete construction preview must be the same typed proposal that is committed;
  incomplete stages must be explicit, non-committable headless DTOs. Snapped operands
  must retain their persistent identities through both paths.
- Under ADR 0029, geometry/interaction decisions (normalization after platform
  measurement, snapping, draft validity, terminal completion, arc normalization and
  lifecycle) belong to `geosolve-constraint-editor`; browser code may render supplied
  DTOs and own presentation styling only.

### Evidence and source pointers

- `geosolve-constraint-editor/src/lib.rs`: normalized `PointerInput` enters
  `ConstraintEditor::{pointer_down,pointer_move}`. `draft_down`/`draft_move` convert
  screen coordinates through `EditorScene.viewport`, call headless `snap_point`, and
  emit `PreviewConstruction`; terminal `draft_down` emits
  `CommitConstruction { expected: scene.design_identity, proposal }` followed by
  `ClearConstructionPreview`. `complete_draft` is the polyline-only terminal route.
- `ConstructionPreview::{Complete,Anchor,ArcRadiusGuide}` makes incomplete circle and
  arc stages explicit; `ConstructionPreview::Complete(ConstructionProposal)` embeds
  the typed committable proposal. `draft_preview` obtains complete previews from the
  same `draft_proposal` helper used for terminal construction.
- `ConstructionProposal::apply` clones and replaces the document only after public
  construction allocation/validation succeeds; the proposal's documented host route
  is an atomic session transaction. `construction_branch_direction` rejects zero or
  non-finite line/polyline segments.
- `geosolve-demo-web/src/workbench/mod.rs`: `pointer_input` is the browser's
  client-to-fixed-SVG coordinate/letterbox adapter; listeners pass its `PointerInput`
  to editor transitions, `dispatch_effects` retains `ConstructionPreview` and calls
  coordinator application only for commit effects. `editor_scene` supplies an accepted
  document and the fixed workbench viewport for rendering/picking.
- `workbench/scene.rs::construction_markup` renders only the returned preview DTO;
  `construction_proposal_markup` does independently derive SVG arc flags from proposal
  endpoints. `styles.css` confines `.wb-draft*` rules to SVG appearance and disables
  preview pointer events. `e2e/m40.mjs` covers visible circle/arc/polyline stages and
  completion/cancellation browser routes, while the embedded qualification draft and
  snap matrices exercise headless proposal equality and snap boundaries.
- ADR 0029 §§Decision/Verification assigns drafting, snapping, interaction geometry
  and typed effects to the editor and limits browser responsibility to platform-event
  mapping, returned-state rendering and effect application.

#### End-to-end construction trace

All tools receive `mod.rs::pointer_input`'s finite fixed-SVG `ScreenPoint` and
modifiers. It letterboxes a DOM client event into 1000 × 700 screen coordinates;
`ConstraintEditor` then converts that screen point with the supplied `EditorScene`
viewport, snaps against `scene.points` at the editor's inclusive `SnapTolerance`, and
stores either `ConstructionPoint::Existing(id)` or `New([x, y])`. The browser does not
calculate either operand. `dispatch_effects` stores the returned preview and only sends
`CommitConstruction` to `RetainedEditorCoordinator::apply_editor_effect`; `scene.rs`
renders the stored DTO using the accepted document for `Existing` lookup. Atomic
`ConstructionProposal::apply` clones the design document, uses the public
`SketchDocument::{add_point,add_scalar,add_curve,add_rectangle}` APIs, and replaces it
only on success. `document.rs` confirms finite point/scalar/full-curve validation and
rectangle's own rollback; session application additionally decides accepted/rejected
lifecycle.

| Tool and stages | Headless preview / proposal | Browser rendering and commit |
| --- | --- | --- |
| Point: one down | `draft_proposal` immediately yields `Point`; no incomplete preview. | No preview is rendered; effect is applied atomically, then clear is harmless. |
| Line: first down; move/second down | First stage is retained but has no DTO. A second finite, nonzero endpoint yields `Complete(Line { start, end })`; terminal recomputes that proposal. | `construction_proposal_markup` draws the supplied two operands; terminal commit clears it. |
| Polyline: first/subsequent downs; Finish/double-click/Enter | Each down appends a nonzero segment. Complete proposals exist after two points, but only `complete_draft` emits `CommitConstruction(Polyline)`; it preserves the exact `Vec<ConstructionPoint>`. | Preview is rendered as the supplied point sequence; the three browser finish routes call the same editor method. |
| Rectangle: first down; move/second down | Second point requires both nonzero axes; `Rectangle { first, second }` is previewed/committed. `apply` canonicalizes min corner and absolute width/height. | SVG draws the proposal rectangle; it does not reproduce rectangle construction or constraints. |
| Circle: center down; move/second down | Center creates `Anchor`; a finite positive radius creates `Complete(Circle { center, radius })`, reused at terminal down. | Anchor/complete circle use only supplied center/radius. |
| Counterclockwise arc: center; start; end | Center creates `Anchor`; start creates `ArcRadiusGuide`; end must be nonzero from center and is radially normalized to start radius before `Complete(CounterClockwiseArc)`. The same helper recomputes terminal operands. Zero sweep is retained at the earlier stage. | Renderer uses supplied normalized endpoints, but independently derives SVG `large-arc`/sweep flags (finding below); apply allocates explicit `DocumentArcSweep::CounterClockwise`. |

- Direct editor tests: `every_core_draft_has_exact_completion_and_cancellation`,
  `preview_and_commit_share_identical_typed_operands`,
  `arc_preview_and_commit_share_the_editor_normalized_endpoint`,
  `arc_draft_publishes_anchor_radius_guide_and_normalized_completion_stages`,
  `degenerate_terminal_candidates_rollback_and_a_valid_retry_completes`, and
  `zero_sweep_arc_endpoint_rolls_back_to_center_and_start` cover the named normal and
  degenerate transitions. `qualification.rs::{draft_matrix,snap_matrix,malformed_matrix}`
  repeats terminal/cancel routes, inclusive snap boundary/identity tie and non-finite
  input evidence in the shared native/WASM corpus.
- Browser evidence: `e2e/m40.mjs` `browser.creation-routes` visibly exercises all
  six construction tools, circle/arc stage DTO rendering, polyline Finish/Enter/
  double-click and Escape/pointercancel; its source scan rejects editor-policy symbols
  in `mod.rs`/`scene.rs`. It does not assert preview-to-committed operand equality or
  a stale accepted/design identity case.

### Decisions / inferred constraints

- **Result:** normal construction drafts are genuinely headless through proposal
  creation: no browser code constructs, snaps, normalizes the arc endpoint, or chooses
  terminal operands. Complete-preview/terminal equality is explicit for circle and
  arc and follows from the shared pure `draft_proposal` path for line, rectangle and
  polyline. Point deliberately has no preview. This is not yet an unconditional
  preview/commit guarantee because the rendered accepted lookup and host application
  have different validity context from the editor scene.
- **Presentation-owned only:** SVG/HTML serialization, escaping, marker/dash/color/
  opacity/stroke width, selected visual state, ARIA/cursor treatment, and making
  `.wb-draft` pointer-transparent (`styles.css:303-336`) are presentation. SVG's
  syntax-specific y-axis conversion is presentation only if it is mechanically tied
  to the explicit headless counterclockwise sweep and cannot select a different arc.
  CSS hover stroke width/cursor is non-authoritative because canvas picking is routed
  to editor `EditorScene::hit_test`, not DOM targets.
- **Forbidden browser policy:** snap candidate/identity and tolerance; model
  coordinate conversion beyond platform-to-editor screen normalization; finite/
  degeneracy/zero-sweep eligibility; rectangle canonicalization; arc radius endpoint
  normalization, sweep/branch selection; draft stage/finish/cancel rules; accepted vs
  retained design choice; and solving/projection/lifecycle inference. A renderer must
  fail visibly or clear a stale DTO, not silently alter/drop its geometry.

#### Concrete divergences and interface leaks

| Severity | Evidence | Divergence / leak | Smallest cleanup |
| --- | --- | --- | --- |
| **High** | `EditorScene::from_accepted` holds an accepted document but labels it with current `design_identity`; `snap_point` accepts every `scene.points` ID; `ConstructionProposal::apply_to` rejects an `Existing(id)` absent from the design document. `mod.rs::editor_scene` deliberately builds this mixed scene from `projected_preview`/accepted geometry plus coordinator's current design identity. | After a rejected retained deletion or other design/accepted topology divergence, a preview can snap to and draw an old accepted point, yet the same `Existing(id)` proposal fails atomic apply against current design. This violates snapped-identity preview/commit consistency rather than accepting bad geometry. | Add an editor-owned construction scene/DTO whose snap candidates are intersected with current design identities (or carry separate accepted/design identities and reject construction input when they differ); render that DTO. Add the retained-delete/rejected-design regression. |
| **Medium** | `scene.rs::construction_proposal_markup` uses `filter_map(operand)` for a polyline and silently omits missing line/circle/arc operands; no typed stale-preview result exists. | The browser can render a shortened/no preview rather than the DTO that will be committed or rejected. This hides the high-severity identity mismatch instead of preserving provenance. | Make renderer consume validated editor preview primitives, or return/display a typed unrenderable/stale-preview notice; never filter/drop operands. |
| **Medium** | `scene.rs:219-240` independently computes `atan2`, `rem_euclid`, SVG large-arc flag and sweep flag for an arc, whereas `lib.rs::draft_proposal`/`apply_to` owns radial normalization and explicit `DocumentArcSweep::CounterClockwise`. The E2E source-policy scan forbids `atan2` only in `mod.rs`, not `scene.rs`. | A presentation module reconstructs branch/sweep interpretation. It presently appears mathematically aligned for normal arcs, but has no assertion that its SVG path is the same directed/sized arc as the proposal/public curve, so an SVG convention change can make preview lie. | Put arc display primitives/path flags in the editor DTO (or a small shared presentation-neutral arc tessellation helper) and have `scene.rs` serialize them; add minor/major and direction regression vectors. |
| **Medium** | `mod.rs::pointer_input` hard-codes 1000 × 700 letterboxing while `scene.rs::viewport` independently hard-codes `[1000,700]`, center and scale. | Browser normalization and editor scene coordinates are a split protocol; changing one side shifts input/picking/drafts while rendering uses the other. This is interaction geometry divided across module interfaces. | Export/retain one editor-provided viewport/canvas contract and make browser measurement map to it; preserve only DOM rect/device measurement in `mod.rs`. |
| **Medium (non-construction preview)** | `mod.rs::dispatch_effects` builds a candidate retained session, calls `without_previous_state_preferences().with_drag`, chooses accepted position, invokes `mark_solved_preview`, and stores `Workbench::projected_preview`. | Browser host code owns projected-preview solve request preferences and acceptance/lifecycle sequencing, contrary to ADR 0029's thin adapter intent. Construction previews are headless, but the broader “every preview” claim is false. | Move request construction/result acceptance/transient lifecycle transition behind one coordinator method/effect handler; `mod.rs` only forwards effect/result and renders supplied preview state. |
| **Low** | `mod.rs::render` independently shows the polyline guide only for `tool == Polyline && construction_preview.is_some()`. The editor has no explicit “polyline draft active” DTO. | First polyline point is retained headless draft state but has no rendered guide; this is UI visibility policy coupled to a preview presence proxy, not a geometry mismatch. | Add an editor draft-stage/presentation DTO and render its guide state; keep wording/layout in browser. |

#### Testable consistency invariants

1. For every complete preview, terminal input at the same normalized sample produces
   exactly one `CommitConstruction` with the same `expected` design identity and
   byte/equality-identical `ConstructionProposal`; then exactly one clear. For Point,
   there is exactly one commit/no prior preview. For polyline, Finish/Enter/
   double-click call the same complete proposal and no pointer-down commits it.
2. Each existing operand in a preview/proposal is present in the exact current design
   addressed by `expected` and retains the accepted visible position used at snap time.
   Apply validates the persistent ID against the retained design and uses that accepted
   position snapshot for branch/scalar construction seeds. Snap inside/on/outside
   tolerance is respectively reused/reused/new, and equal-distance ties select the
   lowest persistent identity in both preview and commit.
3. Complete line/polyline segments are finite/nonzero; rectangle has finite nonzero
   axes; circle radius is finite/positive; arc center→start/end radii are finite/
   positive and its normalized endpoint has start radius and nonzero CCW sweep.
   Invalid terminal samples emit no commit, retain the last valid draft/preview stage,
   and a subsequent valid sample can commit once. Non-finite input leaves draft,
   preview and retained identities unchanged; cancel/tool switch clears transient
   preview and makes later release/finish a no-op.
4. Rendering is a total faithful projection of a valid preview DTO: it either displays
   every operand/proposal with the declared snap IDs and explicit CCW sweep or reports
   the DTO stale/unrenderable; it never `filter_map`s, substitutes a point, recomputes
   a different endpoint, or silently changes minor/major arc choice.
5. Apply failure (stale expected identity, missing existing operand, document
   validation, solve rejection) publishes no accepted construction. It clears or
   retains preview only according to a typed editor result, never leaves a visual
   preview claiming acceptance. Rejected retained design continues to render the old
   accepted scene with distinct provenance and must not supply construction snap IDs.

### Open questions

- Does the coordinator already have an uninspected public operation that can provide
  design-valid construction snap candidates and resolve projected preview effects? If
  not, M40.7 needs the minimal coordinator/editor API addition described above.
- Confirm the intended contract after a failed `CommitConstruction`: does coordinator
  retain a complete construction preview for correction, clear it, or return typed
  failure state? Current allowed evidence establishes atomic document application but
  not a browser-visible construction-failure transition.
- Freeze whether a headless arc preview primitive should be tessellated points or an
  SVG-independent directed-arc descriptor, and test SVG's y-axis/sweep convention
  against it rather than duplicating the formula in web code.

### Out of scope

- Changing editor, sketch, browser, CSS, E2E, session, or solver code.
- Non-construction selection, dimension, diagnostic, persistence, and general M40.7
  usability review except where they directly affect construction preview provenance.

### Historical scoped M40.7 recommendation (implemented)

**Yes—scoped cleanup is warranted.** The normal tool path is sound, but accepted/design
topology divergence can make a snapped construction preview uncommittable, and the web
adapter still owns arc and projected-preview policy. Restrict the cleanup to:

1. `crates/geosolve-constraint-editor/src/lib.rs` (construction-scene/snap and preview
   provenance DTO/invariants) and its qualification coverage in
   `src/qualification.rs` plus `tests/m40_transition_corpus.json`/golden only as
   required by the existing corpus gate;
2. `crates/geosolve-demo-web/src/workbench/mod.rs` (consume the viewport and
   projection-preview boundary, no policy reconstruction), `scene.rs` (serialize
   supplied preview primitives without operand filtering/arc math), and
   `e2e/m40.mjs` (adapter rendering/provenance regressions);
3. `crates/geosolve-demo-web/styles.css` only if a new stale/unrenderable preview
   marker needs visual styling.

Do not change sketch geometry APIs or solver behavior for this cleanup. The smallest
first patch is the design-valid snap candidate/provenance boundary plus a retained
rejected-delete test; the arc/viewport/projection seams can then be removed without
expanding the primitive set.

### Second-pass implementation and qualification result (2026-07-26)

The scoped cleanup is implemented without changing sketch equations or solver policy:

- `ConstructionPoint::Existing` retains the persistent point ID and exact accepted
  model position used by the preview. Construction apply validates the ID but derives
  line/polyline branch directions and arc radius/angle seeds from the retained snapshot.
  `snapped_operand_snapshot_keeps_preview_and_commit_branch_identical` covers accepted
  versus retained-design coordinate divergence after a rejected edit.
- `EditorScene::from_accepted_for_design` continues to render/pick accepted geometry but
  exposes only identities still present in retained design as construction snap points;
  the removed-ID regression remains passing.
- `scene.rs` serializes supplied `ConstructionPreviewGeometry` only. Its native adapter
  test covers explicit minor and major counterclockwise SVG arc flags; source policy
  still forbids proposal/arc reconstruction in the renderer.
- `effect_adapter.rs` owns terminal preview lifecycle at the browser dispatch boundary:
  a failed construction commit suppresses its following clear, while a successful
  commit clears. Both paths have direct regressions.

Qualification passes: `cargo fmt --all -- --check`, `git diff --check`, locked
warnings-denied workspace Clippy, full locked workspace tests, locked
`wasm32-unknown-unknown` check, supported release Trunk build, and release
`e2e/m40.mjs` 14/14. Cargo's existing duplicate `license`/`license-file` warnings are
unchanged. Mechanical remediation is complete; the supervising human subsequently
passed the targeted UAT-C1-F5 recheck and explicitly approved M40.7 on 2026-07-26.
