# Changelog

All notable changes to GeoSolve are documented here. The project follows the
versioning and deprecation policy in `docs/API_COMPATIBILITY.md`.

## [Unreleased]

### Added

- Typed retained-design, solve-attempt and accepted-state identities/views for
  repairable unsolved sketch intent, optional finite candidate geometry and separate
  v1-v4 design/accepted persistence with host-owned revision high-water metadata.
- Cooperative cancellation and deterministic work-budget APIs with typed non-success
  outcomes and publication-time cancellation checks.
- Closed semantic relation, dimension and persistent-measurement catalogs through
  M36-M38, including bounded path-length work and independent evidence validation.
- The pure-Rust `geosolve-constraint-editor` state machine for normalized input,
  persistent selection/drafting state, accepted-scene projection and typed effects.
- Persistent construction/activity roles, revisioned typed host-parameter bindings and
  immutable external 2D snapshot/rebind contracts.
- A disposable, non-persisted M52 host-semantics UAT sidecar over the sole workbench,
  with deterministic typed evidence and direct owning-layer regressions.
- Structured current-attempt problem metadata at the headless-editor seam, including persistent
  element targets and an explicit global fallback for unattributable failures.
- Stable sketch-owned diagnostic snapshots with exact attempt/accepted provenance, persistent
  source/component/dependency identities, separate structural/numerical rank and
  equality/bounded/one-sided mobility, typed host-input failures and non-mutating repair hints.
- Complete headless alpha action DTOs for 13 relation families, five driving/reference dimension
  families, explicit contact span/domain/parameter/winding/neighborhood/orientation edits and
  oriented-angle branch changes with deterministic replay.
- Two reusable M55 scenario leaves for the accepted alpha family catalog and explicit
  branch/rejection recovery, under the existing recursive right-expanding scenario selector.
- Immutable complete-input sketch snapshots, typed prepared edit/reattempt/parameter/external
  jobs, non-mutating candidate patches and exact compare-and-swap publication.
- Safe host-managed concurrency metadata: single-owner session-bearing jobs/patches are `Send`,
  immutable prepared DTOs are `Send + Sync`, and single-threaded WASM uses the same synchronous
  prepare/execute/commit boundary.
- Persistent-ID-indexed runtime maps, reverse document dependency closures, revision-local
  accepted profile caches and stable incremental/full-rebuild execution evidence.
- An honest sketch production-scale assessment: sparse hard steps remain supported while
  numerical rank is dense-SVD authoritative within the 256-row/256-tangent component envelope.
- The separate `geosolve-sketch-ops` companion with complete-stamp prepared operation jobs,
  deterministic identity mappings, typed unsupported/incomplete outcomes and exact-CAS
  application through ordinary retained sketch transactions.
- Equation-free split/break/trim, line extension, exact supported-family mirror, line chamfer,
  public associative-fillet integration, rectangle, regular-polygon, slot and bounded
  linear-pattern expansions.
- Ordered non-overlapping multi-interval visibility over immutable curve supports, including
  exact fixed/contact boundary identity and ordinary point-on-curve-owned trim boundaries.
- The separate `geosolve-sketch-topology` companion with complete-input accepted snapshots,
  worker-movable controlled queries, explicit native/construction/external scope and bounded
  tangency/overlap/touching/T-junction/self-intersection policies.
- Independently checked production wires, outer/hole regions, certified orientation/area and exact
  native-visible-interval or immutable-external-line provenance. Only complete current results
  expose a consumable production profile.
- Direct advanced-workbench presentation for accepted all-family geometry, explicit periodic NURBS
  span/winding and knot edits, public companion operations, stable diagnostics and production
  topology.
- Four stable M61-preparation scenario leaves with right-expanding navigation, deterministic
  guidance/transcript/evidence and complete ordinary-workspace isolation.
- A version-2 desktop workspace envelope with explicit canonical-v4/draft-v5 document encodings,
  exact M58 multi-interval round trips and deterministic legacy-v1 migration.
- Ten movable nonzero-mobility mechanism scenarios from the preserved public alpha corpus,
  including scissor jack/tower and Peaucellier linkage, with preselected drivers, exact reset and
  ordinary-workspace isolation.
