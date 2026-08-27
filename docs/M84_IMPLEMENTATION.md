<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 implementation ledger — Optional code/GUI sketch authoring

Status: **accepted at milestone level on 2026-08-27; exact clean-qualified immutable M84-F012 is
the accepted product candidate; public publication and final closure remain pending**. Exact F011
source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, and snapshot
`/tmp/geosolve-m84-f011-uat.ps736NLh` are historical rollback evidence. Exact F012 source
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, and snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM` are current mechanical nomination authority; no UAT acceptance
is claimed. F010 source `cf463838`,
tree `992e587`, and snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz` are historical rollback
evidence. F009 source `c74651c`, tree
`a904584`, and snapshot `/tmp/geosolve-m84-f009-uat.q8cKIN3v` are withdrawn historical defect
evidence; F007 source `cc2f05e` and its immutable snapshot are likewise historical.
Direct-authoring source
`41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, is withdrawn historical evidence with the initial,
F003 and F004 candidates. Combined F005/F006 source `ff2e142` and its frozen candidate are also
withdrawn by F007. On 2026-08-27 the supervising user accepted M84-U1 through U16 at milestone
level and requested closeout; this does not claim a separately logged row-by-row hands-on replay.
Pages publication and closure remain pending. No M84 Pages publication is claimed, and accepted
M83 remains public authority.

## Baseline and authority

- Product baseline: accepted M83 qualified source `ee18dbd`, approval descendant `2006c86` and
  exact Pages run `32817232564`.
- Controlling architecture: ADR 0041 and `docs/M84_GOALS.md`.
- Numerical authority remains the existing Rust materializer/solver plus independent validation.
- M84 adds structural authoring only. F011 exposes more existing native primitive/relation/
  dimension families through the optional managed adapter, but changes no native equation,
  residual, priority, tolerance, branch rule or JavaScript solve path.

## Files and public seams

- `crates/geosolve-sketch-code/` is the optional pure-Rust code-project layer. It owns bounded
  managed-v1 parsing, authenticated edits, artifact admission, declaration-family execution,
  typed semantic expansion, keyed reconciliation, overrides, composite history, bootstrap and the
  nine bundled projects. The historical F009/F010 catalog contained eight projects; F011 adds the
  PC Water Manifold.
- `CodeProject::managed_only(ProjectKey, source)` is its smallest code-only host seam. It accepts
  only artifact-free managed source and the SDK import, validates the complete envelope, and
  publishes no native geometry by parsing alone.
- `packages/geosolve-sketch-code/` is the optional TypeScript authoring/build package. It owns
  project-branded `FeatureRef`/`OutputRef`, descriptor-generated result types, `definePatch`,
  `p.each`, `p.mapRecord`, caller-side artifact compilation and positive/negative type fixtures.
- `geosolve-sketch-intent::IntentSession::{delegated_checkpoint,
  to_delegated_checkpoint_json,from_delegated_checkpoint_json}` is the neutral history-free seam
  used by a composite host. Unordered atomic patches apply retained rebinds before validating
  deletions, so member replacement is independent of patch-array order.
- `geosolve-demo-web::workbench::code_projects` composes the optional module through public intent,
  coordinator, ownership and scene APIs. Persistence carries either the unchanged plain
  workspace-v8 authority or a bounded authenticated code-project envelope.
- `geosolve-demo-web::workbench::png_export` clones the authoritative composed SVG, removes
  hit/provisional content, applies self-contained presentation styling and rasterizes through a
  browser canvas to a fixed 2000 × 1400 PNG download. It has no document, code-session, history or
  accepted-scene write path.
- `scripts/verify-geosolve-sketch-code-package.sh` packages the real normalized Rust crate, checks
  every crate-owned runtime asset, extracts it and performs a locked offline build with local
  patches for GeoSolve crates that are not yet on crates.io. `scripts/release-gate.sh` runs this
  verifier and both TypeScript packages.

No core, geometry, sketch, linkage, intent or constraint-editor manifest depends on
`geosolve-sketch-code`; only the optional demo composition does.

## Implemented slices

### I1 — bounded managed source

- [x] Parse the exact `"use geosolve managed-v1";` lossless subset in pure Rust under the 4 MiB
  source bound.
- [x] Preserve comments, formatting and all unowned bytes while rewriting authenticated direct,
  aggregate, organization, declared-lens and override value spans.
- [x] Keep unsupported/syntax-invalid text as a non-authoritative draft; retain valid failed code
  intent above the last complete accepted scene.
- [x] Bootstrap a complete supported accepted GUI dependency closure into truthful, dependency-
  ordered managed declarations; reject unsupported recipes instead of inventing lineage. Omit only
  the exact canonical fresh-workspace document foundation and never hide other bootstrap geometry.
- [x] Represent connected Segment endpoints and direct computed Fillet parents with lexical
  declaration members (`line.end`, `line.span`) rather than native IDs or transport DTOs.
- [x] Represent ordinary Horizontal and Vertical span relations as lexical
  `$.constraint.horizontal`/`$.constraint.vertical` declarations, preserve suppression and lower
  them to the existing native constraint kinds.
- [x] Keep one checked-in managed-v1 line/Horizontal/Vertical/line/Fillet source as both a
  TypeScript compile target and the Rust parser/cold-materialization fixture.

### I2 — custom artifacts and typed SDK

- [x] Compile custom TypeScript only in an explicit caller-owned Node step into canonical,
  data-only, digest/interface/ABI-pinned artifacts.
- [x] Admit only the central executable declaration-family catalog, bounded template DAGs,
  schemas, bindings, lenses and keyed collections. All nested structural values count toward the
  65,536-item bound; individual artifacts remain capped at 16 MiB.
- [x] Generate named rectangle results, mapped Fillet records and Polyline-derived corner
  collections from Rust declaration descriptors. Raw IDs, cross-project references, misspelled
  outputs and point/curve/corner mismatch fail TypeScript compilation.
- [x] Publish direct Polyline `vertices` and `segments` as exact keyed root collections. Vertex
  keys map to native Point ports and each directed span's starting key maps to its native CurveSpan
  port; recorded artifact `each`/mapping rules consume those roots without coordinate copies,
  native IDs or ordinal identity.
- [x] Describe direct `computed.filletSet` as an opaque `FilletSetFeature`: its explicit parent
  spans are typed, but it does not falsely expose evaluated child arcs as ordinary native ports.
- [x] Generate `NativeCurveSpanRef` from the central Rust declaration-result catalog only for
  direct line spans, rectangle edges and Polyline segments. Computed Fillet arc outputs remain
  ordinary non-native curve-span references and are not assignable to a direct Fillet parent.
- [x] Keep `patches/*.patch.ts` byte-identical through every GUI edit and never execute them in
  Rust, WASM, browser runtime or load.

### I3 — keyed reconciliation and native composition

- [x] Reconcile by invocation/template/member-key/output path. Reorder is non-semantic; insertion
  advances high-water; removal tombstones; reused retired keys receive a new generation.
- [x] Preserve unchanged logical nodes, ports, reservations, native geometry, computed Fillet
  ownership, overrides and ordinary outside GUI dependents across warm Apply, reload, Undo/Redo and
  generated-point edits.
- [x] Reject generated-output deletion when an outside dependent would dangle; never silently
  cascade or retarget it.
- [x] Lower direct Polyline/rectangle declarations and composed Fillets through the ordinary M83
  graph, cold materializer, native solver, computed-feature evaluator and independent validation.
- [x] Lower direct managed `computed.filletSet` declarations back to the existing Intent
  `ComputedFeature::FilletSet` with exact persisted contact/branch state, without invoking native
  Fillet authoring heuristics or changing any solver equation.
- [x] Authenticate every direct parent as an Intent-backed native span during Rust lowering and
  reject computed host outputs even if a caller bypasses TypeScript branding. Accept radius only
  as a positive finite model-unit number or branded millimetre literal `mm(...)`.

### I4 — one code/editor transaction

- [x] `SketchCodeSession` owns project/files/artifacts/lock/program/expansion/reconcile state,
  current/accepted semantic overlays, current/accepted editor checkpoints and one outer Undo/Redo
  history.
- [x] Every persisted current, accepted, Undo and Redo nested checkpoint is host-validated before
  construction. Hostile session IDs cannot poison allocator high-water.
- [x] Pointer frames use the existing retained native preview without parsing, expansion,
  serialization or durable-panel rebuild. Terminal publication commits the authenticated newest
  accepted preview once, after complete cold/native parity.
- [x] Managed rectangle corner drags reverse-write `lowerLeft`/`upperRight`; generated Polyline
  point drags become generation-bound explicit overrides; Reset removes only that override.
- [x] M84-F005 adds one bounded persistent semantic interaction overlay to this drag ownership.
  Its key is project plus semantic owner/output/field and never-reused owner generation;
  no intent/native alias enters the persisted wire. M84 exposes only finite Cartesian point drafts
  in this overlay; scalar edits remain managed-source lens edits. Point-seed precedence is typed
  overlay draft > legacy generated override > managed source seed, while Reset removes the complete
  semantic edit bundle and restores the applicable lower tier. Equal duplicate terminal writes
  collapse deterministically; unequal same-tier writes reject atomically. The overlay never alters
  constraint priority or solver equations.
- [x] M84-F005 routes selection, drag and deletion through accepted expansion provenance. A
  uniquely selected referenced consumer detaches only that consumer; no semantic preference or a
  selected producer retains ordinary attached ownership. Consumer detachment may replace that
  Segment's intent/native identity while preserving its code-owner generation and rebinding
  retained code-owned and ordinary GUI dependents. Repeated drags use the detached lens; Undo/Redo
  restores the complete attachment/overlay/editor authority. A rectangle corner writes the two
  coupled canonical seeds atomically. Unknown, stale-generation, non-finite and wrong-type drafts
  fail closed, and multiple matching selected lenses reject without mutation. Semantic deletion
  authenticates the exact session, accepted alias and managed/generated semantic owner before it
  rewrites the exact code-owned dependency closure or suppresses one generated child; a dirty
  managed draft, retained code failure, stale token or GUI-owned selection cannot select that
  semantic route.
- [x] M84-F006 hardens the completed authority boundary after adversarial review: imported session
  identities have an allocator-safe ceiling; persisted managed drafts retain the 4 MiB source
  bound; Reset/Restore use typed canonical tokens; retained failure prunes exact direct/generated
  owners; generated detachment carries exact provenance; direct and generated Segment replacements
  plus generated circle-centre replacements rebind their dependents; transient detachment cancels
  exactly; repeated generated drags survive exact Undo/Redo; and same-tier conflicts compare
  persisted IEEE bits, including `+0.0` versus `-0.0`.
- [x] M84-F007 authenticates one exact semantic point lens at pointer-down. No selection chooses a
  unique producer; selected producer stays attached; selected consumer retains local detachment.
  That lens alone authorizes terminal publication and cannot authorize an unrelated sibling edit.
  The pending route retains its exact `CodeSessionIdentity`, pointer and lens; only its dedicated
  authenticated terminal publisher consumes it. Generic saves, foreign/reentrant preparation and
  foreign terminals reject without consuming or replacing the original route. Non-pointer durable
  code actions invalidate it. Every durable Outline, Inspector, source, code, history and Apply
  route retires internal capture before deriving or mutating authority, even without a live
  viewport element; platform capture release is best-effort. A defensive generic-save rejection
  preserves an unexpected live editor/token rather than restoring underneath it. No-motion
  release/cancellation is history-neutral, and exact stored-session mismatch plus Apply/Undo cannot
  let a stale terminal revert newer accepted authority. Ordinary GUI-owned points remain delegated
  to the unchanged editor route.
- [x] M84-F010 retains F007's single-lens authentication while persisting the complete native
  solver-coupled semantic point closure at release. Terminal classification compares the exact
  gesture origin to the accepted terminal checkpoint; referenced-consumer routes preserve their
  exact post-detachment origin. One atomic overlay stages every moved semantic Cartesian seed, then
  cold rematerialization must match terminal design/current-accepted documents, computed features,
  ownership and allocators. Ordinary point aliases stay bit-exact. Rectangle corners are four
  redundant lenses over two seeds: the authenticated corner and diagonal opposite are exact
  anchors, and only the adjacent aliases may normalize under tightly bounded finite roundoff.
  Signed-zero, non-finite and material conflicts still reject.

### I5 — workbench, persistence and demonstrations

- [x] Add managed/custom tabs, Apply/Revert, diagnostics, artifact state, source ownership,
  generated-member groups, edit lenses, override badges and Reset-to-code controls.
- [x] Keep Code discoverable even when ordinary all-or-nothing conversion rejects: show escaped
  read-only diagnostic source alongside the Intent IR fallback and withhold Promote.
- [x] Round-trip complete offline code-project authority/history through save/reload and Copy/Load
  repro under the 64 MiB project bound. Missing, tampered, corrupt and oversized inputs reject
  atomically. M84-F005 carries separate current/accepted overlays and deterministically prunes
  attempted owners removed by a retained structural/native failure without advancing the accepted
  overlay or canvas.
- [x] Ship eight genuine sessions: adaptive rounded Polyline; typed panel/keyed Fillets; GUI↔code
  braced frame; reusable mounting plate; adaptive Lantern Garland; typed Suspension Bridge;
  generated Compass Rose; and artifact-free Neon Manifold.
- [x] On only the canonical, current-and-accepted empty workspace, show a dedicated Code landing
  with one **Start from code** action and eight centrally sourced project cards. Starting installs
  an artifact-free `Authored` project through the shared cold-validated project path, focuses the
  editable source, and never manufactures a GUI scene or promotion entry.
- [x] Persist `CodeProjectOrigin::Authored` separately from bundled/promoted origins; reject a
  conflicting legacy demo identity. Preserve valid and retained-invalid source, accepted native
  checkpoint, whole-source replacement and exact outer Undo/Redo across reload.
- [x] Prove an ordinary GUI reference CurveLength dimension on
  `brace.diagonals.rising` retains its native identity, follows a managed frame rewrite, updates
  its measured value, remains GUI-editable and shares exact outer Undo/Redo.

### Creative catalog amendment and M84-F008/F009

- [x] Add `adaptive-lanterns`, `bridge-cables` and `compass-core` as trusted TypeScript patches plus
  byte-exact canonical Rust/runtime artifacts. Keep Neon Manifold artifact-free managed-v1 source
  with an explicit two-corner direct FilletSet.
- [x] Require exact eight-demo native inventories and cold-materialize every project with finite
  points/scalars, all active computed features current and independently validated Hard residual
  at most `1e-9`.
- [x] Prove representative semantic UX edits: a Bridge tower peak keeps cables/stay attached; a
  Compass spoke keeps its ring/marker attached; a Lantern vertex moves its keyed bulb while five
  Fillets remain current; and a Neon shared endpoint moves its connected consumer while the
  two-corner FilletSet recomputes.
- [x] Expand the separate M84 code-project ledger from four to eight reviewed rows, SHA-256
  `bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`. Preserve the milestone-
  neutral 271-row golden byte-for-byte at
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
- [x] Resolve M84-F008: install code projects with a camera fit over the accepted composed scene,
  falling back to Origin only for empty/unavailable authority; increase the Rounded Polyline
  sample radius from `mm(0.4)` to `mm(4)`. Native adapter tests require the off-origin Mounting
  Plate inside viewport margins and exactly four finite radius-4 Fillet paths visibly clear of
  point markers. No solver mathematics changed.
- [x] Resolve M84-F009 after independent optional-layer review. Multi-output patch-template
  shorthand previously inherited whichever output sorted first, so Mounting Plate `plate.profile`
  resolved to the `ne` Point. Record exact `result_output` provenance in each compiled data-only
  template, publish it independently of renamed/nested paths, preserve nested roots and apply the
  same selection to dynamic collection members. Remove arbitrary prefix inference and fail closed
  when collection result provenance is absent. Exact alias/nesting/mapping regressions and the
  all-eight audit require declared reference and expanded target kinds to equal the reviewed
  catalog. No native geometry or solver behavior changed.

### M84-F011 — PC water-manifold dogfood amendment

- [x] Extend the optional managed declaration catalog, parser/lowering and generated TypeScript
  surface with direct circles, Coincident, FixedPoint, FixedCoordinate, CurveLength and Diameter;
  publish keyed Polyline members through `.byKey` and preserve explicit line roles
  `"profile" | "construction"`. Every declaration lowers to an existing Intent/native family and
  no solver equation or residual changes.
- [x] Add the ninth genuine code project from
  `assets/demos/pc-water-manifold.sketch.ts`, with the byte-matched managed fixture and the
  caller-compiled `water-channel.patch.ts`/pinned data-only artifact in both Rust and TypeScript
  package surfaces. The custom patch uses keyed `p.each` expansion so each current open-channel
  corner owns one adaptive native Fillet. Register it as the ninth centrally sourced code landing
  card without changing the separate **Start from code** path.
- [x] Model a 240 × 120 mm acrylic plate, 60 × 84 mm reservoir bay, three open channel Polylines,
  three closed O-ring loops, eight solved radius-2.5 screw circles with driving diameter 5 mm and
  21 construction datum spans. Exactly one FixedPoint anchors the design and no FixedCoordinate is
  present. The final sample inventory has 138 managed declarations, 82 generated members and 20
  typed outputs; its native result contains 53 points, 58 curves, 58 constraints, six host
  outputs/features and 15 computed edges.
- [x] Keep only the six useful channel-corner Fillets adaptive and Current. The three simple closed
  O-ring loops intentionally remain unfilleted direct geometry so ordinary replay stays practical;
  this changes neither their constraint semantics nor the custom patch contract.
- [x] Add command-bar **Export PNG** through the isolated presentation module. Export clones the
  composed SVG, strips hit/provisional geometry, supplies self-contained dark styling and uses
  SVG-to-canvas rasterization for a fixed 2000 × 1400 PNG without touching sketch or history
  authority.
- [x] Focused native authority assertions require finite accepted geometry, all active features
  Current, independently validated normalized Hard residual `<= 1e-9`, zero numerical/equality/
  bidirectional DOF, one FixedPoint, zero FixedCoordinate, exact route/seal/datum/circle inventories
  and six one-output adaptive Fillets. The expanded nine-row ledger is reviewed separately from the
  unchanged milestone-neutral golden.
- [x] Commit and run the complete clean release qualification; freeze the exact no-rebuild output;
  verify the ninth project plus PNG signature and 2000 × 1400 IHDR in a real browser; then replace
  the retained F010 service only after temporary exact-byte/browser proof. M84-U15/U16, all other
  refreshed UAT, Pages publication and milestone closure remain pending.

### M84-F012 — annotation visibility display/pick parity

- [x] Add one default-checked **Annotations** option to the shared workbench display surface and
  retain it only in each live flat/code workbench session. Apply the value to every composed scene
  without adding it to document, code-session, history, persistence, Intent IR or repro authority.
- [x] Add a default-true `EditorScene` presentation flag and make annotation SVG composition plus
  direct/contextual annotation hit tests consume it. Hidden selected/problem annotations publish
  no hit target; display-option changes clear stale hover so underlying geometry/datums can own the
  next pointer move/down. Preserve derived layout and selection, and keep Fillet affordance picking
  independent of annotation visibility.
- [x] Preserve WYSIWYG PNG behavior by cloning the current composed SVG: annotations are exported
  only when visible. Extend export-only cleanup to draft/inference guides/candidates and existing
  provisional/hit/error paint without mutating the live canvas or durable authority.
- [x] Run focused headless paint/pick, flat/code adapter and PNG-cleanup qualification; commit the
  final implementation; pass the complete clean gate; freeze exact no-rebuild output; verify both
  workbenches and both annotation states in a real browser; then replace retained F011 only after
  temporary exact-byte/browser proof. Do not accept U16 or any other UAT row mechanically.

The semantic-overlay addition intentionally changes the still-unreleased optional persistence
contract. Code sessions now identify as `geosolve-sketch-code-session-v2`; composed workbench
payloads identify as `geosolve-code-workbench-v2`. Prototype-v1 M84 payloads reject rather than
being silently reinterpreted under changed identity-digest semantics. Plain M83 workspace-v8
persistence remains byte/behavior compatible and does not acquire an optional-code dependency.

## Findings

### M84-F001 — generated terminal checkpoint carried stale branch metadata

Reproduction owner: retained code-workbench terminal publication. A generated-point drag could
produce complete valid accepted native geometry, but its reconstructed cold checkpoint retained
schema-derived Segment branch metadata from before the warm override. Strict rehydration then
rejected the valid release and the point snapped back.

Repair: after complete accepted-native parity, terminal publication installs the already staged
warm checkpoint as one outer action. The final explicit branch authority, evaluated computed
geometry, ownership and allocator state must match; only recomputable non-branch Fillet pick seeds
and evaluation revision stamps may differ. Focused generated-point, override/reset, rejected-final-
sample and Undo/Redo regressions pass.

### M84-F002 — aggregate reverse edit used the wrong authentication class

Reproduction owner: managed rectangle drag publication. Planning found the value-owned span for
`lowerLeft`/`upperRight`, but generic application authenticated it only as a top-level declaration
span and returned `managed rewrite does not target an authenticated owned span`.

Repair: `ManagedEditPlan` retains its semantic value target and applies through
`rewrite_managed_value`. Exact stale-source compare-and-swap remains mandatory. Sequential edits
reparse and reauthenticate fresh CST spans; comments, custom source and unrelated managed bytes
remain exact.

### M84-F003 — transport-shaped Structured Source was not lexical authoring code

Reproduction owner: optional GUI-to-code projection. Draw an aligned rectangle, then a Segment
between two rectangle corners. The ordinary Structured Source route serialized
`IntentProjectedPortReference { declaration, output, kind }` into a TypeScript-shaped object. That
is valid bounded transport IR, but it is not a variable-driven reference coupled to the rectangle's
inferred result type. The bundled Braced Frame began as managed code and therefore did not cover
ordinary GUI conversion.

Repair: retain the original DTO as labelled Intent IR, add direct managed
`$.geometry.line`, dependency-order supported GUI bootstrap declarations, and emit endpoint
expressions such as `frame.corners.lowerLeft`. Direct expansion must alias the exact owning native
point rather than copy its coordinates. Ordinary Code preview/promotion must create one genuine
persisted code project/session. Raw strings, transport DTOs, foreign-project and forged reserved-
project references, wrong kinds and misspelled members fail closed. Only the exact canonical fresh-
workspace document foundation is ignored; other bootstrap geometry makes conversion reject
atomically. Same-cell branch normalization is limited to current code-expansion-owned Segments so
ordinary GUI Segments retain explicit branch authority. Focused Rust, TypeScript, workbench/
persistence and browser tests plus the complete replacement qualification and nomination pass.

### M84-F004 — an ordinary computed Fillet made Code undiscoverable

Reproduction owner: ordinary GUI managed projection plus the workbench Code surface. Draw two
connected Segments and place one Fillet between them. Projection was all-or-nothing, but GUI
bootstrap could neither express the second Segment's endpoint as a lexical reference to the first
nor lower `ComputedFeature::FilletSet`. The resulting conversion error caused presentation to omit
the Code tab entirely, so the user saw neither code nor an explanation. After that direct Fillet
slice was implemented, browser replay of the exact mouse-authored path exposed the remaining
closure gap: ordinary drafting had also created Horizontal and Vertical constraints, and bootstrap
rejected those declarations before it could present the otherwise-supported Fillet source.

Repair contract: add Segment-to-Segment lexical endpoint projection and a distinct direct
`$.computed.filletSet` declaration. Each corner carries exactly two ordered lexical
`NativeCurveSpanRef` parents plus explicit parameter, winding, neighborhood, normal-side,
retained-endpoint and periodic-anchor state; endpoint order, sweep and suppression are also
explicit. The descriptor generator grants the native-span brand only to direct line spans,
rectangle edges and Polyline segments. Computed host Fillet arcs remain unbranded; Rust lowering
also rejects such host outputs as parents so branding cannot be bypassed. Radius accepts only a
positive finite model-unit number or branded `mm(...)`, not forged unit records or another length
unit. Direct lowering reconstructs the existing Intent computed feature and must not rerun Fillet
authoring or change solver behavior. The declaration returns an opaque `FilletSetFeature`.
Ordinary projection remains intentionally all-or-nothing. Code is now always discoverable:
supported scenes offer a read-only managed preview and Promote, while unsupported scenes show an
escaped read-only conversion diagnostic with Intent IR still available and no Promote action.
The complete ordinary path additionally emits existing inferred axis relations as
`$.constraint.horizontal(... { curve: line.span })` and
`$.constraint.vertical(... { curve: line2.span })`, preserves suppression, and lowers them to the
existing Intent Horizontal/Vertical kinds. It does not add or reinterpret a constraint equation.
Bootstrap authenticates `accepted.validation.semantic` against the exact current retained intent
before reading declarations. A retained-failed rebind therefore presents Code as unavailable with
no Promote action instead of combining prior accepted geometry with current unaccepted wiring.

Focused fixture: `packages/geosolve-sketch-code/test/managed/line-fillet.managed.ts` is the same
managed-v1 two-line/two-axis-constraint/one-Fillet source compiled by the TypeScript suite, parsed
by Rust and cold-materialized through the ordinary intent/editor authority. It is the focused
fixture exercised within the complete release-candidate qualification below.

Focused Rust GUI-bootstrap/owner, direct-lowering, descriptor-parity, workbench, TypeScript type-
contract and Code-surface diagnostic coverage passes. Exact source `c2cf160`, tree `94a1786`, also
passes the complete clean release gate. Its immutable no-rebuild snapshot
`/tmp/geosolve-m84-f004-hv-uat.FF5RFBZe` was nominated on retained Tailscale after exact served-byte
verification and baseline 4/4, F003 1/1 and F004 2/2 browser suites passed on both temporary and
retained endpoints. The later direct-authoring amendment withdraws those bytes from current
nomination; this remains historical mechanical evidence, not human UAT acceptance.

### M84-F005 — collaborative semantic interaction authority

Reproduction owner: code-enabled canvas manipulation and deletion after a code project has been
cold-materialized. Earlier direct and generated reverse edits handled isolated demonstrations, but
they did not provide one durable semantic authority for literal points, rectangle-corner coupling,
shared references, generated owners, Reset, deletion and retained failure. Selection could not
truthfully distinguish “move the producer and keep consumers attached” from “detach only this
selected consumer,” and implementation aliases were not a safe ownership language.

Repair: `CodeInteractionOverlay` is a bounded, generation-authenticated, point-only semantic seed
layer. Its key contains project plus direct/generated owner, semantic output and writable field;
it contains no intent/native ID. Expansion applies exact precedence
`semantic overlay > legacy generated override > managed source`. Rectangle roles stage their two
canonical seeds atomically. No semantic preference or producer selection preserves shared-point
attachment, while a uniquely selected consumer stages local detachment. Replacement direct and
generated Segments keep stable code ownership and rebind surviving code/GUI dependents; generated
circle centres use the same provenance-owned replacement path. Repeated detached drags and exact
Undo/Redo remain valid. Equal duplicate writes collapse deterministically and unequal writes to one
same-tier address reject before publication.

Selection and deletion now resolve through accepted expansion provenance. A delete token binds the
exact code-session identity, accepted alias and direct/generated semantic owner. Managed deletion
rewrites the exact code-owned source closure; generated-child deletion is reversible suppression;
dirty source, retained failure, stale authority and GUI-owned selections cannot use this route.
Current and accepted overlays persist separately. A parseable structural attempt which later fails
native publication keeps only its exactly owner-pruned current overlay while the accepted overlay
and canvas remain unchanged. `geosolve-sketch-code-session-v2` and
`geosolve-code-workbench-v2` intentionally make that unreleased wire change explicit; plain M83
workspace-v8 remains unchanged.

Owning coverage is concentrated in `crates/geosolve-sketch-code/src/overlay.rs`,
`crates/geosolve-sketch-code/tests/m84_semantic_overlay.rs`,
`crates/geosolve-sketch-code/tests/m84_native_composition.rs` and the code-workbench tests. It
separately exercises direct literals, all rectangle roles, direct and generated shared references,
generated circle centres, seed precedence, exact conflict handling, Reset, managed deletion,
generated-child suppression, retained failure, persistence and Undo/Redo.

### M84-F006 — adversarial persistence and authority hardening

An independent post-F005 audit found no architecture blocker, but identified defensive gaps which
could weaken an otherwise-valid authority contract under hostile persistence or uncommon generated
ownership. Imported session IDs are now capped before allocator adoption; managed editor drafts
retain the 4 MiB source bound; Reset and Restore use typed canonical tokens; and retained-failure
pruning authenticates both direct and generated owners exactly. Generated-reference provenance,
detachment, direct/generated Segment replacement, generated circle-centre replacement, dependent
rebinding and transient cancellation are explicit. Repeated generated drag has exact Undo/Redo
coverage. Same-tier point conflict compares persisted IEEE bits, so identical writes collapse but
`+0.0` and `-0.0` conflict in either order. These are resource, persistence and transaction-
authority repairs only; no equation, residual, constraint, priority, tolerance or branch policy
changed.

### M84-F007 — terminal classification copied coupled solver motion into semantic writes

Reproduction owner: multi-frame code-owned rectangle point drag with either no semantic selection
or the producer declaration selected. Pointer preview correctly solved the coupled rectangle, but
terminal publication classified the final checkpoint after the fact. It collected every moved
code-owned point, including solver-derived coupled corner motion, as if each were an independently
authenticated semantic seed. Tiny roundoff differences then produced multiple unequal same-tier
writes to one semantic address, and the bit-exact F006 conflict rule correctly but undesirably
rejected the legitimate release.

Repair: pointer-down now authenticates one exact `ExpandedWritablePoint` lens and retains it across
all preview frames. With no selection, the unique producer is selected deterministically; selecting
that producer preserves attachment; selecting a unique referenced consumer still performs local
detachment. Ambiguous lenses reject before mutation. Terminal publication independently applies
only the authenticated point lens to the semantic overlay and rematerializes that authority.
The solver-coupled terminal checkpoint remains preview evidence but cannot manufacture additional
semantic writes from incidental roundoff. Ordinary GUI-owned points never gain a code semantic
route and remain delegated to the existing editor.

This paragraph records the historical F007 repair. M84-F010 below supersedes its single-seed
durability rule while preserving the exact pointer-down authentication and route-lifecycle rules.

The pending route is the exact `CodeSessionIdentity`, pointer and authenticated lens. Only its
dedicated pointer-terminal publisher may consume it: a generic save rejects without consumption;
foreign/reentrant preparation and foreign terminals reject while preserving the original route. Any
non-pointer durable code action invalidates it. The final audit routes Outline arrows/drag-drop,
Inspector edits/rename, relation/dimension terminals, annotation reset, feature/offset Apply,
geometry Finish/role, managed/structured source, code actions and history/deletion through one
pre-mutation cancellation boundary. Internal cancellation no longer depends on finding the DOM
viewport; only platform pointer-capture release does. An unexpected generic or foreign save returns
without restoring accepted authority beneath the live route. Its behavioral adapter regression
proves the pending token, serialized session/history, live editor authority and notice remain
unchanged. No-motion release/cancel adds no history, while exact stored-session mismatch, Apply or
Undo cannot change the newer accepted authority.

The earlier corrected provisional release-WASM/browser matrix passed 14/14. Post-audit demo-web
passes 270/270, including real no-motion release, exact stored-session mismatch, mutation-order and
generic-save preservation regressions; the sketch-code suites and focused warnings-denied
Clippy/WASM checks pass. Exact committed source `cc2f05e` then passes the clean release gate and
the refreshed browser matrix on temporary and retained frozen endpoints. This withdraws the
`ff2e142` candidate and establishes historical F007 evidence, but not UAT acceptance. F010 below
supersedes the single-seed durability interpretation without weakening pointer-down authentication.

### M84-F010 — authenticated release omitted its solver-coupled movement closure

Reproduction owner: retained code-workbench semantic terminal publication in **Compass rose**.
Drag the shared center and release a valid newest native preview. The displayed point initially
matches the release, then roughly 500 ms later moves to another valid solution. The delay is the
synchronous cost of terminal rematerialization and persistence, not a timer, worker or background
validation race.

Root cause: F007 correctly authenticated `north.start` as the one pointer-down semantic lens, but
terminal durability persisted only that center seed. Moving the center under the four existing
Horizontal/Vertical relations also moves the four spoke endpoints in the accepted native preview.
Cold overlay materialization therefore combined the new center with stale spoke-end seeds and the
unchanged solver selected a different valid underdetermined solution; the durable render exposed
it only after the synchronous publication returned. The authenticated single lens was sufficient
authorization but not a complete replayable terminal state.

Repair: compare the exact authenticated origin editor with the accepted terminal editor using the
existing closed code-owned-change classifier. A referenced consumer retains the exact
post-detachment checkpoint as that origin. Require the authenticated lens to be part of the
resulting closure, then atomically stage all moved semantic point seeds. Never admit an unrelated
changed leaf: existing provenance and locality classification remain the gate. Durable
rematerialization always receives full native parity validation against both design and
`accepted_state_for_current_input()` documents, computed features, logical/native ownership and
allocator high-water.

Rectangle points require explicit codec canonicalization because four GUI corner lenses describe
two managed seeds. The authenticated corner and its diagonal opposite are exact anchors. The two
adjacent aliases may normalize only when finite component values have the same sign within 8 ULP,
or both values lie within `32 * f64::EPSILON` of zero. Different signed zero remains a conflict;
ordinary point aliases are bit-exact, and every non-finite or material disagreement fails closed.
This policy changes no solver equation, hard/soft priority, relation, tolerance or branch state.

Focused owner regression `compass_rose_shared_center_terminal_matches_the_last_native_preview`
uses the genuine sample and pointer lifecycle. It proves bit-exact accepted release, attached
east/south/west starts, complete terminal design/accepted parity, one outer history revision,
round-trip persistence, finite accepted geometry, current computed features and independently
validated normalized Hard residual `<= 1e-9`. The final post-refinement demo-web suite passes
274/274; formatting, warnings-denied demo-web Clippy, all-feature WASM and diff hygiene pass. On
the mutable `:18090` development listener, the original release remains unchanged at release and
+50/+250/+500/+1000 ms; six successive center drags retain their exact releases, all spoke starts
remain attached and no browser error occurs. Exact committed source `cf463838` then passes the
complete clean gate, immutable no-rebuild freeze, exact temporary/retained HTTP verification,
focused Compass 1/1 and the carried 14/14 browser matrix on both endpoints. Its then-current,
now-F011-withdrawn nomination is recorded below; these automated facts do not accept a human UAT
row.

### M84-F009 — canonical map order selected an unrelated multi-output shorthand

Reproduction owner: optional code-project patch expansion. `publish_invocation_declaration`
published a one-segment template path while iterating each output and used insert-if-absent
semantics. For a multi-output template, canonical `BTreeMap` order therefore chose the first output
as shorthand even when it had another name. Mounting Plate's `profile` template consequently made
`plate.profile` alias its alphabetically first `ne` Point instead of its Profile output.

Repair: the caller-owned TypeScript recorder now persists the exact selected `result_output` on the
data-only template independently of its renamed/nested public path. Rust publishes only that
selection, builds the corresponding nested collection root and removes its prior arbitrary
one-child prefix fallback. Dynamic `each`/`mapRecord` callbacks record and filter by the same
selection; absent collection result provenance rejects. Every fully qualified output path remains
unchanged. A public expansion regression covers `{ nested: { shape: rounded.profile } }`, the
TypeScript runtime covers mapped selection, and the all-eight native-composition audit independently
checks both `FeatureRef.expected_kind` and actual expanded target kind. This is an optional-layer semantic-routing repair only; native geometry,
solver equations, constraints, priority, tolerance and branch state are unchanged.

## Focused F005-F007 qualification

The final pre-nomination implementation state passed the proportional pre-release matrix:

- `cargo test --locked -p geosolve-sketch-code` passes, including
  `m84_native_composition` 11/11 and `m84_semantic_overlay` 8/8;
- the separate M84 code-project golden passes 1/1;
- `RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib` passed 264/264 at the
  F006 checkpoint; the final audited F007 worktree passes the web library 270/270, including
  dedicated consumption, route preservation/invalidation, actual no-motion release, exact stored-
  session mismatch, durable-mutation ordering, defensive generic-save preservation and history
  neutrality;
- milestone-neutral golden survey/check/require-clean passes unchanged at 271/271;
- `packages/geosolve-intent` passes 40/40 and the normalized Rust package closure contains the
  expected 29 files;
- focused warnings-denied Clippy, formatting and `git diff --check` pass during the audits; and
- the pre-final-audit F007 release-WASM/browser matrix passed 14/14; its focused WASM check and the
  post-audit WASM build check pass; the refreshed 14-case browser run subsequently passes on both
  candidate endpoints.

These focused results establish the owning-layer behavior. The exact clean committed-source
nomination below supplies the release-level evidence.

## Historical F007 qualification and immutable nomination

Exact product source `cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
`6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, passed
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
from 00:52:15 through 01:08:51 AEST on 2026-08-27, exit 0 in approximately 996 seconds. Its final
Trunk 0.21.14 `INFO ✅ success` marker was checked. The 6,194-line, 419,126-byte log
`/tmp/geosolve-m84-f007-release-gate.fn6ZHE.log` has SHA-256
`40c73a8856df0905e85e2b877a82db0e0e81da583764ccc18d729e268f01763b`. The gate covers format and
diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature workspace tests and
doctests, the clean 271-row golden, actual WASM, both TypeScript packages, benchmark compilation,
M14/M32/M83 and interaction performance, the independently validated 256-body sparse crossover,
licensing, package contents/extraction and the final release Trunk build. The worktree was clean
before and after. Golden and M84-ledger hashes remain
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

Without rebuilding, the exact seven-file gate output was copied to
`/tmp/geosolve-m84-f007-uat.KgW8fpLf`. Source, copied and frozen manifests are identical; the
directory is `0555`, exactly seven regular non-symlink files are `0444`, and the ordered-manifest
aggregate is `8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`. Complete gate,
manifest, mode, HTTP, browser, service and screenshot evidence is retained at
`/tmp/geosolve-m84-f007-freeze-evidence.rP5rQcTG`.

The same immutable bytes were first served only on Tailscale port `18087` under PID `34895`.
Temporary and final `:8080` eight-path ledgers are byte-identical at SHA-256
`efa609c6bac127753336c3634730b81bed04699a25c6394ab039c7f06b0b2b64`: `/` and all seven files
return HTTP 200, zero redirects, exact MIME/length/body, no `Location`/`Content-Encoding`, and `/`
equals `index.html`. Sequential browser suites pass baseline 4/4, direct authored 3/3, F003 1/1,
F004 2/2 and F005-F007 4/4 on both endpoints, including `1440x900` and `1024x720`. Only after the
temporary byte/browser proof passed was withdrawn PID `4081080` retired. Temporary PID `34895` is
also retired; `geosolve-m84-uat.service`, PID `62376`, served only the exact snapshot at
`http://100.94.63.83:8080/`. Browser checks directly cover no-selection producer, selected
producer, selected-consumer detachment, repeated drag, Undo/Redo, reload, Reset, managed deletion
and generated-child suppression. Foreign/stale terminal and bit-conflict adversarial cases remain
owned by the Rust/WASM regressions because no corresponding public browser gesture exists. This is
mechanical nomination evidence. The F009 replacement below supersedes these served bytes; PID
`62376` is retired.

## Withdrawn F009 qualification and immutable nomination

Exact product source `c74651cc82506e31926042df65a1eeec08a6af9d`, tree
`a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passed
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
from approximately 13:30:01 through 13:53:36 AEST on 2026-08-27, exit 0. Its final Trunk 0.21.14
`INFO ✅ success` marker was checked. The 6,218-line, 421,590-byte log retained as
`/tmp/geosolve-m84-f009-freeze-evidence.3FoVTQ6m/release-gate.log` has SHA-256
`c9b743c8f95d6df7706b04e2d820ac67426f1b11ec447d2bffdc08cd1fe0f6f1`. The gate covers format
and diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature tests and doctests,
actual WASM, both TypeScript packages, package closure, benchmark/performance/licence checks, the
independently validated 256-body sparse crossover and the final release Trunk build. The 256-body
crossover completes in `115.37s`. The worktree was clean before and after. Golden and expanded
M84-ledger hashes are
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

Without rebuilding, the exact seven-file gate output was copied to
`/tmp/geosolve-m84-f009-uat.q8cKIN3v`. Source, copied and frozen manifests are identical; the
directory is `0555`, exactly seven regular non-symlink files are `0444`, and the ordered-manifest
aggregate is `23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
6f13aa5328645e8037214ff54a3a157c6a5ff2d2aea91dc088b3151b65ff85cc  geosolve-demo-web-215ef4b630ae7f0f.js
73ba229ae1b6498c02e0ededcb16d7e8c00fedd7eb8ef9acb18e7a3a3a2f24ba  geosolve-demo-web-215ef4b630ae7f0f_bg.wasm
b11456544856bc8ea9fa909ee41b465134fc716c6fd256eb0b7f6ed5679aebae  index.html
81e24b427990f181b099e7b751dee6739d68024001e3cef592a8be3faed435db  styles-8eda25752cd33a23.css
```

Complete gate, manifest, mode, HTTP, browser, service and screenshot evidence is retained at
`/tmp/geosolve-m84-f009-freeze-evidence.3FoVTQ6m`. The immutable bytes were first served only on
Tailscale port `18089` under PID `3943194`. Temporary and retained `:8080` eight-path ledgers are
byte-identical at SHA-256
`add827e88d17735cfb6cb0bbecec885f5680db0bd11b67bb591673d566b90676`: `/` and all seven files
return HTTP 200, zero redirects, exact MIME/length/body, no `Location`/`Content-Encoding`, and `/`
equals `index.html`. Browser suites pass baseline 4/4, direct authored 3/3, F003 1/1, F004 2/2
and F005-F007 4/4, 14/14 on each endpoint, including `1440x900`, `1024x720`, all eight
fitted finite projects and the collaborative lifecycle/drag/deletion surface.

Only after temporary byte/browser proof passed was superseded F007 PID `62376` retired. Temporary
PID `3943194` is retired. M84-F010 withdraws this nomination. Historical retained PID `3965271`
and temporary development PID `238809` are retired; the immutable F009 snapshot remains preserved.

## Historical F010 qualification and withdrawn replacement nomination

Exact product source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passed
`env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
from 15:58:23.055857854 through 16:17:18.303733569 AEST on 2026-08-27, exit 0. The 6,209-line,
420,425-byte log `/tmp/geosolve-m84-f010-gate.9NvAi3z5/release-gate.log` has SHA-256
`bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and ends with the final
Trunk success marker. The gate ran from clean committed source and covers the complete release
matrix. The unchanged 271-row golden and eight-demo M84 ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

Without rebuilding, the exact seven-file gate output was frozen at
`/tmp/geosolve-m84-f010-uat.7R5eXQoz`. The directory is `0555`, all seven regular files are `0444`,
and the ordered-manifest aggregate is
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`; complete evidence is at
`/tmp/geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary `:18091` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`, with exact bytes and MIME
on all eight paths. Focused Compass 1/1 and the carried 14/14 browser matrix pass on each endpoint.
The focused run performs six center drags, samples each exact release at
+50/+250/+500/+1000 ms, keeps all four spokes attached, proves finite accepted authority and
verifies exact reload. Retained collateral spec/config SHA-256 values are
`4b97f570d5122a353b4ee104b26ea46427ca1c3b79aa5a8d35fee7875302dab0` and
`c0900c1132352ed9471321a2cf5727baf2c004d8eebf1a1bcd8a43146289df9c`.

Only after temporary exact-byte and browser proof passed did
`geosolve-m84-uat.service`, PID `650971`, replace F009 at
`http://100.94.63.83:8080/`; its working directory was the immutable F010 snapshot. Historical F009
PID `3965271` and temporary F010 PIDs `238809`/`621532` are retired, while the F009 snapshot is
preserved. This is the clean immutable F010 replacement nomination, not acceptance. Refreshed
U1-U14 were pending when this evidence was recorded. M84-F011 withdraws the nomination; F010 PID
`650971` is retired and its frozen snapshot remains historical rollback evidence.

## Historical F011 implementation and replacement qualification

The ninth **PC Water Manifold** project is implemented as the milestone's first substantial
AI-authored mechanical dogfood sketch. Its managed source describes a 240 × 120 mm plate, 60 × 84
mm reservoir bay, three open water-channel Polylines, three closed O-ring loops, eight diameter-5
mm screw circles and 21 construction datums. Exactly one FixedPoint anchors absolute position;
there is no FixedCoordinate. The caller-owned custom `waterChannel` patch applies keyed `p.each`
expansion to the six current channel corners, yielding six existing native Fillets while adapting
to the Polyline corner collection.

The native owner test requires a finite accepted solved state, every active feature Current,
independently validated normalized Hard residual `<= 1e-9`, and numerical, equality and
bidirectional DOF all zero. It also requires the exact route, seal, datum, circle, dimension and
Fillet inventories. This qualification uses ordinary M83 Intent/materialization/native-solver
authority; the new optional managed spellings for circles, Coincident, FixedPoint/FixedCoordinate,
CurveLength/Diameter, keyed Polyline members and line roles add no equation or residual.

The command bar also exposes presentation-only **Export PNG**. It derives from the composed
authoritative SVG, hides hit/provisional content, rasterizes with self-contained styling and
downloads fixed 2000 × 1400 output without mutating the document or history. A focused local
Chromium run on the mutable F011 build passes 1/1: it opens the ninth project as accepted, observes
at least 53 points and 58 curves with exactly six finite computed Fillets, confirms all visible
geometry is fitted, inspects managed/custom hybrid source and downloads a PNG without a browser,
console or request error. The 233,543-byte download has PNG signature, IHDR 2000 × 1400 and
SHA-256 `2378a8c74216524c42fc8910d79e28a6dacc54fc437ac705518ab42122363dc2`.

That proportional mutable-build run is superseded by the exact frozen proof below. U15 owns the
manifold dogfood check and U16 owns PNG export; both remain pending alongside U1-U14. No M84 Pages
publication or milestone closure is authorized.

Exact committed source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, passes the complete clean release gate from
18:38:08.068586771 through 18:55:55.205076185 AEST on 2026-08-27, exit 0. Its 6,251-line,
422,664-byte log `/tmp/geosolve-m84-f011-gate.GvBT6f/release-gate.log` has SHA-256
`ceea545929196981e2790a822b384596642f63a6ef93ecea98f76799cdd7e353` and final Trunk success.
The unchanged 271-row golden and expanded nine-demo ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`.

Without rebuilding, its exact seven-regular-file, zero-symlink output is frozen at
`/tmp/geosolve-m84-f011-uat.ps736NLh`, directory/files `0555`/`0444`, ordered-manifest aggregate
`056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`, with complete evidence at
`/tmp/geosolve-m84-f011-freeze-evidence.4deymgss`. Temporary `:18093` and retained `:8080`
eight-path HTTP ledgers are byte-identical at SHA-256
`9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`. The focused frozen
manifold/PNG/authority suite passes 1/1 in 12.8 seconds on temporary bytes and 1/1 in 12.9 seconds
on retained bytes. It verifies the accepted ninth project, hybrid managed/custom source, fitted
finite geometry, six Fillets and PNG export while preserving exact lifecycle, history length,
project title and viewport markup authority around the action. Both 233,543-byte PNGs have SHA-256
`2378a8c74216524c42fc8910d79e28a6dacc54fc437ac705518ab42122363dc2`, valid signature and
2000 × 1400 IHDR; both screenshots have SHA-256
`2db91740662d19adf9f38518ed25deca91ad92e37022dfa3549caad0d608bcb0`.
The retained focused spec and config have SHA-256
`44e40a877c1bfbbf90c9d725ea402a9bbb54d1522241130b5bd1c467ad943bd4` and
`f9b17a19922e8b6d569d55e6fa1a0c4207caf302653557670261beb33dd49827`; both successful
`.last-run.json` files have SHA-256
`91d1c43004802cd49950d78eb11c8fa7d05da8ffffe219a8b13b2f561bc00903`.

Only after temporary proof passed was F010 PID `650971` retired. Retained
`geosolve-m84-uat.service`, PID `1485656`, invocation
`f04bc05089d94947b7a24d8ec6a6f26d`, served only the immutable F011 snapshot from its snapshot
working directory at `http://100.94.63.83:8080/`; temporary `:18093` is retired. M84-F012 withdraws
this nomination. PID `1485656` was retired only after temporary F012 proof passed, and the F011
snapshot remains historical rollback evidence.

## Current F012 replacement qualification

Exact committed source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the complete clean release gate from
20:36:27.840305756 through 21:01:30.826307821 AEST on 2026-08-27, exit 0. Its 6,274-line,
424,393-byte log `/tmp/geosolve-m84-f012-gate.PmjeGNUa/release-gate.log` has SHA-256
`04e35c73fe92ca3e089b87bd13b5221c60835b72c9eeba5ed38916b51150004a` and final Trunk success.
The unchanged 271-row golden and nine-demo ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`.

Without rebuilding, its exact seven-regular-file, zero-symlink output is frozen at
`/tmp/geosolve-m84-f012-uat.nMOymIIM`, directory/files `0555`/`0444`, ordered-manifest aggregate
`166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`, with complete evidence at
`/tmp/geosolve-m84-f012-freeze-evidence.qua6ci1b`. Temporary and retained eight-path HTTP ledgers
are byte-identical at SHA-256
`66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`.

The focused browser spec passes 1/1 on temporary and 1/1 on retained bytes. It proves annotation
paint/pick removal, underlying-target access, authority neutrality, exact restoration and WYSIWYG
visible/hidden export. The visible 233,543-byte PNG has SHA-256
`2378a8c74216524c42fc8910d79e28a6dacc54fc437ac705518ab42122363dc2`; the hidden 154,969-byte PNG
has SHA-256 `8c4af68f0775066d40bd1e88f9fe42d57f70b49137ac41627c7c530d1c398317`; both are 2000 × 1400.
The restored screenshot has SHA-256
`2db91740662d19adf9f38518ed25deca91ad92e37022dfa3549caad0d608bcb0`. The spec, config and
successful `.last-run.json` have SHA-256
`c5335142243652d744bfd89ce2050213556e54b46f40a8777f30355a6cf22e2e`,
`b9fd16011fd58cb4edd33ecdc61c903e5722f6230b11cf2e45071b70692a0898` and
`91d1c43004802cd49950d78eb11c8fa7d05da8ffffe219a8b13b2f561bc00903`.

Retained `geosolve-m84-uat.service`, PID `2241323`, invocation
`b621b1a43b8c4ee281f1e8edddf10e57`, serves only the immutable F012 snapshot from its snapshot
working directory at `http://100.94.63.83:8080/`; the temporary service is retired. This is a clean
immutable replacement nomination whose automation alone claimed no acceptance; U1-U16 were later
accepted by the supervising user's milestone-level close decision.

## Historical F003 focused evidence observed before its withdrawn nomination

- `cargo test --locked -p geosolve-sketch-code --all-features` — 44 unit and 29 integration tests
  pass, including parser/rewrite, artifact, descriptor, bootstrap, reconciliation, native
  composition, direct-line lexical lowering, override, optional-boundary and TypeScript-
  interoperability owners.
- `cargo test --locked -p geosolve-demo-web --lib` — 243/243 pass, including ordinary Intent IR,
  read-only lexical preview, real promotion/persistence, fresh-foundation handling, dependent
  movement and ordinary-Segment branch-authority owners.
- `cargo clippy --locked -p geosolve-sketch-code --all-targets --all-features -- -D warnings` and
  the corresponding `geosolve-demo-web` focused command pass.
- `(cd packages/geosolve-sketch-code && npm ci --ignore-scripts && npm test)` passes its build,
  artifact fixtures, six runtime tests, type failures and managed-source compilation.
- `./scripts/verify-geosolve-sketch-code-package.sh` passes over the real 28-file normalized crate;
  all runtime assets ship and the extracted crate checks locked/offline.
- `./scripts/golden-authoring-scene-oracle.sh --require-clean` passes unchanged 271/271 authority,
  SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
- The separate M84 ledger passes with SHA-256
  `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.
- All-feature WASM check and the shell-provided actual-WASM suite pass. `cargo fmt --all -- --check`,
  `git diff --check`, focused warnings-denied Rustdoc and shell syntax checks pass.

These development/focused results are incorporated into, but do not substitute for, the clean
committed-source gate and frozen nomination below.

## Withdrawn clean qualification and frozen nomination

Exact committed product source `79078eca44a5af4de5cccd92bf6fee570c473624`, tree
`05aefb0cbd3972d423f1713df1e58628b24ec216`, passed:

```bash
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run ./scripts/release-gate.sh
```

The gate ran on 2026-08-25 from 21:31:20 to 21:51:56 AEST. Its 6,107-line, 414,758-byte log is
`/tmp/geosolve-m84-release-gate.GOEuXP.log`, SHA-256
`0f50e6bcdf019c71d70497acc301dcdfd194db1142b248bcd469d0f3ed9efda0`. Workspace
warnings-denied Clippy, locked all-feature tests, Rustdoc, the clean 271-row golden, actual WASM,
both TypeScript suites, package closure, performance/benchmark gates, licensing and Trunk 0.21.14
release assembly all passed. The separate workspace-test log
`/tmp/geosolve-m84-workspace-tests.bQDpzA.log` has SHA-256
`72e9c6efd229b7441f00da5b13c4381e01cb1386c5aac87d2d257981a6306689`. A standalone host
`trunk build` was unavailable because Trunk is not on the host `PATH`; the canonical Nix gate
provided Trunk and passed.

Without rebuilding, the exact gate output was copied to `/tmp/geosolve-m84-uat.aHw5ePSW`. The
directory is mode `0555`; its seven regular non-symlink files are mode `0444`. Freeze evidence is
retained at `/tmp/geosolve-m84-freeze-evidence.7loLImo2`. The ordered-manifest aggregate is
`99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
57918914596207c2a3aa27abae8cbce1e321a54d797620e0ea7e66900d940732  geosolve-demo-web-e0d2c05fe4bed1f4.js
15cdf9f3ecabd8c0b42e419009af43065686a2d66a8f6209a48ae57449bc172e  geosolve-demo-web-e0d2c05fe4bed1f4_bg.wasm
2afe27a4143da8521f07945cb0671e3412985b05d3ab035a26255196079776fe  index.html
9c4cc19e4ead8b15276095c81983cd0824f792e580e5929adc75f979264e2952  styles-5c4359127dc3a0bb.css
```

Local and retained-Tailscale verification covered `/` and all seven files: HTTP 200, zero
redirects, exact MIME type/length/hash, no `Location` or `Content-Encoding`, and root equality with
`index.html`. Ledgers `/tmp/geosolve-m84-temp-verify.dowsOmMZ/results.tsv` and
`/tmp/geosolve-m84-final-verify.HlnJrbWo/results.tsv` both have SHA-256
`dd8e6c1350f56cb6e7a483892a188187edc68ddcb63ee8ba9335401432ba8895`. Focused Playwright
passes 4/4 locally and 4/4 over Tailscale. Its bounded-surface check passes at `1440x900` and
`1024x720`; the suite also covers managed-lens/native/history publication and all four genuine
projects. Config/spec hashes are `f0308eb1d706ead212d370963f9b6b6c87fe8488923a02e9efe62d2d1f457ee0` and
`5ef1b00cc17073a789c8f86af2e29a225f5f1d091941b28172e1546ceba181a0`; local/final log hashes
are `54858c5a1f75cc2e286d07360ed8342c7f8a09290a8f3e7ee1c4d295afe91d9e` and
`37f289b62adc02362e8c34a1ea23377c2a3fc5446d43ac262a5b075c1652f4c0`.

Historical `geosolve-m84-uat.service` PID `2426265` served only that immutable directory at
`http://100.94.63.83:8080/`; it was retired after the F003 replacement passed temporary
verification. The old temporary listener is also retired. This withdrawn nomination claims no
human UAT, approval, Pages publication or milestone closure.

Human UAT subsequently opened M84-F003. This snapshot is preserved as historical defect evidence
and claims no current nomination, approval, Pages publication or milestone closure.

## Withdrawn M84-F003 replacement qualification and frozen nomination

Exact committed product source `b9e67bad7f4935b1e0591ea4f149fae478b32675`, tree
`7062806695e1e134c339cfa47903145d321f6350`, had a clean worktree and passed:

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run ./scripts/release-gate.sh
```

The gate ran on 2026-08-26 from 00:24:02 through 00:46:20.940662 AEST, approximately 22m19s,
and exited 0. Its 6,192-line, 414,397-byte log `/tmp/geosolve-m84-f003b-release-gate.log` has
SHA-256 `eb05d3c1e460f5cb7be410dc44d0af0c4b4eaf4fd775423b676c77a84a433f90`.
Workspace warnings-denied Clippy, locked all-feature tests, Rustdoc, the unchanged 271-row golden,
actual WASM, both TypeScript suites, package closure, performance/benchmark gates, licensing and
Trunk release assembly pass. Golden SHA-256 remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate M84
code-project ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

Without rebuilding, the exact gate output was copied to `/tmp/geosolve-m84-f003-uat.mO67NI`.
Source, copied and frozen manifests are identical. The directory is mode `0555`; exactly seven
regular non-symlink files are mode `0444`. Complete evidence is retained at
`/tmp/geosolve-m84-f003-freeze-evidence.Ue4SCM`. The ordered-manifest aggregate is
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
8c06f0303535f09aa7bec53703136f5e29625c0aa193abff945bc867ab13af99  geosolve-demo-web-c14103084aedc965.js
178016828122190de25998de41a9a1991028a0180da0ff665a843fc170c9b85f  geosolve-demo-web-c14103084aedc965_bg.wasm
d8faa1ccc37a0758aaf8ba94d45cfd12661ba42bc4f80b35d754ae59897311fe  index.html
5b30ea9a86e4be495437705c2b2a9cb30800068ff5a27bcd61a408e13d7e1700  styles-4a93ed51c1144512.css
```

Temporary `:18084` and retained `:8080` verification cover `/` plus all seven files: HTTP 200,
zero redirects, exact MIME type/length/hash, no `Location` or `Content-Encoding`, and root equality
with `index.html`. Both result ledgers have SHA-256
`438d747641522dd567e5790c56663f9bcb596147d48b843e1fe3367830822d0c`. Existing focused browser
checks pass 4/4 and the dedicated F003 GUI draw → Intent IR → lexical preview → promotion → managed
rectangle edit → dependent-line movement → reload flow passes 1/1 on both endpoints.

Temporary listener PID `3728373` is retired. The worktree listener PID `2872083` is retired. Old
withdrawn retained PID `2426265` was retired only after the replacement passed temporary byte and
browser verification. `geosolve-m84-uat.service`, PID `3736900`, served only the immutable F003
replacement snapshot at `http://100.94.63.83:8080/`. M84-F004 withdraws that snapshot from current
UAT even if the endpoint remains reachable. This historical nomination claims no human UAT row,
approval, Pages publication or milestone closure.

## Post-F004 direct code-authoring amendment

This is an intentional M84 scope extension, not M84-F005. A fresh workspace's Code surface now
offers one direct authored entry and the four existing genuine examples. The starter is a complete
artifact-free rectangle-plus-diagonal `sketch.ts`; its line endpoints are lexical
`frame.corners.lowerLeft`/`upperRight` values. `Authored` persists as its own origin and the shared
installer mutates live authority/sample identity only after every fallible candidate conversion
succeeds. Source focus likewise occurs only after successful creation.

Fresh classification is fail-closed: the graph must contain exactly the canonical document
foundation, current and accepted semantic identities must match, and the accepted native evidence
must independently validate zero points, curves, constraints, computed features and residual
failure. Nonempty, unsupported and retained-failed scenes continue through Preview/Unavailable.

Focused commands run from the amended worktree and pass:

```bash
cargo test --locked -p geosolve-sketch-code --lib managed_only_project -- --nocapture
# 2 passed
cargo test --locked -p geosolve-demo-web --lib \
  fresh_code_surface_offers_one_authored_entry_and_every_genuine_sample -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_starter_applies_persists_and_retains_invalid_code_atomically -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_project_accepts_a_complete_source_replacement_with_exact_history -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_origin_rejects_a_conflicting_legacy_demo_identity -- --nocapture
# 1 passed
```

The owner tests independently check finite accepted geometry, Hard residual validation, exact
native corner-ID aliasing after a rectangle move, artifact-free authority, valid and retained-
invalid persistence, whole-source replacement and Undo/Redo. The separate four-demo M84 ledger is
unchanged.

## Direct code-authoring qualification and immutable replacement nomination

Historical committed source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, passed this clean command:

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run './scripts/release-gate.sh' 2>&1 | \
  tee /tmp/geosolve-m84-authored-release-gate.log
```

It ran from 2026-08-26 14:55:50.676940562 through 15:12:22.475576111 AEST, exited 0 in
991.798635549 seconds, and its log was independently checked to end with Trunk 0.21.14
`INFO ✅ success` rather than trusting the tee pipeline alone. The 6,146-line,
415,754-byte log `/tmp/geosolve-m84-authored-release-gate.log` has SHA-256
`34bf408f6a565dec5705745f916eb628397a549b4a7002d865269e8d167e179f`. The gate passed formatting,
diff hygiene, warnings-denied Clippy and Rustdoc, locked all-feature workspace tests/doctests,
actual WASM, both TypeScript packages, Rust package extraction/closure, licensing, performance and
benchmarks, and the final Trunk release build. The 271-row golden remains byte-identical at SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate four-demo M84
ledger remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

Without rebuilding, the exact seven-file gate output was copied to
`/tmp/geosolve-m84-authored-uat.ZYQQyBQQ`. Source, copied and frozen manifests are identical. The
directory is `0555`, every regular non-symlink file is `0444`, and the ordered-manifest aggregate
is `6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`. Complete manifests,
modes, clean-worktree records, gate metadata, HTTP ledgers, browser scripts/logs and screenshots are
under `/tmp/geosolve-m84-authored-freeze-evidence.ngNf7jxZ`.

The same frozen directory was first served on temporary Tailscale port `18086`. `/` plus all seven
files returned HTTP 200, zero redirects, exact MIME/length/hash, no `Location` or
`Content-Encoding`, and `/` equalled `index.html`. The temporary and final `:8080` ledgers are
byte-identical with SHA-256
`1450e4c6d8585ba17dee56feafaf96869c45f764f3400280a0dc37581f9b4eee`.

Sequential Playwright suites against the exact frozen bytes pass on both endpoints: direct authored
lifecycle 3/3, baseline M84 4/4, F003 1/1 and F004 2/2. The authored suite covers `1440x900` and
`1024x720`, zero horizontal/vertical landing overflow, all four cards, Start/edit/Apply, finite
accepted geometry, retained-invalid canvas, Undo, reload/repro and nested card routing. The landing
CSS was corrected during provisional review from 106/342 px vertical overflow to zero at those
resolutions, then the clean gate and exact frozen runs requalified the correction.

Only after temporary byte and browser verification passed was historical F004 PID `3316682`
retired. The temporary PID `4027499` is also retired. M84-F005 withdraws this otherwise qualified
direct-authoring record because it predates the collaborative overlay and semantic-authority
contract. Combined F005/F006 source `ff2e142` and its later frozen candidate are themselves
withdrawn by F007's terminal-lens reproduction. Those snapshot/service records are historical only,
not current candidates. The withdrawn F009/F010 and historical F011 qualification records are
above. GitHub Pages deliberately remains on accepted M83 pending refreshed F012 UAT and explicit
approval.

## Known bounds and truthful limitations

- Managed-v1 is intentionally a closed projectional subset. Arbitrary custom code is caller build
  input and has no browser runtime or general AST round-trip promise.
- The supported accepted GUI closure now includes lexical rectangle/Segment dependencies,
  Segment-to-Segment endpoints and direct computed FilletSets. Other unsupported declarations
  still reject the complete all-or-nothing promotion and expose a read-only diagnostic rather than
  disappearing. The bundled Braced Frame remains a genuine managed code project.
- Integration test sources which inspect workspace TypeScript/manifests are intentionally not part
  of the published Rust archive. Runtime library code and every required asset are self-contained
  and extraction-built.
- General ejection, arbitrary formulas/new constraints, arbitrary TypeScript execution, general
  topological naming, 3D/B-rep behavior and the deferred Offset redesign remain out of scope.
- Human discoverability, presentation feel and repeated real-browser drag/deletion responsiveness
  remain owned by `docs/M84_UAT.md`. F011 adds explicit manifold and PNG checks U15/U16. Pages
  remained M83 through nomination and may change only through the approved, exactly verified
  closeout publication.

## Supervising-user acceptance

On 2026-08-27 the supervising user approved M84 and requested that the milestone be closed before
performance work begins. The decision accepts M84-U1 through M84-U16 against the exact qualified
F012 candidate at milestone level. It does not claim a separate row-by-row replay or invent
unrecorded observations. The qualified product source/tree and immutable no-rebuild snapshot remain
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`,
`429ed56d2a5b3988d6604079d19e1002f9049d64` and
`/tmp/geosolve-m84-f012-uat.nMOymIIM`; this documentation-only acceptance does not rebuild or alter
them.

## Remaining release sequence

1. Publish this accepted descendant to GitHub Pages and exact-verify its separately built hosted
   artifact against the downloaded Pages artifact.
2. Only after that proof, retire `geosolve-m84-uat.service`, preserve the immutable F012 evidence
   and record final M84 closure consistently.
