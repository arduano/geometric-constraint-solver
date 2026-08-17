# API and persistence compatibility

## Release line

GeoSolve `0.2.0` is the current supported preview release; `0.1.0` was the first. The
eight library crates (`geosolve-geometry`, `geosolve-core`, `geosolve-sketch`,
`geosolve-linkage`, `geosolve-sketch-ops`, `geosolve-sketch-topology`,
`geosolve-sketch-features` and `geosolve-constraint-editor`) version and release in lockstep.
`geosolve-demo-web` is a
non-published diagnostic consumer.

Before `1.0`, a minor version may contain source-breaking changes. Patch releases
must remain source-compatible except where retaining behavior would preserve a
soundness issue, false success, invalid accepted geometry or a security defect.
After `1.0`, Rust API compatibility follows Cargo SemVer.

M33 completes the production-embedding contract and baseline freeze without adding
target APIs. M34 adds the retained-design lifecycle and M35 adds cooperative
operation-control APIs; M36-M44 complete the current implementation transition. Cleanup
M46-M53 preserves released v1-v4 wire compatibility and the accepted-state safety contract
while evolving the new pre-1.0 editor/workbench surface. M61 closes the currently approved
advanced-workbench scope, M62 closes approved CAD-style authoring and M63 closes approved canvas
constraint presentation. Approved M64 adds only public alpha fixtures and an explicit
interaction-request preference helper; it does not freeze a new schema. M65 completes approved
predictable, bounded projected dragging without freezing a new persistence language or claiming
final API/schema hardening. M66 completes the explicitly approved pre-1.0 computed-Fillet feature
and authoring cut without changing the canonical sketch language. Its new
`SketchDocument::certify_line_curve_fillet_branch_cell` query is an additive pre-1.0 API backed by
private outward-rounded curve-piece intervals; it returns only the existing
`ContactNeighborhood::Local` type plus a typed error. M66 authoring currently accepts
affine/affine and affine/non-affine parent pairs and types two non-affine parents unsupported until
pairwise continuation exists. That authoring limitation does not narrow or deprecate M28's public
all-family generic Fillet request, association, residual or validation APIs. The doc-hidden
`geosolve_core::AcceptedStatePatch` export exists only as a narrow cross-crate domain integration
boundary for freshly certifying derived accepted coordinates; it is not a supported host API and
may be narrowed or replaced before `1.0`. The
withdrawn M66-only Offset/Mirror authoring surface and M66-only line-offset request APIs were never
released. Their last three-tool candidate is preserved at
`origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`). This withdrawal does not alter the
completed M25 offset constraints or the M58 exact supported-family Mirror operation-companion API.
The unreleased ADR 0030 editor-side `OperationAuthoring*` facade, its coordinator preview/replay
DTOs and the editor's direct dependency on `geosolve-sketch-ops` were introduced after the
published `0.2.0` baseline and removed before another release once ADR 0031 superseded ordinary
Fillet routing. This source-breaking cleanup affects no released API. It does not remove M27/M28
Fillet equations, associations, trim views, persistence or migrations; M58
`SketchOperationRequest::AssociativeFillet`; M25 Offset constraints; M58 Mirror; or the branch-cell
query above. Current grouped Fillet authoring is the separate computed-feature path under ADR 0031.
The unreleased M61
`DocumentSolveRequest::stability_target` field and helper were withdrawn before the next
published minor release because a sample-selected second Temporary target conflicts with M65's
sample-agnostic locality contract; neither API was part of the published `0.2.0` surface. Any
draft-v5 representation remains explicitly unsupported until a future schema-freeze milestone is
deliberately scoped, qualified and approved, and must not be treated as a released wire language.

M67 removed the doc-hidden `M40QualificationCaseResult`, `M40QualificationReport`,
`m40_qualification_corpus`, `run_m40_qualification` and
`validate_m40_qualification_matrix` evidence API after replacing every retained claim with direct
owning-layer tests. That frozen browser-evidence surface was introduced after published `0.2.0`,
had no runtime consumer and was explicitly not a product API. M67 did not remove any supported
domain API or v1-v4 persistence reader. Removing raw topology/lifecycle/redundancy cards from the
non-published demo does not narrow the corresponding reusable domain contracts.