- Reusable headless and sole-workbench authoring for quadratic/cubic Beziers, ellipse/elliptical
  arc, rational quadratic conic, parabola, hyperbola and clamped/periodic NURBS with explicit
  conic/topology/gauge options and public-curve previews.
- Cursor-anchored wheel zoom, middle-drag pan, explicit zoom/Fit controls and scale feedback for
  ordinary and scenario canvas inspection.
- An opaque accepted-hard-nullspace `DocumentDragLocalityPlan` for gesture-local projected
  dragging. It exposes only passive-DOF and anchor-count evidence while keeping solver matrices
  and anchor policy inside `geosolve-sketch`.
- Controlled sketch-operation proposal application through ordinary retained transactions, so
  cancelled or exhausted scratch publication remains mutation-free as well as preparation.
- Reusable headless associative-2D-Fillet authoring with exact accepted-geometry picks, explicit
  branch/radius options, independently accepted scratch previews, recoverable radius hover and
  token/candidate-bound publication through one normal history edit.
- One ordinary editable **2D Fillet playground** for independent lines, line-circle,
  line-quadratic-Bezier, high-valence, multi-corner/sequential and short-middle conflict
  authoring, with no guide, protected state or alternate coordinator.
- ADR 0032 and the completed, supervising-human-approved M68 headless computed-Fillet direct
  manipulation cut:
  an analytic branch-preserving radius rail, explicit contact/retention/bounded-local-alternative
  actions, Current-only coordinator transactions, pointer capture and Rust-first qualification.
  The complete release gate and focused human UAT pass on approved candidate `edffb8a`.
- ADR 0033 and the completed, supervising-human-approved M69 Profile/Construction semantics cut:
  atomic curve-role authoring/conversion, role-aware operation output, exact evaluation-local
  Fillet-discarded construction provenance, shared headless pick/visibility policy and thin
  workbench controls. The complete release gate and focused human UAT pass on approved candidate
  `567141776c78178022f6123cbb399599ba713c62`.
- The implemented and focused-directly-qualified M70 ADR 0034 headless auto-constraint drafting
  cut: semantic native anchors, bounded stage-local wake/reference memory, ranked and hysteretic
  point/contact/midpoint/direction candidates, honest tracking-only guides, semantic suppression
  and one exact atomic construction-plus-existing-relation plan. Persistent point identity is
  reused structurally, candidate enumeration fails closed at hard 32-candidate/eight-reference
  ceilings, and one shared golden transcript qualifies native/WASM transitions. Retained sketch
  sessions additionally preserve field-opaque, checkpoint-serializable persistent-object and
  spline-span allocator high-water across history and application-workspace v5 reload. Cursor
  maps and commit plans are bounded, malformed cursor relationships and derived non-finite output
  reject transactionally, allocator-only changes invalidate prepared CAS, history restore preserves
  current host inputs, and Shift changes do not discard queued foreign-interaction movement. A private
  exact scene-semantics seal makes public mutation revoke inference-publication authority. One
  ordinary editable auto-constraint playground supports the focused human UAT. Historical initial
  candidate `4b16db3a885f5e28f508189b8817797375f05807` passed focused inference 46/46,
  266 editor unit tests plus every relevant integration suite, demo-web 82/82, the sketch library
  33/33, M56 6/6, the complete release gate and byte-verified Tailscale publication. Human review
  then opened Circle-authoring finding `M70-F001`. Replacement source
  `3d157896c87eaf647abee1192c838100ce359ce9` implements that Circle-through-point contract and
  passes focused inference 47/47, 271/271 editor unit tests plus every relevant integration suite,
  demo-web 83/83, sketch 33/33 and M56 6/6. Its complete release gate, frozen Tailscale publication
  and served-byte verification pass. The targeted human recheck and scoped UAT are accepted; the
  supervising human explicitly approved and closed M70 on 2026-08-10.
