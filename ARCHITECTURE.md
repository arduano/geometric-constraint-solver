# Architecture

## 1. Product boundary

GeoSolve is a pure-Rust library for two products built over one numerical kernel:

1. production-capable 2D CAD sketches with editable analytic and parametric curves, dimensions, contact, tangency, continuity, persistence and truthful diagnostics;
2. position- and velocity-level 2D/3D rigid-body kinematics for linkages and CAD assemblies, including explicit assembly modes, continuation, persistence and truthful mobility.

The products share numerical machinery, not a domain object model. `geosolve-sketch` and `geosolve-linkage` remain separate frontends over `geosolve-core`.

M10-M14 are the completed 2D Sketch Playground Alpha cut toward the first product. They establish reusable sketch editing APIs and exercise them through a disposable browser consumer. Alpha completion is not completion of the production 2D CAD deliverable.

This is not a solid modeller, B-rep kernel, mesher, production renderer, collision
engine, statics solver, dynamics engine or global polynomial-root enumerator. Mass,
inertia, force, reaction, friction, impact and time integration are outside the currently approved
roadmap.

## 2. Status of this document

M0-M7 are the frozen domain baseline. M8 accepted the target contracts, M9 implemented component-local linearization, local AD, status and numerical-rank contracts, M10 implemented persistent sessions, bounds and the first sketch consumer, M11 implemented the persistent sketch document, commands/history and JSON/remapping layer, M12 implemented immutable curve jets, editable Beziers and generic curve contact/tangency plumbing, M13 implemented the disposable browser playground over those public APIs, M14 hardened its exact scenarios, failure recovery and performance gates, M15 implemented shared planar/spatial manifold state plus accepted hard sensitivity, M16 implemented sparse hard steps, structural matching, coupled hierarchy and robust planar continuation, M17 implemented persistent gauge-separated planar linkage sessions plus shared velocity linearization, M18 established the independently validated spatial slice, M19 added conics, M20 completed the common spatial joint/mate and position-driver catalog, M21 added locally supported non-rational B-splines, and M22 completed gauge-separated NURBS plus advanced CAD differential constraints. Statements are therefore marked as:

M23-M31 subsequently complete spatial kinematics, sketch embedding identities,
advanced constructions, generic fillets and persistent trim views, interactive
construction/NURBS UAT and certified all-family visual profiles.

- **Baseline:** implemented and accepted behavior through M76. M44 completes focused host-state workbench integration over the M33-M43 production contracts. M45 preserves ten UAT points and inventories the old UI/tests without recording human approval; M46 freezes direct ownership; M47 replaces the broad host composition with five direct fixture groups and removes its controls and M44 E2E infrastructure; M48 directly qualifies the surviving workbench contracts and removes the M40 browser stack; M49 moves every retained M14/legacy semantic claim to a direct owner or reviewed retirement; M50 deletes the final old E2E, legacy route/application and obsolete browser/serving glue; M51 consolidates persistence, evidence, presentation and tests around the one survivor; M52 adds and directly qualifies the disposable in-memory UAT sidecar without product fixture state; M53 receives explicit supervising-human approval; M54 publishes stable persistent-ID diagnostics and moves raw core reports behind explicitly unstable seams; M55 completes the preserved alpha relation, dimension and explicit branch-action surface in the headless editor and sole workbench; M56 adds immutable prepared snapshots, worker-movable jobs, non-mutating patches and exact-input compare-and-swap publication; M57 retains compatible runtime/core state, dependency-local dirtying, revision-local profile caches and bounded rank/scale evidence; M58 adds the equation-free deterministic operations companion and multi-interval visible-support topology; M59 adds the read-only production-topology companion with exact accepted-input provenance and fail-closed completeness; M60 exposes the advanced curves, explicit NURBS branches, companion operations, production topology and versioned desktop workspace through the sole directly tested workbench; M61 completes approved supervising-human advanced geometry/topology UAT after targeted remediation; M62 completes approved CAD-style constraint and dimension authoring; M63 completes approved geometry-anchored canvas constraint and dimension presentation; M64 completes the approved editable sample-library cleanup and 1/2/3-DOF fixture cut; M65 completes approved predictable, bounded projected dragging; M66 completes the approved computed-Fillet feature cut; M67 completes the approved legacy-surface and frozen-harness cleanup; M68 completes the approved ADR 0032 Fillet direct-manipulation cut; M69 completes the approved ADR 0033 Profile/Construction semantics; M70 completes approved ADR 0034 headless auto-constraint drafting; M70B completes bounded workspace reproduction handoff; M71 completes approved retained drafting relations; M72 completes public-workbench fixes and Pages delivery; M73 completes retained-authoring consolidation; M74 completes intrinsic reference geometry and production-style desktop polish under an explicit scoped close decision that defers its hands-on scorecard into M75; M75 completes hover/click ownership parity under scoped approval and exact public verification; M76 completes production-quality annotation geometry, placement, persistence and final presentation refinements under explicit scoped approval. M1-M7 remain the frozen regression baseline.
- **Active target:** M77 implements selected-curve trim, size and ordinary/projective control
  affordances plus exact curve properties through public accepted-domain projections and
  prepared-patch previews. Clean release qualification and immutable served nomination pass;
  human UAT and publication remain open. It changes no solver equation, branch heuristic or
  persistence schema. `docs/M77_GOALS.md` owns the contract.
- **Completed target:** M66 adds a separate computed-feature domain for ordinary multi-corner 2D
  Fillets under ADR 0031. Persistent `FilletSet` intent and stable provenance live outside the
  sketch constraint graph; generated arcs/fragments are evaluated from one exact accepted sketch
  snapshot and have revision-local IDs. The headless editor owns grouped authoring, the retained
  coordinator owns exact sketch/feature publication, and the sole workbench remains a thin
  adapter. The qualified but unapproved solver-owned UI source is preserved at
  `origin/archive/m66-associative-fillet-2026-08-07` (`1034afc`). M27/M28 and M58 associative-
  Fillet APIs remain advanced/backward-compatible behavior with no automatic migration. M66 adds
  no Offset implementation or profile consumption. Implementation and mechanical qualification
  pass on painted-preview-routing candidate source `ac31791`, which extends editable-playground
  source `02649cc` with explicit computed-preview pointer ownership and state-neutral invalid-
  intent rejection. The supervising human explicitly approved and closed the mechanically
  qualified scope on 2026-08-08, accepting `M66-KL001` as a deferred interaction limitation
  without claiming a complete post-PF004 scripted replay.
- **Completed cleanup:** M67 removed the sole workbench's raw Production topology, Host-state
  evidence and Accepted redundancy developer cards, the frozen M40 browser-evidence/transition
  harness and audited unused private code. M50 had already deleted the separately routed
  `/#/dev/lab` application. Reusable topology, lifecycle, redundancy, diagnostic and audit APIs
  remain directly owned below presentation. M67 received explicit human UAT approval on
  2026-08-08.
- **Completed target:** M68 implements ADR 0032's headless computed-Fillet direct manipulation:
  absolute same-branch continuation, an analytic one-dimensional radius rail, explicit bounded
  contact/retention/local-alternative actions, Current-only coordinator transactions and the thin
  pointer-capture/rendering foundation needed by them. Implementation and focused direct
  qualification, the complete release gate and focused human UAT are complete. The supervising
  human explicitly approved M68 on 2026-08-09.
- **Completed target:** M69 implements ADR 0033's Profile/Construction geometry semantics. Persistent
  Construction remains curve-scoped and solver-active; computed Fillets publish discarded source
  complements as evaluation-local implicit Construction with exact native provenance. Headless
  selection scopes and role-aware authoring own the interaction policy. Implementation, complete
  release qualification and focused human UAT are complete; the supervising human explicitly
  approved M69 on 2026-08-09.
- **Completed target:** M70 implements ADR 0034's reusable headless auto-constraint drafting
  intelligence. Semantic anchors, stage-local reference memory, ranked prospective bundles and
  atomic construction-plus-relation commit belong to `geosolve-constraint-editor`; the browser
  remains a thin renderer/event adapter. The target uses existing retained constraint primitives
  only. Implementation, focused direct qualification, integrated release qualification, frozen-
  replacement-candidate publication, served-byte verification and scoped human UAT are complete.
  Circle-authoring finding `M70-F001` is resolved; the supervising human explicitly approved M70
  on 2026-08-10.
- **Completed target:** M70B adds a bounded reproduction-payload transport around the sole
  workbench's authoritative v5 snapshot. `GEOSOLVE_REPRO_V1` is deterministic zlib compressed,
  strict unpadded base64url text with an FNV-1a corruption checksum; decoded bytes still pass the
  ordinary strict workspace decoder and full coordinator reconstruction before an atomic live
  swap. A visible copy/paste overlay provides manual-copy fallback. `M70B-F001` additionally
  corrects existing Local open-branch lowering by placing its effective closed core endpoints one
  representable value inside unchanged semantic metadata. `M70B-F002` keeps radial Normal in the
  headless coordinator as centre-on-complete-supporting-line authoring with a unique retained-
  accepted-geometry projection seed that never reads rejected coordinates, and keeps older
  accepted geometry renderable but detached beneath a
  rejected design without weakening current computed-scene fail-closed publication. F001
  replacement evidence, F002 direct regressions, the F002 complete replacement gate and
  byte-verified publication all pass. Test-only M70B-H1 adds no runtime behavior: it drives every
  resolved constraint/dimension family through the existing headless/retained boundaries and
  directly inventories the four reachable scene-authority states. Its 193-row fixed-seed golden,
  complete release gate and byte-verified replacement publication are clean; targeted human
  recheck remained pending at that historical checkpoint. M70B-H2 gives the unchanged matrix
  milestone-neutral names, makes its
  clean mode a mandatory release step and installs the repo-local layered defect workflow; it adds
  no runtime layer. Test-only M70B-H3 preserved the original 193 H1/H2 rows byte-identically and
  added four process-isolated `feature.fillet` rows. Its historical 197-row checkpoint recorded
  193 `PASS` plus four reviewed `DEFECT` rows, split evenly between `M70B-F003` and `M70B-F004`;
  `--check` passed while `--require-clean` intentionally failed, with no production/runtime or
  release-byte change. Authorized production repairs now make all four stable rows `PASS` without
  changing their input fingerprints. F003 uses active explicit Coincident equivalence for Fillet
  topology; F004 lets persisted circular-plus-affine Fillets traverse their complete certified
  explicit tangent-orientation cell while retaining generic nonlinear and radius-continuation
  locality guards. F005 handles native affine-source rotation without making a stale certificate
  edge a branch boundary: only a persisted-evaluation `NoLocalRoot` may search the retained
  circular support, and fresh seed/candidate certificate overlap must prove one unique transverse
  root remains connected. Accepted movement then refreshes only the derived contact-frame metadata
  under exact prior-preview/edit provenance; it does not change branch semantics, radius
  continuation, generic nonlinear parents or true barrier behavior. Projected dragging publishes
  native and computed geometry atomically, retaining the last complete scene at a genuine limit.
  Exact payload `4228:0823d31f269300af` is frozen by
  owner regression `m70b_f005_line_circle_source_rotation_transports_persisted_branch_cell` and
  golden row `feature.fillet.evaluation.line-circle.source-rotation.retained-start` at
  `input-04658a77db2dc779`. The M70B closing 198/198-`PASS` golden has SHA-256
  `bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Focused owner/golden
  tests, aggregate survey/check/clean modes, formatting, warnings-denied all-workspace Clippy,
  locked all-feature workspace tests and the relevant WASM check pass. Clean F005 source
  `d400c4a8201f6afc531f5b504424d6430dbf3937` passes
  `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`, including the
  152.49-second 256-moving-body sparse crossover. Its immutable seven-file snapshot
  `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` was byte-verified and served at
  `http://100.94.63.83:8080/` for M70B, with ordered-manifest aggregate
  `3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`; that server has since
  retired. The targeted movement
  behavior was reported fixed. Clean closing source `48e3cc3` passes the complete gate with focused
  multi-feature transaction and finite-arc transport regressions; the 198/198 golden and release
  bytes remain unchanged. M70B is closed under the requested scoped sign-off.