M70 adds pre-1.0 headless drafting-inference and atomic construction-plan APIs to
`geosolve-constraint-editor`, including typed policy/input/output DTOs, exact accepted-input/token
authentication and relation-indexed publication results. These are additive to the current
unreleased editor surface; they do not add a residual, persistent relation kind or browser-owned
geometric policy. Public document/revision/stamp fields do not confer publication authority: only
a scene authenticated from the retained session's exact current accepted input may emit an
inferred commit plan. A private exact seal covers every inference-visible public scene semantic;
mutation before binding rejects authentication and mutation after binding revokes publication.
Compatibility/render-only scenes remain useful for inference presentation but are deliberately
non-publishing. M70 also adds field-opaque, checkpoint-serializable
`SketchPersistentIdentityHighWater` retention plus exact-current-input restore and controlled
transaction seams to `geosolve-sketch`. Hosts may serialize, deserialize, inspect the owning
document identity and merge the DTO through its validated API; allocator cursor fields remain
private. Application workspace v5 stores that value, validates its namespace and graph coverage,
and strictly migrates v1-v4. These APIs change no frozen sketch v1-v4 bytes and do not make
draft-v5 supported.
The `M70-F001` amendment extends that unreleased DTO surface with an explicit Circle
circumference subject and reverse-incidence candidate. Its durable result is the existing
PointOnCurve relation; it adds no sketch constraint, residual or persistence variant.
M70 implementation, focused direct qualification, integrated release gate and frozen-candidate
publication are complete on replacement source `3d157896c87eaf647abee1192c838100ce359ce9`.
Circle-authoring finding `M70-F001` is resolved and the supervising human approved M70 on
2026-08-10.

M70B adds a small versioned codec surface to the non-published `geosolve-demo-web` diagnostic
consumer: `GEOSOLVE_REPRO_V1`, bounded encode/decode functions and typed transport failures. Its
payload is compressed text around the existing private application-workspace v5 encoding, not a
new `geosolve-sketch` persistence version, supported domain schema or accepted-state shortcut.
The companion `geosolve-repro` stdin/stdout binary decodes transport for diagnosis only and cannot
validate or publish a coordinator.
V1 text is generated deterministically with canonical fields and strict unpadded base64url; a
future incompatible transport must use a new header rather than silently reinterpret V1. The
FNV-1a field detects accidental corruption only and conveys no authenticity. Successful transport
decode still requires strict `WorkspaceSnapshot` validation and complete coordinator
reconstruction before publication. No library crate API, frozen sketch v1-v4 bytes or draft-v5
support status changes. Qualification and frozen publication pass on source
`6a0d05246a3fbca7487ffd614c1d48bf5bdc9c8b`. Subsequent F001-F005 repairs and close qualification
change no additional public library API; closing source `48e3cc3` keeps the 198/198 golden and
release bytes unchanged and closes M70B under the requested scoped sign-off. ADR 0035 subsequently
activated M71's six ordinary retained definitions. They extend the in-memory document/editor API
and unsupported draft-v5 side section while canonical sketch v1-v4 remain frozen; clean
F005/F006 replacement qualification and byte-verified publication pass on source
`f8a45ae7b355ab9874bf268c9950e369814e8432`; scoped human UAT and explicit M71 approval pass on
2026-08-14. These later lifecycle additions do not alter the frozen sketch v1-v4 wire contract.

M73 completed a pre-release cleanup inside `geosolve-constraint-editor`. Public `ConstraintKind`
and `ConstraintEditor::{available_constraints, constraint_edit}`, together with the dependent
`EditorError::IncompatibleConstraint` variant, were introduced after the published `0.2.0`
baseline and duplicated only part of the later contextual authoring path. The public direct entry
points had no non-test caller; the retained coordinator's internal `ConstraintKind` use was only a
duplicate simple-definition lowering seam. M73 removed that complete direct compatibility surface
before the next published minor release, without a deprecation interval for supported APIs. Hosts
use `ConstraintIntent`,
`ResolvedConstraintKind`, `AuthoringState` and
`RetainedEditorCoordinator::{resolved_constraint, apply_authoring}` instead. This decision does not
remove or deprecate `SketchConstraintKind`, `DocumentConstraintDefinition`, direct sketch builders,
any contextual authoring DTO or any persisted relation. All 20 contextual resolved families remain
available through the retained authoring route, and no wire language changed.

