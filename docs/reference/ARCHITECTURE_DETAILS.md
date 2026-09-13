# Architecture

Detailed technical and historical reference. Current guidance starts in the
[documentation index](../README.md). Historical local artifact names and commands
record evidence from their original runs; they are not maintained startup instructions.

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

This reference preserves detailed subsystem contracts and their historical evolution.
See the [current architecture](../../ARCHITECTURE.md) for the supported ownership map.

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

M81 changes only private layout. `solver.rs` retains public reports/configuration and `Problem`
orchestration; `solver/hard.rs` owns component iteration and hard linear algebra;
`solver/priority.rs` owns lexicographic temporary/preference optimization; and
`solver/validation.rs` owns independent returned-row, rank/bound and diagnostic validation.
Residual evaluators are not reused as validation oracles, and solve order, controller charging,
fallback reasons and trace/source ordering remain contractual behavior.

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

M78 hardening changes no sketch equation or priority rule. It evaluates existing midpoint and
symmetry residuals with an overflow-safe midpoint form, and uses `hypot`-style norms when validating
finite segment and conic axes so representable geometry is not rejected because an intermediate
square or endpoint sum overflows. The midpoint Jacobian remains the same and has direct finite-
difference coverage; independent residual validation and non-finite rejection remain mandatory.
M78-F011 additionally clarifies point-observable drag locality and retains dependent fixed bounds
through both secondary-solve backends; it changes no residual, tolerance, priority, branch rule or
persistence schema.

M81 gives existing private sketch responsibilities cohesive file boundaries. Curve-control/conic
queries and exact Profile Offset document validation remain behind `SketchDocument`; Profile
Offset residual registration/path/incidence/audit assembly remains behind the compiler facade;
independent candidate validation deliberately stays separate in `compiler.rs`. Public paths,
canonical wire DTOs, error strings, registration/incidence/audit order and equations do not change.

### `geosolve-sketch-intent`

M83 adds an equation-free pure-Rust semantic layer which does not depend on `geosolve-sketch`, the
editor or the web crate. It owns the closed declaration schemas, stable nodes/ports/children,
typed input-slot bindings, native reservations and tombstones, separately revisioned instance/organization/external
state, unordered exact-CAS patches, canonical persistence and one bounded composite history.
Dependency-DAG order and stable identity are semantic; display names, cells and source order are
not. Accepted materialization evidence is opaque host-owned bytes at this layer, so the crate
cannot solve, evaluate geometry or claim residual validity.

Post-F007 hardening makes the canonical graph and session wires version 2 and hashes their exact
content and component identities with SHA-256. The reader accepts experimental version 1 only when
its canonical JSON, outer FNV-1a-derived digest, nested identities, evidence and checkpoint
authority all match, then migrates every identity and the exact retained-failed reservation-ledger
shape before emitting v2. That legacy digest is a deterministic integrity fingerprint, not a
cryptographic authentication or security primitive. `IntentSession::identity()` and
`semantic_identity()` are cached ordinary reads refreshed only by atomic mutation/restore paths;
import and explicit session validation independently recompute all identities and reject a stale
cache. Planning and materialization still hash their staged inputs and evidence rather than
trusting the read cache.

Decoded current, accepted, Undo and Redo checkpoints receive the same structural graph, instance,
external-input and reservation-ledger validation. Accepted materialization artifacts are bounded
and their digest is rederived before planning, import or publication. A central declaration
descriptor derives field schema, typed required/literal/conditional/contextual defaults,
closed/contextual choices, output identity flow/native reservation, edit classification and one
semantic projection path for each canonical input, definition field, output and writable leaf
from the same Rust catalog used to validate declarations. Paths are bounded non-empty field-rooted
sequences of named object members and numeric array indices. The 109-family audit rejects
duplicates, leaf/container prefixes and locations which change between object and array.
Inspector, graph/RPC snapshots and typed code clients consume that descriptor. The low-level
Intent IR projection projects the same already validated nodes and stored values as nested ordered `inputs`,
`definition` and `instance` trees but owns no separate defaults or schema table.

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

It depends one way on `geosolve-sketch`, `geosolve-sketch-intent` and, under ADR 0031, on
`geosolve-sketch-features`. The unreleased general ADR 0030 operation-authoring facade and its
dependency were removed when M66 closed. ADR 0037 narrowly reintroduces a direct
`geosolve-sketch-ops` dependency for deterministic Profile Offset proposals; the editor still owns
semantic authoring and the coordinator alone consumes an exact-stamped proposal. These dependencies do not own
equations, accepted-sketch validation, a renderer, DOM, widget toolkit, platform event loop,
storage or host expressions. M40.2 implements accepted scene, picking,
selection, basic relation applicability and the click/drag boundary; M40.3-M40.6
complete and mechanically qualify the state machine under ADR 0029 through one
canonical native/release-WASM report and focused browser platform evidence.

Under ADR 0040, the editor owns deterministic graph-to-document materialization above those same
public domain APIs. It translates exact reservations, records logical/native ownership and reverse
free-leaf routes, cold-reconstructs through the native retained solver and publishes accepted
evidence only after existing independent finite/residual/domain/branch validation. Canvas,
Inspector, structured source, operations and DOM-free RPC submit one closed patch vocabulary and
share the intent session's composite history. Structured-source token edits carry their exact
originating session/revision/digest identity, and both source and Inspector project stable input
bindings read-only through declaration symbols and semantic output paths. Canonical padded storage
slots/selectors never become presentation field names. Direct pointer previews reuse retained
native solver state and keep only
authenticated gesture data transient; the latest accepted sample remains distinct from a later
rejected attempt and owns exact-once terminal publication. They do not replay or serialize the
graph per frame.

The materialization map is accepted only after independent comparison with the exact semantic
identity, declaration graph, instance values, reservation ledger, native sketch document and
computed-feature document. Stable ordering, every port and identity-flow binding, exact reservation
owner/port provenance, node ownership, reverse writable leaf, aggregate topology and native object
existence must agree. Logical-only curve-span, computed-feature and Fillet-corner outputs are also
matched to their exact owning declaration and ordinal; a kind-compatible permutation therefore
rejects. Accepted evidence and the complete map are revalidated on cold restore, never installed
because a digest alone matches.

Property and authoring previews now retain an opaque `PreparedProjectionalTransaction`: the exact
CAS plan and independently cold-validated materialization generated for the visible preview.
Computed-Fillet Apply, Profile Offset Apply, accepted Fillet-radius drop and accepted Profile
Offset-distance drop publish that same prepared transaction after terminal authentication. They
do not plan or cold-solve a second nominally equivalent patch at release, and stale/foreign plans
reject before either intent or accepted native authority changes.

`IntentGraphSnapshot` is the explicit bounded data query for stable declarations, inputs,
definition/instance values, dependencies and central descriptors. It substitutes only kind,
codec, byte length and SHA-256 for an opaque bootstrap payload. Structured Source likewise emits
a TypeScript-shaped object satisfying data-only `IntentSourceSnapshot`; fixed semantic roles are
named members, repeated cardinality uses actual arrays and sparse indices retain `null` holes. It
contains no callbacks, executable expressions or solver authority. DOM-free RPC reserves full graph/workbench/validation
state for explicit Snapshot. Patch and source-edit success return identity/disposition/alias
receipts, Undo/Redo return identity/moved receipts, and Inspector returns only its identity-stamped
projection. The producer conservatively proves a mutation receipt is at most 16 MiB before
planning/publication, and structured plus JSON producers share a 64 MiB response ceiling enforced
before mutation publication. The TypeScript client strictly validates the same closed response
shapes, resource bounds and session branding.

M83-F006 keeps terminal preview/cold authority fail-closed without mistaking recomputable branch
round-off for a semantic mismatch. Exact document equality remains the fast path. Only line-branch
metadata owned by Polyline and the four rectangle schemas may be replaced with the cold value, and
only after each finite preview/cold vector proves membership in the same positive branch cell;
exact draft-v5 equality is then required for every other field. Segment and Midpoint Line branches
remain explicit intent and are never normalized. Midpoint Line lowering reads that explicit field.
This is coordinator comparison policy, not a solver equation, residual, priority or tolerance.

M81 moves unchanged checkpoint encoding/decoding, restore and successful history publication into
private `coordinator/history.rs`. Durable feature candidates evaluate and checkpoint against a
cloned computed-output allocator; rejection publishes none of it, while success installs feature
intent, snapshot and allocator in the established order before the unchanged history/transient/
selection epilogue. This is the M81-F001 transactional correction, not a generic transaction layer.

M77 owns selected-only `SceneCurveControl*` identities, finite cage/guide/rail paint and hit
geometry, stored-point alias precedence, exact property metadata and the direct gesture lifecycle.
Only independently accepted prepared candidates preview; invalid later samples retain the last
valid result, and stale, cancelled or no-op work publishes nothing. The adapter never chooses
another owner from paint order.

For completed M78, ADR 0036 assigns the exact `GeometryToolFamily`/`GeometryToolVariant` catalog,
semantic stages, Shift regularization, branch actions and typed stored/coordinate/contact operands
to this crate. Every interactive recipe, including geometry-only output, lowers through one
authenticated construction plan with one explicit role per created curve and typed relation
provenance. Recipe-intrinsic and regularization relations precede ambient inference and shadow only
the redundant/conflicting source they own; compatible ambient orientation survives. Controlled
publication charges document validation and proposal-specific lowering before candidate allocation.
One cloned retained session solves and independently validates the whole plan before one
publication/history entry, and the coordinator records that exact publication before a positive
acknowledgement can consume the draft.

Rejected plans retain semantically typed draft stages. On the next exact accepted scene, persistent
points, prospective contacts, remembered references and Tangent Arc endpoint jets are
reauthenticated before reuse; missing dependencies remain a local recoverable draft issue. Derived
midpoint/reflection/circle projection, normalized circumcircle and Tangent Arc arithmetic validates
local incidence and emits only finite status measurements. Initial candidate
`1b2ce0f9d843c036e3a7023674cbf219c9f593b7` passed the clean release and immutable
served-artifact gates but is withdrawn historical evidence. The F011 replacement source
`793e9de39d78bdabfded15d8c8e79f86df0f52bc` passes both gates and remains qualified product
authority; approval descendant `a6d504e` passes exact public publication.

M79 preserves the M70 ownership split while separating automatic hover memory from explicit
candidate selection. An unpreferred sample performs the ordinary bounded generation and seals its
exact frame, ranked candidates and non-candidate guides. A preferred sample can only select from
that seal; it neither regenerates candidates nor changes anchor, datum, direction, concentric,
tracking or wake latches. Foreign frame/ID combinations clear stage state and expose an empty,
non-cycleable stale result. The additive `DraftInferenceResolution::next_cycle_candidate_id()` is
the sole headless wraparound policy. The thin browser binds a choice to pointer identity, exact
screen position and modifiers, drains the newest queued sample before Tab and retires the choice
on movement or lifecycle change.

Retained publication still trials the exact authenticated visible plan first. Only that tokenized
editor effect carries private eligibility for a second trial, and only accepted redundancy
evidence can remove a fully redundant auto direction paired with surviving point, datum,
midpoint, curve, semantic-centre or complete two-axis positional intent. The retry starts from the
original retained session and must pass the ordinary finite solve, hard-residual and redundancy
validation before one publication. Its pruned plan becomes replay truth while the original token
remains acknowledgement authority. Generic public construction plans, partial or positional
redundancy and direction-only bundles retain strict transactional rejection.