- **Completed target:** M71 implements amended ADR 0035's retained drafting relations. Point-pair
  Horizontal and Vertical, native line/polyline midpoint-axis Horizontal and Vertical,
  semantic-center Concentric and native affine-support Collinear join the ordinary
  `DocumentConstraintDefinition` lifecycle. The midpoint-axis family owns one linear hard row per
  axis with independent validation; the original four reuse existing runtime mathematics. Frozen
  canonical v4 is first isolated behind private wire DTOs; unsupported draft v5 owns the complete
  side-section round trip. The headless editor owns contextual authoring and bounded M70 inference
  extensions. M71-F004 gives remembered point/native-midpoint axes their own candidate identity so
  one may compose with a complementary exact Cartesian line/polyline direction at the exact
  coordinate intersection. M71-F005 extends that identity with a second point-tracking component:
  orthogonal Horizontal and Vertical axes from distinct remembered stored-point references may own
  one exact endpoint intersection, while one reference cannot masquerade as point identity through
  two redundant axes. M71-F006 changes only `DraftInferenceTolerances::default()` to inclusive
  `6/9 px` point/midpoint, `8/12 px` curve and `3/5 degree` direction enter/leave thresholds;
  explicit custom policy remains authoritative. Same-axis, same-reference, oblique, ambiguous and
  exhausted evidence remains fail-closed. Clean source `f8a45ae7b355ab9874bf268c9950e369814e8432`
  passes the complete release gate and its seven-file replacement is byte-verified through the
  Tailscale-only endpoint. F003/F004 evidence remains historical; the supervising human accepted
  the scoped U1-U5 review and explicitly closed M71 on 2026-08-14.

A target statement must not be exposed as an implemented capability before its milestone gate passes.

`PLAN.md` owns current execution numbering. Milestone labels in the preserved M8
completion record and in ADRs accepted before the playground rebaseline describe the
allocation at acceptance time; their architectural decisions remain accepted, but
current accepted ownership is the completed M10-M76 sequence, including the completed M70B reproduction cut,
listed in section 15.

## 3. Crate responsibilities

### `geosolve-geometry`

Owns pure immutable numerical geometry:

- 2D and 3D points, vectors and validated frames;
- planar curve evaluation and regularity/domain metadata, including validated clamped and periodic B-splines/NURBS with local basis jets, homogeneous refinement and differential geometry;
- `Pose2`, `Pose3`, `SE(2)` and `SE(3)` operations under ADR 0006;
- angle wrapping/unwrapping, normalization and degeneracy-safe helpers.

It does not know about variable IDs, constraints, iterations, design entities or rigid-body topology.

### `geosolve-core`

Owns domain-independent numerical infrastructure:

- stable runtime IDs for variable, residual and source blocks;
- packed ambient state and normalized tangent coordinates;
- residual incidence, category, scaling and structured audit metadata;
- canonical component-local linearization and analytic/local-AD adapters;
- fixed/alias elimination, decomposition, dense and sparse assembly;
- strict hard, temporary and preference hierarchy;
- nonlinear iteration, factorization, rank and diagnostic policy;
- persistent solve sessions, bounds, active sets and validated transactions;
- a doc-hidden exact accepted-state synchronization boundary for domain-derived coordinates that
  rebuilds hard/rank/bound/secondary/diagnostic/audit evidence before atomic commit;
- continuation primitives and complete solve reports.

It does not contain CAD entities, curve-definition variants, rigid bodies, joints, mates, branch labels or persistence schemas from either domain.

### `geosolve-sketch`

Owns the 2D design graph:

- public `SketchDocument` and `SketchSession` workflows;
- persistent design points and typed design scalars;
- closed, versioned built-in curve definitions;
- semantic features, dimensions, contacts and constraints;
- explicit branch, span, winding, tangent-orientation and contact-neighborhood state;
- typed commands and accepted-command undo/redo history;
- versioned JSON serialization, strict import and deterministic runtime remapping;
- compilation into core residuals, validators and commit mappings;
- source-level audit and persistence mappings.

The frozen baseline includes points, segments, circles, oriented arcs and the M5/M7 constraint corpus. M10 adds the session consumer; M11 now migrates baseline entities and editing into the persistent generic design graph with opaque document IDs, strict JSON, deterministic lowering, accepted-state projection and accepted-only command history; M12 adds editable quadratic/cubic Bezier curves and generic point/contact/tangency plumbing. The M10-M14 alpha geometry surface is point, line/polyline, rectangle command macro, circle, circular arc and quadratic/cubic Bezier. M19 adds conics, M21 adds clamped and periodic non-rational B-splines, and M22 adds gauge-separated NURBS with stable semantic spans, explicit knot sides/winding, local-support incidence and transactional homogeneous refinement. M25 adds separately named supporting-line and exact translated-segment offsets, ordinary-constraint point-defined mirrors and coordinated mirrored B-spline refinement under ADR 0020. Its reusable constraint surface includes the baseline corpus plus generic contact/tangency, tangent and sided-normal direction, signed or branch-explicit magnitude curvature, and ordered endpoint G0/G1/G2/rate-explicit parametric C2 continuity, with driving/reference dimensions and explicit discrete branch state.

M22 completes the production 2D CAD curve and generic differential-constraint surface. M25 extends its construction and persistence layer without introducing a mirror residual: rectangles and mirrors remain command expansion into ordinary geometry and constraints. M27 adds the associative line-fillet foundation, and M28 generalizes it to every regular curve family through common jets while adding one persistent equation-free visible interval per stable support span under ADR 0023.

M26 adds a separate read-only visual line-profile layer under ADR 0021. It reads
accepted line/polyline geometry and explicit coincidence topology, creates only
ephemeral crossing fragments and publishes bounded contour provenance with typed
completeness. Its output is not a sketch entity, region, solver source or
persistence/history record.

M28 visible-interval APIs are the authoritative consumer boundary for rendering,
hit testing, contact visibility and line-profile analysis. Trim views do not rewrite
support definitions or spline controls and add no equation rows. Suppression freezes
accepted intervals; explosion retains fixed intervals. One support span has at most
one visible interval, so arbitrary multi-fragment trim topology remains out of scope.

M30 exposes completed construction and NURBS behavior through reusable scenario
builders plus browser controls that submit public document transactions. Scenario
instructions, selection state and control widgets remain non-authoritative web
state. Every advertised free lab has a tested projected motion; no browser formula
constructs accepted geometry.

M32 diagnostic scene capsules wrap canonical sketch JSON and explicit analysis
budgets in a checksummed compressed text envelope. Capsule import still enters only
through the public document-session JSON solve/validation path. The envelope is a
private disposable-browser interchange format, not a domain schema or accepted-state
shortcut.

M70B does not restore that deleted capsule or its lab. Its new `GEOSOLVE_REPRO_V1` transport wraps
freshly encoded complete application-workspace v5 bytes, including design/accepted document
payloads, computed-feature intent and allocator/revision high-water already owned by
`WorkspaceSnapshot`. It adds no solver or document authority: transport decoding is followed by
strict workspace decoding and complete coordinator reconstruction, and the live workbench changes
only after all three steps succeed. The text, compressed stream and decoded workspace are bounded
independently. Transient tool/selection state, camera, sample identity and command history are
deliberately absent.

For M70B-F003, `SketchDocument::point_coincidence_representatives` exposes one deterministic
representative per persistent point from the transitive components of active explicit Coincident
constraints. Suppressed constraints do not join components, and equal or nearby coordinates never
imply topology. This is semantic topology metadata for consumers such as computed-feature
authoring; it adds no residual, persistence field or coordinate-based branch inference.

M31 supersedes the line-only geometry scope of ADR 0021 under ADR 0024 while
preserving its visual-only boundary. Family-specific linear, circular, polynomial,
analytic-conic and rational/spline pieces provide bounded intersection and integral
enclosures. A component is complete only when every relevant root, outgoing tangent
order, area sign and containment decision is resolved within explicit budgets.
Sampling is rendering only and cannot prove arrangement topology.

M33 accepts the production-embedding identity, host-input, operation-control and
companion-boundary contracts without adding target behavior. M34 implements retained
design intent, attempted candidates and accepted solved state as separate typed views.
M35-M43 continue the implementation transition with a standard planar
relation/dimension surface, typed construction/activation semantics, host-supplied parameter values,
immutable external 2D snapshots and cancellation. M54 owns stable persistent-ID diagnostics;
M55 consumes the public domain/action contracts through the headless editor without adding a solver
equation; M56 completes revision-checked prepared jobs and M57 completes incremental solving. Host expressions,
projection and application history remain outside this crate. One solve attempt consumes immutable
input revisions and never calls host code.

M77 keeps inverse configuration and storage truth here. `DocumentCurveControl*` enumerates stored
point aliases and derived controls; typed projections preserve scalar domains, directed trims,
arc sweep and hyperbola branch. Rational nonzero control uses `P1 = Qh / w`, validates a finite
precision-preserving homogeneous round trip, and retains zero weight as explicit projective `Qh`.
`PreparedSketchPreview` is immutable; only its opaque prepared patch can win exact compare-and-swap
publication. These APIs add no residual or persistence field.

### `geosolve-constraint-editor`

Owns presentation-independent sketch interaction policy over public `geosolve-sketch` and
`geosolve-sketch-features` APIs:

- validated viewport transforms and deterministic accepted-scene primitives;
- screen-space persistent point/span picking and ordered selection;
- normalized gestures, drafting, snapping and action applicability;
- persistent interaction context such as remembered hover/snap identities, prospective
  inference candidates and deterministic guide/tolerance activation;
- constraint/dimension and computed-feature applicability, operand progression and explicit
  branch/side option state;
- typed document-edit, preview, commit and cancellation effects; and
- deterministic transition/replay fixtures for native and WASM qualification.

It depends one way on `geosolve-sketch` and, under ADR 0031, on
`geosolve-sketch-features`. The unreleased ADR 0030 editor facade and its direct
`geosolve-sketch-ops` dependency were removed when M66 closed; the independent M58 operations
companion remains public and the workbench may consume it directly. These dependencies do not own
equations, accepted-sketch validation, a renderer, DOM, widget toolkit, platform event loop,
storage or host expressions. M40.2 implements accepted scene, picking,
selection, basic relation applicability and the click/drag boundary; M40.3-M40.6
complete and mechanically qualify the state machine under ADR 0029 through one
canonical native/release-WASM report and focused browser platform evidence.

M77 owns selected-only `SceneCurveControl*` identities, finite cage/guide/rail paint and hit
geometry, stored-point alias precedence, exact property metadata and the direct gesture lifecycle.
Only independently accepted prepared candidates preview; invalid later samples retain the last
valid result, and stale, cancelled or no-op work publishes nothing. The adapter never chooses
another owner from paint order.

M55 expands the closed headless action/applicability surface to every preserved M13-M14 alpha
constraint, dimension and explicit branch choice. It lowers only through typed public
`geosolve-sketch` edits and reports typed disabled/rejected outcomes. The editor may own operand
applicability, action progression and branch-choice state; it may not reproduce residual equations,
interpret unstable core reports or infer a discrete branch from canvas coordinates.

The completed M55 surface includes 13 relation identities, five dimension identities in
driving/reference modes, selection-scoped contact and oriented-angle branch metadata, exact replay
state and persistent contact/scalar identity through accepted or retained-rejected edits. Complete
contact changes are one domain transaction over semantic span, parameter domain/value, winding,
neighborhood and tangent orientation. No new solver equation was introduced.