- The completed M70B reproduction-payload cut: deterministic single-line
  `GEOSOLVE_REPRO_V1` text over freshly encoded authoritative application-workspace v5 bytes, with
  zlib compression, strict unpadded URL-safe base64, canonical byte length, FNV-1a corruption
  detection and independent text/compressed/decoded resource limits. Decode remains
  non-authoritative until the ordinary strict workspace decoder and complete coordinator
  reconstruction both succeed. A narrow native stdin decoder exposes bounded workspace JSON for
  recipient-side diagnosis without publication authority. The sole workbench presents copy/paste
  in a visible overlay with a manual-copy fallback; transient UI/camera/history/sample state and
  legacy lab/raw-storage formats are excluded. Human payload finding `M70B-F001` corrected the
  mismatch between semantic-open Local contact intervals and closed effective core bounds without
  changing persisted branch metadata or strict validation. Replacement source
  `b4ec279e221df38816b7376a6978712e21df02c2` passes direct/release qualification, frozen Tailscale
  publication and served-byte verification. F002-F005 add the radial-Normal/accepted-scene,
  Coincident-closure Fillet, certified branch traversal and movement-continuity corrections at
  their owning layers. The canonical oracle is 198/198 `PASS`. Clean closing source `48e3cc3`
  passes the complete gate with final multi-feature transaction and finite-arc transport
  regressions and produces release bytes identical to the F005 candidate. M70B is closed under the
  supervising human's requested scoped sign-off.
- The completed M74 production-style reference UX cut. Every sketch exposes immutable intrinsic
  Origin/X/Y datums with no persistent identity, variables, history, geometry count or Fit
  contribution. Ordinary retained Origin coincidence, point-on-axis, line-collinear-with-axis and
  point-pair symmetry-about-axis relations have checked analytic Jacobians, structured audit,
  independent residual validation and normal lifecycle. Contextual authoring, pixel-bounded datum
  inference, protected selection, reference presentation, adaptive visual grid, camera/HUD/cursor,
  shortcut and letterbox behavior remain owned by the headless editor or thin demo adapter as
  appropriate. Canonical sketch v1-v4 stays frozen and rejects datum relations with
  `UnsupportedM74State`; draft-v5 side records remain unsupported. Product source `5569337` passes
  focused native/WASM owners, the reviewed 270-row golden, independent review, the complete clean
  gate and frozen-artifact verification. The supervising caller approved scoped closure on
  2026-08-16 while explicitly deferring, rather than passing, the hands-on U1-U8 scorecard into an
  unstarted bug-fixing/UAT follow-up milestone.
