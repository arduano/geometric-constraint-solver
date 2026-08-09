<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0034: Headless auto-constraint drafting intelligence

Status: accepted for M70; implementation and focused direct qualification are complete, while
integrated release qualification, publication and supervising-human UAT are pending

## Context

`geosolve-constraint-editor` already owns presentation-independent drafting, accepted-scene
picking, endpoint snapping and retained interaction state under ADR 0029. It also contains a
manual provisional-inference seam, but that seam does not generate candidates, remember semantic
references, rank competing intent, refer to geometry allocated by the construction it accompanies,
or commit geometry and inferred relations as one retained transaction. Implementing those missing
parts in `geosolve-demo-web` would make a browser the authority for what a drawing gesture means.

Ordinary CAD systems provide useful precedents without defining one mandatory UI personality:

- [Onshape](https://cad.onshape.com/help/Content/Sketch/automatic_inferencing.htm) wakes points,
  lines and midpoints through hover and later offers dotted alignment, parallel, perpendicular and
  coincident inferences;
- [FreeCAD](https://wiki.freecad.org/Sketcher_Workbench#Auto_constraints) distinguishes visible
  proposed auto-constraints from the placement click that creates them;
- [SolidWorks](https://help.solidworks.com/2025/english/SolidWorks/sldworks/t_using_automatic_relations.htm)
  exposes configurable snap/relation families and may preview several compatible relations;
- [Fusion](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/SKT-CREATE-LINES.htm)
  exposes semantic object and midpoint snaps during sketch creation; and
- [SolveSpace](https://solvespace.com/ref.pl#Automatic) previews near-horizontal/vertical intent,
  commits on click, supports suppression and avoids automatic redundant relations.

GeoSolve is an embeddable engine rather than one of those applications. The reusable boundary
therefore needs typed semantic input, policy and output, while a host remains free to choose
styling, modifier mapping and which inference families it enables.

## Decision

### Separate anchors, reference memory, candidates and commit plans

M70 replaces the dormant provisional-inference seam with one drafting-inference subsystem in
`geosolve-constraint-editor`. Its four layers are deliberately distinct:

1. **semantic anchors** identify persistent points, native curve positions and line/polyline
   midpoints together with exact source metadata;
2. **ephemeral reference memory** records eligible anchors and real affine spans awakened by the
   current construction stage;
3. **prospective candidate bundles** combine at most one positional relation and one compatible
   directional relation, together with guides and an adjusted preview; and
4. **construction commit plans** apply the complete construction and the exact displayed inferred
   relations in one retained transaction.

The public headless seam exposes `DraftInferenceBehavior`, `DraftInferenceTolerances`,
`DraftInferenceLimits`, per-family `DraftInferencePolicy` and `DraftInferenceInput`. A stateful
`DraftInferenceEngine` consumes `DraftInferenceFrame`/`DraftInferenceSample` and publishes typed
`DraftReferenceAnchor`, `DraftInferenceRelation`, `DraftGuide`, `DraftInferenceCandidate`,
`DraftInferenceResolution` and commit-plan DTOs. These values carry stable
session-local identities, exact persistent sources, raw and adjusted model coordinates,
constraint-backed versus tracking-only classification, ranking evidence, ambiguity, suppression
and resource-completeness state. `DraftPointSlot`, `DraftSpanSlot` and
`DraftContactDescriptor` lower the selected candidates into ordered `InferredRelation` values, so
a `ConstructionCommitPlan` may refer to persistent geometry and to identities allocated by the
same construction. `ConstructionCommitResult` maps each relation occurrence to its allocated
contact, constraint and source identities.

`ConstructionProposal::apply` remains available to geometry-only consumers. Normal editor use
commits the role-aware construction plan through the retained coordinator and returns the allocated
points, curves, contacts and constraints.

### M70 inference families

M70 uses only constraint definitions already admitted by the ordinary retained sketch workflow:

- reuse an existing persistent point identity instead of creating a duplicate point plus a
  redundant coincidence source;
- create explicit PointOnCurve contact state for native line, circle/arc, Bezier, conic, B-spline
  and NURBS spans;
- prefer a line/polyline Midpoint relation over generic PointOnCurve at a semantic midpoint;
- adjust and constrain a newly authored line or live polyline span to Horizontal or Vertical when
  it enters the angular tolerance; and
- remember real line/polyline spans and use them for later Parallel or Perpendicular inference,
  including a midpoint anchor combined with a perpendicular new span.

Point identity is structural intent rather than an inferred solver relation. When another
construction uses the point, the selected identity lowers directly to `ConstructionPoint::Existing`.
When the standalone Point tool confirms an identity that already exists, there is no new durable
intent: the editor emits no construction plan and creates no history checkpoint.

Every construction stage represented by `ConstructionPoint` participates in positional inference.
Directional inference applies only when the live stage defines a genuine line or polyline span.
Bare-point horizontal/vertical tracking may be published as `TrackingOnly`; the retained ordinary
workflow cannot yet persist arbitrary point-pair horizontal/vertical intent, so M70 neither adjusts
from nor commits that guide by default. It must not emulate the missing lifecycle with
`FixedCoordinate`, a zero dimension or hidden construction geometry.

Computed Fillet arcs are not inference targets. Fillet-discarded implicit Construction resolves to
its complete native source and remains subject to ADR 0033 role, scope, visibility and overlap
policy. Profile and explicit Construction native geometry otherwise participate according to the
same current headless interaction policy.

### Deterministic resolution and hysteresis

Default entry/leave thresholds are expressed in host-normalized screen space or angular error:

- point or midpoint: enter at `8 px`, retain through `12 px`;
- native curve: enter at `10 px`, retain through `14 px`; and
- direction: enter at `4 degrees`, retain through `6 degrees`.

Entry boundaries are inclusive. Policy values must be finite and validated. Configured policy has
hard ceilings of 32 candidates and eight remembered references. Default scene-query bounds are
4,096 semantic anchors and 16,384 tessellation chords, with larger values accepted only within the
separate implementation safety ceilings.

Candidate generation is bounded while it runs: it stops at the first unique semantic bundle that
proves the configured limit insufficient instead of allocating every possible combination first.
`DraftInferenceCompleteness::CandidateLimit.required` is that first proven lower bound, normally
`limit + 1`, rather than an unbounded full-scene count. Candidate or scene-query exhaustion returns
raw unadjusted coordinates and no partial candidate/guide/anchor prefix, because a prefix could
silently select a different semantic winner.

Ranking is lexicographic:

1. applicable constraint-backed intent before tracking-only guidance;
2. persistent point identity before Midpoint before PointOnCurve;
3. remembered Parallel/Perpendicular before an equivalent world-axis direction;
4. ADR 0033 Profile/Construction hit priority; and
5. geometric distance or angular error.

Persistent identity stabilizes output order only. If all semantic and geometric ranking fields
tie, the result is `Ambiguous` and no relation is auto-committed. A host may explicitly select one
published candidate identity; it may not submit a different inferred relation under that identity.

Entering an eligible hover boundary wakes the reference immediately; there is no timer. Reference
memory is bounded to the active construction stage and clears after its placement click. It also
clears on cancellation, tool exit, mutation, Undo/Redo, reload, viewport or geometry-policy
change, or stale identity. It is never serialized into sketch or workspace state.

### Policy, suppression and confirmation

Hosts control guide visibility, coordinate adjustment and durable relation creation independently
per inference family where those choices are semantically coherent. Point identity is structural
operand reuse rather than a solver relation, so policy validation rejects persisting it while
coordinate adjustment is disabled; guide-only identity remains available. Suppression is semantic
input to Rust, not a hard-coded keyboard rule. A
suppressed sample acquires no reference, clears active inference latches/guides and cannot commit a
stale candidate; a suppressed placement uses the raw construction sample. Releasing suppression
recomputes from the current pointer input. The demo may map Shift to this input.

The ordinary placement click is explicit confirmation of the currently displayed bundle. There is
no second Apply action. If no candidate is active, placement is geometry-only. If inference is
active, adjusted geometry and every displayed compatible relation commit together or not at all.

### Retained publication remains authoritative

The coordinator applies a complete commit plan to a cloned retained session, solves once and
requires a newly independently accepted state for the exact plan. Every inferred source must
survive ordinary domain validation and redundancy/conflict evidence. A newly fully or partially
redundant inferred source rejects the inferred transaction rather than becoming hidden duplicate
intent.

Only a scene authenticated from the retained session's exact current accepted document, design
filter and `PreparedSketchInput` is publication-authoritative. Caller-assembled document,
revision or detached-stamp combinations cannot manufacture that authority: compatibility and
render-only scenes deliberately carry no prepared-input binding. They may expose the same anchors,
guides and prospective inference for presentation, but cannot emit an inferred construction plan.
Because `EditorScene` remains an ergonomic DTO with public presentation fields, a private exact
seal captures its accepted revision, design identity, viewport, native inference curves and
construction snap anchors after trusted construction. Binding requires an exact seal match.
Changing any covered field after binding revokes publication authority while retaining detached
presentation behavior; no caller-maintained dirty bit or collision-prone digest is trusted.

The terminal editor transition retains that exact authenticated input and a frozen copy of the
displayed `ConstructionCommitPlan` behind a session-local commit token. Coordinator effect
dispatch authenticates all three values and separately compares the input with the live session's
current `accepted_prepared_input()` before any mutation. A copied effect therefore cannot replace
the plan, and parameter changes, external-snapshot changes or reattempts cannot publish a preview
formed from an older accepted input.

Stale, ambiguous, invalid, cancelled or exhausted work cannot silently fall through from the
displayed candidate to another relation. Rejection leaves live document/history unchanged and
retains the exact draft plus last preview for correction. A successful plan swaps the exact trial
session and creates one history/replay checkpoint, so one Undo/Redo removes/restores construction
and inferred relations together.

The retained session owns field-opaque, checkpoint-serializable
`SketchPersistentIdentityHighWater` metadata for persistent
objects and curve-local spline spans. Undo may remove an inferred construction from the current
graph, but it never rewinds those cursors; Redo, reload and a new divergent edit therefore cannot
reuse a retired identity. Coordinator checkpoints merge the lifecycle maximum back into restored
graphs and restore historical sketch content under the current exact parameter batch and external
snapshot set. Application workspace v5 serializes this checkpoint value, validates that its
namespace and componentwise maxima cover the stored design/accepted graphs, and strictly migrates
v1-v4 by deriving their graph-visible maxima. This adds no field to frozen sketch v1-v4 document
JSON or current unsupported draft-v5 document JSON.

### Keep the browser adapter thin

`geosolve-demo-web` maps browser events and semantic suppression, renders only the headless guide,
candidate and adjusted-preview DTOs, and supplies accessible labels/styles. It owns no anchor
generation, tolerance, wake memory, ranking, adjustment, branch choice or inferred document edit.
Native and WASM consumers must observe identical headless state transitions.

## Scope boundary

M70 adds no residual or new persistent constraint definition. Equality, symmetry, concentric,
quadrant, certified intersection/extension/collinear, nonlinear tangent/normal, arbitrary durable
point-pair horizontal/vertical, grid/axis and angle-increment inference are deferred. The candidate
primitive and branch-policy backlog is recorded in `docs/M71_GOALS.md`; that document does not make
M71 active or authorize implementation.

M70 also adds no inferred-state persistence, hidden construction geometry, canonical sketch-schema
migration, browser-owned geometric policy, browser E2E, mobile behavior, global root enumeration or
change to solver acceptance, rank, priority or branch semantics. Application-workspace v5 is the
narrow host-checkpoint migration described above.

## Verification

Direct `geosolve-constraint-editor` tests are authoritative for validated policy, exact threshold
and hysteresis boundaries, reference lifecycle, ranking/ambiguity, suppression, finite resource
limits, every `ConstructionPoint` stage, line/polyline directional inference and identical native/
WASM replay. Tests cover zoom/scale/order invariance and reject NaN/Inf inputs plus non-finite
derived projection output before candidate identities or inference state publish.

The deterministic cross-target oracle is
`crates/geosolve-constraint-editor/tests/m70_transition_parity.rs`, with expected bytes in
`crates/geosolve-constraint-editor/tests/fixtures/m70_transition_parity.golden.txt`. The same test
runs natively and through `wasm-bindgen-test-runner`; the release gate invokes its WASM form
explicitly. Its transcript covers every M70 inference family, tracking, ambiguity, suppression and
release, stale/clear lifecycle, atomic midpoint-plus-perpendicular publication, rejection,
Undo/Redo and reload. Focused native tests separately own exact resource-limit boundaries.

Direct coordinator tests are authoritative for point identity reuse, all-family native
PointOnCurve metadata, Midpoint, line/polyline Horizontal/Vertical, remembered Parallel/
Perpendicular, compatible two-relation bundles, one-solve atomic publication, retained rejection,
one-step Undo/Redo and deterministic reload/replay. They also prove that only the retained
session's exact accepted input can authenticate publication, that caller-assembled/detached scenes
cannot acquire authority, and that compatibility scenes remain renderable but non-publishing.
They also prove pre-bind semantic mutation rejection, post-bind authority revocation, the bounded
and cancellable 32-relation commit-plan envelope, process-local prepared CAS epochs, current-input
history restoration and bounded/validated persistent identity high-water across process reload.
The focused inference selection passes 46/46, the editor crate passes 266 unit tests plus all
relevant integration suites, demo-web passes 82/82 tests, the sketch library passes 33/33 and M56
passes 6/6.
ADR 0033 scope, Profile overlap, implicit-source mapping and computed-Fillet exclusion remain
mandatory regressions.

Thin workbench tests own Shift translation and queued-RAF ownership, guide/glyph markup,
accessibility and the absence of browser-owned inference calculations. One ordinary editable **Auto-constraint drafting
playground** supports focused human UAT after the full mechanical gate passes.

## Consequences

- The same drafting semantics can be embedded in a browser, native CAD or workplane-based 3D host.
- Hover can communicate persistent intent without mutating the document before a placement click.
- Visible adjustment and durable relations cannot diverge into separate history or solver paths.
- Semantic priority, ambiguity and incomplete work are inspectable rather than hidden in cursor
  behavior.
- The collision-free seal clones inference-visible native tessellation and construction snap
  anchors, roughly doubling that bounded portion of an authoritative scene. Replacing it without
  weakening authentication requires a broader immutable/accessor-based scene DTO.
- M70 deliberately leaves common higher-level CAD conveniences for a later primitive and branch-
  policy milestone instead of simulating them with misleading constraints.
