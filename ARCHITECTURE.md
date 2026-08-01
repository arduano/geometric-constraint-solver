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

- **Baseline:** implemented and accepted behavior through M65. M44 completes focused host-state workbench integration over the M33-M43 production contracts. M45 preserves ten UAT points and inventories the old UI/tests without recording human approval; M46 freezes direct ownership; M47 replaces the broad host composition with five direct fixture groups and removes its controls and M44 E2E infrastructure; M48 directly qualifies the surviving workbench contracts and removes the M40 browser stack; M49 moves every retained M14/legacy semantic claim to a direct owner or reviewed retirement; M50 deletes the final old E2E, legacy route/application and obsolete browser/serving glue; M51 consolidates persistence, evidence, presentation and tests around the one survivor; M52 adds and directly qualifies the disposable in-memory UAT sidecar without product fixture state; M53 receives explicit supervising-human approval; M54 publishes stable persistent-ID diagnostics and moves raw core reports behind explicitly unstable seams; M55 completes the preserved alpha relation, dimension and explicit branch-action surface in the headless editor and sole workbench; M56 adds immutable prepared snapshots, worker-movable jobs, non-mutating patches and exact-input compare-and-swap publication; M57 retains compatible runtime/core state, dependency-local dirtying, revision-local profile caches and bounded rank/scale evidence; M58 adds the equation-free deterministic operations companion and multi-interval visible-support topology; M59 adds the read-only production-topology companion with exact accepted-input provenance and fail-closed completeness; M60 exposes the advanced curves, explicit NURBS branches, companion operations, production topology and versioned desktop workspace through the sole directly tested workbench; M61 completes approved supervising-human advanced geometry/topology UAT after targeted remediation; M62 completes approved CAD-style constraint and dimension authoring; M63 completes approved geometry-anchored canvas constraint and dimension presentation; M64 completes the approved editable sample-library cleanup and 1/2/3-DOF fixture cut; M65 completes approved predictable, bounded projected dragging. M1-M7 remain the frozen regression baseline.
- **Active target:** M66 adds exceptionally polished reusable headless authoring for associative
  2D Fillets. `geosolve-sketch-ops` remains the equation-free proposal owner,
  `geosolve-constraint-editor` owns operand/branch/preview/commit progression under amended ADR
  0030, and the sole workbench remains a thin adapter. The unapproved three-tool candidate is
  preserved at `origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`); active M66 removes
  only its Offset/Mirror authoring, UI, samples and M66-only offset requests, not the completed M25
  offset constraints or M58 Mirror companion API. The implemented authoring branch policy is
  deliberately narrower than M28's generic Fillet domain surface: affine pairs use full support,
  line/curved pairs use one interval-certified curved-parameter cell, and two non-affine parents
  remain typed unsupported until pairwise continuation exists.
- **Planned sequence:** no milestone is assigned after M66. Every newly scoped milestone ends in
  its own human UAT.

A target statement must not be exposed as an implemented capability before its milestone gate passes.

`PLAN.md` owns current execution numbering. Milestone labels in the preserved M8
completion record and in ADRs accepted before the playground rebaseline describe the
allocation at acceptance time; their architectural decisions remain accepted, but
current ownership is the completed M10-M65 sequence and active M66 cut listed in section 15.

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

### `geosolve-constraint-editor`

Owns presentation-independent sketch interaction policy over public `geosolve-sketch` APIs and the
equation-free `geosolve-sketch-ops` proposal seam:

- validated viewport transforms and deterministic accepted-scene primitives;
- screen-space persistent point/span picking and ordered selection;
- normalized gestures, drafting, snapping and action applicability;
- persistent interaction context such as remembered hover/snap identities, prospective
  inference candidates and deterministic guide/tolerance activation;
- constraint/dimension and helper-operation applicability, operand progression and explicit
  branch/side option state;
- typed document-edit, preview, commit and cancellation effects; and
- deterministic transition/replay fixtures for native and WASM qualification.

It depends one way on `geosolve-sketch` and, under ADR 0030, on the public equation-free
`geosolve-sketch-ops` proposal seam. It does not own equations, accepted-state
validation, persistent sketch identity, a renderer, DOM, widget toolkit, platform
event loop, storage or host expressions. M40.2 implements accepted scene, picking,
selection, basic relation applicability and the click/drag boundary; M40.3-M40.6
complete and mechanically qualify the state machine under ADR 0029 through one
canonical native/release-WASM report and focused browser platform evidence.

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

M66 adds a separate Fillet-only `OperationAuthoringState` rather than overloading the fixed-arity
M62 constraint/dimension collector. The editor owns finite
model-space picks, operand progression, explicit options and branch corrections, typed warnings,
scratch accepted previews, Apply/Escape semantics and repeated-mode re-arming. The retained
coordinator captures exact operation snapshots, executes public proposals on scratch retained
state and publishes only through the operations companion's exact-input compare-and-swap path.
Presentation code forwards events and renders DTOs; it does not locate fillet roots or apply
document operations independently. M25 Offset constraints and the M58 exact Mirror operation API
remain available to domain consumers, but M66 does not author them in this state machine.