The completed M55 contextual-authoring follow-up replaces those equation-shaped workbench
identities with eleven reusable `ConstraintIntent` values. Selection resolution is headless and
publishes `ResolvedConstraintKind`, explicit `ConstraintRelationChoice` and typed disabled reasons;
the workbench does not reproduce the dispatch matrix. Curve hit testing retains the picked
parameter to seed contact-bearing actions, while endpoint continuity uses exact bounded endpoint
parameters. All contact, direction, curvature and continuity branches remain explicit state.

Under ADR 0031, M66 replaces ordinary Fillet use of the fixed two-pick operation collector with
reusable grouped feature authoring. A preselected interior polyline point remains one corner
target rather than flattening to two curve operands; repeated corner or curve-pair picks accumulate
one batch. The editor owns finite picks, explicit per-corner branch choices, remembered/shared
radius, preview progression, warnings and Apply/Enter/Escape semantics. Numeric input or a preview
arc/radius grip edits the shared radius, and Apply creates one persistent FilletSet without a
final radius-confirmation click.

M70B-F003 makes that existing Fillet collector consume the sketch-owned active-Coincident
representatives. Point-to-corner incidence, same-polyline span-pair eligibility and retained-
endpoint hints therefore recognize an explicitly Coincident first/last polyline join while still
keeping the distinct persistent point identities. Either coincident endpoint and either span order
resolve the same closure corner; no coordinate tolerance manufactures a join, and suppression
removes it from this topology.

The retained coordinator combines one sketch session, feature document and current computed
snapshot. It publishes only when complete sketch input/accepted identity, feature revision/digest
and evaluator policy still match. Generated-arc interaction resolves stable feature/corner
provenance; drag changes only feature radius, delete removes that corner and suppression applies to
the set. Presentation code forwards events and renders DTOs rather than locating roots, composing
claims or creating sketch objects. Computed arcs never enter constraint/dimension authoring, while
native source geometry retains normal editor interaction.

Feature picks and numeric/branch option changes cross that boundary as coordinator-owned
transactions. The editor state advances only when any resulting complete provisional FilletSet
evaluates to `Current`; a failed, suppressed, stale or exhausted preview preserves both the prior
authoring state and exact held preview. Screen picking examines a bounded deterministic candidate
set, builds corner incidence once, permits fallthrough only for an incomplete single-span endpoint
or duplicate pending support, and reports a high-valence junction as ambiguous rather than choosing
an underlying curve.

ADR 0032 tightens this boundary for M68. A completed corner is continued from its exact absolute
accepted branch state; relative authoring toggles remain defaults only for collecting new corners.
The editor owns idle, radius-drag, named-parent contact-drag and branch-preview interaction with
exact stamps, pointer identity, origin configuration, a frozen model-space rail and the last exact
`Current` preview token/sample. Authoring, published dragging and numeric edits share one
Current-only transaction. Invalid release, cancellation, stale/exhausted work, a foreign/second
pointer or camera cancellation cannot publish or create history.

The editor also owns model-space grip/spoke/rail, typed contact metadata, retained-direction and
bounded-local-alternative DTOs with stable action IDs, applicability and disabled reasons. Named
contact continuation remains headless and has no endpoint canvas handle. One resolver governs
hover and click: a validated visible arrow outranks an overlapping Fillet radius surface, the
central grip remains authoritative where it visibly covers an arrow, and the generated arc/radius
surface outranks native support. Painted SVG ownership remains only a hint and still requires
independent exact provenance and proximity validation.

The same headless scene boundary tessellates native curves, computed source fragments and generated
Fillet arcs for both presentation and picking. Non-linear spans receive a bounded seed subdivision
before chord-error refinement so an inflection cannot alias to its endpoint chord. The workbench
selects one finer pixel-error policy for native and computed scenes; SVG code only serializes the
resulting polylines.

The archived ADR 0030 editor facade is no longer implemented. The underlying M27/M28/M58
compatibility behavior remains, and the sketch domain still exposes the small, non-mutating
`SketchDocument::certify_line_curve_fillet_branch_cell` query. It reuses the outward-rounded
all-family curve-piece interval kernel to prove that
`cross(curve_tangent(t), fixed_line_direction)` is finite, nonzero and one signed orientation on
the returned open `ContactNeighborhood::Local` cell. Current feature evaluation calls it over the
complete bounded curved span or one explicit unwrapped period. Affine line/polyline spans instead retain
`Interior`; current ADR 0031 feature evaluation consumes the certificate, while two non-affine-
parent authoring returns a typed unsupported warning rather than
guessing a pairwise branch. None of this narrows or replaces M28's public all-family Fillet
definition, residual or validation path.

Fillet endpoint-angle materialization is not an unchecked post-solve geometry patch. After the
sketch domain derives branch-safe Start/End angles, it allowlists only active Fillet-owned angle
variables for a revision-checked core synchronization. Core freshly certifies Hard residuals,
rank, bounds, diagnostics, audit and the complete Temporary/Preference row vectors at the exact
patched state; cancellation or changed evidence rejects without mutation. Publication requires
bit-exact equality between the packed problem and `SolveReport::accepted_state`, followed by a
zero-difference domain materialization pass. The allowlist is a trusted internal domain assertion,
not a security boundary; independent certification remains authoritative.

M40.7 separates non-authoritative `ConstructionPreview` from complete committable
`ConstructionProposal`. A preview may represent an incomplete anchor or arc-radius
guide, while only a complete proposal may enter a document transaction. Terminal
construction effects are ordered commit then clear. Provisional geometry is rendered
as wire guidance only; accepted profile analysis remains the sole owner of area fill.
An existing construction operand carries both its persistent retained-design point ID
and the accepted visible position at snap time. Apply validates the ID against the
retained document but derives branch directions and arc scalar seeds from that exact
snapshot, preserving preview/commit consistency when retained design and accepted
geometry differ after rejection. The browser dispatch adapter suppresses the terminal
preview clear only when the preceding construction commit failed.

#### Headless interaction-intelligence rule

All behavior that changes the meaning or progression of an editing gesture belongs in
`geosolve-constraint-editor`, including behavior that spans several pointer events.
The editor may remember that a draft previously hovered or snapped to a persistent
point, then later use that identity to offer horizontal, vertical or coincident
assistance when the current sample enters a typed tolerance boundary. That remembered
identity, candidate ranking, boundary transition, guide/preview DTO and eventual
confirmation effect are headless state-machine behavior with native replay coverage;
the renderer must not reconstruct any of them from coordinates or DOM hover history.

This rule is intentionally broader than M40's implemented endpoint snapping and does
not add a new M40 gate. It constrains future interaction work. A desktop browser, a
native application or a 3D CAD host editing on a sketch plane may own camera/ray-to-
plane conversion, event delivery, rendering and styling. After producing the editor's
normalized 2D input, each host consumes the same headless previews, guides, inference
candidates and effects, so changing UI technology cannot change sketch behavior.

ADR 0034 makes this prospective rule concrete in M70. One headless inference engine separates
semantic native anchors, bounded stage-local wake/reference state, ranked candidate bundles and
an atomic construction commit plan. Its policy independently controls guide publication,
coordinate adjustment and durable relation creation where semantically coherent. Persisting
point identity without adjustment is rejected because structural operand reuse necessarily uses
the accepted point position. The active scope is persistent-point reuse,
native PointOnCurve, line/polyline Midpoint, new-span Horizontal/Vertical and remembered affine
Parallel/Perpendicular. Bare-point H/V is tracking-only because the ordinary retained editor path
cannot yet persist that relationship; it is never emulated with a fixed coordinate, zero dimension
or hidden construction object. Point identity lowers into a construction operand rather than a
Coincident source; standalone Point confirmation of an existing identity is a history-neutral
no-op. Candidate enumeration stops at its configured bound and fails closed without a partial
semantic prefix. A Circle circumference click is instead a radius sample: near an existing point or
line endpoint it creates PointOnCurve(existing point, created circle) atomically, without a hidden
rim point or any arbitrary line-interior contact/tangency fallback. This paragraph describes
implemented and human-accepted M70 behavior. `M70-F001` passed direct regressions, replacement
qualification/publication, served-byte verification and its targeted human recheck before the
milestone closed.

M71-F006 prospectively supersedes only M70's default capture envelope. It does not reinterpret the
historical M70 candidate, its accepted behavior or any explicitly constructed policy: current
defaults use inclusive `6/9 px` point/midpoint, `8/12 px` curve and `3/5 degree` direction
enter/leave thresholds, while caller-supplied valid tolerances remain unchanged.

Publication authority is not derivable from public scene fields. Only the retained coordinator can
authenticate an `EditorScene` against its exact current accepted document, design filter and
prepared input. A private collision-free seal captures the accepted revision, design identity,
viewport, native inference curves and construction snap anchors produced by trusted scene
construction. Changing any covered public semantic before binding rejects authentication;
changing it after binding revokes plan publication while preserving presentation-only inference.
Compatibility/render-only scenes can expose the same inference presentation but have no
prepared-input authority and cannot publish a plan. Terminal dispatch additionally binds one
session-local token to the frozen displayed plan and rechecks the exact accepted input before
mutation.

### Sketch companion APIs

M58 completes `geosolve-sketch-ops` for split/break/trim, line extension, exact
family-supported mirror, chamfer, existing fillet integration and ordinary drafting
macros/patterns. It constructs deterministic public sketch proposals from complete stamped
snapshots, applies them only through the ordinary retained transaction boundary and owns no
private residual equation, solver state or B-rep topology. Several visible intervals may share
one immutable support through exact fixed/contact boundary identity; canonical sketch v4 remains
the supported language until a future schema-freeze milestone is explicitly scoped.

The ADR 0030 solver-owned ordinary-UI candidate wrapped the existing public M28 associative-
Fillet definition. ADR 0031 supersedes that ordinary routing, and M66 close-off removes its
unreleased editor `OperationAuthoring*` facade and direct editor-to-operations dependency.
M27/M28 definitions and
`SketchOperationRequest::AssociativeFillet` remain supported advanced/backward-compatible APIs;
existing documents are not migrated. This also does not remove M25's signed Offset constraints or
M58's exact supported-family Mirror operation API.

M59 completes `geosolve-sketch-topology`, a read-only companion for revision-stamped production
wires, nesting, holes and exact source provenance. It accepts only the current independently
accepted state for the complete retained input, uses visual-profile analysis solely as bounded
candidate evidence, and independently checks declared source coverage, parameter provenance,
fresh endpoints, closure, orientation/area and output limits. Complete output may feed a host
B-rep feature, but the companion owns no B-rep entities and never changes sketch solve state.
Cancelled, exhausted, truncated, skipped, ambiguous or stale results cannot be consumed as a
production profile.

### `geosolve-sketch-features`

M66 adds a separate persistent computed-feature domain under ADR 0031. Among workspace crates it
depends only on `geosolve-sketch` and `geosolve-geometry`; the sketch, core, linkage, operations and production-
topology crates do not depend on it. It owns no residual, solver variable, accepted sketch state,
canonical sketch schema or B-rep object.

`ComputedFeatureDocument` is separately versioned and owns stable feature/corner IDs, allocator
high-water, labels, suppression and closed feature intent. The first definition is `FilletSet`:
one shared radius plus explicit native source spans, picked parameters, neighborhoods/winding,
normal sides, retained endpoints, endpoint order and sweep. Generated geometry is never persisted.