- The scoped-approved M76 production-quality annotation cut. Public editor DTOs now describe exact
  linear, radial, angular and compact-glyph paint/hit geometry, deterministic automatic placement
  and presentation-only manual offsets for all seven dimension and twenty constraint families.
  The demo's optional workspace-v6 annotation cache is fail-soft and remains outside canonical
  sketch/reproduction authority. After initial review, shared-endpoint acute/right line angles were
  refined to use their finite-ray interior wedge, and the intrinsic Origin retained all headless,
  authoring, protection, tree and inspector semantics while dropping its redundant canvas marker,
  label and focus target. This pre-existing angle-side behavior is treated as an M76 feature, not
  an `M76-Fxxx` defect. Feature commit `a9fd6f6` is included in final source
  `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
  `248cba4509a992aeff7a02dd6d57a1a2481380a4`, which passes the complete local release gate and
  exact immutable Tailscale verification at aggregate
  `967f0c1943c16b9c4a9975aeb973ad0cfe2c6e3dbfab45f414d0dac1bb9088f3`, then passes GitHub Pages
  run `31961652265`. Its
  `184.090683967s` sparse corpus remains below the enforced `240s` shared-runner ceiling after all
  semantic checks, while the preceding `209.696267408s` and `208.757508921s` attempts are retained
  as timing-only infrastructure history. Artifact `9267811418`, deploy job `95204687455` and
  deployment `5933831093` succeed; root and all seven hosted files exact-verify against ordered
  manifest aggregate `41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96` at expected media types.
  The caller accepted U1-U4 for scoped closure without claiming or requiring a separately logged
  post-refinement replay. M76 is complete.

### Changed

- Completed, approved and publicly verified M75 hover/pointer-owner parity. M75 consolidates Select
  hover prediction and primary pointer-down targeting behind one private
  headless resolver. The shared order is validated Fillet radius, draggable point/semantic-centre
  geometry, visible annotation occurrence, remaining native/computed geometry, intrinsic datum,
  then none. Problem-forced annotations now participate in pointer-move as well as pointer-down,
  exact annotation ties are deterministic, contextual corridors remain targetless, and stale hover
  is revoked when tool, selection/visibility, camera, accepted-scene, geometry-policy or non-canvas
  input ownership changes. M75-F001 routes uncaptured relation/dimension and grouped-Fillet
  movement to the same domain-aware compatible-candidate resolver as click, including wrong-kind
  overlap fallback and computed-preview radius authentication; captured Fillet-radius movement
  remains intact. M75-F002 reconciles the complete uncaptured Fillet paint stack with the exact
  headless computed-radius owner so a native item painted above the grip, rail or spoke cannot hide
  the promised radius interaction from hover or pointer-down. The workbench paints only the
  returned headless owner; a computed DOM item is an independently validated intent hint, and
  selectable canvas CSS/DOM hover cannot supply a competing semantic target. Existing hit
  tolerances, geometry-role ordering, equations, persistence bytes and golden behavior remain
  unchanged. Exact product source `553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
  `83df4efb99ca66cf0cebc0caec4515b61afd33cf`, passes the complete clean gate and immutable
  Tailscale byte verification. On 2026-08-16 the supervising caller accepted that candidate, the
  focused F001/F002 hover recheck and U1-U12 for scoped closure. The detailed UAT steps were not
  individually logged, so this disposition is not represented as a separate step-by-step replay.
  Documentation-only approval descendant `f80235978fbcdccd58c45a08bccf3969a20110c9` passes Pages
  run `31939764951` and deploys artifact `9261974799` through deployment `5929879555`. All seven
  public files byte-match the downloaded artifact, repository-prefixed URLs and media types are
  correct, and public M72/M74/M75 Chromium checks pass. M75 is complete.
- Completed, approved and publicly verified M73 retained-authoring semantic consolidation. One
  private `ConstructionStageSemantics` description now owns the remaining line/polyline
  stage/span/reference-handoff facts. The unreleased `ConstraintKind`,
  `ConstraintEditor::{available_constraints, constraint_edit}` and
  `EditorError::IncompatibleConstraint` compatibility surface is removed in favor of contextual
  `ConstraintIntent`/`ResolvedConstraintKind` authoring; all 20 contextual families remain, with
  14 simple families sharing one exhaustive lowerer and six contact-bearing routes retaining
  specialized branch construction. Private confirmation now carries and authenticates the exact
  selected inference candidate through guides, relations, references and commit lowering. That
  editor surface postdated published `0.2.0` and had no non-test direct caller, so no supported API
  received a shortened deprecation interval. Accepted product source `4c93ac5` passes the complete
  gate, focused approval and exact GitHub Pages artifact verification. No residual, solver, branch,
  persistence, public commit DTO or browser behavior changed.
- Completed and received supervising-human approval for M67 on 2026-08-08. The non-published
  workbench dropped its raw Production topology, Host-state evidence and Accepted redundancy
  developer cards while retaining Problems,
  canvas attribution and the reusable domain APIs/tests beneath those views. M50 had already
  removed the separately routed `/#/dev/lab` application. M68 subsequently completed the ADR 0032
  Fillet-interaction cut, and M69 subsequently completed the ADR 0033 Profile/Construction cut;
  both received explicit supervising-human approval on 2026-08-09. M70 subsequently completed ADR
  0034 headless auto-constraint drafting intelligence. Implementation, focused direct and
  integrated release qualification, frozen replacement-candidate publication and served-byte
  verification are complete. `M70-F001` is resolved and the scoped human UAT is approved. M70B
  subsequently completed its bounded reproduction-capsule work and requested scoped sign-off.
  M71 was then activated under ADR 0035 and now implements six ordinary retained drafting
  definitions: point-pair and native-span-midpoint Horizontal/Vertical, Concentric and Collinear.
  M71-F003 through M71-F006 resolve native-midpoint durability, endpoint-axis/direction bundling,
  distinct-reference orthogonal point-axis composition and overly broad default capture. Clean
  source `f8a45ae7b355ab9874bf268c9950e369814e8432` passes the complete gate and supplies the current
  byte-verified replacement. On 2026-08-14 the supervising human confirmed the corrected
  two-constraint auto-placement, accepted the scoped U1-U5 review and explicitly closed M71.