Exact product source `6874aa1`, tree `f2b70c0`, remains qualified authority. The caller accepts
U1-U5, and documentation-only approval descendant `2560ca5`, tree `bad5662`, passes Pages run
`32116835502`, artifact `9317131695` and deployment `5959116526`. Root and all seven public files
exact-verify at aggregate `5692d4a994d9d14b2bd867dd8740af0f83c497fa88888cc189b7b1fcc0a994ca`.
Pages is final public-byte authority; the separately built preview snapshot remains accepted UAT
evidence. This closeout changes no ownership or compatibility boundary.

Completed M80 is a native sketch association under ADR 0037, not an ADR 0031 computed feature. One
persistent driving dimension owns one positive scalar and a face or open-chain operand. A face
contains material-left outer and hole loops; an open chain preserves manual collection traversal.
Both persist ordered source-target same-family edge pairs, exact shared-point/endpoint-contact
junction provenance and explicit miter-turn or tangent branch state. Open terminals additionally
own normal-translation policy. Full circles are face-only; single lines and circular arcs are valid
open chains.

The ADR 0037 amendment adds a separate native publication terminal to ordinary Fillet authoring.
**Apply computed** remains ADR 0031's default and its revision-local fragments remain unavailable
to Offset. **Apply native profile** is limited to one authenticated standalone line-line corner. It
preserves the two line identities, replaces their sharp shared endpoint with two tangent contacts,
adds one ordinary Profile `CircularArc`, two endpoint `LineCurveTangency` constraints and one
driving Radius dimension. The held preview carries explicit sweep, endpoint mapping and tangent
orientations; the coordinator prepares the complete independently accepted patch beside that exact
preview and Apply only stamp-authenticates and consumes it. No `FilletSet` or Fillet provenance
survives, so Profile Offset later sees only ordinary native line/arc topology.
Radius-gesture rollback retains that exact single-owner sketch patch. Because discarded radius
samples advance revision-local computed IDs, the coordinator renews only the restored patch's
computed-scene parity and checkpoint from the current monotonic allocator; it does not reconstruct
or re-solve the native edit.

The runtime keeps `DimensionKind: Copy` by storing a `ProfileOffsetId` into a sketch-local arena.
One document dimension lowers to one source whose `SketchSourceMapping::residual_ids` names all
ordered sparse blocks. Lines reuse ADR 0020's supporting-line rows. Circles/arcs add equal-center
and signed-radius-difference rows. Open terminals and tangent joins add tangential anchors. Every
row has structured audit and finite-difference Jacobian ownership, while independent acceptance
rechecks side/direction, traversal, arc endpoint, terminal, miter and tangent predicates.

Both source and target circular-arc endpoint angles remain active. Two explicit
`ResidualCategory::Preference` rows retain the source Start/End angles as deterministic gauges
only when hard equations leave the common angle modes free. Hard target endpoint drivers take
precedence and propagate to the source; no weighted objective substitutes for a hard equation.

Equation validity alone cannot publish M80. The sketch domain reconstructs the selected source and
target operand connectivity and proves unchanged edge/loop/hole cardinality, cyclic order, family
pairing, simplicity, non-contact, orientation and hole nesting. Unrelated sketch arrangement
geometry is outside that certificate. Collapse, self-intersection, contact, split/merge, hole loss
or a branch-barrier crossing retains the last complete accepted scene. The solver never trims,
drops or creates operand topology to satisfy this dimension.

Generated targets and their shared-point or endpoint-contact connectivity are ordinary native
sketch state created atomically with the scalar/dimension. Removing or suppressing only the
association leaves those objects. Ellipses, conics, Beziers, B-splines, NURBS, external,
Construction, computed and arrangement-derived partial edges are unavailable rather than
approximated. A later variable-cardinality Offset remains an ADR 0031 computed-feature milestone.

M80 does not promote supported canonical v4. Private v2-v4 dimension DTOs freeze their seven-
variant language; canonical export rejects M80 state with `UnsupportedM80State`. Private draft-v5
stores complete Profile Offset operand/branch state in an omitted-when-empty side section, so
workspace v6 and repro-v1 retain their existing strict domain transport while historical empty
bytes stay unchanged. Compatible annotation placement is retained only by ordinary workspace v6;
repro-v1 omits and ignores that disposable cache so placement is recomputed.

`geosolve-sketch-ops` owns deterministic target construction and immutable proposals over the
authenticated topology/input stamp. The headless editor owns separate Offset authoring,
authenticated face/ordered-chain collection, branch capture, non-selectable preview and atomic
proposal consumption. The demo renders Modify → Offset in the
standard persistent bottom-left panel and owns no equation, miter intersection or topology policy.
One movable grouped annotation uses the disposable M76 cache. The exact clean-gate distribution
must remain byte-verified on preview through focused UAT before Pages publication or closure.

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

ADR 0037 extends this companion with deterministic Profile Offset target construction and an
immutable exact-stamped proposal over a complete `geosolve-sketch-topology` operand. It neither
duplicates Profile Offset residuals nor mutates retained state; the editor coordinator previews
and consumes the proposal through the existing exact-CAS transaction boundary.

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

### `geosolve-sketch-code` (optional M84 layer)

ADR 0041 places `geosolve-sketch-code` adjacent to the intent graph and headless editor. It may
depend on their public APIs; neither they nor core/sketch/linkage may depend back on it. Plain M83
sessions, workspace v8 and direct solver deployments therefore remain available without code
authoring. `@geosolve/sketch-code` has the same optional relationship to `@geosolve/intent`.

For a code-enabled project, managed `sketch.ts` plus pinned `PatchModuleArtifact` values lower to
an `AuthoringProgram`, an equation-free keyed expansion and an ordinary `IntentGraph`. The existing
editor-owned cold materializer, native solver and independent validation then produce accepted
authority. The code layer owns no geometry equation, priority or branch heuristic.

The pure-Rust managed-v1 parser accepts one bounded lossless CST subset and rewrites only
authenticated declaration, organization, declared-lens and explicit-override spans. Custom
`patches/*.patch.ts` files are user/AI-owned and never rewritten. They run only in an explicit
caller-owned Node build step which emits canonical, digest/ABI-pinned, data-only templates;
Rust/WASM/browser runtime and workspace restore validate artifacts but never execute TypeScript.

Matching paths retain
logical/native identities; insertion advances high-water; removal tombstones; reused retired keys
receive a new generation. Typed clients expose project-branded semantic refs, fixed named outputs,
mapped records and keyed/derived collections rather than raw wire IDs. Direct Polyline lowering publishes `vertices` and `segments` as typed keyed root collections beside
its exact member paths and `filletableCorners`: every vertex key maps to its native Point port and
each directed span's starting key maps to its native CurveSpan port. Recorded artifact
`each`/mapping rules may consume those roots without copied coordinates, raw IDs or ordinal
identity.

`SketchCodeSession` owns one history over project files, artifact locks, program, expansion,
current/accepted semantic overlays and one delegated nested editor checkpoint. Pointer frames
continue to use the existing retained native preview and never parse/expand code. Terminal
publication stages the matching semantic edit and commits exactly once after cold validation
parity. Code-project save, reload and repro contain complete offline authority within the 64 MiB
envelope; managed files and individual artifacts are bounded at 4 MiB and 16 MiB respectively.

M84-F003 separates transport IR from the user-facing code projection. The M83
`IntentSourceSnapshot` and `IntentProjectedPortReference` remain bounded data DTOs for audit, RPC
and Inspector consumers; they are not authored TypeScript. The optional code layer explicitly
bootstraps a supported accepted GUI dependency closure into managed-v1 source. It emits
declarations in dependency order and expresses dependencies only as lexical branded results, for
example `frame.corners.lowerLeft`, never by repeating a serialized declaration/output identity.
Those endpoint expressions parse as `ManagedValue::Reference` and direct line expansion aliases
the exact owning native point rather than copying coordinates. Raw strings, transport-shaped
objects, foreign or forged reserved-project references, wrong kinds and misspelled output paths
reject before publication. Promotion constructs a real persisted `CodeProject` and
`SketchCodeSession` before the Code surface becomes editable. It may omit only the exact canonical
fresh-workspace document foundation; every other bootstrap object participates in the all-or-
nothing conversion. Branch normalization is restricted to current code-expansion-owned Segments,
so ordinary GUI Segments beside a code project retain exact explicit branch authority.

M84-F004 extends that supported complete closure to connected Segments and existing computed
Fillets. Connected endpoints reuse lexical `.start`/`.end` members. Direct
`$.computed.filletSet` takes exactly two ordered lexical `NativeCurveSpanRef` parents per corner and
preserves all persisted contact/branch choices explicitly. The central declaration-result catalog
brands only direct line spans, rectangle edges and Polyline segments; computed host Fillet arcs are
not native spans and reject both statically and during Rust lowering. Radius accepts only a
positive finite model-unit number or branded `mm(...)`. It lowers to the existing Intent computed
feature without invoking Fillet authoring heuristics or adding solver behavior, and returns opaque
`FilletSetFeature` authority rather than synthetic native arc ports. The same checked-in managed-v1
line/Horizontal/Vertical/line/Fillet source is compiled by TypeScript, parsed by Rust and cold-
materialized. All-or-
nothing projection remains; a rejection keeps Code discoverable with an escaped read-only
conversion diagnostic, Intent IR fallback and no Promote.
The exact ordinary mouse path also retains inferred Horizontal/Vertical span relations. They are
projected as managed constraint calls against the same lexical native-span expressions, including
suppression, and lower to the existing Intent constraint kinds; the optional layer owns no new
relation or residual.
GUI bootstrap additionally authenticates the accepted validation semantic against the exact
current retained intent before serialization. A retained-failed graph can therefore render only a
truthful unavailable Code state, never source that mixes old accepted geometry and new wiring.

The optional public `CodeProject::managed_only(ProjectKey, source)` constructor is the direct
artifact-free host seam. It admits only a valid project brand and complete managed-v1 SDK source,
and rejects custom patch imports because those require pinned artifacts. Construction and parsing
do not publish solver authority: installation still performs ordinary expansion, intent
materialization and independent native validation before replacing the live project. Persisted
`CodeProjectOrigin::Authored` distinguishes this route from bundled demonstrations and promoted
ordinary scenes.

On an exact canonical fresh workspace only, the demo workbench presents one **Start from code**
action beside nine genuine sample cards. Freshness requires the canonical document foundation,
matching current/accepted semantic identities and independently validated empty native/computed
authority. Starting installs a complete editable rectangle-plus-diagonal project whose dependency
is lexical `frame.corners.*`; valid Apply, retained-invalid intent, whole-source replacement,
Undo/Redo, reload and repro remain one atomic code-session authority. The starter is not a bundled
project and does not add a row to the separate nine-demo ledger.

M84-F005 adds a bounded persistent interaction overlay between managed code seeds and intent
instance leaves. Its addresses carry project identity, readable semantic owner/output/field and
never-reused owner generation, never an intent/native ID or opaque implementation alias. Only
finite Cartesian points are overlay-writable in M84; scalar edits use authenticated managed-source
lenses. Point-seed precedence is typed overlay draft > legacy generated override > managed source
seed; Reset removes the complete semantic edit bundle and restores the applicable lower tier. This
is seed precedence, not solver priority: it adds no equation, residual or constraint. Equal
duplicate terminal writes collapse deterministically and unequal same-tier writes reject.

No semantic preference or a selected producer retains ordinary shared-point ownership. A uniquely
selected referenced consumer detaches only that consumer; its projected Segment can acquire new
intent/native identity while its code-owner generation remains stable, and retained code-owned plus
ordinary GUI dependents rebind. Repeated consumer drags use the detached point lens, and Undo/Redo
restores/reapplies the complete attachment, overlay and editor checkpoint. Rectangle-corner drags
produce one atomic two-seed update; ambiguous selected lenses and unknown/stale/type-mismatched/
non-finite drafts reject before mutation.

