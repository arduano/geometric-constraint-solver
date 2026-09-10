<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# `GeoSolve` sketch engine

Headless evaluation of managed code projects and independently admitted generator
records. Both use the existing equation-free authoring lowering, native geometry,
explicit computed-feature authoring and independent hard residual validation.
The engine has no browser, renderer, filesystem or TypeScript execution dependency.

`SketchEngine::evaluate_generated_json` admits an evaluated record; `evaluate_managed_json`
admits the existing authenticated editable project. Both return immutable accepted
model-space data. Failure preserves `last_accepted`. Cloned `AcceptedEvaluation`
handles own the exact native authority and remain exportable after later evaluations.
A generator cannot obtain managed source-edit tokens from this API.

Profile export uses production topology and bounded model-space sampling. Current
computed line/circle/arc boundaries are projected into a private native query
document with authenticated endpoint joins. Independent acceptance must preserve
every projected coordinate and scalar exactly. Unsupported or incomplete geometry
fails export while the original accepted evaluation remains intact.
Mixed computed export supports lines, polyline spans, circles and circular arcs.
Native joins come from complete-span endpoint ownership or exact computed fillet
contact boundaries. Pre-trimmed native contacts at interior parameters can fail
closed when combined with computed features; no coordinate-based joining is used.

`export_profiles_for_output` selects complete regions containing at least one
outer-boundary fragment owned by the named output and preserves their holes. An
individual edge can select its whole containing region. Outputs use JSON Pointer paths in generator
mode. The export contains bounded arrangement faces: nested circles yield an
annulus and its interior disk. Selecting the outer circle yields only the annulus;
an output owning both circles may select both faces. No material/cut interpretation
is inferred from those boundaries.

The `profile_export` capability means the operation is available, not that any
geometry or tolerance is exportable. Evaluation currently uses a 1 mm model scale.
The native topology query can reject much larger geometry when it cannot certify
area at that scale; export preserves that diagnostic instead of changing the
geometry or relaxing validation.

`compile_project_json` assembles the existing validated offline managed project
from complete compiler authority, local files, artifacts and pins. Local module
paths may use ordinary `.ts`/`.js` and ESM/CommonJS extensions, must remain
relative and canonical, and cannot contain traversal or absolute-path components.

`geometry` exposes exact native curve definitions and their current visible
intervals plus current computed fragments/arcs in model coordinates.
`named_geometry` maps each output to native point/span IDs and explicit computed
edge ordinals. Generated metadata preserves names, groups, applications and
original parameter units. Standalone engine evaluation is read-only in both modes.

`EditableSession::open` accepts a complete managed project and optional
`EditableDesign`. `apply_project`, `apply_overlay`, `undo` and `redo` require the
exact current `CodeSessionIdentity`, including its session and revision. They use
the existing `SketchCodeSession` transactions and shared native materialization.
Failures preserve accepted geometry, semantic state and history; old accepted
results remain exportable. History checkpoints reference private immutable native
authorities. They cannot be imported as external state.

`design()` exports `geosolve-design-v1`: project identity, keyed reconciliation and
authored semantic overrides. Reopening reconstructs and independently validates
source plus those overrides without serialized solved geometry or hidden history.
`export_project_json()` exports the complete canonical accepted source project,
including compiler authority, dependency artifacts and source allocation high-water
state, to durably pair with that design and its `source_design_digest()`.

Managed mutations and captured Apply use opaque preparation handles over the exact
accepted source, compiler artifact, expansion and session identity. The host executes
the compiler request and returns its receipt; the engine reauthenticates the live
input and independently validates native geometry before publication. Semantic value
batches resolve their current lexical expectations at preparation. Checked inverse
data never rolls back global history; a collaborative host must additionally enforce
target lifetimes and contribution ownership, including same-value writes.

`point_gesture_targets()` exposes explicit semantic point addresses and their keyed
allocation/generation. `begin_point_gesture` forks accepted native authority once;
`RetainedPointGesture::advance` processes up to 4,096 contiguous model-space samples
through the shared retained coordinator. Frames perform no source compilation, overlay
publication or history write. Referenced consumers detach once at gesture start, using
the existing semantic point codec. Camera mapping stays fixed for one gesture.

`scene_json()` returns a detached provisional scene. `finish` consumes the checked native
terminal and returns its replayable semantic command. `replay_point_gesture` authenticates
the source/design basis and recomputes every sample; caller-provided coordinates never
prove acceptance. The trusted host still owns document epochs, user authorization and
latest-state target/branch admission. Stale source/design bases reject explicitly.

The server calls `prepare_point_gesture_commit` to independently replay the command and
stage a complete semantic overlay. Publication preserves moved companion points, explicit
consumer detachment, rectangle source seeds and derived corners. It requires independently
validated native geometry, current computed features and exact ownership, allocator,
branch/contact and semantic-input parity. The existing bounded rectangle roundoff policy
applies only to authenticated derived geometry; authored seeds remain exact.

`PreparedPointGestureCommit` exposes the candidate result, semantic design and
`source_design_digest`. The host durably records the candidate, then installs it through
`apply_point_gesture_commit`, which rechecks the exact live session token and creates one
history contribution. Discarding a candidate leaves the live session unchanged. Native hosts
without external persistence can use `commit_point_gesture` for synchronous replay and commit.

`source_design_digest()` identifies complete source, generated lifecycle state and overrides
independently of the process/session. Reopening and independently validating the same source
and design reproduces it. Individual evaluation `input_digest` values include prior session
state and should not serve as durable recovery identities.

`AuthoringPrediction::apply_overlay` is a whole semantic update and must not be called for
every pointer frame.

`begin_construction` retains `Segment`, `Polyline`, `CenterRadiusCircle` or
`TwoPointAlignedRectangle` drafting on an isolated accepted coordinator. Ordered Move/Click,
Complete and `StepBack` samples use the existing shared inference and recipe regularization.
Frames expose resolved model-space preview and inference guides; `scene_json` exports the
detached scene. Terminal plans validate through `apply_construction_editor_effect` on the
fork. These local drafts never change the live engine's source, design or history.

`finish` produces a bounded semantic `ConstructionCommand`, including source-level resolved
operand and branch witnesses. The server's `prepare_construction` replays the exact basis
and requires those semantic witnesses to match before preparing a managed compiler request.
Shared reverse projection preserves true source references and inferred constraints; the
existing monotonic name allocation preserves native allocation order. The compiler receipt
passes through `resolve_construction`, which independently stages native geometry and proves
whole-model terminal parity, including exact authenticated declaration label transitions.

`PreparedConstructionCommit` exposes complete project/design/result/digest and created
declaration names. The host persists those inputs and lifecycle changes before calling
`apply_construction_commit`. Foreign/stale candidates cannot publish; failed receipt or
parity validation keeps all live source, geometry, history and allocation state. Model-space
positions and explicit source references reconstruct through ordinary cold opening.

Construction uses the default shared inference cohort; explicit candidate cycling and
advanced construction variants are outside this boundary. Stale source/design commands
reject before replay; latest-state rebase and browser worker/UI integration remain separate
work. Focused native/WASM validation does not establish end-to-end interaction latency.