- Consolidated M68 close-off code and tests without changing accepted behavior: one workbench
  painted-action resolver replaces three copies, redundant routing/panel wrappers and brittle
  presentation literals are gone, and two manual radius-transition sequences are superseded by
  the retained exhaustive 240-transition model. Distinct feature, editor, coordinator,
  persistence and web regressions—including all five M68 findings—remain directly owned.
- Retired the unreleased, doc-hidden M40 browser-evidence qualification API and frozen JSON
  matrix/corpus/golden harness after moving every retained transition claim to a direct current
  test owner. This evidence-only surface postdated `0.2.0` and had no runtime consumer.
- Removed an unused private generic local-AD prototype, that prototype's unused normalized-tangent
  fused-storage branch, orphan workbench styles and other audited private/duplicate cleanup without
  changing solver equations, branch state, priority semantics or independently validated success
  publication. The stale M32 supporting-offset timing witness now verifies the edited endpoint
  instead of requiring incidental motion from its other free endpoint.
- Rebased the post-M44 roadmap: M45 preserves cleanup evidence without human approval;
  M46-M53 replace and purge legacy browser E2E/playground infrastructure, consolidate one
  directly tested workbench and perform post-cleanup host-semantics UAT. M53 received explicit
  supervising-human approval. At that checkpoint the later functional/release sequence was
  forecast as M54-M64, with a dedicated M55 alpha constraint/dimension/branch-action parity gate
  inserted before concurrency and scale; the roadmap-reset entry below supersedes that forecast.
- Closed M61 with explicit supervising-human approval for its recorded advanced-workbench scope.
  At that checkpoint the previously forecast M62-M64 hardening sequence was removed and M62 was
  left intentionally unscoped. M62-M64 were subsequently scoped, completed and individually
  approved. New milestones normally end in hands-on UAT; M74 records an explicit scoped exception
  that defers its unexecuted scorecard without calling it passed.
- Completed the M46 ownership freeze: every old M14/M40/M44 browser/static assertion and
  legacy inline test has a named direct-test owner or reviewed retirement, while no old
  fixture, E2E script or playground code was deleted early.
- Completed M47 with five direct host-semantics fixture groups and deterministic typed
  finding capture, then removed the broad M44 host fixture, fixture-only controls and
  `e2e/m44.mjs` browser qualification infrastructure.
- Completed M48 direct editor/workbench qualification and removed the M40 browser E2E,
  serving script, static scans and browser-only delivery checks.
- Completed M49 legacy semantic extraction with direct owning-layer coverage or explicit
  retirement for every M14 browser group and legacy inline test.
- Completed M50 by deleting the final M14 E2E/CDP/server stack, legacy playground route and
  runtime, hidden DOM/CSS, stale serving glue and release-gate browser invocation. One directly
  qualified workbench remains with pruned dependencies and WASM features.
- Completed M51 by removing the survivor's design-only storage migration, duplicate M40
  report/evidence fixtures and stale M32 distribution copy; one workspace snapshot and directly
  tested presentation, persistence, effect and typed-evidence transformations remain.
- Replaced the M52 candidate's one-off bottom launcher and overlay for M53 review with a
  reusable typed scenario catalog, a top nested **Scenarios** selector and a contextual guide
  sidebar. The original six scenarios preserve the same ten objective points, deterministic
  reset/evidence behavior and ordinary workspace isolation, with no browser-owned domain semantics.
  Nested groups now open as right-expanding hover/focus flyouts, with an inline narrow-screen
  fallback, instead of requiring a separate disclosure toggle at each level.
- Extended the M53 catalog to eight scenarios with attributed-conflict and global-input-error
  recovery examples. The accepted canvas now presents separate current-error highlights and
  accessible non-mutating markers while the Problems panel remains canonical.
- Consolidated the WASM consumer to one directly tested workbench and removed the legacy
  playground, routes, browser E2E, serving/download glue and browser-owned qualification path.