Workbench selection maps through accepted expansion provenance. Semantic deletion carries and
reauthenticates the exact code-session identity, accepted expansion alias and managed declaration
or generated-child address. Managed deletion rewrites the exact code-owned dependent closure while
retaining/rebinding survivors; generated-child deletion is reversible suppression. Dirty source,
retained failure, stale target or GUI ownership cannot acquire that semantic route.

Current and accepted overlays are separate authorities. If a structural attempt removes an owner
and later fails native publication, its current overlay is deterministically pruned while the exact
accepted overlay/canvas remain visible; persistence and Undo restore both. This addition
intentionally bumps the still-unreleased optional formats to
`geosolve-sketch-code-session-v2` and `geosolve-code-workbench-v2`. Prototype-v1 M84 payloads reject;
plain M83 workspace-v8 persistence remains unchanged.

The direct-authoring source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, and its immutable snapshot are historical
qualification evidence only. M84-F005 withdraws that nomination; no refreshed UAT or replacement
snapshot is claimed.

M84-F006 hardens this boundary without widening it. Imported session identities are capped before
allocator adoption, persisted managed drafts retain the 4 MiB source bound, and Reset/Restore use
typed canonical tokens. Retained failure authenticates and prunes exact direct/generated owners.
Generated Segment and circle-centre detachment carries explicit provenance through replacement,
dependent rebinding, transient cancellation, repeated drag and exact Undo/Redo. Same-tier point
conflicts compare persisted IEEE bits, including signed zero. These are persistence/resource and
transaction-authority rules, not solver equations or priority semantics.

M84-F007 separates native preview motion from semantic terminal intent. Pointer-down authenticates
one exact semantic point lens through accepted expansion provenance and retains it across frames.
No selection resolves the unique producer, selected producer keeps consumers attached and selected
consumer retains local detachment. Ordinary GUI-owned points remain on the delegated M83 route.
Its pending route stores the exact `CodeSessionIdentity`, pointer and lens, consumable only by that
pointer's dedicated authenticated terminal publisher. Generic saves, foreign/reentrant preparation
and foreign terminals reject without consuming/replacing the route; every non-pointer durable
workbench mutation first cancels capture and invalidates it; and an unexpected generic-save
rejection preserves the live native authority rather than restoring under an unconsumed token.
No-motion release/cancel is history-neutral, and Apply/Undo cannot let a stale terminal revert newer
accepted authority. Genuine unequal semantic writes still reject bit-exactly under F006.

M84-F010 supersedes only F007's single-seed durability interpretation. One lens still authorizes
the gesture, but release classifies the complete terminal movement closure against the exact
authenticated origin and persists all solver-coupled semantic point seeds atomically. A detached
consumer retains its exact post-detachment origin checkpoint. Durable rematerialization must match
the complete terminal design and current accepted document, computed features, logical/native
ownership and allocator state. Ordinary point aliases remain exact. Rectangle corners are four
semantic lenses over two canonical seeds: the authenticated corner and diagonal opposite are exact
anchors, while the two redundant adjacent aliases may normalize only under bounded finite
roundoff; signed-zero and material conflicts still reject. This is semantic transaction and parity
policy, not a solver equation, tolerance, constraint priority or branch rule.

The combined F005/F006 source `ff2e142` and its frozen candidate are withdrawn by this reproduction. The post-audit demo-web library passes 270/270 with real no-motion, exact stored-session mismatch,
durable-mutation ordering and generic-save preservation coverage; sketch-code suites and focused
Clippy/WASM checks pass. Exact replacement source
`cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
`6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, then passes the complete clean gate. Its exact
no-rebuild snapshot `geosolve-m84-f007-uat.KgW8fpLf`, aggregate
`8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`, passes identical temporary
and retained HTTP ledgers and refreshed 14/14 browser cases on each endpoint.

The creative-catalog amendment adds Adaptive Lantern Garland, Suspension Bridge, Compass Rose and
Neon Manifold. All eight projects cold-materialize through ordinary accepted native authority with
finite geometry and validated Hard residuals; representative producer drags keep their keyed
generated consumers attached/current. M84-F008 fits a newly installed project to its accepted
composed scene and falls back to the canonical Origin camera only for empty/unavailable authority. Rounded Polyline uses `mm(4)` so its four already-valid computed Fillets are visibly clear of point
markers. M84-F009 prevents canonical map order from choosing an unrelated output as a multi-output
patch result. The caller-owned compiler records the exact selected `result_output` independently of
its renamed/nested public path; Rust publishes only that selection and preserves nested collection
root structure. Exact alias/nesting/mapping regressions and exhaustive all-eight coverage check each
semantic output's declared reference kind and expanded target kind, including Mounting Plate
`plate.profile` as Profile rather than the
`ne` Point. These are optional-layer API, routing, sample and presentation changes only; no native
geometry, equation, constraint, priority, tolerance or branch inference changes. Exact source
`c74651c`, tree `a904584`, passes clean qualification; its immutable no-rebuild snapshot
`geosolve-m84-f009-uat.q8cKIN3v` passes byte-identical temporary/retained HTTP verification
and the 14/14 browser matrix on both endpoints. M84-F010 withdraws those bytes from acceptance and preserves them as historical evidence.

Historical F010 source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passes the complete clean gate, ending in successful
Trunk assembly, and its exact no-rebuild seven-file output is frozen at
`geosolve-m84-f010-uat.7R5eXQoz`. Temporary `:18091` and retained `:8080` served byte-identical
eight-path ledgers. The focused Compass case passes 1/1 on each endpoint across six drags,
release/+50/+250/+500/+1000 ms, reload and four attached spokes; the carried 14/14 browser matrix
also passes on each. M84-F011 withdraws this as historical rollback evidence. This is not human acceptance or public authority.

M84-F011 dogfoods the optional boundary with a ninth real sketch rather than a transport-only
fixture. Managed source defines one 240 × 120 mm manifold plate, a 60 × 84 mm reservoir bay, three
open water routes, three closed O-ring-groove centreline loops and eight 5 mm screw circles. Exactly
one `FixedPoint` anchors absolute placement; 21 Construction line spans, Coincident relations and
driving curve-length/diameter dimensions locate the remaining geometry. There is no
`FixedCoordinate`. The existing native solve remains authoritative, and independent qualification
requires finite accepted geometry, normalized Hard residual at most `1e-9`, all active features
Current and numerical/equality/bidirectional DOF zero.

The separately compiled AI-authored `waterChannel` patch accepts a keyed Corner collection and a
bend radius. Its data-only `each` template emits one existing native Fillet per current corner, so
the three managed invocations adapt across keyed route edits without listing corner IDs. The
managed program owns coordinates, roles, relations and dimensions; the custom patch owns only the
structural repetition. Supporting Circle, Coincident, FixedPoint, curve-length/diameter, keyed
Polyline `.byKey` and profile/construction-role syntax lower directly to existing intent/document
semantics and add no equation or runtime TypeScript dependency.

M96 replaces the manifold's centreline-only patch with finite-width water and silicone
custom patches, plus a water variant with caps at both ends for the separate stair passage. The patch-private `computed.polylineChannel` composite lowers in Rust
to ordinary native supporting-line offset dimensions, computed Fillets, Center
Arcs and endpoint contacts. The input polyline remains the geometric dependency;
numeric calculations provide initialization and explicit contact branches only. Native solving, independent residual validation and authenticated restoration remain
authoritative. The TypeScript recorder publishes a fixed semantic result inventory without
executing geometry or admitting arbitrary plan-dependent operation outputs. A validation-only host request checks accepted native line/arc boundaries after
Fillet composition, including closure, intersections and expected components. It creates no persistent computed-feature kind or solver equation. The maintainer accepted and closed M96's 12 mm amendment on 2026-09-08. Product `d77228559f9b860ce69cc03ceea6a5d34d8a3660` passes all 243 integrated obligations
in clean-source run `20260908T090848-ddc447b8`; [M96_CLOSURE.md](../M96_CLOSURE.md)
binds the accepted frozen artifact and records input limits. M96-F006 recognizes large
full-rank hard components as having no preference motion, preserving the existing rank
policy, actual free-motion behavior and independent residual validation.

PNG export remains outside every sketch/code authority boundary. The browser wraps the already
composed SVG viewport in self-contained paint rules, hides hit/provisional/error-only presentation,
rasterizes through Canvas at 2000 × 1400 and downloads `geosolve-sketch.png`. It neither reads the
native document to reconstruct geometry nor mutates selection, persistence, history, accepted
authority or the optional code session. The feature therefore remains available to plain flat
workbench deployments as presentation functionality.

M84-F012 keeps annotation visibility in that same presentation boundary. `EditorScene` carries a
default-true transient flag consumed by both SVG composition and the headless direct/contextual
annotation hit tests. Both flat and code workbenches expose one session-local **Annotations**
control and apply it before paint or pointer routing. When false, selected and problem-forced
constraint/dimension annotations publish neither paint nor hit corridors, stale annotation hover is
cleared, and the underlying geometry or datum owns pointer move/down; Fillet affordances remain an
independent computed-feature pick surface. The flag neither deletes derived annotation layout nor
enters selection, accepted-scene identity, document/code history, Intent IR, persistence or repro.
Because PNG export clones the composed SVG, annotation inclusion is WYSIWYG; export-only styling
also removes draft/inference candidates and guides along with existing hit, error and provisional
paint without mutating the live scene.

Exact F011 source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, passes the complete clean gate and freezes without
rebuild at `geosolve-m84-f011-uat.ps736NLh`, ordered-manifest aggregate
`056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`. Temporary and retained
eight-path ledgers are byte-identical at SHA-256
`9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`; focused frozen
manifold/PNG/authority checks pass 1/1 on each endpoint and preserve lifecycle, history length,
project title and viewport markup authority. M84-F012 withdraws this
nomination;

Exact F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the complete clean gate and freezes without
rebuild at `geosolve-m84-f012-uat.nMOymIIM`, ordered-manifest aggregate
`166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`. Temporary and retained
eight-path ledgers are byte-identical at SHA-256
`66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`; focused annotation paint/
pick, authority-neutrality and visible/hidden PNG checks pass 1/1 on each endpoint. Its automation alone claimed no human acceptance. U1-U16 later passed by milestone-level approval; Pages run `33068058169` and exact hosted-byte
verification pass.

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

M81 moves existing endpoint-claim conflict attribution, native-source interval composition,
discarded Construction validation, combined source-role selection and evaluation-local output-ID
creation into private `evaluation/composition.rs`. Root finding, continuation, explicit branches,
tolerances, work charging, evaluator input/output DTOs and independent geometry validation remain
unchanged.

### `geosolve-linkage`

Owns planar and spatial kinematic domain models:

- rigid bodies and body-local point, axis, plane and frame features;
- physical grounding, joints, mates, drivers and assembly modes;
- branch-preserving continuation and velocity-level queries;
- domain validation, source mapping and persistence.

The frozen baseline is planar `Pose2` linkage kinematics. M17 migrated it to persistent topology/state/session, gauge-separated mobility and shared accepted-linearization velocity; M18 established the spatial slice and M20 added stable-clock axis/plane features, common joints/mates, position coordinates/drivers, explicit mode monitors and atomic position transactions under ADR 0013. M23 completes spatial kinematics with independently published natural/pseudo-arclength continuation, typed endpoint branch events, accepted-state hysteresis and atomic explicit mode changes under ADR 0016; multi-driver spatial velocity, concrete feature fields and optional accepted-rank physical motion bases under ADR 0017; and canonical versioned persistence with deterministic runtime remapping under ADR 0018. Embedded-planar, closed-chain, mixed-scale, generated differential/property and connected sparse/release-crossover gates pass. No linkage API implies physics.