The sketch domain exposes the small, non-mutating
`SketchDocument::certify_line_curve_fillet_branch_cell` query. It reuses the outward-rounded
all-family curve-piece interval kernel to prove that
`cross(curve_tangent(t), fixed_line_direction)` is finite, nonzero and one signed orientation on
the returned open `ContactNeighborhood::Local` cell. The editor calls it over the complete bounded
curved span or one explicit unwrapped period. Affine line/polyline spans instead retain
`Interior`; two non-affine-parent authoring returns a typed unsupported warning rather than
guessing a pairwise branch. None of this narrows or replaces M28's public all-family Fillet
definition, residual or validation path.

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

### Sketch companion APIs

M58 completes `geosolve-sketch-ops` for split/break/trim, line extension, exact
family-supported mirror, chamfer, existing fillet integration and ordinary drafting
macros/patterns. It constructs deterministic public sketch proposals from complete stamped
snapshots, applies them only through the ordinary retained transaction boundary and owns no
private residual equation, solver state or B-rep topology. Several visible intervals may share
one immutable support through exact fixed/contact boundary identity; canonical sketch v4 remains
the supported language until a future schema-freeze milestone is explicitly scoped.

M66 wraps the existing public M28 associative-Fillet definition without adding an equation or a
second publication path. The M66-only single-span and joined-chain line-offset requests from the
superseded candidate are withdrawn from the active API. This does not remove M25's separately
named signed offset constraints, and it does not remove M58's exact supported-family Mirror
operation-companion API. Those pre-M66 capabilities retain their original ownership and history;
only their attempted M66 authoring/UI exposure is out of active scope.

M59 completes `geosolve-sketch-topology`, a read-only companion for revision-stamped production
wires, nesting, holes and exact source provenance. It accepts only the current independently
accepted state for the complete retained input, uses visual-profile analysis solely as bounded
candidate evidence, and independently checks declared source coverage, parameter provenance,
fresh endpoints, closure, orientation/area and output limits. Complete output may feed a host
B-rep feature, but the companion owns no B-rep entities and never changes sketch solve state.
Cancelled, exhausted, truncated, skipped, ambiguous or stale results cannot be consumed as a
production profile.

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
- prospective coincident/horizontal/vertical inference remains uncommitted until
  explicit user confirmation; future candidate generation and interaction memory must
  come from the headless editor, while this crate only presents and dispatches it;
- direct native/WASM tests qualify its adapters and it always renders accepted geometry and audit data from the same result;
- it is desktop-only for all future work; responsive, tablet and mobile support are
  not implementation or acceptance targets;
- it remains non-authoritative and replaceable.

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
snapshot and direct presentation/evidence owners. Human acceptance is recorded at completed M40.7,
M53 and M61-M65; every newly scoped milestone from M66 onward ends in its own UAT after direct
qualification.

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

M66 supersedes the Fillet-authoring part of that browser boundary. The workbench remains a direct
read-only consumer of `geosolve-sketch-topology`, but Fillet enters `geosolve-sketch-ops` only
through coordinator-owned headless authoring. The editor stamps
accepted picks, prepares bounded scratch proposals, exposes independently accepted preview state
and commits only the exact token/candidate-bound result. The browser no longer depends directly
on the operations companion or interprets its Fillet request vocabulary. Offset and Mirror have no
M66 tool, options panel or sample; their pre-M66 domain APIs remain separate from this boundary.

M66 advances only the application workspace envelope to version 3. It retains the explicit
canonical-v4/draft-v5 document encodings from M60 and adds whether the stored accepted
materialization belonged to the stored current design. Workspace v1/v2 inputs migrate
conservatively without that provenance. The persisted flag is only a routing hint: the sketch
domain independently exact-certifies the accepted graph, checks compatible activity/runtime
topology, materializes it against the supplied design and requires exact full-document equality
before restoring current-design acceptance. Canonical sketch v4 and draft-v5 formats are
unchanged.

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

M66 places Fillet in the **Modify** palette. Compatible ordinary selection may seed a headless
candidate, while empty selection enters repeated operation mode. Only an independently accepted
scratch result is drawn as operation preview; Apply/Enter publishes that exact proposal through
normal retained history and Escape clears the candidate before exiting the tool. Fillet controls
are a viewport-clamped canvas overlay rather than children of the scrolling palette. Operation
hover and click both consume the same headless, preview-aware inclusive 12-pixel acquisition
result, including the exact boundary and persistent-identity tie policy; a preview-only foreground
item blocks click-through but cannot become a live operand. An invalid unconfirmed hover clears
only the transient candidate and retains both selected parents and Fillet mode. Accepted-state
eligibility compares the current publication semantics rather than literal one-shot drag request
payloads, while exact proposal compare-and-swap remains strict. Affine-parent contacts use full
`Interior` support. A line/curved pair persists the curved root in an outward-rounded certified
`Local` cell bounded by its support and tangent-parallel barriers over one bounded span or one
explicit period; a two-curved-parent attempt is visibly typed unsupported until pairwise
continuation exists. The single **2D fillet workshop** leaf is an ordinary editable
workspace, not a guided or protected scenario. Camera navigation remains web-only and usable
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
- M66: active headless authoring polish for associative 2D Fillets, ending in its own focused UAT;
  the unapproved three-tool candidate remains archived at
  `origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`).