- Moved raw sketch core reports and bound reports behind explicitly named unstable compatibility
  seams; the headless editor and workbench now consume stable sketch diagnostics without
  interpreting runtime core IDs or audit enums.
- Completed M55 action-surface parity in the sole workbench. Applicability and disabled reasons
  remain headless; glyphs, dimensions and branch controls render typed domain/editor metadata;
  the deleted playground, `/#/dev/lab`, browser E2E and legacy harnesses remain absent.
- Completed M56 prepared concurrency without changing equations or accepted-state validation.
  Stale, out-of-order, cancelled and work-exhausted worker results cannot publish over newer
  session state; no `unsafe`, solver mutex, internal scheduler or schema change was added.
- Completed M57 dependency-local retained solving. Compatible parameter, external-reference,
  activation and geometry attempts retain runtime/core identities and clean-component caches;
  source-shape/topology changes use an explicit full rebuild, and all paths still perform fresh
  hard-row, derivative, domain/branch, projection and rank validation before publication.
- Completed M58 without adding residuals, solver/session ownership or a private commit path.
  Unsupported exact transforms are never sampled into approximations; stale, cancelled,
  exhausted and foreign-input proposals cannot mutate the live session. Canonical sketch v4
  remains frozen and rejects M58-only topology pending a future explicitly scoped schema freeze.
- Completed M59 without promoting visual-profile output directly or adding B-rep state.
  Candidate evidence is independently checked for declared-source coverage, parameters,
  endpoints, closure, orientation, area and output limits. Stale, cancelled, exhausted,
  truncated, ambiguous and uncovered-source results cannot be consumed as production topology.
- Completed M60 without changing equations or restoring a legacy harness. The sole workbench now
  consumes public operations/topology companions directly, labels only complete current production
  profiles consumable, retains the full M55 action surface and all ten prior scenario identities,
  and was directly qualified for the subsequently approved M61 human UAT.
- Replaced the withdrawn first M61 candidate after five human blockers. Active scenario selection
  and projected drag now target the rendered ephemeral coordinator; recursive third-level desktop
  flyouts no longer clip; invalid advanced construction remains atomic. No deleted playground,
  `/#/dev/lab`, browser E2E or legacy UAT harness returned.
- Fixed `M61-F001`: twin-roller cam drag now preserves the non-dragged roller through a
  coordinator-owned transient stability target in either direction, preventing independent-DOF
  jumps and the associated interaction lag.
- Fixed `M61-F002`: dynamic contact/relation selectors now fall back to the first current
  headless-provided choice instead of restoring an empty or obsolete option, allowing untouched
  point-on-circle contact defaults to dispatch.
- Fixed `M61-F003`: projected WASM pointer moves now retain only the latest pending sample per
  animation frame and flush it at most once before release, preventing expensive contact solves
  from accumulating a stale-event backlog. Exact payload replay preserves the truthful ambiguous
  contact rejection and demonstrates recovery on a small projected retry.
- Fixed `M61-F004`: the host-state sidebar no longer performs legacy full visual-profile analysis
  during every synchronous render. It now reports cheap accepted geometry-role declarations and
  leaves consumability to the qualified production-topology companion, eliminating the supplied
  workspace's startup and post-interaction render lock.
- Fixed `M61-F005`: compact circle Tangent now unambiguously means shared contact plus tangent
  alignment, while Perpendicular / Normal constrains the circle or arc centre onto the selected
  line. Direction-only line/curve Parallel/Perpendicular dispatch is no longer exposed, and a
  reusable circle-relations scenario demonstrates both geometric meanings.
- Completed M62 with selection-sensitive CAD-style relation/dimension authoring owned by the
  headless editor, including repeated authoring modes, acute-degree angle seeding and direct
  coverage of every exposed resolution path.
- Completed M63 with stable geometry-anchored constraint/dimension annotations, occurrence-level
  proximity and picking, contextual hover corridors, density fan-out, right-angle geometry and
  shared accessible CAD icons.
- Completed M64 by reducing samples to ordinary editable workspace documents grouped by purpose,
  removing guide/reset/transcript/read-only behavior and directly qualifying representative
  one-, two- and three-DOF mechanisms.