### `geosolve-demo-web`

Is a separate, non-authoritative WASM workbench and public audit/API consumer whose
primary purpose is interactive sanity checking:

- it uses public sketch document, session, command, history, serialization and audit APIs;
- React is the sole browser presentation host and owns rendering placement, accessibility, browser
  event translation and guarded browser `localStorage`; it calls an instance-scoped
  `WorkbenchHandle`/`WorkbenchBridge` rather than an installed Rust-DOM singleton;
- the retired Rust-DOM installer, global callback registries, Trunk host and classic HTML/CSS assets
  are not a compatibility fallback;
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

For completed M78 this crate presents returned family/variant metadata in persistent bottom-left
overlays, remembers only session-local menu/options state and maps platform modifiers/actions. It
does not construct rectangles/circumcircles/tangent arcs, project ellipse trims, choose a sweep or
compose intrinsic and inferred relations. Draft-local rejection and finite-only measurement DTOs
are rendered as supplied; the adapter cannot convert a nonrepresentable derived value into a status
measurement or global problem. The implementation preserves that boundary. Initial candidate
`1b2ce0f9d843c036e3a7023674cbf219c9f593b7` is withdrawn historical evidence; F011 replacement
source `793e9de39d78bdabfded15d8c8e79f86df0f52bc` passes the clean/served-artifact gates and remains
qualified product authority. Human UAT and exact Pages publication pass.

For M83 this crate renders only editor-owned Outline, structured-source, read-only History,
Inspector and diagnostic DTOs and forwards normalized typed edits/RPC requests. Outline/cell drops
resolve painted upper/lower halves into exact before/after organization slots. Workspace v8 stores
the canonical intent session plus authenticated accepted materialization; retained-invalid
migrated/bootstrap reload reconstructs the exact prior accepted scene while preserving current
failed intent and Undo. Strict v1-v6 workspaces first restore through their historical decoder and
then become per-object bootstrap declarations, while abandoned v7 rejects. The M76 annotation-
layout field remains a disposable,
compatibility-filtered presentation cache outside intent/materialization/history authority and is
recomputed when absent or invalid. M83-F007 enables New through the same canonical empty
projectional authority used by startup, clearing durable authored geometry/declarations/history
and transient authoring state, returning to Select and autosaving workspace v8 without invoking
the retired flat path. Post-F007 hardening additionally rejects workspace v8 unless its nested
intent session owns accepted authority; an absent nested authority cannot be replaced by the old
flat design/accepted fields, including the all-absent case. The web adapter renders the data-only
source and compact graph snapshot supplied by the editor and uses the strict typed RPC client; it
does not expand bootstrap payload bytes or infer field defaults. The former `fafea4e` artifact is
historical pre-hardening evidence and `1e70f3f` is the superseded post-hardening candidate. F008
enables Copy/Load repro only by encoding and validating the complete workspace snapshot before
atomically replacing live projectional authority; annotation layout is still discarded as a
disposable cache. Clipboard denial or an insecure origin leaves the complete payload selected for
manual copy. F009 composes native-parent hiding only from active computed Fillets. Projectional
scene errors are never erased with `.ok()`: legitimate missing accepted authority renders an empty
state, other failures render `data-scene-state="unavailable"` and a frame-local `Canvas scene
unavailable: ...` status, and the next successful frame clears it. F010 renders Inspector fields
as the same nested semantic object/array tree, uses one-based labels only for human display while
typed paths remain zero-based and exact, renders closed enums as selects, and authenticates edits
with the Inspector projection's own identity. The immutable F010 snapshot
`geosolve-m83-f010-uat.Qmrz2R36`, aggregate
`e01d642438ae8337e9abe1ddeadb7b176375ae40f8b411785edae717e30b5d54`. The maintainer accepted M83 on 2026-08-25 without claiming a separate row-by-row replay. Approval descendant `2006c86`, tree `c4a59d2`, passes Pages run `32817232564`; downloaded artifact
SHA-256 `06bce15ddea6d21048a25e3630a368ebe0ba883be98ee296869f77c47b86218b` and hosted-results
SHA-256 `bb7423477868aafc7752b766ea2f6fb5461e1d31846dd14f9ebafad7ede42ace` prove exact publication.

Workspace encode/decode shares the reproduction codec's 64 MiB admitted-workspace ceiling. A
narrow version probe and disposable-cache visitor avoid an arbitrary `serde_json::Value` tree;
outer canonical comparison streams; one serialized digest payload serves SHA-first and conditional
legacy checks; ordinary v2 intent is authenticated once and passed into snapshot validation. This
keeps hostile-input cost explicitly bounded without making presentation cache state authoritative.

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
seven-file snapshot at `geosolve-m70b-f005-uat.Q5c9Wi` was served and byte-verified at
the archived preview for M70B with ordered-manifest aggregate
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
completed M40.7, M53 and M61-M77. Newly scoped milestones normally end in hands-on UAT after direct
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
rank; only uncovered point-observable passive mobility is anchored. A hard-nullspace direction
that changes scalar curve/contact state but no persistent point position neither admits nor
requires a point anchor. Candidate points are chosen by greatest rank gain, then lower mobility
rank, then compile order. Anchor coordinates are captured from the gesture-start accepted visible
geometry, not from advancing numerical seeds.

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

Every coordinate bound whose lower and upper values are identical is an equality during a
secondary solve. Dense-nullspace and projected-CGLS working sets retain all such `Fixed` rows even
when their projected normals are linearly dependent; rank-revealing projection handles the
redundancy. The independence check still runs so bounded-operation accounting is unchanged.
Dropping a dependent fixed row could otherwise let roundoff rediscover the same equality forever
as a zero-length bound event.

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

See the [roadmap](../../PLAN.md) and [milestone history](../history/README.md).

## M89 executed, reversible managed source

Status: **Superseded by M90's closed clean-break contract. M89-F004/F005 implementation, final
provisional dirty-tree mechanical qualification and immutable F005 nomination completed, but its
Compass Rose retest, targeted F004/F005 preflight and row-by-row human UAT were not run and are not
retrospectively passed or waived.** F005 and M89-F003 are historical rather than current product
candidates; M90 owns replacement product authority.
ADR 0043 and the M89 goal, implementation and UAT documents own the exact candidate contract and
evidence. M89 adds no solver equation, tolerance, branch inference or browser-owned acceptance
authority.

M89 makes managed `sketch.ts` one reversible authority rather than a parser-shaped seed plus
GUI-owned exceptions. A closed-subset parser captures imports, attached comments, declaration
names, groups, order, expressions and explicit suppression in versioned `ManagedSketchIrV2`; a
canonical printer normalizes that IR back to source. Unsupported control flow, helpers, mutation,
async, dynamic imports and ambient I/O reject before execution. `.patch.ts` remains the separately
pinned liberal extension point and is not reversible managed source.

Pinned compilation injects stable source-site IDs into supported SDK calls. Deterministic recorder
execution emits `ExecutedSketchArtifactV2`: the complete declaration/result tree, generated patch
members and value-consumer provenance. Durable edits use those injected IDs rather than stack
traces; stacks/source maps are diagnostic only. Browser and pinned-Deno source hosts must emit the
same normalized source, IR, artifact and digests. Canonical compiled projects continue to expand,
solve, independently validate and render through pure Rust without either host.

Every structured code, panel or completed canvas edit is a digest-bound two-phase transaction.
Rust prepares the exact permitted mutation, a compiler host mutates IR/prints/executes the
candidate, then Rust validates the ticket and semantic delta before cold materialization and
independent native acceptance. Source, IR, artifact, scene and one outer history row publish
together. Preview, cancellation, stale input, unsupported syntax, non-finite state and failed
validation publish none of them. Code projects have no GUI-owned fallback.

Newly drawn recipes become user-facing declaration closures under a visible `Canvas additions`
group with a persisted non-reusing high-water name. Most closures contain one declaration. Profile
Offset retains one explicit aggregate helper plus its operation root; closure ownership is admitted
only from exact authenticated IR semantics. The helper remains selectable, editable and source-
navigable, but cannot independently reorder, suppress or delete. Root reorder/delete operates on
the complete closure, while an aggregate with another semantic consumer remains an independent
root. Top-level declaration order is real source order; generated members nested in the declaration
panel retain their owner; both top-level and generated suppression are explicit source/IR state. V2
needs neither a terminal `$.outputs(...)` nor `p.editLens`: declarations materialize independently
and executed/template provenance owns parameter fan-out.

Geometry source projection is complete across the M78 25-variant catalog without embedding native
transport details. Segment uses direct `$.geometry.line(...)`; an exactly proven one-native-curve
Polyline uses direct `$.geometry.polyline(...)`; the remaining 23 variants use one compact
lossless `$.geometry.recipe(...)`. That compact object contains semantic paths and authored data,
not selectors/equations. Rust derives input slots, definition-field schemas, writable leaves and
the complete result path/kind tree from the central `IntentNodeKind` descriptors and rejects any
source declaration that differs. TypeScript provides parser/printer/recorder ergonomics and dynamic
lexical result references, but it neither copies geometry equations nor becomes a second recipe
schema authority. Transport-level `$.intent.recipe(...)` remains available for other explicitly
authenticated non-geometry fallbacks; it is no longer the canvas-authored geometry surface.

The persistent constraint catalog uses an equally explicit split. Of 35 persistent
`ConstraintKind` variants, 33 cold-replay from standalone managed source. Horizontal and Vertical
retain direct SDK builders; the other 31 use one descriptor-authenticated
`$.constraint.recipe(...)` declaration whose canonical inputs, fields, writable values and exact
result tree are re-derived by Rust. Transport-level `$.intent.recipe(...)` is not emitted for any
of those 33 variants. `ExternalPointCoincident` and `ExternalLineCollinear` remain fail-closed:
they remain compile-visible and schema-authenticated, but require separately supplied host
snapshots. F004/F005 implement no standalone host-snapshot execution path, so they are gated rather
than serialized as if that authority existed.

Computed Fillet has a direct reversible source contract rather than a transport fallback. One
canvas gesture emits one `$.computed.filletSet(...)` declaration with lexical native-span parents,
exact parameter/winding/neighborhood/normal-side/retained-endpoint/periodic-anchor/endpoint-order/
sweep state, radius, display name and explicit source-owned suppression. Cold materialization
reconstructs the same computed owner and explicit branch/contact metadata. Selection resolves to
the whole authenticated declaration; radius edits preserve selection; suppress/restore and delete/
Undo reproduce exact source and accepted authority. A computed host arc cannot silently become a
native parent, and non-lexical, wrong-kind or malformed state rejects before publication.

F004/F005 add no Offset architecture. The previously implemented Profile Offset declaration
closure and its helper ownership remain unchanged; no broader Offset family or source claim is
introduced.

Untouched managed-v1 projects and checked-in samples retain an isolated compatibility path in M89.
The first structured source-aware edit upgrades only the active copy; unrepresentable persisted
GUI/source hybrids reject entirely. ADR 0043 owns the complete boundary. M90 is planned to
normalize the sample corpus before deleting legacy support; M89 neither rewrites bundled legacy
bytes nor removes the v1 path.

Final mechanical evidence is deliberately provisional dirty-tree evidence, not a clean-source
claim. Exact gate
`nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh'` passes in the pinned
shell with `wasm-bindgen-test-runner 0.2.121`; its final counts include sketch-code unit `114`,
compact geometry `4/4`, constraint matrix `1/1`, direct Fillet `7/7`, editor insertion `19/19`,
demo-web `358/358`, TypeScript runtime `28/28`, mutation `19/19`, Deno parity `2/2` and frontend
`55/55`. The preceding ambient-shell WASM-parity `HARNESS_ERROR` was caused only by the runner
being absent in the original environment and is not a product defect.