M74 is a completed additive pre-1.0 extension. `SketchDatum`, the four datum-backed document
definitions, datum selection/scene DTOs, reference visibility, contextual resolved kinds and the
typed protected-datum failure expose immutable Origin/X/Y operands without giving those datums a
document ID, variable, allocator entry or persistent identity. Scene-clipped axis endpoints are
presentation data and must not be serialized as datum identity. Ordinary relations that refer to a
datum own normal constraint IDs and lifecycle. Canonical sketch v1-v4 remains frozen: encoding a
datum relation as v4 returns `UnsupportedM74State`, and its representation only in draft-v5 side
records does not make v5 a supported input or canonical output language. This compatibility
disposition is accepted under the 2026-08-16 scoped M74 close decision; no supported `0.2.0` API or
wire reader is removed. Deferred hands-on UAT does not make the compatibility contract
provisional. M74-F001 adds `SymmetricAboutDatumAxis` to the in-memory document/runtime
enums, `Sketch::add_symmetric_about_datum_axis` and the matching contextual resolved kind. It uses
the same unsupported draft-v5 side section and exact canonical-v4 rejection; no datum identity,
hidden line or frozen-wire syntax is introduced.

M75 is an additive pre-1.0 interaction correction in `geosolve-constraint-editor`. The existing
pointer-move entry points remain source-compatible wrappers, while
`ConstraintEditor::{pointer_move_with_problem_items,
pointer_move_with_problem_items_and_draft_inference}` let a host supply the same current
problem-forced annotation visibility already accepted by pointer-down. Select hover and primary
pointer-down then share one private target resolver. Finding M75-F001 adds
`RetainedEditorCoordinator::{pointer_move_authoring, pointer_move_feature_authoring}` so ordinary
relation/dimension and grouped-Fillet hosts can request the exact compatible item that the
unchanged domain-aware press resolver would consume. These methods reuse existing authoring,
scene, pointer, tolerance, selection and effect DTOs. A feature-authoring painted item is only an
intent hint: current candidate, retained preview, accepted/design/computed provenance, policy and
headless radius proximity are independently validated before it can produce a computed-corner
hover. Candidate enumeration and precedence remain private. Existing pointer-leave, cancellation
and retained-state paths revoke proximity state when a host remaps the camera, scene or input
owner. M75-F002 changes only the private web translation of that hint: during uncaptured Fillet
authoring, the complete SVG paint stack is reconciled with the exact headless radius owner so an
overlying native item cannot hide the grip, rail or spoke; final authentication remains in the
coordinator. No general public hit-test hierarchy is introduced. This changes no supported `0.2.0`
domain API, solver behavior, hit tolerance, constraint or dimension kind, canonical sketch v1-v4
bytes, unsupported draft-v5 disposition or persistence schema. This compatibility disposition is
accepted under the supervising caller's 2026-08-16 scoped M75 close decision against exact product
source `553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`. That decision accepts the candidate, focused
F001/F002 hover recheck and U1-U12 without claiming an individually logged replay of every UAT
step. Documentation-only approval descendant `f80235978fbcdccd58c45a08bccf3969a20110c9`
subsequently passes Pages run `31939764951`, artifact `9261974799` and deployment `5929879555`.
The exact public bytes and M72/M74/M75 browser contracts verify, completing this additive pre-1.0
interaction correction without changing the accepted compatibility boundary.