- M65 replaces the M61 twin-roller special case with sample-agnostic locality planning from the
  independently accepted hard nullspace. The cursor is the sole Temporary target; deterministic
  frozen PreviousState anchors cover only passive mobility, and accepted previews continue and
  release without a sample-owned driver or retry policy. Each non-stale sample performs one
  bounded retained attempt, rejected/exhausted work keeps the full last accepted preview, stale
  results are no-ops, circle handles preserve their pointer offset and exact release uses the same
  finite operation envelope.
- M65 independently certifies Hard publication and, on the single-component dense path, preserves
  every attained positive Temporary row through Preference work within
  `max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`. The
  reproducibility floor does not relax Hard or Temporary acceptance.
- Geometry-dependent sketch operations require accepted geometry compatible with the current
  retained publication input, not merely the same design. The domain comparison ignores only a
  transient one-shot `candidate_request` after a successful point edit while matching publication
  policy, inputs and attempt/accepted identity; genuinely stale/rejected work and exact proposal
  compare-and-swap remain strict.
- Closed M66 on 2026-08-08 with explicit supervising-human approval of its mechanically qualified
  computed-Fillet scope. U1-U5 are accepted under that scoped close decision, while
  `M66-PF001` through `M66-PF004` are mechanically closed by direct regressions rather than claimed
  as individually repeated human tests. Accepted limitation `M66-KL001` records that radius drag
  measures from the held/old arc center while evaluation moves center/contacts and that
  post-placement contact/root, retained-parent direction and alternate-arc controls remain
  unintuitive, especially near the line-circle playground's radius-`0.5` branch fold. Numeric
  editing, explicit persisted branch state, independent validation, rollback and sketch-state
  invariance remain correct. Potential grip-rail/derivative and explicit branch-control work is
  unassigned and is not M67 scope.
- Narrowed M66 from the unapproved three-tool candidate to exceptionally polished associative 2D
  Fillet authoring. Fillet interaction policy remains headless, local synthesis remains bounded
  and branch-explicit, invalid exploratory hover retains both parents, hover/click share one
  preview-aware acquisition path and controls use a canvas overlay. Affine line/polyline contacts
  use full `Interior` support; a line/curved pair persists an outward-rounded certified `Local`
  cell that cannot cross a tangent-parallel barrier over the complete bounded span or one explicit
  period. Two non-affine-parent authoring is typed unsupported pending pairwise continuation, while
  M28's underlying all-family generic Fillet API remains unchanged. Build-source commit `c1b0336`
  passed the full native, warnings-denied, WASM and release qualification gate; human M66 UAT was
  still open at that checkpoint.
- Corrected the M66 post-Apply lifecycle with one tested host completion handoff: successful Fillet
  publication exits the headless collector and explicitly restores ordinary Select, while a failed
  Apply attempt re-arms collection. A default Reference Fillet is immediately draggable through
  its semantic center both before and after deleting its non-driving radius dimension. Replacement
  build-source commit `ff15c78` passes the complete native, warnings-denied, WASM and release gate.
- Corrected Fillet accepted-state coherence after UAT showed that a newly authored Fillet could
  immobilize both itself and every parent-polyline point. Branch-safe endpoint derivation now
  exact-synchronizes only active Fillet-owned angle coordinates, freshly certifies hard/rank/
  bound/secondary/diagnostic/audit evidence and requires bit-exact problem/report/materialized
  state before publication. Independent Fillet tangency validation now scales by model scale rather
  than radius. Replacement build-source commit `87e72b3` passes the complete native,
  warnings-denied, WASM and release gate plus the exact pointer-authored UI lifecycle.
- Added `M66-PF003` on candidate `02649cc`: the stable Fillet sample is now a focused editable
  playground, and SVG canvas interaction suppresses native browser text-selection/element-drag
  defaults without affecting the sibling Fillet options or other HTML. Direct Rust qualification
  and the full native, warnings-denied, workspace, WASM and release gate pass; no browser E2E is
  restored or claimed. Human M66 UAT was still open at that checkpoint.
