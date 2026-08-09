<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70 implementation — Headless auto-constraint drafting intelligence

Status: implementation and focused direct qualification are complete. Integrated release
qualification, release publication and supervising-human UAT are pending; M70 remains open.

Architecture owner: ADR 0034

Candidate source: **PENDING — NOT YET FROZEN**

Integrated release-gate result: **PENDING — NOT YET RUN ON A CLEAN NOMINATED SOURCE**

Tailscale release distribution and byte manifest: **PENDING — NOT YET PUBLISHED**

## 1. Files and APIs

M70 replaces the dormant manual provisional-inference seam inside
`geosolve-constraint-editor`; it adds no solver crate, residual, persistent constraint definition
or canonical sketch persistence version. The workbench envelope advances to v5 solely to persist
host-owned sketch identity allocator high-water that canonical graph bytes cannot retain after an
Undo followed by a divergent delete.

- `crates/geosolve-constraint-editor/src/inference.rs` owns `DraftInferenceBehavior`,
  `DraftInferenceTolerances`, `DraftInferenceLimits`, per-family `DraftInferencePolicy`, semantic
  `DraftInferenceInput` and stateful `DraftInferenceEngine` resolution. It consumes exact
  `DraftInferenceFrame`/`DraftInferenceSample` values and publishes typed anchors, contacts,
  candidates, guides, ranking evidence, ambiguity/suppression and explicit complete,
  candidate-limited or scene-limited outcomes. The whole prospective result is finite-validated
  before candidate/guide identities or wake state can publish; overflow in derived projection or
  screen/model conversion is a transactional `InvalidFrame`, not a usable snap.
- `crates/geosolve-constraint-editor/src/commit_plan.rs` adds `DraftPointSlot`, `DraftSpanSlot`,
  `DraftContactDescriptor`, `InferredRelation`, `ConstructionCommitPlan` and relation-indexed
  `ConstructionCommitResult` mappings. Plans can refer both to persistent geometry and to geometry
  allocated by the accompanying `ConstructionProposal`; direct geometry-only apply remains a
  compatibility route. A plan admits at most 32 inferred relations and charges each relation to a
  caller-controlled operation, so oversized, cancelled and exhausted publication cannot construct
  or retain a partial plan.
- `ConstraintEditor` runs positional inference at every `ConstructionPoint`-backed stage and
  direction inference only for real line/polyline spans. A point-identity candidate rewrites the
  relevant construction operand to `ConstructionPoint::Existing`; it is not lowered as a
  Coincident relation. A standalone Point-tool confirmation of an existing identity emits no plan
  and is a history-neutral no-op.
- Publication-authoritative `EditorScene` values can be created only by authenticating a scene
  against a retained session's exact current accepted document, design filter and
  `PreparedSketchInput`. Caller-assembled document/revision/stamp combinations cannot grant that
  authority. Compatibility/render-only scenes may still expose anchors, guides and inferred
  previews, but carry no prepared-input authority and cannot emit an inferred construction plan. A
  private collision-free `DraftInferenceSceneSeal` captures accepted revision, design identity,
  viewport, native inference curves and construction snap anchors. Pre-bind public mutation rejects
  authentication; post-bind mutation revokes publication authority. The terminal transition
  freezes the authenticated input, its exact displayed plan and one session-local
  `ConstructionCommitToken`.
- `RetainedEditorCoordinator` authenticates the token, plan and prepared input, applies one trial
  plan to a cloned retained session, solves once, independently validates fresh acceptance and
  rejects any newly fully or partially redundant inferred source. Successful publication swaps
  the exact trial as one history/replay checkpoint; failure leaves the live session and draft
  recoverable.
- `geosolve-sketch` adds field-opaque, checkpoint-serializable
  `SketchPersistentIdentityHighWater`,
  exact-input restoration
  counterparts for retained session restore, and the caller-controlled `transact_in_controller`
  seam needed for one bounded compound publication. Coordinator checkpoints retain persistent
  object and spline-span allocator maxima so Undo, reload and divergent history cannot reuse a
  retired identity. A collision-free process-local prepared-state epoch makes allocator-only
  advancement stale for CAS, while restored incarnations cannot alias each other's prepared work.
  The spline cursor map is bounded and streaming-decoded; namespace, object-cursor, curve-identity
  and span-cursor relationships are validated before merge or retention. History restoration uses
  the exact current parameter/external inputs and never presents historical accepted geometry
  under incompatible host truth. Workbench v5 stores that host-owned value and strictly migrates
  v1-v4; frozen sketch v1-v4 JSON and current unsupported draft-v5 JSON remain unchanged.