Evaluation consumes one exact independently accepted sketch snapshot and publishes a separately
stamped `ComputedFeatureSnapshot`. Generated edge IDs are evaluation-local; stable provenance maps
them to feature/corner identity and exact source intervals. Output containers allow variable
cardinality so later topology-changing features such as self-intersecting Offset do not require a
new persistence model. M66 provides no Offset definition, evaluator or UI.

The evaluator independently validates finite geometry, radius, tangency, domains, sides, branch,
order, sweep and offset regularity. Endpoint claims compose without mutating
`DocumentCurveTrimView`: different sets may own opposite ends of one source span, while duplicate,
crossed or consumed claims fail all participants. One invalid corner withholds its whole set;
unrelated sets remain current. A sketch edit is not rejected merely because computed output fails,
and no failed set retains a stale ghost.

M68 adds same-branch continuation and radius-rail evidence without moving equations into the
editor. For offset points `O_i(t_i,r) = p_i(t_i) + s_i r n_i(t_i)`, the feature layer solves the
two-parent differentiated intersection for `dt_i/dr` and derives `dC/dr` independently from each
parent. Non-finite, ill-conditioned or disagreeing results reject, and central finite differences
remain the independent oracle. Pointer projection uses the rail frozen at gesture start. At a
fold, same-branch continuation retains the last current result and requires an explicit local
branch action; it never searches or switches roots implicitly.

One accepted M68 configuration transaction may replace the shared radius and re-anchored absolute
corner intent atomically while preserving stable feature/corner IDs. It uses the existing
separately versioned feature document and workspace-v4 envelope; M68 adds no schema migration.

Version-one references name native constrained spans only. Computed-on-computed chaining,
Bake/Explode, cross-revision output topological naming and production/profile consumption are
deferred.

### `geosolve-linkage`

Owns planar and spatial kinematic domain models:

- rigid bodies and body-local point, axis, plane and frame features;
- physical grounding, joints, mates, drivers and assembly modes;
- branch-preserving continuation and velocity-level queries;
- domain validation, source mapping and persistence.

The frozen baseline is planar `Pose2` linkage kinematics. M17 migrated it to persistent topology/state/session, gauge-separated mobility and shared accepted-linearization velocity; M18 established the spatial slice and M20 added stable-clock axis/plane features, common joints/mates, position coordinates/drivers, explicit mode monitors and atomic position transactions under ADR 0013. M23 completes spatial kinematics with independently published natural/pseudo-arclength continuation, typed endpoint branch events, accepted-state hysteresis and atomic explicit mode changes under ADR 0016; multi-driver spatial velocity, concrete feature fields and optional accepted-rank physical motion bases under ADR 0017; and canonical versioned persistence with deterministic runtime remapping under ADR 0018. Embedded-planar, closed-chain, mixed-scale, generated differential/property and connected sparse/release-crossover gates pass. No linkage API implies physics.

### `geosolve-demo-web`

Is a separate, non-authoritative WASM workbench and compatibility/audit consumer whose
primary purpose is interactive sanity checking:

- it uses public sketch document, session, command, history, serialization and audit APIs;
- rendering, accessibility, browser event translation and browser `localStorage` exist only here;
- it contains no residual, curve, measurement, inference-commit or document-validation equations;
- auto-constraint candidates remain uncommitted until the placement click explicitly confirms the
  currently displayed headless plan; candidate generation, wake/reference memory, ranking,
  adjustment and commit composition come from the editor, while this crate only translates
  semantic suppression and presents/dispatches returned DTOs;
- direct native/WASM tests qualify its adapters and it always renders accepted geometry and audit data from the same result;
- it is desktop-only for all future work; responsive, tablet and mobile support are
  not implementation or acceptance targets;
- it remains non-authoritative and replaceable.

For M77 this crate renders the exact published cage, direct grips, hover state and property rows,
then forwards typed pointer/property requests. It neither recomputes curve controls nor owns
inverse projection, effective rational weight, branch choice or hit priority.

M70B's reproduction codec is a pure deterministic transformation over freshly encoded
`WorkspaceSnapshot` v5 JSON. The single-line envelope is
`GEOSOLVE_REPRO_V1:zlib-base64url:<workspace-bytes>:<fnv1a64>:<body>`. Strict unpadded base64url
and one fully consumed zlib stream are required; 16 MiB text, 12 MiB compressed and 64 MiB decoded
limits fail closed before publication. FNV-1a detects accidental corruption but is not an
authentication or security primitive. The visible overlay is browser delivery only: denied
automatic clipboard access leaves the complete text selected and available for manual copy. A
native stdin/stdout decoder exposes bounded workspace JSON for diagnosis only and cannot construct
or publish a coordinator.

M70B-H1 is qualification infrastructure rather than another interaction layer. Native integration
tests call `AuthoringState` and `RetainedEditorCoordinator`, then independently inspect current
accepted domain state through public sketch APIs. Workbench unit tests call the private thin scene
composer only because scene composition is presentation-owned. The shell driver isolates every
authoring and scene row in its own bounded process, records semantic failures, panics, hard-kill
timeouts and harness errors without stopping later rows, verifies the exact case/family inventory
and compares stable effective-input fingerprints with a checked six-column golden. It adds no
runtime dependency, equation, inference policy, persistence field, scene state or browser harness.

M70B-H2 leaves those semantics and golden bytes intact while giving the test, fixture, aggregate
driver, environment variables and scene survey milestone-neutral names. The complete release gate
now requires the clean matrix. `.agents/skills/geosolve-harden-defect/` defines the owner-first
defect workflow: exact regressions remain with the narrowest public Rust owner, and this broad
matrix expands only for a systemic missing dimension. This remains repository/test infrastructure,
not another product layer.

M70B-H3 adds only the systemic computed-Fillet dimensions exposed by F003 and F004. Two
`feature.fillet` rows drive Coincident-closure point and curve-pair collection through the public
headless feature-authoring and retained-coordinator APIs. Two more capture and execute the public
computed-feature evaluation API for lower same-cell and periodic-seam line-circle cases, using the
public contact-reseed path only to prove that a valid root remains in the persisted branch cell.
The oracle independently requires current accepted sketch hard validity and finite geometry, then
checks Fillet incidence, radius, tangency, signed normal side, native source/span identity, contact
parameter, winding and same-cell root membership. It does not trust an evaluation status as its
geometric oracle. At the historical test-only checkpoint, the original 193 rows remained byte-
identical and four reviewed defect rows made the inventory 197 without adding runtime code,
product authority or release bytes; that fixture had SHA-256
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`.

The authorized F004 repair separates persisted-evaluation search policy from radius continuation.
For a Circle or CircularArc parent paired with affine support, constant curvature means a fixed-
radius offset cannot fold inside one certified tangent-orientation cell, so persisted evaluation
may search that complete explicit cell without changing branch. Generic nonlinear curves keep the
narrow seed-connected guard, and radius continuation still stops at folds rather than selecting a
remote root. Together with F003, this changes the four stable H3 rows from `DEFECT` to `PASS` while
retaining their exact input fingerprints.

F005 distinguishes a durable branch witness from its stale numeric certificate. The ordinary
persisted evaluation remains the fast path. Only when it returns `NoLocalRoot`, and only for one
Circle/CircularArc plus one affine parent, a bounded fallback searches the retained circular
support. Search-time and publication-time validation independently re-certify cells at the stored
seed and proposed contact and require strict cell overlap, finite transverse geometry and one
unique material root. This transports the same current orientation branch across conservative
interval edges without rewriting feature state or allowing an opposite root across a real
parallel-tangent barrier. Exact payload `4228:0823d31f269300af` and the named feature-owner
regression preserve that distinction. The new systemic source-rotation row extends the current
fixture to 198 all-`PASS` rows at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Focused owner/golden and
aggregate golden qualification, formatting, warnings-denied all-workspace Clippy, locked all-
feature workspace tests and the relevant WASM check pass. Clean F005 source
`d400c4a8201f6afc531f5b504424d6430dbf3937` passes the complete release gate, and its immutable
seven-file snapshot at `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` was served and byte-verified at
`http://100.94.63.83:8080/` for M70B with ordered-manifest aggregate
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`; that server has since
retired.

Source movement continuity is an accepted-state protocol above that static root proof. A computed
snapshot carries internal current-corner contact, winding, periodic-certificate and transverse-
orientation evidence into only the same accepted input or its authenticated direct successor.
After a successful native edit, the coordinator derives a refreshed feature sidecar from Current
output, then proves an ordinary capture with no continuation hints reproduces the exact generated
geometry, contact metadata, construction fragments and feature dispositions. Projected release
stages the native session, refreshed sidecar, evaluation allocator, checkpoint, history entry and
transcript transition before publishing any of them. Replay binds the transition to the exact edit,
drag target, retained publication policy and activation/parameter/external input stamps; only the
process-local prepared epoch and non-durable previous-state preference are rebound. Failed sets
retain their prior intent and recovery hints, but cannot contribute generated geometry or durable
re-anchors, and non-Edit actions never persist an unrecorded refresh. During projected dragging,
every previously Current set must remain Current before either the native or computed preview
advances. This preserves one last-complete scene and release point across a genuine
parent/fold/work limit, while transient targeted problem metadata lets the ordinary canvas
highlight the responsible corner and sources. The general native-only preview boundary and
intentional direct-edit failure semantics remain separate.

M64 removes the completed-review harness that historically served M53-M63. A crate-private sample
catalog now owns only stable sample keys, titles, purpose grouping and public fixture selection.
Opening a sample constructs a fresh ordinary `RetainedEditorCoordinator`, replaces the sole
workspace, resets history, fits the camera and then uses normal persistence and editing. There is
no hidden coordinator, guide/action/transcript/evidence state, reset/exit lifecycle or save
suppression. The selector is presentation-only and its three one-level groups use right-expanding
hover/focus flyouts.

The public headless editor publishes only the latest failed/rejected attempt as structured
`EditorProblemMetadata`: attempt/design identity, high-level category, explicit global/targeted
scope, human-readable message and deterministic persistent point/curve/constraint/dimension
targets. Targeting maps core conflict sources and typed rejection identities through the attempted
document mappings, then expands document-owned operands; it never derives blame from labels,
geometry proximity or residual magnitude. The workbench renders that metadata as a separate
overlay over the authoritative accepted scene. Missing or non-resolving attribution becomes a
global marker, and the Problems panel remains the canonical complete presentation.

Historically, M13 implemented the disposable alpha playground interactions and M14 hardened
its E2E, import/error recovery and performance. M46-M50 replaced every retained semantic
claim with a direct owner or reviewed retirement and then removed that runtime. None of the
historical or surviving consumer behavior moves equations or authoritative state into the
web crate.

M39 begins and M44 continues a staged rewrite into a CAD-like sketch workbench with a
command bar, tool palette, sketch tree, retained canvas scene, property inspector,
status bar and Problems/Profile/Audit panels. The rewrite remains a sketch-only demo:
it has no solid-feature tools, computes no constraint or measurement
formula and consumes stable domain diagnostics. M40.5 removed duplicated selection,
gesture, drafting, lifecycle and history policy and made this crate a thin adapter over
`geosolve-constraint-editor`. Cleanup M46-M50 replaces direct-test ownership and removes the
second legacy application and old browser E2E; M51 consolidates the survivor around one workspace
snapshot and direct presentation/evidence owners. Human acceptance dispositions are recorded at
completed M40.7, M53 and M61-M76. Newly scoped milestones normally end in hands-on UAT after direct
qualification; M74 records the explicit exception that deferred its unexecuted scorecard without
calling it passed, while M76 records the caller's explicit scoped acceptance without a separate
post-refinement replay or invented step-level observations.

M55 makes the surviving workbench render and dispatch the complete alpha action surface returned by
the headless editor. Presentation may own layout, labels, accessibility and tooltips, but not action
applicability, branch selection, equations or accepted-state authority. The deleted playground,
`/#/dev/lab`, legacy harnesses and browser E2E remain retired.