M76 is an additive pre-1.0 presentation extension in `geosolve-constraint-editor`.
`AnnotationLayoutKey`, `AnnotationPlacement`, `AnnotationLayoutEntry` and
`AnnotationLayoutState` expose bounded semantic placement state, while exact scene-annotation
geometry publishes the baselines, witnesses, leaders, arcs, arrowheads, label bounds and glyph
bounds shared by painting and picking. These DTOs add no equation, variable, residual, branch or
accepted-sketch mutation. The workbench's workspace-v6 annotation cache is demo-local, optional,
self-versioned and fail-soft; canonical sketch v1-v4, unsupported draft-v5, `GEOSOLVE_REPRO_V1`
and every accepted document/reproduction contract remain unchanged. A malformed or stale cache is
discarded while valid sketch state restores and deterministic automatic layout recomputes.
Shared-endpoint angle wedge selection and omission of the redundant Origin canvas marker are
presentation refinements only: the accepted oriented-angle value/branch and all intrinsic-Origin
picking, authoring, protection, tree and inspector semantics are unchanged. This additive
disposition is accepted under the caller's scoped M76 close decision without claiming a separate
post-refinement UAT replay. Final source `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
`248cba4509a992aeff7a02dd6d57a1a2481380a4`, passes GitHub Pages run `31961652265`, artifact
`9267811418` and deployment `5933831093`; root and all seven hosted files byte-match the artifact's
ordered-manifest aggregate `41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96`
at their expected media types. The separately built Tailscale candidate remains qualification
evidence rather than a claim of Pages byte identity. M76 is complete without changing the accepted
compatibility boundary.

M77 is an additive pre-1.0 curve-control and presentation extension. `DocumentCurveControlId`,
`DocumentCurveControlKind`, `DocumentCurveControlTarget`, `DocumentCurveControlAvailability`,
`DocumentCurveControlWithholdingReason`, `DocumentCurveControl`,
`DocumentCurveControlProjection` and `DocumentCurveControlError` expose a closed accepted-domain
control catalog and typed inverse projection. `DocumentRationalConicControlMode` and
`DocumentRationalConicControl` give nonzero Euclidean `P1` and zero-weight projective `Qh`
unambiguous state; `DocumentEdit::SetRationalConicControl` is the atomic numeric/mode edit, while
spatial movement retains the existing `SetConicWeightedMiddle` lowering. `PreparedSketchPreview`
adds only immutable candidate views to the existing opaque exact-CAS patch.

`SceneCurveControl*`, `CurvePropertyFamily`, `CurveNumericPropertyKind`,
`CurveNumericPropertyMetadata`, `SelectedCurvePropertyMetadata` and the corresponding
`ConstraintEditor`/`RetainedEditorCoordinator` population, preview, commit and property-setter
methods are presentation-independent host APIs. They add selected-only transient identities and
finite paint/hit geometry; they do not create persistent sketch points or constraint operands.
`DocumentTrimProjectionError::CrossesOppositeEndpoint` makes the existing non-periodic directed-
trim invariant explicit at projection time. Callers that exhaustively match these pre-1.0 enums
must handle the additive variants under the documented minor-release policy.

M77 changes no solver equation, residual, constraint or dimension kind, hard/soft priority,
rank/DOF rule, automatic branch policy, canonical sketch v1-v4 bytes, unsupported draft-v5
disposition, workspace/reproduction schema or annotation cache. Curve-control cages are recomputed
from accepted geometry and selection. Exact source
`cc99b11071dc62732e02b630ba7a1381d754b04c`, tree
`3315a2bdd0137f59657ea2500962ef971a23ea15`, passes the complete clean gate and immutable Tailscale
nomination. The supervising caller accepts U1-U6 and requests closure. Publication descendant
`66a89b7` passes Pages run `32012819635`, artifact `9283439225` and deployment `5942438795`; root
plus all seven hosted paths exact-verify at aggregate
`872719a0f4323f978bf31a4e567646b61a8bd607a2dbc384e47b676054979f15`. M77 is complete. This
approval and publication change no compatibility boundary.

The minimum supported Rust version is `1.89`. Raising it requires a minor release
before `1.0`, a major release after `1.0`, and a changelog entry.

## API tiers

The supported domain entry points are:

- `SketchDocument` and accepted-only `SketchDocumentSession` for persistent sketches;
- `RetainedSketchDocumentSession` for separate retained design, attempt and accepted views;
- `PlanarLinkageDocument` and `PlanarLinkageSession` for planar kinematics;
- `SpatialAssemblyDocument` and `SpatialAssemblyDocumentSession` for spatial
  kinematics;
- immutable geometry and accepted domain result/audit types returned by those
  workflows;
- `geosolve-sketch-ops` immutable snapshots, controlled prepared proposals and exact-input
  application for equation-free sketch operations;
- `geosolve-sketch-topology` complete accepted-input production-wire and region profiles; and
- `geosolve-sketch-features` persistent computed-feature intent plus independently validated,
  exact-stamped revision-local output; and
- `geosolve-constraint-editor` state, scene, normalized input and typed effect APIs for
  presentation-independent constraint, dimension and computed-feature authoring over those sketch
  workflows.

Legacy direct `Sketch`, `Linkage` and `SpatialAssembly` builders remain supported
compatibility facades in the `0.2` line.

Compiler products, runtime ID maps, direct `geosolve-core` reports and fixture or
performance builders are public for advanced diagnostics and verification, but are
explicitly unstable before `1.0`. They must not be persisted or used as application
identity. M29 has reviewed these exports and retains them intentionally because the
diagnostic consumer and independent audit tooling inspect the same validated report;
new application APIs should prefer persistent domain IDs and domain-owned views.

Public error and status enums may gain variants. Callers should include a wildcard
arm unless an enum is documented as closed. Public structs intended as reports may
gain fields in a minor `0.x` release. Request and persisted document types are not
extended silently within a schema version.

## Deprecation

A supported domain API is deprecated before planned removal. Deprecation includes:

1. a Rust `#[deprecated]` annotation with a replacement and target release;
2. an entry under `Unreleased` in `CHANGELOG.md`;
3. at least one minor release before removal in the `0.x` line;
4. removal only in a later minor release before `1.0`, or a major release after
   `1.0`.