- Added `M66-PF004` on candidate `ac31791`: painted computed-preview arcs now own radius presses
  ahead of overlapping native support collection through stable item metadata, exact held-preview
  and scene-provenance validation, and an independent owner hit. Stale/foreign hints and a second
  radius press reject state-neutrally, modifier keys cannot toggle the radius owner away, and a
  direct overlap regression covers the surviving gesture through move/release. The full formatting,
  warnings-denied workspace Clippy/test, WASM and release Trunk gates pass; the candidate was
  HTTP-verified over Tailscale before the scoped close decision above.
- Preserved the superseded Fillet/Offset/Mirror candidate for historical inspection at
  `origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`). It is not an active or approved
  M66 candidate.

### Removed

- Withdrew the unreleased `DocumentSolveRequest::stability_target` field and
  `with_stability_target` helper before the next published minor release. They encoded a second
  sample-selected Temporary target and are superseded by the opaque accepted-state
  `DocumentDragLocalityPlan`.
- Removed the discarded alternate-assembly search/proposal UI and its two branch-only samples
  from the reduced M65 candidate. Explicit branch state remains authoritative; automatic branch
  search is not part of M65.
- Removed the unreleased M66 Offset/Mirror authoring states, palette/options/icons, samples and
  authoring-only tests, plus the M66-only single-span/joined-chain line-offset request APIs. The
  completed M25 signed Offset constraints and M58 exact supported-family Mirror
  operation-companion API/history are intentionally preserved.
- Removed the unreleased, superseded ADR 0030 editor `OperationAuthoring*` facade, its coordinator
  operation-preview/replay/DTO/state slice, `ReplayAction::Operation`, editor-side
  `operation_authoring_input()` and the editor's now-exclusive direct `geosolve-sketch-ops`
  dependency. This source-breaking cleanup removes no published `0.2.0` API and preserves M27/M28
  Fillet equations/associations/trim views/persistence/migrations, M58
  `SketchOperationRequest::AssociativeFillet`, M25 Offset, M58 Mirror, the branch-cell certificate,
  M62 constraint authoring and all ADR 0031 computed-feature behavior.

## [0.2.0] - 2026-07-22

Post-expansion sketch preview and release hardening.

### Added

- Interactive public-API UAT for associative offsets, mirrors, directed angles,
  generic fillets and advanced NURBS editing.
- Certified read-only visual-profile analysis across all 15 built-in planar curve
  families, including self-intersections, bounded curved area and containment.
- Deterministic `GEOSOLVE_SCENE_V1` diagnostic capsules with canonical sketch JSON,
  exact profile budgets, checksum and atomic retained-state import failures.
- M32 command/profile mutation coverage and native/browser performance/resource
  envelopes.

### Changed

- Explicit accepted-contact topology and root-isolation retries harden movable
  fillet closure and NURBS self-intersection behavior.
- Cycle-area integration apportions the unchanged uncertainty target across directed
  fragments before independently validating the complete cycle.
- The release gate now includes M32 mutation/performance suites and an unfiltered,
  no-retry desktop browser run.

## [0.1.0] - 2026-07-21

Initial supported preview release.

### Added

- Pure-Rust normalized nonlinear solving with independent residual validation,
  component-local rank and mobility, strict hard/temporary/preference priority,
  sparse hard steps, bounds, diagnostics and persistent sessions.
- Persistent 2D sketch documents covering analytic curves, Beziers, conics,
  B-splines, NURBS, generic contact/tangency, differential constraints,
  associative constructions, visual profiles, generic fillets and trim views.
- Persistent planar and spatial linkage/assembly models with explicit modes,
  gauge-separated mobility, continuation and validated velocity queries.
- Canonical sketch JSON v4 with v1-v4 input migration, planar linkage JSON v1
  and spatial assembly JSON v1.
- Separate WASM diagnostic playground consuming the public domain APIs.

### Compatibility

- Rust `1.89` is the minimum supported Rust version.
- This is a `0.x` preview. Domain workflows and persisted schemas follow the
  compatibility policy; low-level compiler/runtime inspection remains unstable.

[Unreleased]: docs/API_COMPATIBILITY.md
[0.2.0]: docs/API_COMPATIBILITY.md
[0.1.0]: docs/API_COMPATIBILITY.md