Immutable snapshot `geosolve-m89-f005-uat.hzNuDxF0`, manifest
`geosolve-m89-f005-uat.hzNuDxF0.sha256` and aggregate
`fb488ad2bf29e8897cf9811c002b748693e5d211bae4bb54c83ed060db5db668` preserve the historical M89
F005 nomination. Its `16,333,537`-byte `assets/geosolve_demo_web_bg-52ybei8k.wasm` has SHA-256
`3e6f515ff1e5de0f668c13e86c02d280c0dc085bbd89b314bf9ece6c82aae575`; staging and live ten-route
ledgers are byte-identical at SHA-256
`4de184eb1eb237f98997b70702567a2b110b40d96df5d0d9653f024ac5f23e8d`. The focused Fillet browser
case passes `1/1` and the normal frozen product passes `15/15` on both endpoints. automated evidence accepts no human row.

## M90 typed executed sketch clean break

Status: **Closed by explicit scoped maintainer approval on 2026-09-04. M90-F005/F006 repairs,
collateral qualification, the complete dirty-tree release gate, optimized release-WASM build and
immutable preview nomination pass. M90-U1 through M90-U10 transfer/defer, without passing or
waiver, into M91's composite UAT. The exact closing candidate remains private preview at
the archived preview; no public deployment was made for this milestone.**
`docs/M90_GOALS.md`, `docs/M90_IMPLEMENTATION.md` and `docs/M90_UAT.md` own the current contract.
The M89 section above remains its historical compatibility-stage record and does not describe
current managed-source authority.

The historical pre-F001 pinned dirty-tree release gate exited `0`; its log has SHA-256
`2dd3663430c730eea84303954598f3a0696868aa4d433f3e32836a42024b250b`. The unchanged 271-row
milestone-neutral golden passes `--survey`, `--check` and `--require-clean`; this is unchanged oracle
evidence, not clean-source qualification. The historical frozen release output is
`geosolve-m90-uat.vuI7sBKt`, with manifest `geosolve-m90-uat.vuI7sBKt.sha256` and aggregate
`b5bae1aca28787f026a11100c94e425d1c5e057ce3539170f4399b7cd8b05dc2`. Its `20,007,307`-byte
`assets/geosolve_demo_web_bg-BAUG7n7P.wasm` has SHA-256
`51fc04d4129dd73791afb20b4403efe1f4fb95af6d607037b2a95956865fe7f5`. Local and preview HTTP
ledgers matched at SHA-256 `17477e87e897b5ac080547df41b528bc16c252d4c67a4634ad21a384f4ccd29b`. M90-F001 withdraws those bytes from continuing UAT while preserving
all hashes and identities above as exact reproduction evidence. At that
historical checkpoint no M90-UAT row had been accepted and the milestone remained open.

M90 admits one managed directive, `"use geosolve sketch"`, and one compiler-envelope pair:
`geosolve-managed-sketch-ir-v3` plus `geosolve-executed-sketch-artifact-v3`. Parsing owns lexical
names, comments, order, groups, expressions and UTF-8 source spans. Instrumented execution owns
declaration results and source-value consumer provenance. Stable source-site identities join those
views, while Rust authenticates their complete source/IR/artifact envelope before materializing
Intent. Stack traces remain diagnostic evidence and never choose an edit target.

The public TypeScript surface consists only of named, typed builders under `geometry`,
`constraint`, `dimension`, `operation`, `aggregate` and `computed`. Named argument objects expose
semantic roles such as `start`, `control`, `firstControl`, `secondControl`, `end`, `first` and
`second`. Public tuple-key input tables, generic recipe declarations, edit lenses, result manifests
and operation-output transport payloads are removed. Patch modules remain separately compiled,
pinned extension points; their private transport representation is not a public managed-language
escape hatch and need not reverse into managed lexical IR.

Every accepted canvas declaration returns to `sketch.ts` under a monotonic generated name. Value
edits, insertion, reorder, suppression and deletion begin as digest-bound Rust-prepared mutations.
A browser or pinned Deno host applies the candidate and emits a compiler receipt; Rust validates
the permitted semantic delta, cold-materializes it, solves and independently validates it before
source, IR, artifact, accepted scene and exactly one outer history row publish together. Stale,
unsupported, non-finite or invalid candidates retain the complete previous accepted authority.
There is no GUI-only fallback for a managed source project.

An insertion's semantic closure includes generated SDK helpers. TypeScript recursively derives the
closed generated unit set (`mm`, `rad`) from authenticated draft values and appends each missing
binding once to the existing `@geosolve/sketch-code` named import that owns `sketch`. It preserves
all existing import declarations and binding order. Rust independently derives the same expected
imports before calculating candidate semantic authority, so receipt validation admits exactly this
delta and rejects missing, extra, reordered or unrelated import edits. Unit-free insertion and all
non-insertion mutations leave imports exact.

Ordinary movement of an existing code-owned point is deliberately outside that compiler route. It
is a persistent solver-instance edit: pointer preview and release stage the semantic point set into
`CodeInteractionOverlay`, incrementally materialize from accepted continuation, validate terminal
parity and synchronously publish the prepared project overlay with exactly one outer/native history
action. Source, lexical IR, executed artifact and compiler identity remain unchanged; no managed
ticket or browser compilation is admitted. Explicit source/scalar edits, declaration insertion,
reorder, suppression, deletion and computed-Fillet radius edits retain the compiler transaction.

M90-F001 confirms a code-workbench/Rust-bridge ownership regression in this boundary. Exact user
reproduction is to open **Compass Rose**, drag a point and observe the entire canvas blocked by
`pointer input is unavailable while a managed-source mutation is compiling`. Pointer-up wrongly
converted the drag into `ManagedSketchMutation::SetValues`. Cold replay of the underconstrained
sketch selected a different valid configuration from the native continuation accepted during
preview; parity correctly rejected the mismatch, but the rejected ticket remained in
`pending_managed_mutation` and the global bridge guard disabled all subsequent input. This is not a
solver, rank/DOF, branch or equation defect.

The implemented repair covers ordinary pointer-down/move/up and selected-reference detachment:
finite independently valid accepted movement, persistent overlay publication, exact source/IR/
artifact/compiler identity retention, exactly one native history action, no browser compiler
request or pending managed mutation, and an immediately usable next pointer gesture. A failed or
mismatched delegated terminal clears its semantic route and restores the exact accepted editor,
selection, project, source, code-session, persistence and history authority. This changes no solver
equation, rank/DOF rule, tolerance or explicit branch state.

The five focused bridge/session commands pass `1/1` each; full demo-web `--lib` passes `282/282`,
and sketch-code all-feature, affected multi-crate, frontend `56/56`, TypeScript runtime `31/31`
plus types/managed/build, release-WASM two-drag Compass `1/1`, locked WASM parity/`actual_wasm`/
check, format, diff and warnings-denied workspace Clippy pass. The generic golden survey is clean;
two transiently failing scene rows pass exact rerun and the subsequent complete unchanged
`--check`/`--require-clean` pass. The dirty monolithic release gate ended by harness termination at
exit `143` and is not a pass.

The historical pre-F002 provisional dirty-tree replacement is frozen read-only at
`geosolve-m90-uat.xk0AGnz0`, manifest `geosolve-m90-uat.xk0AGnz0.sha256`, aggregate
`030e9f4aa98690b8cd35cdbb51a29220674f1bfcfba310192afc467f5afc38a4`, with nine mode-`0444`
files, two mode-`0555` directories and no symlinks. Its `19,958,913`-byte
`assets/geosolve_demo_web_bg-B6mOdH7K.wasm` has SHA-256
`9607cfd48f1ec23b2c29e120704277bbb70247bdbed6a67945c5cc64b7af8761`. All ten staging/live
routes byte-match with correct MIME, no redirects/compression and ledger SHA-256
`56a5aff7b23000e1b009f2eb479b9545fcfb17dbe5d1a4f9721f7c3951a761ae`; optimized release-WASM
two-drag Compass passes `1/1` on both. Evidence is
`geosolve-m90-f001-replacement-freeze-evidence.UpMqFyrm`. M90-F002 retired the unit.

This immutable publication is historical pre-F002 evidence, not a completed normal release gate or
clean-source qualification. M90-F002 replaced quota-limited project persistence with serialized
raw IndexedDB authority and froze `geosolve-m90-uat.TN2NP9eF`; M90-F003 now withdraws those
post-F002 bytes from continuing UAT as well.

M90-F003 is owned jointly by the managed mutation printer and Rust prepared-mutation authority.
From the exact empty coded starter, a two-click Center-Radius Circle produced `radius: mm(...)` but
left the SDK import at `{ sketch }`, so parsing rejected the candidate as an unsupported managed
value expression. The repair adds only the recursively required `mm`/`rad` helpers under the exact
cross-language rule above. It does not hide the issue by preloading the empty starter. After the
import compiled, cold materialization exposed a second contract mismatch: the reverse projector's
optional `label` and `role` were not admitted by direct circle lowering. The lowerer now requires
`center`/`radius`, admits only those two optional presentation fields, maps them to native display
name/geometry role and rejects any other field.

Exact TypeScript mutation, Rust receipt/native-lowering and retained-bridge regressions own the
empty-starter candidate, helper idempotence and forgery rejection, one finite independently
validated circle, one source/history action, Undo and immediate next-pointer availability; these
focused checks pass. The equivalent optimized release-WASM browser flow passes against byte-
verified staging and live bytes. Snapshot `geosolve-m90-uat.yIPVNICT`, manifest
`geosolve-m90-uat.yIPVNICT.sha256`, aggregate
`d31e311c4e0e69974690d299819df6a33b7ae13b7eff5d037406e602c033f33a` and historical private preview. M90-F004
withdrew those bytes from continuing UAT and retired the service; no human-UAT result was accepted.

M90-F004 aligns generated names for simultaneously ready same-family declarations with the durable
native-symbol allocation order used by cold materialization. This lets one Segment snapped to two
circles publish its Segment, two Point-on-Curve relations and inferred Horizontal atomically without
exchanging declaration/native ownership. Its post-F004 snapshot is historical because M90-F005
superseded it.

M90-F005 makes authored scalar units follow the owning Intent leaf rather than native storage alone:
an angle-backed periodic contact `Parameter` remains dimensionless. Point and curve-control release
then project the exact solver-accepted terminal onto the independently reconstructed candidate and
certify that numerical continuation before publication. Undo and Redo use each target history
position's authenticated accepted materialization, reconciling only candidate-owned allocator
high-water. Continuation grants no source, declaration, operation, branch, history or success
authority. Constraint-editor all-features passes with unit layer `439/439` plus every integration;
demo-web passes `286/286`; frontend Vitest passes `74/74`; warnings-denied Clippy, format/diff, the
unchanged 271-row golden, optimized release-WASM build and nine-file distribution validation pass.

M90-F006 preserves authenticated presentation state across successful atomic `project.import`.
Before repair, bridge replacement restored default host size/pixel ratio while the unchanged DOM box
emitted no new `ResizeObserver` sample, leaving SVG letterboxing and pointer normalization in
different spaces. Successful import now retains the live `host_size` and `pixel_ratio`; failed
restore remains non-mutating. Exact regression
`successful_project_import_retains_live_viewport_and_pointer_alignment` covers a non-default
letterboxed viewport and semantic point hover. This changes no document, solver, branch, tolerance,
history or persistence authority.