- `geosolve-demo-web` maps Shift to semantic suppression and renders only returned guides,
  adjusted previews and relation glyphs. It owns no anchor generation, wake memory, ranking,
  tolerance, geometric adjustment or inferred document edit. Modifier changes invalidate/replay a
  queued pointer sample only when construction drafting owns suppression; projected drags and other
  foreign interactions keep their exact queued terminal sample.
- `crates/geosolve-demo-web/src/workbench/samples.rs` adds the ordinary editable
  **Constraints & dimensions → Auto-constraint drafting playground**, including separate Profile
  and Construction point specimens, native curve targets, midpoint/affine references, ambiguity
  and deterministic redundant-Horizontal rejection. It has no guide, script, alternate
  coordinator, protected state or read-only mode.
- `crates/geosolve-constraint-editor/tests/m70_transition_parity.rs` and
  `tests/fixtures/m70_transition_parity.golden.txt` provide one deterministic native/WASM oracle.
  `scripts/release-gate.sh` runs the WASM form explicitly, the editor crate uses
  `wasm-bindgen-test`, and `shell.nix` supplies Node for that runner.

## 2. Mathematical behavior

M70 changes no residual equation, Jacobian, row scaling, solver priority, convergence status,
rank classification, independent validation rule or branch cell. It composes only existing
retained relations after presentation-independent geometric intent resolution:

| Intent | Positional adjustment | Durable result on placement |
| --- | --- | --- |
| Existing persistent point | exact accepted point | reuse identity structurally; no Coincident source |
| Native line/circle/arc/Bezier/conic/B-spline/NURBS position | exact accepted evaluation | PointOnCurve with span/domain/parameter/winding/neighbourhood |
| Native line/polyline midpoint | exact semantic midpoint | Midpoint, preferred over generic PointOnCurve |
| Near-horizontal/vertical authored span | adjusted endpoint | Horizontal or Vertical on the new line/live polyline span |
| Remembered affine span | direction-adjusted endpoint | Parallel or Perpendicular to the native reference span |
| Bare remembered point H/V | no adjustment by default | tracking-only guide; no fabricated relation |

One candidate bundle contains at most one positional and one compatible directional relation.
Point identity ranks before Midpoint before PointOnCurve; remembered Parallel/Perpendicular ranks
before an equivalent world-axis direction. ADR 0033 role priority applies before final geometric
error. Persistent identity stabilizes ordering but does not break an otherwise exact semantic
tie, which remains Ambiguous and cannot auto-commit.

Per-family guide, adjustment and persistence switches remain independent where their semantics
permit it. Persistent-point identity is structural operand reuse rather than a solver relation, so
persist-without-adjust is rejected instead of previewing one point and committing another.

Default inclusive enter/leave thresholds are `8/12 px` for points/midpoints, `10/14 px` for curves
and `4/6 degrees` for directions. Configured policy has hard ceilings of 32 candidates and eight
remembered references. Default scene-query bounds are 4,096 semantic anchors and 16,384
tessellation chords. Candidate generation stops at the first unique bundle proving overflow;
`required` is the first proven lower bound, not a full unbounded count. Candidate or scene
exhaustion publishes raw coordinates and no partial semantic prefix. All emitted candidate,
reference, guide, ranking and raw/adjusted screen/model values must remain finite; a finite input
whose derived projection overflows rejects transactionally and preserves prior engine state.

Wake memory is immediate, stage-local and non-persistent. It clears after stage placement, cancel,
tool exit, scene mutation, Undo/Redo, reload, policy/viewport change or stale identity.
Suppression acquires and applies nothing, clears the active latch and places the raw pointer sample
if the user clicks while suppressed.

The placement click confirms exactly the visible plan. A copied token cannot authorize a
substituted plan, and parameter/external-input changes or reattempts invalidate the prepared input
captured by the preview. Staleness, ambiguity, redundancy, conflict, cancellation or exhaustion
cannot substitute another inference. Rejection changes no live document/history and leaves the
draft correction-ready. One successful placement and one Undo/Redo treat construction and every
inferred relation atomically, while allocator high-water deliberately does not rewind on Undo.