Immediate removal is reserved for unsoundness, false-success paths or security
defects and must be called out prominently in the changelog.

## Persistence

Schema versions are independent from crate versions. Import always validates size,
syntax, IDs, references, finite values, geometry, branch state and the solved
candidate before publication. Unknown future versions reject atomically.

| Domain | Accepted input | Canonical output | Migration |
| --- | --- | --- | --- |
| Sketch | v1, v2, v3, v4 | v4 | Frozen old languages migrate directly to v4 |
| Planar linkage | v1 | v1 | None required |
| Spatial assembly | v1 | v1 | None required |

Canonical output is byte-stable for the same accepted document and schema version.
Runtime generational IDs never form persisted identity. A schema language is never
expanded after release; new fields or variants require a new schema version and a
frozen reader for each retained old version.

Any future sketch v5 transition must retain direct deterministic migration from v1-v4
and uses separately versioned host-parameter, immutable external-snapshot and desktop-
workspace envelopes. Host expressions, PDM keys, projection callbacks and application
undo are not added to canonical sketch equations. The current table remains the supported
contract until a future explicitly scoped schema milestone updates it.

The project supports reading every schema listed above throughout the `0.2` line.
Dropping an input schema requires a minor release before `1.0`, a major release
after `1.0`, a changelog entry and an external migration path. A migration that
cannot preserve explicit branch or ownership state must reject or retain the old
semantics; it must not infer a different branch from coordinates.

Planar and spatial v1 in-memory records are the frozen v1 language for `0.1.0`.
Before either model gains a new persisted field or variant, it must first be split
behind a private v1 wire DTO in the same manner as sketch persistence.

## Features and platform support

The release has no optional Cargo feature contract. Native Linux x86-64 and
`wasm32-unknown-unknown` are release-gated. Other Rust-supported targets are
best-effort unless added to a future release matrix. Linux, Windows and macOS release expansion is
not currently scheduled. No C ABI is in the currently approved roadmap. The WASM workbench is
not a separate product API and does not define document semantics; cleanup qualification
uses direct Rust/WASM tests rather than browser E2E, and there is no mobile or responsive
support contract.

## Publication

The publishable crates are released in dependency order:

1. `geosolve-geometry`;
2. `geosolve-core` after the matching geometry version is visible;
3. `geosolve-sketch` and `geosolve-linkage` after the matching core version is
   visible;
4. `geosolve-sketch-ops`, `geosolve-sketch-topology` and `geosolve-sketch-features` after the
   matching sketch version is visible;
5. `geosolve-constraint-editor` after the matching sketch and sketch-features versions are visible.

Cargo cannot create a registry-ready dependent archive before its path dependency
version exists in the registry. The pre-publication gate therefore checks the exact
archive file list for all eight crates and builds every workspace target from path
dependencies. Each package includes `LICENSE` and `README.md`. Registry publication
itself remains a maintainer action after a repository URL and release tag exist.