The current immutable optimized snapshot `geosolve-m90-uat.EtWyWQlt`, manifest
`geosolve-m90-uat.EtWyWQlt.sha256` and freeze evidence
`geosolve-m90-f006-freeze-evidence.HqyaA7Qp` have ordered aggregate
`b3fd72b9ec98d318d7bfa7bf8c09d0fcbd3856ea0723d81e01e64945e301debe`. Its
`19,990,463`-byte `assets/geosolve_demo_web_bg-Dc5MH04n.wasm` has SHA-256
`bad16242c2ec0fa0c6c0ba6882428372c1bf0b7dd1a70235467b5febdfc80712`. Staging/live HTTP ledgers
match at SHA-256 `41d11e1c56bad8dcc57edf229f0bfec20d8f5e602b3c385e5e68d54dd42c816a`, and the optimized
release-WASM browser bundle passes `4/4` against each endpoint.

The final full dirty-tree gate command
`env GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from `23:20:45` through `23:47:22 AEST` on 2026-09-03 and exited `0` after `1,596,726 ms`.
Its `573,421`-byte log `geosolve-m90-f006-full-gate.hKSTh4/release-gate.log` has SHA-256
`bda7f5f92f15a5f0a0cf26ed93cb514943d9a9d1ad49bf0ba0e148c9239b205d`. It preserves the reviewed
271-row golden unchanged, passes the release-only 256-moving-body performance row in `137.82 s`,
frontend Vitest `74/74`, and optimized nine-file distribution validation. This is complete dirty-
tree qualification, not clean-source qualification.

The maintainer explicitly approved scoped closure on 2026-09-04. M90-U1 through M90-U10
remain unexecuted and transfer/defer—not pass or waive—into M91's composite UAT. The F006 snapshot
and service remain private preview closing publication authority; no GitHub Pages deployment or public
push was authorized or made. Automated qualification accepts no human row. M90 is closed; `PLAN.md`
owns the complete evidence and M91 intake ledger.

All twelve bundled samples are canonical V3 projects. There is no managed-v1/v2 parser, upgrade or
compatibility route. The replay inventory covers all 25 geometry variants, 33 standalone
constraints, all eight dimensions, every ordinary operation, both aggregates and computed
`FilletSet`. `ExternalPointCoincident` and `ExternalLineCollinear` remain explicitly input-gated;
M90 does not fabricate standalone host authority for them. Profile Offset may retain its explicit
aggregate-helper/root closure, and custom patches remain the non-reversible extension boundary.

Compiled projects can be inspected, solved and rendered through pure Rust without a browser,
JavaScript runtime or web server. Raw-source headless editing is deliberately a three-stage
Rust-prepare → pinned-Deno-mutate → Rust-resolve workflow using the same mutation receipt as the
browser. Existing output directories are never overwritten. TypeScript records declarations but
does not own equations, native materialization, convergence, branch/domain validation, accepted
scene publication or history. Every success-like result still requires finite native state,
Current active computed features and independent normalized Hard-residual validation at most
`1e-9`.

## M91 cohesive code-driven-authoring architecture

Status: **Complete and publicly closed on 2026-09-04 after explicit maintainer approval.**

The following architecture is implemented, clean-qualified, immutably frozen and byte-verified on
its private preview service. The maintainer's blanket/composite approval accepts M91-U1 through
M91-U14 as Pass without claiming a separately logged row-by-row replay.

### Contact topology, authored limits and continuation

`ContactDomain` owns intrinsic topology derived from the referenced curve: bounded, periodic or the
explicit supporting-line variant. `ContactAdmissibleRange` is an optional inclusive authored limit,
not duplicated topology. Lowering intersects topology, selected locality and authored limits while
retaining winding, orientation, neighborhood and active-bound evidence as explicit state.

When source structure changes, the authenticated prior retained/accepted pair may provide numerical
continuation only. Candidate source still solely owns objects, topology, host inputs, branches,
ranges and publication. A formerly accepted contact outside the new range is projected to its
feasible interval and re-solved; only finite independently validated geometry may publish. Failure
retains the exact prior accepted scene and one truthful rejected history position with contact/range
diagnostics. This does not introduce implicit branch selection or soften a hard constraint.

### Language intelligence is advisory

The React code editor communicates with a dedicated TypeScript 5.9.2 Worker. Its virtual project
contains the authored files and generated declarations from the exact pinned managed SDK.
Diagnostics, completion, hover and signature help have independent request generations, bounded
file/UTF-8 byte/result limits and harmless stale/disposal recovery. Language results neither execute
managed source nor mutate source, IR, artifact, native document, accepted scene or history. Apply
continues to use the authenticated M90 compiler transaction; native materialization/solver errors
remain a separate diagnostic channel.

### One source-authoritative sample catalog

All 37 visible entries are `CodeProject`s whose `sketch.ts` and compiled envelope form their
authoring authority. The 25 former native samples retain private constructors only as reference
oracles for semantic comparison, not as a user-facing second backend. Export is deterministic and
typed. Current source preserves explicit Segment, Midpoint Line and Polyline branch directions;
legacy branch normalization is confined to authenticated historical contact-schema migration.

Spline authoring has four explicit construction variants: open/periodic control B-spline and open/
periodic control NURBS. The GUI construction collector intentionally maps its two B-spline recipes
through the NURBS construction form before canonical lowering; the named adapter and tests make that
boundary explicit. Parametric C2 retains `firstRate` and `secondRate`. Every sample is required to
cold-materialize to finite independently validated native geometry and preserve source/IR/artifact
inverse mutations; the three highlighted mechanisms additionally expose deterministic managed
drags.

### Visibility is presentation state

Explorer row and group choices, group isolate/restore and the global Construction filter are
retained presentation state keyed by stable semantic identity. Effective visibility controls paint,
picking, controls and annotations. It does not alter source, IR/artifact authority, constraints,
solve participation, suppression, selection ownership or design history. Individual descendant
choices survive a hidden ancestor/global filter and recompose when it becomes visible. Persistence
and managed reprojection rebind keys by stable semantic identity rather than transient row index.

### Dual-backend semantic parity

The golden harness retains the native owner as its mathematical oracle, exports applicable cases to
managed source, compiles them through pinned TypeScript, cold-materializes through the production
managed path and compares canonical semantic snapshots. Comparison covers geometry/constraint
meaning, dimensions, operations, explicit branch/contact state, rank/DOF, independent residual
validation, lifecycle/history and accepted-scene authority; backend IDs and serialized bytes are not
the oracle. Computed Fillet executes through both paths and cannot be excluded.

The reviewed exclusion ledger contains exactly four fail-closed boundaries:

- `constraint.external-line-collinear.*` requires an immutable external-line snapshot/binding;
- `constraint.external-point-coincident.*` requires an immutable external-point snapshot/binding;
- `dimension.profile-offset.*` cannot recover an aggregate helper/root closure from one flattened
  native document;
- `spline.noncanonical-knot-topology.*` cannot preserve a knot-inserted control structure through
  the current typed recipe.

The ledger rejects missing, duplicate or stale classifications. An exclusion never counts as parity.

### M91-F001 release-bundle boundary

The pre-repair integrated build embedded about `10.96 MB` of raw compiler-envelope JSON for the 37
projects via `include_str!`, yielding a `27,296,927`-byte optimized WASM and `36,085,444`-byte
distribution. Both exceeded the strict release ceilings (`< 20 MiB`, `< 30 MiB`). Commit
`d2170c46775b412785b3a2f288c170aba9ac8155` resolves M91-F001 with existing pure-Rust
`miniz_oxide`: the build deterministically zlib-compresses only those envelopes and runtime lazily
reconstructs exact bytes into `OnceLock<String>`. Compressed/decompressed lengths remain bounded by
`MANAGED_WIRE_LIMIT`; exact output length, complete input consumption, valid zlib status/checksum and
UTF-8 are mandatory. The public `compiled_source: &'static str` and every authenticated envelope byte
remain unchanged.

### M91-F002 package-verifier boundary

The first otherwise-complete clean gate exposed a packaging-harness defect: the package verifier
patched three unpublished local crates but omitted the new direct `geosolve-sketch-features`
dependency, so archive verification failed at the crates.io lookup. Commit
`6d0155151133ba2540fd1dc4b2b071f141b86064` adds that dependency and a fail-closed comparison between
the verifier patch list and direct path dependencies. The focused offline archive check and the
subsequent full clean gate pass.

The nominated implementation source is that commit at tree
`972ad507c2cdfad2c9cd664e49feaf79ae381c81`. Its clean release-log SHA-256 is
`cc4f4580a0637cfddad5d96d7510d4a5f4dc707d99010b300f1a43451a7cc8cd`; immutable snapshot
`geosolve-m91-uat.17Q5LnSg` has external-manifest aggregate
`b1e95b608b465a545791e55cc762052704f2d7139b8e4c3a9f8a68b0411a009b`. The release WASM is
`assets/geosolve_demo_web_bg-tvc8MGYX.wasm`, `18,368,160` bytes at SHA-256
`6832d1b6fd984076a47440ccac82ece0dfd205a9e93346dfb3cbd6783240e961`; staging/live HTTP ledger
SHA-256 is `35531210b63479565e4350b44593ebe62d228e756e67378f829c99399e86bab4` and both frozen
endpoints pass Chromium `20/20`. The nominated preview was verified against those bytes.
At nomination no public push or GitHub Pages deployment had occurred. Human acceptance now passes.
Publication descendant `177227941af97f24307fe4229797bbd84857e458` subsequently deployed the
exact release through workflow `33878060784`, artifact `9938976843`, deployment `6265455733`.
Downloaded archive SHA-256 `7dd107d94b22c1f364fbcd824980170cf9ce33f80b6cbefd6118967101b3783d`
has exact ten-file aggregate
`a8133280c286771ece4a2069880f417ea05f72980fbfa034cb774cb7a2156bad`. Eleven public routes and
Chromium 20/20 pass before both accepted M90/M91 listeners were retired. M91 is closed.

## M92 canonical bundled-sample architecture