M60 makes the same workbench a direct public consumer of `geosolve-sketch-ops` and
`geosolve-sketch-topology`. Prepared operation proposals are applied through their ordinary
exact-CAS retained transaction boundary. Production-topology
presentation exposes consumable wires/regions only from a complete current
`TopologyProductionProfile`; skipped, truncated, cancelled, exhausted, unavailable and stale
evidence remains non-consumable. The application workspace v2 envelope labels each document
payload as frozen canonical v4 or explicitly unstable draft v5 and migrates legacy workspace v1.
These presentation/persistence additions own no equation, branch inference or B-rep state.

M67 removed the raw production-topology card and direct topology dependency from the
non-published workbench together with the Host-state evidence and Accepted redundancy developer
cards. This is a presentation-consumer cleanup only: `geosolve-sketch-topology`, stable lifecycle/
redundancy DTOs and their direct owning-layer qualification remain reusable and unchanged.

M66 makes the ordinary Fillet tool a direct presentation of headless computed-feature authoring.
The workbench renders grouped corner candidates, shared-radius preview, stable feature/corner
selection, a **Features** tree and attributed feature issues. It never creates M28 contacts,
dimensions or trim views and never composes endpoint claims itself. Generated arcs are not sketch-
constraint operands; native source points and spans retain ordinary selection and drag behavior.

Checkpoint `M66-PF003` keeps the stable `fillet-workshop` sample key but presents it as the
ordinary editable **2D Fillet playground** under **Samples → Curves & constructions**. Its fixed
reference islands cover independent intersecting lines, a line/circle pair, a
line/quadratic-Bezier pair and a true three-line shared junction. Two unlocked four-point
polylines cover multi-corner/sequential composition and a deliberately short middle span for
claim-conflict recovery. Opening the leaf still creates the sole ordinary coordinator and adds no
guide, scripted action, read-only state, alternate route or sample-owned authoring rule.

Canvas platform policy remains local to the web adapter. The SVG canvas and its descendants opt
out of native text selection and element dragging, and the adapter prevents only `selectstart`
and `dragstart` defaults at that boundary. The sibling Fillet options overlay, sidebar and other
HTML retain normal selection and input behavior. Focused native presentation tests qualify this
scoping; no browser E2E/CDP harness is restored or claimed.

Follow-up `M66-PF004` makes painted computed-preview intent explicit across that adapter boundary.
The workbench resolves the nearest stable `data-editor-item` owner from the painted DOM target and
passes the resulting `SelectionItem` only as a hint; it owns no geometry fallback rule. The
coordinator admits a `FeatureCorner` only when it belongs to the exact held whole-feature preview,
the collector still represents that preview's complete candidate, and the scene matches current
accepted and computed provenance. The headless editor then independently requires the pointer to
hit that owner's computed curve. A stale or foreign owner is rejected without becoming a native
support pick. While one radius gesture is active, any further radius press is rejected before
mutation so the original gesture remains valid. The explicit radius path uses replace-selection
semantics even when Shift, Control or Command is held; ordinary selection clicks retain their
existing modifier behavior.

Invalid computed output is withheld rather than drawn from an older snapshot. A valid sketch edit
still publishes and may leave a repairable feature failure. Base-only profile/fill presentation is
also withheld with typed “computed geometry not yet included” status whenever active computed
geometry would make it misleading. At the M66 checkpoint the workbench remained a read-only
production-topology consumer and did not pass computed output to that companion; M67 removed that
raw developer presentation without changing either computed-feature or topology domain behavior.

M69 adds an explicit semantic layer over that composition without changing the constraint graph.
Effective computed edges carry Profile/Construction role metadata. A successful open-parent trim
also publishes each materially non-empty discarded start/end complement through a separate
evaluation-local construction-fragment collection. The fragment records exact source, interval,
base interval, owning Fillet corner and claimed endpoint; it is never an effective edge or a
persistent feature object. Full-period parents, failed/suppressed features and noncurrent work
publish no discarded fragment.

The editor maps an implicit construction fragment back to its native `CurveSpan` and picked
parameter. It therefore remains inspectable and constrainable through the complete native source
without inventing a fragment identity. Persistent source role and implicit presentation origin are
orthogonal: a source can be explicitly Construction, while a Profile source's discarded Fillet
tail is implicitly Construction only for that computed revision. The workbench renders these
facts and exposes headless `All`/`Profile`/`Construction` scopes; it does not infer role from CSS or
SVG paint order.

M66 advances the application workspace envelope from version 3 to version 4. It retains the
canonical-v4/draft-v5 document encoding and current-materialization provenance, then adds the
separately versioned computed-feature document. Workspace v1-v3 inputs migrate to an empty feature
document bound to the restored sketch. Existing M28 Fillets are not reinterpreted. Feature intent,
stable IDs and allocation high-water persist; evaluation regenerates fresh output IDs after
restore. Canonical sketch v4 and draft-v5 formats are unchanged.

Accepted limitation `M66-KL001` is presentation/interaction state, not a mathematical exception.
Radius drag measures pointer distance from the held/old arc center while evaluation moves the
center and contacts, so tracking may drift or feel inverted; post-placement contact/root,
retained-parent direction and alternate-arc choices lack intuitive controls, especially for
line-circle Fillets. Numeric radius editing, explicit persisted branch state, independent
validation, rollback and sketch-state invariance remain correct. The playground line-circle
specimen starts at radius `0.5`, near a branch fold. The one-dimensional radius rail, frozen
absolute branch intent, typed contact metadata and its internal continuation seam,
retention/continuation actions, bounded local-alternative previews and friendlier sample were not
M67 scope. ADR 0032 assigned that completed work to M68 while retaining the fold as a distinct
regression fixture; M66's scoped approval remains historical and unchanged.

For M68, the workbench renders one visible midpoint radius grip and spoke/rail, solid current
retention arrows, outlined alternatives and dashed complementary/local previews. Named contact
metadata and its internal continuation seam remain headless; there are no endpoint contact dots,
canvas hit zones or compact-panel contact controls.
The same stable actions appear in a compact accessible panel; raw relative Flip-first,
Flip-second and Alternate-arc checkboxes are not the ordinary branch UI. The adapter captures and
releases pointers for point, Fillet and pan gestures, cancels/restores live Fillet manipulation
before a camera change, and owns no root selection or rollback logic. A friendly line-circle
specimen is separate from the retained radius-`0.5` fold stress case. These completed M68 surfaces
passed the mechanical gate and focused human UAT.

M61 remediation keeps those boundaries intact while making the candidate genuinely interactive.
Advanced construction state and proposal/preview generation live in `geosolve-constraint-editor`;
complete preview curves are sampled by applying a localized proposal to a temporary public
`SketchDocument` and calling public curve-jet/visible-interval APIs. The web crate owns only the
toolbar, option parsing, SVG markup and event normalization. Invalid conic/NURBS options and
topology reject before publication, and a NURBS gauge always names a weight exactly equal to one.

M64 supersedes M61's temporary active-scenario interaction boundary. Samples have no preselected
driver or drag metadata. M65 keeps that ordinary sample-agnostic boundary and replaces retry-based
stabilization with one opaque gesture-local plan derived by `geosolve-sketch` from the
independently accepted hard nullspace. The active point's nullspace response establishes active
rank; only uncovered passive mobility is anchored. Candidate points are chosen by greatest rank
gain, then lower mobility rank, then compile order. Anchor coordinates are captured from the
gesture-start accepted visible geometry, not from advancing numerical seeds.

The compiler receives the cursor point as the sole Temporary target and the selected anchors as
the sole PreviousState Preferences. The plan remains presentation-independent and contains no
sample identity. Circle circumference gestures map to their document-owned center while retaining
the initial pointer offset. This architecture permits geometry required by the active mechanism to
move while holding mathematically independent passive controls stationary.

The retained coordinator owns pointer identity, monotonically increasing request identity,
design/accepted identity, the locality plan and the complete last independently accepted preview.
Each non-stale sample executes exactly one retained attempt from that preview. Rejection or
operation exhaustion leaves the entire preview unchanged; a subsequent valid sample may recover
in the same gesture. A stale or out-of-order sample is a no-op. Release independently validates
and publishes the exact preview as one ordinary history edit; Cancel publishes nothing. Transient
targets remain attempt evidence and never enter persisted design intent. Canvas camera state
remains web-only.

Core priority handling may publish only a finite candidate that independently validates Hard rows
and the applicable attained-Temporary contract. On the single-component dense path, a positive
Temporary attainment is protected as its complete normalized residual vector: Preference work may
publish only after freshly preserving every vector entry within
`max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`.
That machine reproducibility floor compares the post-Preference vector with the independently
attained positive Temporary vector; it does not relax Hard validation or turn an unsatisfied
Temporary target into convergence.
Coupled-priority solving is unchanged and continues to protect each scalar attained Temporary
level. Failure rejects or retains the certified attained state rather than exposing raw
post-Temporary drift. Accepted and no-motion report construction rejects invalid-geometry or
numerical-failure termination and requires successful audit-row evaluation; a truthfully
non-optimal secondary `Stalled` or `IterationLimit` status remains distinct from independent Hard
validity. These requirements do not replace
Hard/Temporary/Preference ordering with weights or relax success tolerances.

One projected sample is synchronously bounded to `16,384` each validation, dependency and lowering
items; `256` each nonlinear iterations, factorizations and rank kernels; `512` rejected trials;
`1,024` component linearizations; `256 × 256` dense kernels; `512` diagnostic candidates; and
`1,024` diagnostic trials. Exhaustion is an ordinary typed rejection retaining the last valid
preview. M65 adds no alternate-assembly search, preview UI or fixture.

M66 keeps Fillet in the **Modify** palette but changes its authority. Compatible preselection or
repeated clicks collect one or more grouped corner targets. Preview starts from remembered radius
or `0.1 * model_scale`; numeric input and preview arc/radius-grip drag edit one shared value.
Apply/Enter creates one FilletSet, while a later Apply creates another set. There is no final
radius-confirmation click and no ordinary Driving/Reference dimension choice.

Only output evaluated from the exact current sketch/feature stamp is drawn. A generated arc maps
to stable feature/corner provenance and may edit only feature radius; it is not a sketch operand.
Feature failures remove that set's output and keep source geometry editable. Per-corner branch
controls remain explicit. Affine/affine and affine/non-affine corners are in scope; two non-affine
parents are typed unsupported without narrowing M28. Camera navigation remains web-only and usable
during authoring.

## 4. Numerical representation and linearization

A problem contains variable blocks `x` and residual blocks `r_i(x_incident)`. Every variable has an ambient representation, a tangent dimension, a local retraction and positive finite characteristic step scales. Every residual declares its source, priority category, ordered local incidence, output dimension, positive finite residual scales, evaluator, Jacobian path and audit rows.

Residual values and Jacobian columns are normalized before convergence or rank decisions:

```text
r_normalized[row] = r_raw[row] / residual_scale[row]
J_normalized[row, col] = d(r_normalized[row]) / d(delta_normalized[col])
delta_local[col] = step_scale[col] * delta_normalized[col]
```

Implemented variable blocks are scalar, `Vec2`, `Vec3`, manifold `Pose2` and quaternion-backed `Pose3`. `Pose3` has seven ambient coordinates and six right/body-local tangent coordinates. Baseline assembly can materialize global dense columns, while reduced components are solved independently.

The M9 implementation provides one canonical component-local linearization under ADR 0005. It evaluates only incident blocks, writes into caller-provided storage, never allocates global columns for a component, and feeds the dense component solve. The caller-storage method is public and unstable before 1.0 because it extends the existing public residual evaluator trait; the local AD formula trait/adapter and normalized-coordinate storage marker remain private. M15 makes local AD, fixed/alias residuals and finite differences use the same manifold retraction. M16 adds indexed block coordinates and materializes triplet/COO and sparse storage from that IR. Analytic Jacobians remain valid and central finite differences remain an independent oracle. Branch, span, winding, active-bound and assembly-mode state are fixed discrete inputs outside AD.