## 3. Commands and outcomes

Focused qualification recorded during implementation passed. The focused inference selection is
exactly **46/46**. The complete editor crate run passes **266 unit tests plus all relevant
integration suites**; no aggregate integration-suite count is inferred or claimed here. The
complete demo-web run passes **82/82**. The sketch library passes **33/33** unit tests and its M56
prepared-work integration suite passes **6/6**.

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch --lib
cargo test --locked -p geosolve-sketch --test m56 --all-features
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity \
  --target wasm32-unknown-unknown
cargo clippy --locked -p geosolve-sketch -p geosolve-constraint-editor -p geosolve-demo-web \
  --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
git diff --check
```

The direct matrix covers exact point, curve and direction hysteresis boundaries; deterministic
ranking and ambiguity; order/zoom/uniform-coordinate-scale/translation invariance; NaN/Inf input
and non-finite derived-output rejection; fail-fast candidate and scene caps; bounded reference
memory with scope-aware affine eligibility; every construction stage and native curve family;
line/polyline direction inference; suppression and lifecycle clearing; exact token/plan/input
authentication; the 32-relation plan bound; atomic allocation/publication; redundancy,
cancellation and per-relation work exhaustion; one-step Undo/Redo/reload/replay under current host
inputs; process-local prepared epochs; process-reloaded persistent object and absent-spline identity
high-water; bounded streaming cursor decode and cursor exhaustion; pre-bind scene-mutation rejection
and post-bind authority revocation; thin browser Shift/RAF ownership and the ordinary editable
sample. Workspace persistence tests cover strict v1-v4 to v5 migration plus malformed, foreign,
behind-graph and trailing-input rejection.

The native/WASM golden transcript covers point identity, PointOnCurve, Midpoint, Horizontal,
Vertical, Parallel, Perpendicular, tracking, ambiguity, suppression/release, stale/clear lifecycle,
midpoint-plus-perpendicular publication, exact Redo/reload state, redundant-plan rejection and
unchanged rejected history. Resource-boundary values remain focused native-test evidence rather
than being overstated as golden-transcript rows.

Before human UAT, one frozen source must still pass:

```text
nix-shell shell.nix --run './scripts/release-gate.sh'
```

Those focused counts do not nominate a release source. The integrated result, nominated source
hash, release Trunk distribution, manifest and served-byte comparison are intentionally
**PENDING**. No clean release-gate or deployment success is claimed by this document yet.

## 4. Acceptance criteria passed and pending

The implementation and focused direct matrix satisfy the headless ownership, inference-family,
hysteresis, ranking, suppression, atomic plan, exact-input authentication, never-reuse history,
native/WASM parity, thin-adapter and editable-sample criteria in `ACCEPTANCE.md`.

The following milestone gates remain pending:

- [ ] clean integrated `scripts/release-gate.sh` on one nominated source;
- [ ] frozen candidate hash and release Trunk distribution;
- [ ] Tailscale publication plus byte-for-byte manifest verification; and
- [ ] supervising-human approval of every area in `docs/M70_UAT.md`.

M70 is not complete and human UAT has not begun.

## 5. Known limitations and next blocker

M70 uses only existing ordinary retained constraint primitives. It does not infer equality,
symmetry, concentric/quadrant, certified intersection/collinear/extension, nonlinear tangent/
normal, grid/axis or angle-increment intent, and does not persist arbitrary point-pair H/V from a
tracking guide. Computed Fillet arcs remain ineligible. Inferred state is session-local; canonical
sketch schemas remain unchanged, while workspace v5 adds only host-owned identity high-water.
Browser E2E, mobile behavior, global root enumeration and
browser-owned geometric policy remain excluded.

The exact scene seal intentionally clones inference-visible native tessellation and construction
snap anchors, roughly doubling that bounded portion of each authoritative scene. This avoids a
collision-prone digest and preserves the current ergonomic public presentation DTO; eliminating
the duplicate would require a broader immutable/accessor-based scene API change outside M70.

`docs/M71_GOALS.md` is a temporary candidate backlog only. M71 is not active, ordered, scoped or
authorized for implementation. The immediate blocker is the pending M70 integrated release and
publication gate, followed by human UAT—not any M71 work.