The authorized pruning amendment reduces the public catalog to 16 entries in `4/8/2/2` categories.
[Replacement qualification and immutable nomination](../M92_VISUAL_AUDIT.md#pruned-catalog-qualification-and-immutable-nomination)
pass at `b153a28`; M92 is closed under the [scoped user sign-off](../M92_UAT.md#maintainer-closure--2026-09-06).
Pre-pruning `b854d74` evidence and its
20-sample snapshot are preserved; its listener is retired. Frozen production checks pass
19 ordinary workbench rows, the separate bounded Jansen workflow and pruned-menu/Recent probes,
independently of catalog size.

M92 replaces accumulated catalog variants with one manifest-driven registry owned by
`geosolve-sketch-code`. `BundledSampleSpec` binds a stable ordinal/key/title/category/summary to its
typed `sketch.ts`, authenticated compiler envelope, witnesses, expected mobility, ordered functional
groups and provenance. `bundled_sample_catalog()` returns the exact 16-entry order and
`bundled_sample(key)` resolves without a parallel enum. The frontend manifest is generated from the
same registry; web and headless consumers do not curate independent lists.

Assets live together under `assets/bundled-samples/<key>/`: `manifest.json`, `sketch.ts`,
`sketch.compiled.json`, `witnesses.json`, and `NOTICE.md` where required. Registry validation rejects
ordinal gaps, duplicate keys/titles, category-count drift, missing assets, source/compiler mismatch,
invalid expected mobility, duplicate/missing functional-group ownership and incomplete provenance.
The fixed distribution is four mechanisms, eight fabrication/products, two atlases and two scale
labs. The public assets for Prusa MINI, NEMA 17, HevORT and the twin-roller Bezier cam are retired;
survivor keys and source/compiler envelopes remain unchanged while ordinals become `1..=16`.
The cam source/envelope remains only as a private test fixture for exact tangent-offset and
passive-follower-locality coverage, outside registry discovery and runtime catalog assets.

All samples are source authoritative. TypeScript records equation-free declarations and groups;
native materialization, solve, independent validation, rank/mobility, accepted scene and history
remain Rust-owned. A manifest describes expected meaning but cannot override a failed solve. No new
primitive, residual, solid, CAM, collision, LOD or optimization contract is introduced by a sample.

Headless input is clean-broken from demo terminology to sample terminology. Deterministic report v2
adds numerical right nullity, equality DOF, bidirectional bounded DOF and the exact ordered source
group names/counts. It intentionally excludes wall-clock timing. Source, project and report outputs
remain immutable/new-path publications.

The systemic native/managed oracle dimension follows the owning-layer strategy in
`.agents/skills/geosolve-harden-defect`: public Rust semantic authority is compared first; WASM is a
thin adapter parity check; human UAT owns presentation and manipulation feel. Exclusions are
explicit reviewed non-passes, never inferred from backend failure.

The workbench bridge keeps managed compilation exclusive without rejecting browser capture cleanup.
While an authenticated managed mutation is pending, new pointer gestures remain unavailable, but a
decoded and validated terminal `PointerPhase::Up` with zero buttons is a non-mutating host release:
it returns `null` without consuming the compiler ticket, changing revision/history/persistence or
publishing a snapshot. `M92-F002` freezes this distinction at the DOM-free bridge owner and at the
paired frontend event lifecycle; it changes no solver, compiler or accepted-scene equation.

Release-browser qualification has two explicit scopes. The self-built release harness enables the
harness-only pinned compiler page and runs `20/20`; immutable production distributions deliberately
omit that page and run the remaining `19/19` ordinary workbench rows on each frozen endpoint. The
compiler-envelope row remains required in the release harness rather than being misreported as a
production route.

### M92-F013 bounded complete-history persistence

Internal code-workbench v5 retains readable canonical project/source fields and compresses only
the complete canonical code-session string through the existing pure-Rust reproduction codec.
Limits remain 12 MiB compressed, 16 MiB text and 64 MiB decoded, with exact length, checksum,
UTF-8 and complete-stream checks. V4 owner decoding remains supported. Decoded session bytes feed
the unchanged validating checkpoint decoder; compression integrity alone grants no accepted
authority.

Historical presentation wrapping could enlarge an otherwise valid 64 MiB owner payload beyond the
ordinary 40 MiB bridge request. A dedicated v4-only recovery accepts one strict raw presentation
envelope at most 96 MiB, retains the 64 MiB inner bound, reconstructs through the
code/accepted-scene owners, restores presentation and checks that new persistence fits the
ordinary transport. It neither widens ordinary requests nor admits nested fallback envelopes.
Frontend recovery forwards original bytes without JSON rewriting, validates a candidate before
replacing its live handle, and preserves saved project and separate unapplied-draft bytes with
automatic saving paused on failure. Draft restoration is read-only; its write/delete effect
remains protected until successful explicit Save. `docs/M92_VISUAL_AUDIT.md` records evidence.

### M92 sample presentation checks

The separate 16-row sample browser audit checks two edits per sample (32 edits), rendered geometry
through history/reload, selection ownership and group restoration. It supplements the release-harness and
immutable-endpoint scopes above. F014 keeps Explorer rows and child grids within their panel and
widens the default panel share; these presentation changes do not alter accepted geometry or
history.

### M92-F015 bounded mechanism dragging

Interactive retained-editor previews use finite operation budgets. An unlimited native trajectory
and a browser source-edit/history audit do not qualify this boundary. The repaired Jansen crank
reproduced work exhaustion before core repair under the ordinary 256-factorization preview limit; the previous valid
geometry remained retained, but no movement or Undo entry was accepted. Browser reproduction reached
the same disabled-Undo outcome. Core repair `a9d7532` extends existing first-improvement
backtracking to all Temporary objectives without changing budgets, tolerances or certification.
The pre-pruning ten default-policy witnesses and Jansen/scissor browser checks passed. The active
four-mechanism catalog passes all eight default-policy witnesses in the replacement gate. The
required boundary preserves hard validation, explicit branches, locality and exact
history while completing meaningful ordinary drags within the configured budget. The detailed
reproduction and failed clean-gate receipt are in `docs/M92_VISUAL_AUDIT.md`.

### M92 Gridfinity harness contract

The second audited clean gate failed on a stale Gridfinity source-test assumption. Its linked
plan and section each own one scalar Y datum, with no fixed points; symmetry, dimensions and
`planMatchesSection` preserve width propagation, while `cavityFloorAtBase` retains floor height.
The corrected test agrees with the compiled-declaration and independent geometry contracts and
passes 2/2. This `HARNESS_ERROR` changes no production semantics and receives no finding ID.
`docs/M92_VISUAL_AUDIT.md` records the failed gate receipt; complete qualification subsequently passed in the final ledger.

## M93 qualification architecture — accepted and closed

[M93_GOALS.md](../M93_GOALS.md) scopes release infrastructure, leaving solver/domain contracts
unchanged. One stage/case inventory drives dependency planning, resource-bounded execution,
input-authenticated evidence reuse and complete coverage reconciliation. Build caches and passing
test receipts are distinct. Each reused result retains its original provenance; unknown inputs
force fresh work. Newly built or moved artifacts receive fresh transport verification. A complete
fresh-results mode remains available. Corrected-source functional qualification and serial/parallel parity pass. The maintainer accepted closure on 2026-09-07 with the [recorded C5 timing miss and unperformed historical four-entry replay](../M93_QUALIFICATION.md).
[Release qualification](../RELEASE_QUALIFICATION.md) owns the implemented commands and policy.

## M94 accelerated canvas architecture — accepted and closed on 2026-09-07

[M94](../M94_GOALS.md) introduces a typed presentation draw frame after Rust authenticates and
composes EditorScene. TypeScript/PixiJS consumes ordered finite paint primitives; geometry,
annotation layout, hit priorities and accepted/history authority remain Rust-owned. Bridge v2
changes transient frame transport only. Native SVG/PNG exports and persisted formats remain.
One WebGL2 canvas replaces the live SVG tree; native React controls and popovers remain outside it.

Live CSS extents now govern camera, Fit and pointer coordinates independently of raster DPR.
Navigation reprojects authenticated retained scenes and preserves ordered camera input; guarded
frame-only point previews and immutable prepared/delegated publication avoid redundant work.
Terminal/error snapshots, native validation and exact history remain authoritative. Pixi 8.20.1
context restoration clears invalid upload caches and rebuilds retained paint resources once.
[The M94 closure audit](../M94_CLOSURE.md) records accepted source `7727cbf`, complete
qualification and the remaining dense-editing/context-compatibility limits.

## M95 connected selection — accepted and closed on 2026-09-07

Native accepted output ownership now has a navigation API separate from singular declaration
mutation selection. The WASM bridge caches declaration/group/member mappings by Intent, code,
source and visibility authority; an instance token rejects cross-project stale requests. Exact
output bindings keep generated vertices and spans distinct even when they share one producer.
Borrowed operands never become owned outputs. Source spans remain attached to their direct
statements; selecting a partial parent does not acquire unselected helper statements.

One transient navigation snapshot projects selected/partial Explorer rows and all relevant source
ranges. Selection deltas retain source/parameters/project objects and durable revision. Rust
validates UTF-8 boundaries and accepted source identity; TypeScript converts CodeMirror UTF-16
positions exactly. CodeMirror decorations are separate from text selection, with automatic reveal
only when selected identities change. Explicit reverse navigation opens Split when needed.
Dirty/retained-invalid text disables source links while preserving accepted row browsing; active
tools and captured gestures block explicit cross-view selection. M94 pan/zoom and point-preview
routes retain their narrow frame transport. Persisted formats and solver behavior do not change.
[M95_GOALS.md](../M95_GOALS.md), [evidence](../M95_IMPLEMENTATION.md) and
[closure](../M95_CLOSURE.md) record the accepted scope and product identity.

## M97 dimension presentation — accepted and closed

`DimensionPresentationState` is a native presentation owner separate from design
history and manual annotation placement. It resolves Focused, All and Hidden into
the shared accepted scene before numeric drawing, SVG rendering or annotation
picking. Standalone scenes retain their previous defaults unless a host applies
the policy. Complete measurement entries remain available when their callouts are
hidden; relevance follows direct operands, selected curve endpoints and accepted
producer ownership.

The workbench supplies accepted ownership and source editing authority, defaults
to Focused and persists display mode and at most four exact annotation identities.
Automatic layout is transient retained state. Camera and selection changes reuse
slots; geometry changes invalidate affected anchors. Bounded placement and a
six-callout contextual Focused limit keep ordinary navigation readable. Authored
overview metadata keeps all Gridfinity measurements and curated design sizes in other
samples eligible beyond that contextual limit, subject to the same collision policy.
Overview flags belong to source and its design history; personal pins remain separate.
The earlier catalog-owned selectors and implicit first-six defaults have been replaced
by the source-native amendment below. Unmarked source retains contextual discovery.
Explicit Fit and first host measurement retry hidden slots after reserving
visible placements; ordinary navigation never requests that reconsideration. The frontend owns
idle-hover timing and Inspector controls while Rust owns visibility and hit tests.
Public patch parameters retain their source values and existing edit transaction;
generated dimensions do not acquire writable authority from their displayed value.
[M97_GOALS.md](../M97_GOALS.md) owns the approved contract.
[M97 qualification](../M97_QUALIFICATION.md) records the original dimension UX gate.
[Closure](../M97_CLOSURE.md) identifies the final authoring amendment and acceptance on
2026-09-09; no solver equations or branch semantics changed.

### M97 source-native metadata amendment — accepted and closed

[M97_AUTHORING_METADATA.md](../M97_AUTHORING_METADATA.md) specifies the implemented
authoring API. Document title/description and authored
dimension defaults belong to `sketch` options as `dimensions.areKeyConstraintsByDefault`.
Local dimension `isKeyConstraint`, parameter `isKeyParameter`, existing
`label`, optional `description`, named public parameters and patch input schema
presentation travel through authenticated managed IR and execution artifacts.
Rust projects accepted metadata into the workbench and prepares bounded structural
source edits; metadata is not a solver consumer or an independent GUI sidecar.

Named parameters retain stable source identity and one public edit route across
their consumers. Inline inputs can inherit patch defaults; named parameters own
their presentation independently. The catalog derives title/summary/groups from
compiled source and retains ordering/category/legal/test metadata. Its priority
selectors and the first-six fallback have been removed after migrating all live sources.
Existing view preferences retain their current owners. V4 envelopes authenticate metadata,
while the bounded V3 reader preserves old source and upgrades only through a source
transaction. [Implementation evidence](../M97_AUTHORING_IMPLEMENTATION.md) records
the passing 244-obligation clean-source gate on `e26270cb89e5849092145b329d0cf95821a81b27`
and the frozen production preview's exact served-byte and actual-WASM verification.
Generated per-instance overview overrides remain deferred.

## M98 local projects and headless embedding — qualified for maintainer review

The ordinary TypeScript SDK records an evaluated generator program separately from the
managed compiler's lexical/execution receipts. Both admit through shared Rust lowering and
independently validated sketch materialization. `geosolve-sketch-engine` and its dedicated
WASM crate expose geometry, diagnostics, editable sessions and profile export without the
workbench or React. The TypeScript engine retains immutable accepted result handles;
rejection, cancellation and supersession cannot replace the previous accepted result.

The folder-v2 manifest declares entry and mode. Authoring labels, input schemas, parameters,
groups and outputs belong in TypeScript. The loader snapshots complete local imports,
manifest, optional semantic design and generator inputs, including absent-file identities;
terminable workers execute only those captured bytes. Virtual candidate graphs compile and
admit before a journal publishes authored files. Sidecar `geosolve-design-v1` contains project
identity, keyed reconciliation and overrides, without a solved-geometry copy. Derived
history is bounded, reconstructed after current authored state and never trusted as source
or digest authority during Undo. Generator mode has no reverse-edit authority.

In the original single-editor mode, each canonical Linux folder has one advisory bridge
lock and one active editor lease.
Requests bind installed source, session epoch, lease, interaction revision and immutable
intent; an observed snapshot grants nothing until the host installs it. Operation IDs resolve
lost responses before stale-authority checks. A recoverable journal retains original,
displaced and candidate bytes, with explicit inspect/resolve operations. This does not promise
atomic CAS against independent writers retaining old descriptors. Publication failure restores
native state and truthfully marks any differing disk revision. Native actor timeouts/crashes
retire one worker generation and reconstruct its last accepted checkpoint. Navigation avoids
source compilation, rollback capture and disk publication; the HTTP queue is bounded.

Computed export projects accepted analytic boundaries into existing production topology and
sampling, retaining explicit joins and failing closed on unsupported or uncertain geometry.
Named output fragments select complete regions with holes; they do not imply pocket depth
or material removal. See [M98 implementation plan](../M98_IMPLEMENTATION_PLAN.md),
[engine API](../M98_ENGINE_IMPLEMENTATION.md) and [journal contract](../M98_WORKSPACE_STORAGE.md).

[M98 qualification](../M98_QUALIFICATION.md) records the complete clean-source gate and
verified previews. maintainer acceptance and milestone closure remain open.

The [M98 loading-feedback amendment](../M98_LOADING_FEEDBACK.md) adds presentation-only
activity counting, delayed canvas feedback and named folder SSE activity. Standalone WASM
runs the existing adapter in an ordered module worker, preserving immutable snapshot
sequence and canvas-only fast paths across transport. Activity grants no scene authority.

The [local canvas amendment](../M98_LOCAL_CANVAS.md) is mechanically qualified and delivered.
Folder navigation moves into a dedicated Rust/WASM presentation worker over a bounded
`EditorScene` transport. The imported scene retains analytic geometry, computed boundaries
and dimension placement, with a permanent detached marker and no prepared or accepted-input
capability. Camera, picking, selection and dimension disclosure use shared Rust semantics;
ordinary pointer hover, wheel and resize require no HTTP request and publish canvas-only
frames. Native selection metadata retains picked curve occurrences and parameters for the
server's later constraint and Fillet authoring.

Server sketch commands carry the installed scene identity and absolute local presentation.
Source/epoch/lease/revision guards and transaction rollback remain server-owned. A new scene
preserves the latest camera and reconciles surviving selections. Selection Inspector chrome
may settle in the background; acknowledgement must not rewind later clicks. During slow
server edits the delayed veil remains visible while local navigation works. Exact server
construction/inference frames are retained only when their viewport and selection match the
current client; navigation uses the detached accepted scene instead of reusing stale pixels.

## M98 collaboration amendment — mechanically qualified

[The collaboration contract](../M98_COLLABORATION.md) separates shared working source,
server-accepted source/model and each client's presentation/prediction. The reusable Rust
`geosolve-collaboration` crate, its dedicated WASM adapter and TypeScript package own
protocol state and raw Automerge text with explicit UTF-16 indexing. Incomplete source
synchronizes before parsing. Apply captures an immutable draft revision and publishes
only after the existing compiler receipts and independent model validation succeed.

One authoritative server admits semantic operations in order, replays intent against the
latest accepted model and authenticates original target lifetimes and explicit branches.
Stable generations, server ID allocation and checked contribution inverses prevent stale
commands or personal Undo from acquiring another editor's changes. Accepted checkpoints,
source, history and operation outcomes persist before acknowledgement; exact operation
retries recover their original result after lost acknowledgements and restart. External
file mirrors and CLI edits use the same gateway.

HTTP commands and bounded SSE distinguish durable document events from disposable
presence. Separate compiler/solver workers leave text and read paths available during
model work. Dedicated browser Rust/WASM workers own local navigation, picking, selection,
visibility and provisional authoring; prediction grants no server publication authority.
Optional server prediction uses the same native semantics. Editor/viewer invitations bind
trusted identities, and each client retains independent camera, tool and Inspector state.
Browser outboxes persist pending intent and recover uncertain operation outcomes.

[Current integrated qualification](../M98_QUALIFICATION.md#qualified-shared-toolbar-parity)
covers all 25 geometry variants, 13 constraint tools, five dimension tools, Fillet/Profile
Offset, contextual options and source-authoritative geometry roles in the current
React workbench. The underlying four-browser/32-client authority, recovery and load
contracts remain required. Production identity providers and full offline semantic
reconciliation remain outside this delivery. The original single-editor mode and
previews remain available. Existing shared previews serve the exact qualified artifacts;
maintainer acceptance and milestone closure remain open.

### M98 shared tool continuation — qualified toolbar amendment

Construction now retains all 25 native recipes. A separate engine tool-operation
continuation delegates relation/dimension collection, Fillet and Profile Offset to
their existing presentation-independent owners. Neither the browser nor server
adapter duplicates geometry equations. A sequenced camera event updates native
picking tolerances after local navigation; the command retains its original viewport
and all intervening input for deterministic independent replay.

Accepted source bindings map personal selection into the prediction engine namespace,
including implicit curve occurrence/interval owners. Source-authoritative operations
carry exact options, operand dependencies and resolved declarations or role writes.
The server authenticates the original checkpoint and target lifetimes, replays against
its current accepted state, validates genuine compiler receipts and persists before
publishing. Existing contribution history supplies personal Undo/Redo.

`beginToolOperation` returns a private prediction with `initialFrame`, ordered `advance`,
paint-only `presentationJSON`, `finish` and `cancel`. The server's prepare/replay/resolve/
apply lifecycle carries semantic `tool_operation` commands and optional `operation`
previews. Options apply before preselection; current native frame capabilities govern
Finish, Step Back, Reset and two-stage Escape. Tagged source operands retain exact
binding/span/parameter or curve-occurrence ownership. Ordered traces have a cumulative
1 MiB bound. A refused Apply never replaces independently accepted native authority.

Detached operation presentation also carries native pending/hover/provisional states
and Offset chain cues. This payload grants paint only. Local navigation continues to
use its separately authenticated accepted scene while prediction or durable model work
is pending. [Scope and qualification](../M98_TOOL_PARITY.md) retain precise current
workbench capability limits and maintainer acceptance status.

Browser paint completion is separate from native scene authority. The WebGL2 backend
submits one draw and a fence, then checks completion with zero-timeout polls scheduled
at 4 ms. Only a completed fence followed by the existing GL error/context validation can
advance the exact submitted frame, surface and context-epoch witness. New camera/native
inputs coalesce while one draw is in flight; they cannot relabel older pixels as a newer
frame. Context loss, disposal, failed waits and an unsignaled five-second deadline cannot
publish late frames. Background-throttled tabs may still accept a signaled fence after
the nominal deadline. Render telemetry includes submission through validation; the fence
does not claim physical display scan-out or eliminate all driver/shader IPC costs.

The first ordinary render prepares the exact horizontal/vertical blur programs and
their actual filter draws, after backend installation. A small private target and
final-canvas composition exercise both GPU pipelines; the real native scene clears
preparation pixels in the same submission. All temporary resources are released.
Context restoration repeats preparation. Startup readiness includes preparation and
the real frame's fence/error validation. After asynchronous validation, the newest
coalesced input can submit immediately without another RAF wait. Synchronous draws
retain RAF coalescing. Both paths preserve one in-flight draw, immutable frame/surface
witnesses, hidden suspension, failure latching and context epochs.

Changing frames use display DPR, including delayed native predictions and peer updates.
Text textures retain minimum 2× resolution. After 1.5 seconds of quiet following
completed rendering, low-DPR canvases regain minimum 2× supersampling. New input
cancels only unsubmitted refinement; submitted work retains its exact frame, surface,
quality and context identity. Idle Select hover paint coalesces for at most 32 ms;
queued wheel input suppresses unsubmitted hover until its native response or queue
completion. Native point setup remains validated but its unchanged origin need not
paint ahead of the already-queued movement. These F042 presentation changes do not
alter native geometry, picking, input order or publication authority; replacement
qualification is recorded separately in `docs/M98_HARDENING.md`.

### Source-derived free Polyline references at a point terminal (M98-F041)

Native free spans may rotate continuously while retaining dormant branch vectors.
Before source publication, the code owner can explicitly transport only an
unenforced, source-derived reference using authenticated origin/terminal/source
documents. Authored `branchDirections` and enforced branches remain source-owned.
The sketch owner verifies exact origin branch metadata, unchanged durable schema
and finite nonzero span directions. Ordinary terminal parity, cold reconstruction
and independent residual validation still gate publication. This seam is shared
by the headless engine and standalone workbench and adds no protocol authority.

## M99 shared authoring and host cleanup

The [completed cleanup](../M99_CLEANUP.md) preserves the M98 protocol and
all durable project, sidecar, history and recovery formats. Standalone, folder
and collaborative consumers use the consolidated experimental APIs.
[Mechanical qualification](../M99_QUALIFICATION.md) and
[maintainer acceptance/closure](../M99_CLOSURE.md) are complete and remain
separate records; M98 human acceptance is unchanged.

Native construction receipts retain defining samples, operands, branches and
created declaration correspondence. `geosolve-sketch-code` owns source names,
metadata defaults, declaration projection and complete native/source terminal
consistency. Engine construction and contextual tools retain distinct state
machines while sharing compiler preparation, validation and installation.

Detached camera, picking, exact selection correspondence, visibility, dimensions
and captured navigation belong to `geosolve-constraint-editor`; they hold no solver,
compiler or publication capability. The renderer paints prepared accepted or
provisional presentation. Read-only engine inspection retains exact accepted
source/native authority, and source navigation belongs to the source owner.
`AcceptedBrowsingSession` retains exact accepted source/native authority and an
independent personal view. Standalone and folder semantic hosts use the engine's
`EditableSession`; collaborative publication retains its authority and contribution
history policies over the same authoring services. Engine interaction seeds let
the browser initialize read-only chrome and detached rendering locally.

Native workspace and reproduction codecs belong to `geosolve-constraint-editor`;
source checkpoint admission and the existing compressed source-workspace wire
belong to `geosolve-sketch-code`. Engine persistence uses those public owners,
preserving unfinished text, accepted authority and complete personal history.

React consumes a capability-based `WorkbenchSession`. Browser persistence,
folder disk receipts/leases and collaborative text/outbox/personal history keep
separate policy owners. Node production hosting/build/package output belongs to
`packages/geosolve-cli`; semantic Node hosts consume the engine/compiler packages
without a demo-WASM execution runtime. Installed CLI archives also own the browser
distribution. MiniCAD consumes public `geosolve bake` output with exact installed
package and source provenance. Worker lifecycle helpers share correlation,
stale-generation fencing, settlement and teardown mechanics. Their callers retain
the different ordered editing, coalesced presentation, text admission and
durable-callback drain policies.

## M100 preparation boundary

[M100](../M100_FINAL_CLEANUP.md) is prepared as a final maintenance and pause
readiness pass over this accepted architecture. Shared native JSON decoding and
compiler receipt mechanics, permanent qualification tooling, current documentation
and restart/artifact retention are the bounded audit targets. The plan preserves
distinct host policies and durable compatibility; it introduces no mathematical,
protocol or public API change merely by being recorded here.