Public and best-effort audit evaluate fresh raw/normalized values at one state and independently require successful canonical Jacobian/fused validation before marking a row `Evaluated`. A structured derivative failure marks the row `Failed` while retaining any fresh finite displayed values and its category/message. Successful numeric IR blocks are `Evaluated`; any failure aborts before partial IR consumption.

## 5. Solve pipeline and persistent state

The logical target pipeline is:

1. validate domain topology, geometry, scales and discrete state;
2. compile or incrementally update immutable topology and source parameters;
3. eliminate trusted fixed and alias relationships;
4. split the reduced incidence graph into deterministic components;
5. determine dirty components and active bounds;
6. linearize each dirty component in normalized local coordinates;
7. solve the strict hard/temporary/preference hierarchy;
8. independently re-evaluate all hard rows and domain/branch validators;
9. compute rank, mobility and bounded diagnostics at the returned state;
10. atomically commit only a finite, independently valid accepted patch;
11. retain prior accepted state and discrete state on rejection.

Baseline `Problem::solve_decomposed` has component caching but relies on caller-supplied edited variable IDs. M10 replaces that hint-based lifecycle with the persistent `SolveSession` and revision/dirty tracking in ADR 0007, with `SketchSession` as the first domain consumer. M11 layers `SketchDocumentSession` over that validated boundary: document commands lower persistent semantic IDs deterministically to fresh runtime IDs, solve through existing sketch equations, project only independently accepted continuous/contact state back to persistent IDs, and clone-and-swap the document/history atomically. Rejected full-document attempts expose retained accepted geometry/mappings separately from attempted diagnostic mappings. Clean components may reuse zero nonlinear iterations, but all hard and secondary rows, Jacobian/derivative statuses, audit snapshots, rank and bounded diagnostics are freshly evaluated at every returned state. Residual evaluators are behavior-pure; interior mutable telemetry cannot affect equations. M16 adds sparse storage, structural matching, bounded symbolic cache reuse and the continuation contract in ADR 0011. Natural continuation stops before a parameter reversal. Pseudo-arclength parameter/control rows are ephemeral, and only a separately re-solved, independently validated ordinary physical problem may be committed or published. No benchmark or performance policy may bypass independent validation.

M34 implements three explicit sketch views through `RetainedSketchDocumentSession`:
structurally valid design intent, an optional finite attempted candidate and the last
independently accepted solved state. Design may remain unsolved after a conflict or
unavailable input, but neither design nor attempted geometry gains an accepted
revision or authoritative audit. The older `SketchDocumentSession` remains an
accepted-only command/history workflow. Frozen v1-v4 graphs persist design and
accepted views separately; revision high-water metadata remains host-owned pending a future
supported wire freeze. M41-M43 extend attempt identity with immutable
activation/parameter/external revisions. M56 adds a complete prepared-input stamp over those
domains plus current design, attempt, accepted/high-water, request and solver policy identities.
Typed work executes only against a captured session clone. A completed candidate can replace the
live session only when `commit_prepared_patch` compares that complete base stamp equal; stale or
out-of-order candidates leave every live identity unchanged.

M70 adds field-opaque, checkpoint-serializable `SketchPersistentIdentityHighWater` above frozen
document JSON. It retains persistent-object and curve-local spline-span allocator maxima even while
Undo removes the corresponding graph objects. Coordinator checkpoints merge that lifecycle
maximum into Redo, reload and divergent history, so no retired identity is reused. Historical
graph restoration uses the current exact parameter batch and external snapshot set rather than
silently restoring default host input. Application workspace v5 stores and validates these
cursors; strict v1-v4 migration derives them from the stored design and accepted graphs. This
changes neither frozen sketch v1-v4 bytes nor current unsupported draft-v5 bytes, nor does it
change host-owned lifecycle revision high-water.

M57 retains compatible `DocumentRuntimeMap`, `CompiledSketch` and `SolveSession` state across
accepted document attempts. Persistent point/curve/source/contact joins are indexed. One scratch
compile is a compatibility oracle only: exact variable/source/residual/bound mappings must match
before changed shape values and the transitive persistent source closure enter a core
`SessionPatch`. Parameter and immutable external-reference updates with unchanged request shape
take this path directly. Topology or source-shape changes are explicit full rebuilds. Both paths
freshly validate all hard rows, derivatives, domain/branch state, projection and numerical rank
before publication. Profile caches belong to one accepted revision and cannot affect solving.
Sparse hard steps do not imply sparse rank authority; production rank remains dense-SVD
authoritative within the declared 256-row/256-tangent connected-component envelope.

M35 adds additive `OperationControl`/`OperationController` boundaries shared by core
and sketch operations. A monotonic library token carries host cancellation; overflow-safe
deterministic counters authorize lowering, iteration, factorization, rank, diagnostic,
validation and profile work without consulting wall time. Outcomes distinguish
cancellation and work exhaustion from numerical or geometric rejection and from
independently validated convergence. Controlled mutations perform work on scratch
state and check cancellation immediately before atomic publication. Controlled dense
factorization and rank kernels are bounded to 256 rows and 256 columns per kernel in
M35; larger controlled inputs fail closed before kernel entry.

M56's concurrency contract is host-managed and safe-Rust only. A native host exclusively owns the
live session, moves a `Send` prepared job to one worker, then returns its patch to the owner for CAS
commit. Session-bearing snapshots/jobs/patches are not promised `Sync` because solver caches use
safe single-owner interior mutability; immutable prepared stamps, operations and commit metadata
are `Send + Sync`. Single-threaded WASM runs the identical prepare/execute/commit boundary
synchronously. GeoSolve adds no worker pool, mutex around numerical state, `unsafe` implementation
or browser scheduling policy.

## 6. Hard validity and secondary optimum status

Hard validity is independent from nonlinear termination, rank classification and secondary-objective completion.

Starting with M9, the report has these orthogonal facts:

- `HardValidity::Valid`: every hard row was freshly evaluated, finite and within the configured normalized tolerance, and all domain/branch validators accepted the same returned state;
- `HardValidity::Invalid`: evaluation completed but at least one hard row or domain/branch validator failed;
- `HardValidity::NotEvaluated`: complete independent validation could not be performed;
- hard nonlinear termination: why hard iteration stopped;
- one secondary result per requested temporary/preference level: `NotRequested`, `Optimal`, `Acceptable`, `Stalled`, `IterationLimit` or `EvaluationFailure`;
- rank, structural class, singularity and diagnostic completeness as separate fields.

A state is hard-valid only for `HardValidity::Valid`. A domain may commit a hard-valid state even when a secondary objective is not optimal, but it must report that secondary status and the domain interaction policy may reject it. No secondary success can turn invalid or unevaluated hard geometry into a success-like result.

`Optimal` is reserved for a zero-cost least-squares level or a feasible space
with no remaining direction. Finite multi-scale curvature samples can discover
descent but cannot prove nonnegative curvature for an arbitrary evaluator. A
positive-cost first-order stationary level with no detected negative sample is
therefore `Acceptable`, with converged termination so domain transactions may
commit it, not `Optimal`.

Baseline transition: through M8, `SolveReport` exposes `hard_residuals_validated` and hard norms, but top-level `SolveTermination::Converged` also requires every priority pass to terminate as `Converged`. That frozen behavior remains accepted and is not a failure of the M1-M8 baseline. M9 introduces the orthogonal fields above and makes them mandatory for all new reports. M10 `SolveSession` commits consume the M9 hard-valid field as authoritative. Compatibility wording must not call a secondary stall a hard-constraint failure.

## 7. Rank and mobility contract

### 7.1 Numerical rank

Starting with M9, numerical rank is computed independently for each reduced connected component from its finite, normalized, component-local hard Jacobian `J_c`. Let:

- `m_c` be active hard scalar rows;
- `n_c` be active tangent coordinates after trusted fixed/alias elimination;
- `sigma_max` be the largest singular value, or zero for an all-zero matrix;
- `tau_rel` be the configured relative tolerance;
- `d_c = max(m_c, n_c, 1)`;
- `tau_machine = EPSILON * d_c * max(sigma_max, 1)`;
- `tau_c = max(tau_rel * sigma_max, tau_machine)`.

The numerical rank is the count of singular values strictly greater than `tau_c`. The report includes `tau_rel`, `tau_machine`, `tau_c`, `sigma_max`, the smallest retained singular value and enough spectrum/estimator information to reproduce the classification. Rank is invalid if any required value or decomposition result is non-finite.

For valid rank `r_c`:

```text
right_nullity = n_c - r_c   // equality mobility in tangent coordinates
left_nullity  = m_c - r_c   // dependent hard-row space
```

Whole-problem rank and nullities are sums of component-local values. A global largest singular value never sets another component's threshold.

This M9 contract governs core equality/position reports and every sketch/linkage position solve built from them. Starting at M17, persistent and compatibility linkage velocity queries use the same accepted component-local hard linearization, residual scales and rank thresholds; they do not assemble or rank a separate global dense velocity matrix. Linkage position conditioning summaries use within-component spectra and M9 `near_singular`; they never compare concatenated extrema from disconnected components.

A component is numerically singular when `r_c < min(m_c, n_c)`. A distinct near-singular warning is raised without changing rank when the smallest retained singular value is at most `near_singular_factor * tau_c`; the configured factor and ratio are reported. The initial target factor is `100`. A warning is not convergence and a rank drop is not nonlinear failure.

Baseline transition: M1-M8 use normalized component-local Jacobians and default `tau_rel = 1e-10`, report right nullity as local DOF, and flag a rank drop. They use only `tau_rel * sigma_max`, do not report the machine floor or left nullity, and have no separate near-singular band. That behavior remains the accepted frozen baseline. M9 atomically adopts the machine-floor threshold, numerical left/right nullity and near-singular reporting above; existing rank fixtures remain regression oracles.

### 7.2 Structural classification

Numerical rank and graph structure answer different questions. M16 computes maximum structural matching on the reduced hard incidence graph before numerical values are considered. The public evaluator seam declares incidence by variable block rather than scalar formula slot, so each incident block contributes its complete tangent-coordinate envelope, including explicit zero entries. Structural rank and DM partitions describe that stable declared envelope; they are not a proof that every slot is analytically nonzero and never replace numerical SVD rank. For structural rank `s_c`:

```text
structural_right_nullity = n_c - s_c
structural_left_nullity  = m_c - s_c
```

Classification is:

- `Under`: right nullity is positive and left nullity is zero;
- `Well`: both nullities are zero;
- `Over`: left nullity is positive and right nullity is zero;
- `Mixed`: both are positive; the report includes Dulmage-Mendelsohn under, well and over partitions rather than hiding them in one label.

Baseline structural summaries report reduced counts and deterministic signatures only. Count comparisons may be displayed as count heuristics, never as structural matching or numerical rank. M16 implements matching and partitions.

### 7.3 Active bounds

M10 reports every bound as inactive, active-lower, active-upper or fixed. Equality rank is retained before adding bounds. For bidirectional mobility, append independent active-bound coordinate normals to the equality Jacobian; the nullity of this augmented matrix is the lineality dimension of the feasible tangent cone. The report includes:

- equality right nullity before active bounds;
- bidirectional DOF after the active set;
- active bound IDs and sides;
- whether a nonzero one-sided feasible tangent direction exists.

An active lower bound permits inward positive motion and an active upper bound permits inward negative motion, so subtracting one DOF per active bound is not a sufficient mobility analysis. Bound activation is explicit state and cannot be inferred from a post-solve clamp.

### 7.4 Gauge versus internal mobility

A domain-certified free world action contributes gauge DOF: three for a floating planar component and six for a floating spatial component. Reports split numerical right nullity into `gauge_dof` and `internal_mobility`; they do not blindly subtract three or six unless the domain certifies the corresponding invariant action. Physical grounding removes physical gauge freedom. A numerical gauge only chooses coordinates and must not remove reported physical mobility. ADR 0009 governs this split; M17 applies it to planar linkage and M18 to spatial linkage.

## 8. Diagnostic completeness and budgets

Redundancy and conflict candidates are bounded explanatory diagnostics, not proofs of a globally minimal dependent set or unsatisfiable core. Every bounded diagnostic section carries:

- `status`: `Complete`, `Truncated` or `Skipped`;
- the configured budget, including applicable maximum component tangent dimensions, scalar rows, candidate sources and deletion/rank trials;
- actual work consumed;
- a machine-readable reason for `Truncated` or `Skipped`;
- deterministic candidate IDs in source order.

`Complete` means every candidate in the documented algorithmic scope was examined. `Truncated` means at least one eligible candidate was examined but the budget stopped remaining work. `Skipped` means no eligible analysis was performed, for example because diagnostics were disabled, rank/evaluation was invalid, hard constraints were valid for conflict analysis, or the first component already exceeded budget.

An empty candidate list is meaningful only together with its status. In particular, an empty list with `Skipped` or `Truncated` must never be presented as “no conflict” or “no redundancy”. A `Complete` result still claims completeness only for the documented bounded deletion/rank algorithm, not global minimality.

Baseline transition: conflict deletion currently has fixed limits of 12 candidate sources and 24 active tangent dimensions and silently omits over-budget components; redundancy runs only after valid hard evaluation/rank. The baseline report has no completeness or budget fields, so empty baseline candidate vectors are ambiguous. M10 makes those bounded candidate budgets configurable/reportable in the session report. M16 structural matching is complete for each declared block envelope, while sparse backend/fallback evidence is deterministic and unbudgeted; neither is represented as a bounded candidate search.

## 9. Priority semantics

Hard, temporary and preference rows are different categories, not weights in one undocumented least-squares objective. The implemented baseline uses a lexicographic hierarchy and reprojects secondary steps onto hard validity. The target retains this ordering through component-local and sparse paths:

1. attain and validate hard constraints;
2. optimize temporary objectives in the valid hard tangent/null space;
3. optimize previous-state preferences without worsening the attained temporary level beyond documented numerical resolution;
4. independently validate hard rows and report each secondary outcome.

M65 refines step 3 only for the single-component dense path: a positive attained Temporary
residual vector is preserved component by component within
`max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`, while
separable Preference motion remains permitted when that vector is unchanged within that
reproducibility band. This is not a Hard acceptance-tolerance change. Coupled-priority solving
retains its existing scalar attained-level semantics.

Bounds participate through the M10 active-set policy. Secondary objectives spanning hard components are implemented in M16 without merging hard components or weakening hard tolerance.

## 10. Manifold and frame conventions

ADR 0006 defines body-to-world transforms, right/body-local retraction, tangent ordering, local difference, quaternion ordering and sign canonicalization. M15 completed the tested transition from additive `Pose2` increments to manifold `Pose2` and quaternion-backed `Pose3`; finite differences perturb tangent coordinates through the same retraction. Exact quaternion half turns have one deterministic representation, while explicit winding and assembly choices remain separate domain state.

M15 also exposes revision-stamped accepted hard linearizations and independently validated sensitivity solves. The API returns reduced hard-equality results in body-local tangent coordinates and distinguishes unique, underdetermined minimum-norm and inconsistent rates. Active-bound tangent cones, secondary-objective sensitivity and world/spatial velocity conversion are not implied by this core API.

Planar geometry is evaluated in local 2D coordinates. A workplane maps it into world coordinates as:

```text
p_world = origin_world + u_world * x + v_world * y
```

Same-plane constraints remain 2D. A planar body pose composes with the workplane frame; redundant `z = 0` rows are not added per point.

## 11. Sketch design and curve architecture

ADR 0008 defines persistent external IDs, runtime generational keys, command history and a closed versioned `CurveDefinition`. “Closed” means an exhaustive built-in serializable enum, not that every represented curve is periodic. Evaluation uses internal traits/adapters until built-in line, circle, arc, Bezier, conic, B-spline and NURBS families prove the seam.

The M11 implementation stores document-local entity/source/contact/scalar identities
as fixed lowercase hexadecimal 128-bit values under a separate document identity.
Runtime slot-map keys are never serialized. Import normalizes store order and validates
version, resource limits, uniqueness, references, typed scalar ownership/domains,
finite geometry and every discrete branch/contact field before lowering. Coupled
contact transitions update parameter, winding, neighborhood and both tangency
orientations atomically; undo/redo preserves the allocation high-water mark so an
accepted or undone identity is never reused.

Generic curve constraints use latent contact coordinates and explicit discrete state:

```text
point on curve:       P - C(t) = 0
curve/curve contact:  C1(t1) - C2(t2) = 0
tangent alignment:    cross(unit(C1'(t1)), unit(C2'(t2))) = 0
```

Design controls, weights and contact parameters that are active variables must all appear in residual incidence and derivatives. Parameter domains, spans, winding, contact neighborhoods and tangent orientation remain outside AD. Bounded endpoints use M10 bounds/active sets. Cusps, zero-speed jets, invalid knots, rational poles and ambiguous neighborhoods are explicit evaluation/domain outcomes and cannot converge through normalization.

M11 migrates baseline entities, commands and persistence topology. M12 proves generic editable-curve differentiation with Bezier curves. M19 adds conics, M21 B-splines and M22 NURBS plus curvature/G2 and separately named parametric C2 continuity. M27 composes ordinary line jets into an associative line-line fillet. M28 generalizes that association across regular curve families using four center/normal-offset rows and two output-radial rows. Associated output-arc angles are solver coordinates, so ordinary point, contact, tangency, curvature and continuity consumers differentiate through derived endpoints. Explicit side, span, neighborhood, winding, endpoint order and sweep state select the intended local branch outside AD.

## 12. Kinematic architecture

Rigid bodies own local features; joints and mates relate features rather than reconstructing rigidity with sketch distance webs. Branch/assembly state is persistent domain state. Physical ground and numerical gauge are distinct under ADR 0009. Position and velocity queries use the same accepted-state reduced hard linearization and rank policy.

M17 migrated planar linkage to shared persistent sessions, physical-ground/numerical-gauge certification and accepted-linearization velocity. M18 added a spatial vertical slice. M20 completed common spatial joints/mates, position drivers and assembly-mode transactions under ADR 0013. M23 completes natural and explicit pseudo-arclength continuation, typed branch-boundary hysteresis, multi-driver velocity, planar/spatial consistency and canonical spatial persistence under ADRs 0016-0018. Private gauges and augmented rows remain ephemeral, and only separately solved ordinary physical sessions are published. These milestones do not add forces, reactions or dynamics.

## 13. Equation audit and persistence

Every executable residual row has structured audit metadata generated with the equation, never duplicated in a UI. An accepted-state audit groups rows by persistent domain source and reports:

- runtime and persistent source identity;
- readable source label, equation template and named feature bindings;
- hard/temporary/preference category;
- target, units and characteristic scale;
- raw and normalized finite values or an explicit evaluation failure;
- elimination, active-bound, redundancy, conflict and singularity annotations;
- diagnostic completeness links where candidate analysis is bounded.

Persistence stores domain topology, continuous accepted state and every discrete branch/span/winding/gauge/assembly choice in a versioned envelope. Runtime slot-map keys are remapped deterministically and are never serialized as persistent identity. M11 establishes the alpha sketch document, M17 establishes the first planar linkage document and gauge schema, M22 and M23 complete each product schema, M24 freezes the first sketch wire DTO and migration dispatch, M25 migrates strict sketch v1 input to canonical v2 for associative construction definitions, M27 advances canonical sketch output to v3 for associative fillets, M28 freezes v1-v3 input and advances canonical output to v4 for generic fillets and trim views, and M29 finalizes public compatibility policy.

Work may develop an explicitly unstable draft-v5 representation while v1-v4 remain frozen
supported languages. A future explicitly scoped schema milestone may freeze final sketch v5 plus
separate versioned parameter, external-snapshot and desktop-workspace envelopes. Host/PDM keys,
formula graphs and application undo remain host state rather than canonical sketch
equations.

Application metadata is not solver or sketch equation state. Under ADR 0019,
embedders join typed `SketchAttributes<T>` through persistent document-element and
source-owner identities. Sidecars own their codec, migration and history policy;
they do not enter canonical sketch JSON, runtime lowering or audit equations.

## 14. Linear algebra policy

- Dense QR/SVD remains the correctness and diagnostic path for small components.
- Successful Cholesky never proves rank.
- M16 introduces pure-Rust `faer` sparse storage after canonical component-local assembly exists. Under ADR 0012 sparse QR supplies validated damped LM steps but is not the authoritative rank-revealing path; dense SVD retains the M9 rank contract.
- Dense and sparse paths must agree on independently validated geometry, rank/nullity, mobility, diagnostics and branch state.
- Sparse crossover values are benchmark-derived and reported; they never alter correctness tolerances.
- The workspace remains `unsafe_code = "forbid"`; native solver FFI is not permitted.

## 15. Roadmap allocation

- M8: accept contracts, ADRs and deterministic representative baselines.
- M9: canonical component-local linearization, internal local AD, orthogonal solve status and the complete numerical-rank contract.
- M10: persistent sessions, bounds and `SketchSession` as the first consumer.
- M11: persistent `SketchDocument`, generic sketch graph, commands, history and JSON.
- M12: editable quadratic/cubic Bezier and generic point/contact/tangency curve plumbing.
- M13-M14: disposable browser playground, E2E/import/error/performance hardening and the alpha gate.
- M15: completed manifold `Pose2`/`Pose3`, validated frames and accepted hard-equality sensitivity.
- M16: completed sparse structure, matching, hierarchy and robust planar continuation.
- M17-M22: completed persistent planar architecture, the first spatial slice, conics, the common spatial joint/mate catalog, B-splines and NURBS/advanced CAD continuity; M23 completes spatial kinematic product behavior.
- M22: completed NURBS and the built-in curve/generic differential-constraint surface; later host
  embedding work is not currently scheduled.
- M24: completed persistent element/source identity, typed application attributes and explicit sketch version dispatch.
- M25: completed associative line offsets, point-defined mirrors, directed-angle workflows and sketch JSON v2 migration.
- M26: completed visual-only line-profile detection with explicit topology, ambiguity and budgets.
- M27: completed independently validated associative line-line fillets with derived ordinary-arc endpoints and explicit ownership/branch state.
- M28: completed common-jet generic fillets, differentiable output arcs and persistent parent trim views.
- M29: completed the `0.1.0` API/persistence policy, documentation, licence,
  mutation-fuzz, performance and native/WASM/browser release gates for both
  deliverables.
- M30: completed interactive construction, fillet and NURBS UAT over public document APIs.
- M31: completed certified all-family visual profile intersections, curved topology and bounded area/containment.
- M32: completed post-expansion UAT, mutation, performance and `0.2.0` release hardening.
- M33-M38: completed production contract, retained design/accepted-state separation, cancellation, semantic operands and the standard relation/dimension catalog.
- M39: completed CAD-like core workbench foundation.
- M40: completed and mechanically qualified the headless-editor architecture, accepted scene, persistent picking/selection, drafting, projected gestures, actions, lifecycle/history and thin browser adapter; supervising-human M40.7 approval passed after targeted F4/F5 remediation.
- M41-M44: completed construction/activation, typed parameters, immutable external references and host-state workbench integration.
- M45: completed cleanup investigation and UAT-point capture; no human approval.
- M46: completed direct-test ownership/retirement freeze without deleting old infrastructure.
- M47: completed focused host-state replacement and M44 fixture/E2E purge.
- M48: completed direct workbench qualification and M40 browser-E2E/serving purge.
- M49-M50: legacy semantic extraction followed by deletion of all remaining old E2E and the legacy application.
- M51: completed single-workbench consolidation and direct-qualification hardening.
- M52: completed direct-qualified post-cleanup candidate preparation.
- M53: completed selector-led supervising-human UAT 2 gate.
- M54: completed stable diagnostics and mobility evidence.
- M55: completed alpha constraint, dimension and explicit branch-action parity in the headless
  editor and sole workbench.
- M56: completed prepared jobs and concurrency contract.
- M57: completed incremental production-scale solving.
- M58: completed sketch-operation companion and multi-interval visible topology.
- M59: completed production-topology companion.
- M60: completed advanced-workbench integration and direct qualification.
- M61: completed and approved supervising-human advanced geometry/topology UAT.
- M62: completed and approved CAD-style constraint/dimension palette, headless operand collection,
  single-owner canvas/tree input routing, relation-scoped and occurrence-preserving contact
  metadata, parameter-consistent bounded-contact neighborhoods, accepted-geometry dimension
  seeding, branch-explicit acute-degree line-angle presentation, retained dimension target
  editing and ordinary-workspace UAT.
- M63: completed and approved typed geometry-anchored constraint/dimension annotations, separate headless geometry
  reveal context and exact-occurrence proximity/picking, contextual density policy, accessible
  shared CAD SVG icon presentation and a dedicated human UAT.
- M64: completed and approved editable sample-library cleanup with purpose grouping, ordinary
  persistence/editing and directly qualified 1/2/3-DOF mechanism examples.
- M65: completed and approved predictable, bounded projected dragging for the existing editable
  mechanism samples, including twin-roller hit routing and rank-one pantograph-guide projection.
- M66: completed and explicitly approved computed-feature architecture and grouped authoring for ordinary multi-corner 2D
  Fillets outside the constraint graph, with an ordinary editable Fillet playground, SVG-scoped
  browser-default isolation and explicit painted-preview radius routing on candidate `ac31791`,
  closed under scoped human approval with `M66-KL001`; the superseded qualified solver-owned UI source remains archived at
  `origin/archive/m66-associative-fillet-2026-08-07` (`1034afc`).
- M67: completed and approved cleanup of the three developer-oriented inspector cards, frozen M40
  qualification harness, obsolete UI tombstones and audited dead code. It preserves reusable
  domain diagnostics, topology and persistence.
- M68: completed and approved ADR 0032 headless computed-Fillet direct-manipulation cut, including absolute branch
  continuation, analytic radius rail, explicit contact/retention/local alternatives,
  coordinator-owned Current-only interaction/history, thin pointer capture and a dedicated human
  Tailscale UAT. Implementation, direct/release qualification and supervising-human acceptance are
  complete as of 2026-08-09.
- M69: completed and approved ADR 0033 Profile/Construction semantics: atomic role authoring/conversion,
  role-aware operation output, explicit/implicit construction scene metadata, Fillet-discarded
  complements, shared headless pick scopes and a focused human UAT. Implementation, direct/release
  qualification and supervising-human acceptance are complete as of 2026-08-09.
- M70: completed and approved ADR 0034 headless auto-constraint drafting cut: semantic native anchors, bounded
  stage-local references, ranked/hysteretic candidate bundles, semantic suppression, atomic
  construction-plus-existing-relation publication, one editable playground and a dedicated human
  UAT. Implementation, focused direct qualification, integrated release qualification, frozen-
  replacement-candidate publication, served-byte verification and scoped human UAT are complete.
  Circle-authoring finding `M70-F001` is resolved.
- M70B: completed bounded reproduction-capsule cut under the requested scoped human sign-off. It
  transports the authoritative
  workspace v5 envelope without copying raw browser storage, restores through the strict workspace
  decoder and a newly constructed coordinator before atomic swap, and exposes a visible
  copy/paste overlay with manual fallback. It restores no legacy lab/E2E path and serializes no
  transient UI, camera, history or sample metadata. `M70B-F001` keeps persisted Local branch
  intervals open while lowering effective closed core bounds one representable value inward.
  `M70B-F002` corrects radial Normal from unintended finite-segment containment to explicit
  supporting-line incidence, seeds the affine contact at the accepted centre projection and
  separates detached accepted presentation from current fail-closed inference-publication
  authority. F001 replacement evidence, both F002 direct owner regressions, and the F002 complete
  replacement gate plus byte-verified publication all pass. M70B-H1 freezes a clean 193-row
  test-only authoring/scene oracle; its complete release gate and fresh byte-verified publication
  also pass. M70B-H2 preserves those exact bytes under milestone-neutral infrastructure. Test-only
  M70B-H3 historically retained all 193 original `PASS` rows and added four reviewed
  `feature.fillet` `DEFECT` rows without changing production/runtime behavior or release bytes.
  Authorized F003/F004 repairs now make those same four cases pass: active explicit Coincident
  equivalence owns closure topology, and persisted circular-plus-affine evaluation may traverse its
  complete certified explicit branch cell. F005 adds bounded certificate transport for a moved
  affine source when fresh interval cells overlap, preserving rejection at a real tangent barrier,
  fold, singularity or ambiguity. The M70B closing fixture is 198/198 `PASS`; F005's exact
  owner/golden and aggregate golden qualification, formatting, warnings-denied all-workspace
  Clippy, locked all-feature workspace tests and the relevant WASM check pass. Clean F005 source
  `d400c4a8201f6afc531f5b504424d6430dbf3937` passes the complete release gate. Its immutable
  seven-file snapshot `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` was served at
  `http://100.94.63.83:8080/` for M70B; every file and `/` byte-matched, with ordered-manifest
  aggregate `3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`. That server has since
  retired. Clean closing source
  `48e3cc3` passes the full gate with the focused multi-feature and finite-arc regressions and
  produces byte-identical release output. M70B is closed.
- M71: completed and explicitly approved ADR 0035 retained-drafting-relations milestone. It
  promotes point-pair
  Horizontal/Vertical, point-to-native-span-midpoint Horizontal/Vertical, Concentric and Collinear
  into the ordinary retained lifecycle, extends the headless authoring/inference contract and
  preserves frozen v4 through explicit wire isolation. Its original four nine-row relation
  families extend the current canonical fixture to 234/234 `PASS` at
  SHA-256 `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`.
  Focused `M71-F003` coverage owns the midpoint-axis correction, focused `M71-F004` coverage owns
  complementary endpoint-axis/direction composition, `M71-F005` owns distinct-reference
  Horizontal-plus-Vertical point-axis intersection and `M71-F006` owns the tighter default capture
  envelope. Clean post-F005/F006 qualification and byte-verified replacement publication pass;
  F003/F004 evidence remains historical and the scoped focused human UAT is approved.
- M72: completed and explicitly approved public-workbench bulk-fix and GitHub Pages milestone. It
  removes rectangle auto-dimensions, clears stale Problems, moves persistent tool overlays into the
  scene and publishes the artifact-based Pages workflow without changing solver semantics.
- M73: completed and explicitly approved retained-authoring semantic consolidation. One private
  construction-stage description and authenticated terminal inference candidate replace duplicate
  unreleased editor seams without changing accepted geometry, persistence or workbench behavior.
- M74: completed intrinsic Origin/X/Y reference geometry, datum-backed relations, axis symmetry,
  bounded datum inference and production-style desktop presentation. Direct Rust/WASM owners, the
  270-row reviewed golden, independent review, complete clean gate, Chromium checks and frozen
  artifact pass. The supervising caller explicitly approved scoped closure on 2026-08-16 while
  transferring U1-U8 without a retroactive M74 pass into the M75 bug-fixing/UAT follow-up.
- M75: completed hover and primary pointer-owner parity milestone. One headless resolver owns the
  Select target order and invalidation lifecycle; domain-aware ordinary/Fillet resolvers own the
  exact next compatible authoring operand. The initial clean immutable candidate passed but was
  withdrawn after M75-F001 exposed discarded authoring movement. Its correction passed the clean
  replacement gate, then M75-F002 exposed native paint hiding a computed radius grip/surface and
  identity-free rail/spoke markup. The complete-paint-stack/shared-affordance correction passes
  focused native/WASM/web/browser qualification and the complete clean replacement gate. Exact
  source `553fd912730b1de3b39736c49b669e94cabdd2c3` is frozen and byte-verified. The supervising
  caller accepted the qualified carried/new interaction scope. Pages run `31939764951`, artifact
  `9261974799` and deployment `5929879555` are exact-verified, completing M75. M76 has since
  superseded that publication as current public authority.
- M76: completed the production-quality annotation cut. `geosolve-constraint-editor` owns exact
  linear, radial, angular and compact-glyph paint/hit geometry plus deterministic automatic and
  presentation-only manual placement; the demo persists only a fail-soft workspace-v6 cache.
  Shared-endpoint acute/right line angles use the actual finite-ray interior wedge, while obtuse
  joins retain a value-consistent acute supporting-line presentation. Origin remains an immutable
  selectable/inference-capable datum and Reference-tree target, but its X/Y-axis intersection now
  communicates zero without a redundant canvas marker, label or focus target. Feature commit
  `a9fd6f6a71edf5be9d9fb5856074d291192a898d` is included in final clean-qualified source
  `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
  `248cba4509a992aeff7a02dd6d57a1a2481380a4`. Its exact no-rebuild seven-file Tailscale snapshot
  `/tmp/geosolve-m76-final-uat.65Y8J1`, aggregate
  `967f0c1943c16b9c4a9975aeb973ad0cfe2c6e3dbfab45f414d0dac1bb9088f3`, remains separate
  byte-verified candidate evidence. That final source passes Pages run `31961652265`, including
  qualification job `95200423007` and its `184.090683967s` sparse corpus after all semantic
  assertions. That observation exceeds the retained `180s` advisory target but remains below the
  enforced `240s` shared-runner ceiling; the earlier `209.696267408s` and `208.757508921s` attempts
  were timing-only infrastructure failures. Deploy job `95204687455` and deployment `5933831093`
  succeed. Artifact `9267811418` is 2,164,829 bytes, has Actions digest
  `dba7e2f5e1b7a51390ec1d840e7869d69968114bcf13250e641448a02d0cb60b`, extracted-tar SHA-256
  `be18173d61fef8ead3d00cf2dd560f893a7731eff7fa3bdfc0b81aadab6298e5` and ordered seven-file
  aggregate `41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96`. Root and all seven public
  files byte-match that Pages artifact at the expected media types; this makes no byte-identity
  claim against the separately built Tailscale snapshot. The caller accepted U1-U4 for scoped
  closure and explicitly waived a separate post-refinement replay. M76 is complete. M77 is
  clean-qualified and immutably nominated, with human UAT and public closeout still active.
- M77: implemented selected-curve control cages, exact trim/size/rational projections, immutable
  prepared previews, unified headless direct manipulation and exact property metadata. Focused
  native/WASM, retained-coordinator, replay, demo and unchanged-golden evidence passes. Resolved
  findings F008-F011 retain point-alias ownership, stored rational fallback, directed trim
  orientation and precision-preserving homogeneous controls. Exact source `51a3b95`, tree
  `8d154a1`, passes the clean gate; snapshot `/tmp/geosolve-m77-uat.1mDjQv` is read-only,
  byte-verified and live for review. Human/public acceptance remains open, so M77 is not part of
  the accepted baseline yet.
