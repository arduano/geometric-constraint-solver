<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M87 implementation ledger — Cohesive managed parameters and browser-free design loop

Status: **COMPLETE and accepted on 2026-08-31**. This ledger retains the historical dirty-worktree
implementation record over baseline `1ea4940`; exact product source
`32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`, now passes the complete clean release gate. The sound
managed-control, headless, routing-board, twelve-demo CNC/Gridfinity, shared-renderer,
graphics-audit and retained-camera recovery work remains, and the complete experimental adaptive-
detail/LOD prototype remains deleted. The supervising user's milestone-level close decision accepts
U9/U10 without a separate row replay. No immutable freeze, public deployment or service retirement
is inferred.

`docs/M87_AUDIT.md` owns the findings, `docs/M87_GOALS.md` and the completed M87 sections in
`ACCEPTANCE.md`/`docs/SCENARIOS.md` own the behavioral gate, ADR 0042 owns the architecture, and
`docs/M87_HEADLESS.md` documents the stateless native workflow.

## Authority and scope

- Managed controls cover explicit code-defined non-DoF values. They are authenticated source
  capabilities, not solved values or solver authority.
- Point coordinates and other solver-instance degrees of freedom keep M84's existing semantic
  draft/overlay path. M87 never writes a solved coordinate to managed source.
- Shared literals remain one token and expose their complete authenticated consumer fan-out.
  `EditLens` remains compatible presentation metadata and cannot manufacture inverse authority.
- Every accepted live edit belongs to one outer `SketchCodeSession` transaction. The neutral
  editor may preview a delegated group, but it may not publish nested code-mode history.
- Static output consumes independently accepted native/feature authority. SVG/report bytes are
  deterministic authority; PNG is deterministic visual evidence for pinned inputs, not a solver
  or cross-platform mathematical oracle.
- M87 changes no primitive, constraint, dimension, residual, Jacobian, solver priority, tolerance
  or implicit branch rule and adds no TypeScript runtime to Rust/WASM/headless execution.
- The CNC and Gridfinity additions are 2D/2.5D design sketches only. They add no CAM, toolpath,
  cutter-compensation, boolean, solid, print-fit or manufacturing-validation authority.
- Diagnosed performance/stack work is not retroactive M87 scope; it became an ordered prerequisite
  in the subsequently completed M88.

## Component ledger

### I1 — Bounded transient managed controls

Implementation status: **implemented and covered by dirty-worktree mechanical qualification;
clean exact-source/tree nomination remains unclaimed**.

- [x] Add `crates/geosolve-sketch-code/src/managed_control.rs` and export the public control DTOs,
  schemas, tokens, consumers, read-only/navigation outcomes, edit batches and errors from
  `geosolve-sketch-code`.
- [x] Derive a deterministic `ManagedControlManifest` from parser-owned value spans, direct
  declaration semantics, exact artifact dependency bindings and accepted direct/generated
  provenance.
- [x] Authenticate project/source authority, expected typed value and generated generation. Float
  CAS compares IEEE bits, including signed zero; expansion content is digest-validated before a
  token is issued.
- [x] Apply unordered batches atomically, reject duplicate/stale/foreign/wrong-schema or over-bound
  work before rewrite, losslessly replace the exact spans and reparse only the complete candidate.
- [x] Bound controls, batch edits and consumer edges independently. Shared/transitive inputs expose
  complete invocation-local fan-out rather than copying one value per generated child.
- [x] Add `ManagedScalarBinding` as one expression-free lexical sharing form. Its right-hand side is
  exactly one finite number or unit literal and it may feed only later declaration arguments;
  chains, aliases, expressions, child paths, sketch outputs and organization membership reject.
- [x] Make PC Water Manifold's one `channelBendRadius = mm(5)` binding feed the upper, middle and
  lower `waterChannel` invocations, with one control and all six generated Fillet consumers.
- [x] Classify solver-instance/DoF values, references, structural identities, structure, explicit
  `null`, incompatible schemas and unproven transforms explicitly as read-only; references may
  navigate to their exact owner. Absence has no parser-owned span and is not fabricated as a row.
- [x] Reject unknown top-level `CodeProject` JSON fields at the strict canonical authority boundary.
- [x] Keep manifest/token material transient. The focused tests require byte-identical project and
  expansion wire authority before and after derivation.

Principal public seams:

- `managed_control_manifest`
- `apply_managed_control_batch`
- `apply_managed_control_manifest_batch`
- `ManagedControlManifest::controls_for_generated`
- `ManagedControlSchema`, `ManagedControlToken`, `ManagedControlConsumer` and
  `ManagedControlReadOnlyReason`

Focused owner: `crates/geosolve-sketch-code/tests/m87_managed_controls.rs`. Its nineteen cases cover
Typed Panel's one radius/two-Fillet fan-out, stale/foreign/generation authentication, positive unit
domains, boolean/choice/signed-zero edits, atomic batches, read-only classes without fabricated
absence, transient wire bytes, all bundled leaves, invocation locality, lexical shared-scalar
restrictions, Water Manifold's three-way shared binding, expansion digest validation and the
independent batch bound.

### I2 — Shared target-neutral sketch renderer

Implementation status: **implemented and covered by dirty-worktree mechanical qualification;
clean exact-source/tree nomination remains unclaimed**.

- [x] Add pure-Rust `geosolve-sketch-render` and move camera fitting, adaptive grid/datums, scene
  SVG composition, icons and standalone SVG wrapping behind shared APIs consumed by the web crate.
- [x] Freeze the pre-extraction browser scene in
  `crates/geosolve-demo-web/tests/fixtures/m87_renderer_scene.json`; the asserted composed-byte
  digest is `bd364332a1c4c1024aa17434ec862d5e4968124a2e8cb09829b04c677b898605`.
- [x] Keep interactive browser scene composition delegated to the shared crate instead of
  duplicating equations or SVG semantics in the adapter.
- [x] Add native-only PNG rasterization with pinned `resvg 0.47.0`, a bundled Share Tech Mono font,
  resource/dimension bounds and no system font or external image dependency.
- [x] Freeze static dimensions and fitting policy: logical `1000 x 700`, margin 64 px, scale clamp
  2--2000 px/model-unit, 0.25 px chord tolerance and PNG `2000 x 1400`.
- [x] Keep static output free of selection/hover/draft/inference/action/error presentation while
  retaining accepted grid, datums, geometry and annotations.

Principal public seams include `CanvasCamera`, `compose_static_scene_svg`,
`compose_fitted_static_scene_svg`, `standalone_export_svg`, `render_scene_png` and
`render_default_scene_png`.

### I3 — Browser-free headless library and CLI

Implementation status: **implemented; the original focused library/CLI integration tests are
covered by the pre-dogfood provisional dirty-worktree gate, routing-board evidence is additive and
the current ten-test headless suite covers all twelve demos plus deterministic manufacturing
products. Clean exact-source/tree nomination remains unclaimed**.

- [x] Add the pure-Rust `geosolve-headless` library and binary as a workspace member.
- [x] Admit exactly artifact-free managed source plus project key, canonical complete pinned
  `CodeProject` JSON, or one of the twelve bundled demos. Never execute custom TypeScript.
- [x] Expose `bundled_demo_keys`, `inspect`, exact-CAS `edit`, `render` and `publish_render` APIs and
  matching `demos`/`inspect`/`edit`/`render` CLI commands.
- [x] Cold-expand/materialize, solve, evaluate computed features and independently validate finite
  accepted geometry/residuals before producing a success-like report.
- [x] Use deterministic digest-derived identities and deterministic report/control/SVG products.
  Produce the fixed semantic PNG properties from I2 without a browser, server, DOM, network, Node
  or TypeScript runtime.
- [x] Publish only to a new directory through atomic staging. An existing destination or any input,
  CAS, solve, feature or publication failure cannot overwrite prior output or leave a partial
  generation.

Focused owner: `crates/geosolve-headless/tests/m87_headless.rs`. Its coverage includes one shared
inspect/edit/solve/render authority, all three input classes, deterministic non-overwriting output,
the board-specific report/control/logical-scene/SVG/PNG equality check, an actual CLI subprocess
path, all-twelve-demo cold materialization and both manufacturing deterministic-render cases.

### I4 — Dedicated code-control RPC and typed client

Implementation status: **implemented and covered by dirty-worktree mechanical qualification;
clean exact-source/tree nomination remains unclaimed**.

- [x] Add a closed version-one code-control vocabulary in
  `crates/geosolve-demo-web/src/workbench/code_control_rpc.rs`: `inspect_managed_controls`,
  `edit_managed_controls`, `undo` and `redo`.
- [x] Authenticate every mutation against an expected outer `CodeSessionIdentity`, return bounded
  typed success/failure envelopes and install a replacement editor only for the exact accepted
  outer checkpoint. A retained source failure advances only the deliberate outer receipt while
  preserving prior accepted canvas authority.
- [x] Install/retire the WASM callback with the code workbench and expose the browser-free JSON
  boundary separately from Intent RPC.
- [x] Reject mutating Intent RPC while code owns authority rather than allowing nested history to
  bypass the outer session.
- [x] Add bounded wire DTOs, validators, encoder/decoder, `CodeControlClient` and positive/negative
  runtime/type coverage to `@geosolve/sketch-code`; add the corresponding code-authority failure
  vocabulary to `@geosolve/intent`.
- [x] Finish the complete Inspector/Code-panel crossover on the settled worktree. The
  Code panel must enumerate all invocation-specific controls and the Inspector must resolve an
  exact selected semantic path through the same manifest without property-specific authority.
- [x] Prove the crossed Typed Panel `4 -> 2` flow, stable selection, one outer history row,
  Undo/Redo/reload parity and unchanged ordinary GUI Inspector fallback through the live browser
  adapter.

The RPC's local Rust tests cover strict vocabulary/bounds, Typed Panel outer history and the
unavailable bridge. Those tests and both TypeScript suites pass together after crossover
integration; alone they are proportional evidence, while the complete dirty-worktree release gate
provides the provisional mechanical qualification recorded below.

### I5 — Presentation-independent grouped Fillet preview seam

Implementation status: **neutral editor seam and live code-workbench ownership/release integration
are implemented and covered by dirty-worktree mechanical qualification; clean exact-source/tree
nomination remains unclaimed**.

- [x] Add `ProjectionalEditorSession::delegate_computed_fillet_radius_drag` for one authenticated,
  bounded group of current computed Fillets with the same exact source radius.
- [x] Prepare one multi-definition preview patch per pointer frame so every consumer displays the
  proposed shared radius without parsing source or publishing history.
- [x] Return `DelegatedComputedFilletRadiusProposal` through
  `ProjectionalEditorPointerOutcome::delegated_computed_fillet_radius` on release and suppress the
  ordinary nested Intent terminal transaction for that delegated route.
- [x] Reject empty, duplicate, stale, mixed-radius, repeated-definition and over-bound groups while
  retaining prior accepted authority. Preserve the ordinary single-Fillet route.
- [x] In `CodeProjectWorkbench`, authenticate the selected generated child, resolve its one
  manifest control and complete consumer feature group, delegate pointer-down/frames, then consume
  the terminal proposal through one manifest-bound outer source edit.
- [x] Qualify cancellation, stale/mixed groups, M86 picking priority, point
  overlays and ordinary non-code Fillet gestures through the browser pointer lifecycle.

Focused prepared-route cases live in
`crates/geosolve-constraint-editor/src/intent_editor_prepared_tests.rs`.

### I6 — Retained camera liveness and full-detail presentation boundary

Implementation status: **implemented and proportionally qualified in the dirty worktree; clean
exact-source/tree nomination remains unclaimed**.

- [x] Make retained camera admission two-phase. A desired camera remains retryable until the
  accepted-scene transform and camera HUD paint both succeed; failed or stale callbacks record no
  completed camera presentation.
- [x] Route projectional and flat presentation failures through exact transient reprojection. A
  missing `.wb-accepted-scene` group can therefore rebuild from retained accepted authority and
  cannot strand the animation-frame queue.
- [x] Remove the experimental adaptive-detail policy completely: no policy module or workbench
  field, scene-complexity count, camera-route hook, DOM attribute, display control, CSS suppression
  rule or LOD-specific test remains. Every retained camera frame paints the complete scene.
- [x] Keep `RetainedCameraQueue` concerned only with desired/exact camera authority, admission,
  completion, retry and reconciliation. A future profiled renderer may wrap the two shared camera-
  presentation boundaries, but it must not add policy state back to the queue or six event sites.

The post-removal focused owner check passed:

- `cargo test --locked -p geosolve-demo-web --lib retained_camera -- --nocapture` — 5/5;

The earlier adaptive-detail browser evidence is withdrawn with that prototype. The retained-camera
failure injection and resize recovery remain relevant liveness evidence, but not LOD evidence.

The prior scoped-disposition graphics audit also resolved **M87-F002** at
`geosolve-sketch-render`. A finite-input
pan could overflow while computing the next model centre, and the public grid-path loop admitted
invalid/non-advancing spacing. The former failed its new exact regression with exit `101`; the
latter reproduced under a bounded timeout with exit `124`. Camera construction/mutation is now
validated and transactional, while grid-path construction is private, fallible, progress-checked
and capped at 4,096 total lines. These focused owning cases do not expand the golden matrix.

Post-repair checkpoint evidence:

- `cargo test --locked -p geosolve-sketch-render` — 24/24;
- both exact M87-F002 owner regressions — 1/1 each;
- `cargo test --locked -p geosolve-demo-web --lib retained_camera -- --nocapture` — 5/5;
- `cargo test --locked -p geosolve-headless --all-targets` — 9/9;
- focused warnings-denied renderer/headless Clippy — exit `0`;
- frozen pre-extraction renderer bytes and ordinary adaptive-grid regression — 1/1 each;
- `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown` —
  exit `0`, with only the established target-specific dead-code warnings;
- `/tmp/geosolve-m87-full-detail-resize-uat.mjs` against the mutable Tailscale development server
  — exit `0`: no LOD control/attribute/CSS remains, forced retained-scene loss recovers exactly
  across `1440 x 900`, `1024 x 720` and `1920 x 1080`, and a later frame is camera-only with the
  accepted scene present.

The audit preserves current renderer extraction, compositor bytes, paint order, native raster
bounds and headless scene authority. It defers request-object/wildcard-export/style consolidation
because no reproduced z-order defect justifies that risk in M87. Atomic no-clobber headless
publication is now documented accurately as Linux/Android/Apple/Redox-only; other targets fail
closed.

### I7 — Additive robotic cable-harness routing-board dogfood

Implementation status: **implemented and focused-qualified in the shared dirty worktree**.

- [x] Add `robotic-routing-board.sketch.ts` as the tenth bundled project plus byte-identical managed
  package fixture. The board is 360 x 220 mm and carries four mounting bores, two eight-connector
  banks and eight keyed ten-vertex open route Polylines.
- [x] Add the caller-owned `harnessRoute` TypeScript patch and pinned data-only artifact with exact
  Rust/package/test copies. Its keyed mapping produces 80 clip circles and 64 existing host Fillets
  from 72 native route spans; no Rust/WASM/headless path executes TypeScript.
- [x] Keep board, bore, connector-centre and route-endpoint authority fixed while interior route
  points remain solver-instance movable. `serviceRoute.serviceLoop` is the focused generated-point
  overlay lens and never receives a managed-source coordinate edit.
- [x] Derive one shared `mm(2.4)` clip-radius control with 80 consumers and one shared `mm(5)`
  bend-radius control with 64 consumers. Localizing only `serviceHarness` partitions consumers
  70/10 for clips and 56/8 for Fillets; shared/local exact-CAS edits remain independent.
- [x] Cover keyed `inspectionClip` insertion/removal/reinsertion. Exactly one point, segment, clip
  and Fillet owner is added; unrelated logical/native/host identity remains stable and reused
  retired owners advance generation.
- [x] Factor exact endpoint/shared-point/active-Coincident connectivity into
  `PreparedEndpointTopologyQuery`/`EndpointTopologyIndex`. Intent aggregate validation no longer
  invokes whole-scene visual arrangement, while Offset operands retain complete visual-profile
  authority.
- [x] Make noninteractive code composition build and translate each exact native Fillet candidate,
  then publish all nodes in one atomic intent patch and requested suppressions in at most one
  additional patch. Interactive presentation-preview materialization and one-owner-per-corner
  semantics are unchanged.
- [x] Append the exact separate-ledger row without changing PC Water Manifold:
  `f57cf8ecc0ee2785a82437c3725e0f4c547cb0242f6acdd56cd861b5b7c61687` managed,
  `f4f453578a36ee9c71c4c84f3fe26164a8748a72e3bebc930a8204a6052c8b1b` artifact and
  `9f8c97d035bd96ccf4425442b62531e7da8047d215973886f0b2e85a2998e7ec` custom source,
  with 76 declarations, 317 generated members and 25 typed outputs.
  The complete ten-project ledger has SHA-256
  `65f7eb813d006292ee476fc030ac1ae56a6bcf9f0ba3b51296c05c1f264007fc`.
- [x] Add board-specific repeated headless equality for pretty report/control bytes, logical scene,
  standalone SVG and pinned-build PNG. The accepted report inventory is 104 points, 176 curves, 41
  constraints, two dimensions, 64 features and 136 computed edges.

Focused owners are `crates/geosolve-sketch-code/tests/m87_routing_board.rs`,
`crates/geosolve-sketch-topology/tests/endpoint_topology.rs`, the batched-composition regressions,
the board-specific web history/reload case and `crates/geosolve-headless/tests/m87_headless.rs`.
The separate code-project ledger grows from nine to ten rows; the historical PC Water Manifold row
and the milestone-neutral 271-row authoring/scene golden are not reclassified.

### I8 — Additive CNC and Gridfinity manufacturing-sketch dogfood

Implementation status: **M87-F003 relational-authority repair clean-qualified and accepted**. The
earlier dirty-gate evidence still predates this repair; exact source `32c7289`, tree `38f7175`,
passes the later clean gate. No immutable candidate is nominated.

- [x] Add **CNC joinery fit coupon · keyed corner reliefs** as the eleventh bundled project. It
  contains one constrained 120 x 140 mm female blank, three 70 mm-wide mortises with
  loose/nominal/press heights of 18.4/18.0/17.6 mm and three 95 x 18 mm tabs.
- [x] Replace its seven unrelated component `FixedPoint` locks with exactly one `FixedPoint`, zero
  `FixedCoordinate` rows and seven horizontal/vertical length-governed construction datums. Keep
  the same nominal positions, topology, dimensions, circles and Fillets.
- [x] Add the caller-owned `cornerReliefs` `mapRecord` patch with byte-identical package, managed
  and Rust source copies plus canonical package/Rust artifacts. One shared 3.175 mm input fans out
  to twelve ordinary circles. Reuse the existing record-Fillet patch for four blank and six tab
  handling corners at one shared 6 mm radius.
- [x] Add **Gridfinity 1×1×3U section · keyed standard profile** as the twelfth bundled project.
  Its single symmetric closed material contour has exactly 26 keyed vertices and records 41.5 mm
  outer width, 35.6 mm base bottom, 4.75 mm staged base, 7 mm cavity floor, 21 mm body, 0.95 mm
  walls and 4.4 mm nominal stacking-lip rise. Existing mapped Fillets produce two 2.8 mm floor and
  two 0.6 mm lip Fillets from independent controls.
- [x] Replace its unconstrained literal-seed contour with zero `FixedPoint` rows, one Y
  `FixedCoordinate`, thirteen `symmetricAboutDatumAxis` relations and ten orthogonal construction
  spans governing five diagonal stages. Keep all 26 contour vertices and nominal standard geometry.
- [x] Freeze CNC control locality: localizing only press reliefs partitions circles 8 shared/4
  local; localizing only press-tab handling partitions Fillets 8 shared/2 local. A fit-station
  dimension edit must affect only that station, while removing and reinserting one relief key must
  retire/recreate only its circle and advance only that generation.
- [x] Freeze the Gridfinity contour's closure, symmetry, exact reference dimensions, four Current
  Fillets and independent 2/2 radius fan-outs with finite accepted geometry and independently
  validated Hard residuals.
- [x] Extend exhaustive native, web, headless/CLI and package inventories from ten to twelve; append
  CNC then Gridfinity rows after routing board without changing the first ten reviewed rows; and
  freeze repeated report/control/logical-scene/SVG/PNG products for both additions. The accepted
  post-F003 native inventories are, in declaration/generated/output/point/curve/constraint/
  host-output/feature/computed-edge order, CNC
  `(62, 69, 14, 29, 47, 36, 10, 10, 23)` with 33 dimensions and Gridfinity
  `(62, 66, 3, 31, 36, 31, 4, 4, 11)` with 18 dimensions.

The conservative corner-centred CNC circles are authored dogbone-style overcuts, not inferred
cutter compensation. Neither entry is CAM, a toolpath preview, a boolean/solid model or a claim of
machinability/print fit. Control, keyed reconciliation, native materialization, computed Fillets and
rendering remain separately composable owners.

### M87-F003 — Relational manufacturing-sample authority

The supervising user reported that the two retained samples were not properly fully constrained
and asked for a design-intent constraint scheme with a reasonable minimum of fix authority. The
narrowest public cold-materialization reproduction classified this as a sample-authoring/rank-DOF
defect: Gridfinity had 26 literal-seeded points and no native constraints, while CNC used seven
unrelated full-point locks to locate independent components.

The focused manufacturing oracle now requires finite accepted coordinates/scalars, independently
validated Hard residual `<= 1e-9`, Current computed features, numerical and structural left/right
nullity zero, equality DOF zero and bidirectional bounded DOF zero. It additionally requires exactly
one absolute datum per sketch: CNC `(FixedPoint, FixedCoordinate) = (1, 0)` and Gridfinity `(0, 1)`.
The Gridfinity owner freezes all thirteen symmetry relations; accepted inventory and dimension
counts freeze both relational constructions. No residual, Jacobian, solver policy, primitive or
constraint implementation changed, so no finite-difference Jacobian addition is applicable.

### Post-provisional keyed-Fillets drag and parameter-authority hardening

The 2026-08-30 focused follow-up remains part of the same open dirty-worktree implementation. It
does not nominate a clean source/tree or represent a separately replayed human-UAT row.

- The intermittent **Typed Panel · keyed Fillets** upper-left release snap-back was reproduced at
  the terminal-publication seam. A live native drag and an independently staged source
  rematerialization can differ by machine roundoff in redundant rectangle-corner aliases and the
  continuous computed Fillet geometry causally rebuilt from them. Terminal publication now treats
  the authenticated lower-left/upper-right rectangle seeds as canonical and permits only the
  bounded redundant-alias cell needed to reconcile those two independently solved projections.
- The remaining apparently random release was frozen at retained target 23, screen `[303, 247]`:
  every native pointer step succeeded, while publication alone rejected
  `edge[4].arc.start_angle` represented as `-pi` versus `+pi`. Only authenticated recomputed arc
  start/end angles under the rectangle-alias policy now receive one-turn periodic equivalence, and
  the unwrapped residual must still fit the existing finite tolerance. Exact policy remains
  bit-exact. The real 23-release regression requires case 22 to emit the periodic trace stage,
  selected turn, unwrapped value and residual before proving outer publication and exact reload.
- Roundoff authority is local and composable. Every normalized redundant alias carries its own
  rectangle-local scale; accepted-domain propagation maps only exact `NativeCurveSpanSource`
  values, narrows a Polyline to the incident segment, and narrows clamped or periodic B-spline and
  NURBS curves to each basis span's exact `support()`, including periodic wraparound. Splines with
  no affected control skip basis construction. A computed edge is admitted only when its geometry,
  contacts and provenance authenticate the same exact sources and Fillet owner; an unrelated
  distant point, span, spline or large second rectangle cannot inflate a local edge's cell.
- The relaxation remains deliberately presentation/publication-only. Feature radius, topology,
  source ownership, endpoint order, sweep, tangent orientation, winding, retained branches,
  provenance and IDs remain exact, as do all exact-policy comparisons including signed zero.
  Suppressed Fillets receive no re-anchoring allowance. Focused regressions reject one-ULP
  encoded-radius changes, inconsistent geometry/provenance pairs, nonincident Polyline/spline
  spans, unrelated policies and material local alias or angle differences.
- Inspector metadata now states one of **Modifiable in sketch.ts**, **Modifiable instance**,
  **Encoded** or **Blocked** beside every selected code-owned parameter. For either generated
  Typed Panel Fillet, radius resolves to `cornerFillets.radius`, displays exact source `mm(4)` and
  discloses its two generated consumers. Generated contact/branch state is Encoded; solver-owned
  point coordinates remain Modifiable instance; a dirty source draft or retained failure becomes
  Blocked. Code-owned Display name is likewise Encoded/Blocked and emits no edit action, while an
  ordinary GUI-owned name remains editable.
- The corresponding implicit-performance review removed repeat authority work: one managed
  manifest `Rc` and one Inspector descriptor index serve each durable render. The transient cache
  is keyed by exact code-session, project, source and expansion identity; dirty text is excluded,
  retained failures use their own identity rather than the prior clean entry, and successful
  failure-state presentation is reusable without entering persistence. Ordinary GUI and clean
  point-instance Inspector edits decline before manifest construction, while dirty/failed
  code-owned routes still block first. Inspector and grouped-Fillet mutations independently derive
  one fresh borrow-scoped `ManagedControlAuthority` for exact CAS; bounded indices/cursors replace
  repeated source, provenance, fan-out, instance-leaf and source-markup scans.

Focused post-follow-up evidence includes the roundoff filter 10/10, the real 23-release seam case
1/1, the cache/early-route/ordinary-GUI owner regressions, `geosolve-demo-web` 344/344, managed
controls 19/19, actual WASM 3/3 and warnings-denied all-target/all-feature browser Clippy.

## Current qualification record

The following focused checkpoints and historical dirty-worktree gates qualify their exact recorded
slices in the shared dirty worktree. They are not evidence for a clean exact-source/tree nomination,
immutable candidate, public release or a human disposition for M87-U9 and M87-U10:

- `cargo test --locked -p geosolve-sketch-code --test m87_managed_controls -- --nocapture` passed
  19/19; the locked crate suite and warnings-denied crate Clippy also passed at that checkpoint.
- `cargo test --locked -p geosolve-sketch-render` passed 22/22; focused renderer Clippy, WASM
  consumer check, frozen browser-composer bytes and standalone export tests also passed at that
  checkpoint.
- Before the dogfood amendment,
  `cargo test --locked -p geosolve-headless --test m87_headless -- --nocapture` passed 8/8,
  including the actual CLI subprocess and historical all-nine-demo cold-materialization cases.
- The routing-board additive headless suite passed 9/9 in 47.60 seconds with the historical ten-
  demo catalog and the focused board equality regression. The isolated board test passed 1/1 in
  31.66 seconds and the actual CLI inventory/edit/render test passed 1/1 in 18.87 seconds.
- `cargo test --locked -p geosolve-sketch-code --test code_project_golden
  m84_code_project_ledger_is_deterministic_and_reviewed_separately -- --exact` passes 1/1 after the
  reviewed routing-board row is appended. The PC Water Manifold row remains unchanged from the
  incoming M87 worktree.
- Focused grouped Fillet preview/cancel/stale/mixed owner tests pass, and the post-follow-up
  `cargo test --locked -p geosolve-demo-web --lib -- --nocapture` passes all 344 browser-library
  tests, including
  the Typed Panel Inspector, grouped grip, Code RPC, ordinary GUI fallback, point-overlay and M86
  picking regressions.
- The actual WASM code-control export regression passed 1/1, and the broader
  `cargo test --locked -p geosolve-demo-web --lib actual_wasm_ --target wasm32-unknown-unknown`
  run passed 3/3 through `wasm-bindgen-test-runner`.
- Both TypeScript package suites pass (`@geosolve/intent`: 40/40 runtime tests;
  `@geosolve/sketch-code`: 14/14 runtime tests plus fixture, type and managed-source checks).
- `cargo fmt --all -- --check`, `git diff --check`, browser test compilation, browser
  warnings-denied all-target/all-feature Clippy and the locked all-feature WASM check pass. The
  WASM check reports only the two pre-existing target-specific dead-code warnings that the release
  gate does not deny.

### Historical pre-F003 additive manufacturing focused qualification — 2026-08-30

During this bounded pre-F003 sequence, the dirty-worktree source/document signature remained exactly
`be7c5ef9cb7c6f7a28c4a86eae9534821e8858f7317df1a7ab202d1fd036308e` before and after this
sequence:

- `cargo test --locked -p geosolve-sketch-code --test m87_manufacturing_sketches -- --nocapture`
  passed 3/3 in 5.28 seconds, covering CNC fan-out/locality, relief generations and the closed
  symmetric Gridfinity profile with independent radius owners;
- `cargo test --locked -p geosolve-headless --test m87_headless -- --nocapture` passed 10/10 in
  71.42 seconds, including cold acceptance of all twelve bundled demos and byte-deterministic
  manufacturing report/control/logical-scene/SVG/PNG products;
- `cargo test --locked -p geosolve-sketch-code --test m84_native_composition -- --nocapture` passed
  13/13 in 32.20 seconds across the complete twelve-project native inventory;
- `cargo test --locked -p geosolve-sketch-code --test code_project_golden
  m84_code_project_ledger_is_deterministic_and_reviewed_separately -- --exact` passed 1/1 in 0.13
  seconds. CNC and Gridfinity are appended after routing board, and the first ten reviewed rows
  remain byte-unchanged.

This qualification is retained for its exact pre-F003 sources only. It does not qualify the later
relational-authority repair and does not manufacture clean nomination, human visual acceptance or
publication evidence.

### Post-F003 focused qualification — 2026-08-30

- `cargo test --locked -p geosolve-sketch-code --test m87_manufacturing_sketches -- --nocapture`
  passed 3/3 in 21.29 seconds. The shared inventory assertion independently freezes finite accepted
  geometry/scalars, Hard residual validation, Current features, zero numerical/structural nullity,
  zero equality/bidirectional bounded DOF and one minimal absolute datum. The CNC owner requires
  `(FixedPoint, FixedCoordinate) = (1, 0)`; Gridfinity requires `(0, 1)` plus thirteen symmetry
  relations. Representative contour, datum and component seed coordinates are deliberately
  displaced and must cold-solve back to the same accepted geometry, proving literals are not
  hidden locks.
- `cargo test --locked -p geosolve-sketch-code --test code_project_golden
  m84_code_project_ledger_is_deterministic_and_reviewed_separately -- --exact` passed 1/1 in 0.11
  seconds against the final reviewed post-F003 ledger.
- `git diff --check -- ACCEPTANCE.md PLAN.md docs/SCENARIOS.md docs/M87_AUDIT.md docs/M87_GOALS.md
  docs/M87_HANDOVER.md docs/M87_HEADLESS.md docs/M87_IMPLEMENTATION.md docs/M87_UAT.md
  crates/geosolve-sketch-code/tests/golden/m84_code_project_ledger.tsv` passed.

- `cargo test --locked -p geosolve-sketch-code --test m84_native_composition` passed 13/13, and
  the complete `cargo test --locked -p geosolve-sketch-code` suite passed, including 64/64 library
  tests, every integration suite and the doctest.
- `cargo test --locked -p geosolve-headless --test m87_headless -- --nocapture` passed 10/10,
  including all twelve cold materializations and byte-deterministic CNC/Gridfinity
  report/control/logical-scene/SVG/PNG products.
- `npm test` in `packages/geosolve-sketch-code` passed build, artifact-fixture parity, 14/14 runtime
  tests, type checks and all managed-source checks.
- `cargo fmt --all -- --check`, `git diff --check` and
  `cargo clippy --locked -p geosolve-sketch-code -p geosolve-headless --all-targets --all-features
  -- -D warnings` passed. Generated-radius lowering additionally rejects non-finite targets at its
  owning boundary; no residual equation changed.
- `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown` and
  `./scripts/verify-geosolve-sketch-code-package.sh` passed. The WASM check reports only existing
  target-specific dead-code warnings.
- `env NO_COLOR=true nix-shell ../../shell.nix --run 'trunk build --locked'` from
  `crates/geosolve-demo-web` refreshed the mutable development distribution. All seven files served
  at `http://100.94.63.83:8080/` byte-match the local output; this is not release qualification.
- Fresh post-F003 products were generated at
  `/tmp/geosolve-m87-post-f003.FPHP3b/{cnc,gridfinity}`. Both reports independently validate Hard
  residuals and Current features; the maximum normalized Hard residuals are respectively
  `7.105427357601002e-15` and `2.886579864025407e-15`.

At this historical checkpoint no full post-F003 release gate, clean nomination, publication or
human UAT was claimed. The clean closeout record below supersedes only that current-status claim.

The complete provisional gate was run as:

```bash
GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

It exited `0`. Formatting and diff hygiene, warnings-denied workspace Clippy, all-feature workspace
tests, exact golden `--require-clean`, locked WASM including `actual_wasm_` 3/3, TypeScript 40/40 for
`@geosolve/intent`, TypeScript 14/14 plus fixture/type/managed-source checks for
`@geosolve/sketch-code`, warnings-denied Rustdoc, benchmark compilation and release-performance
checks, `cargo-deny`, all 12 package-list checks, packaged `geosolve-sketch-code` verification and
the final Trunk assembly all passed.

This command explicitly allowed a dirty worktree. It supplies complete provisional mechanical
qualification, not a clean gate, source/tree nomination, immutable candidate, human UAT decision or
publication result or M87 closure.

### Additive routing-board focused qualification — 2026-08-30

The routing-board amendment is mechanically qualified in the current dirty shared worktree. This
record is additive to the pre-dogfood qualification above; it did not itself establish a clean exact
source/tree, nominate or freeze an immutable candidate, record human UAT, authorize publication or
satisfy the then-pending M87 visual-UAT boundary.

- Code-owned point release is history-free below the outer code owner. The delegated editor consumes
  the exact accepted preview into one linear `DelegatedPointDragProposal` without an inner Intent
  transaction or history publication. Terminal work is zero native-preview, Intent-materialization,
  computed-evaluation and history-publication attempts; authenticated outer publication records
  managed-parse/expansion/accepted-publication work `0/1/1`.
- Incremental overlay materialization receives the terminal preview's exact accepted numerical
  continuation. Requested, terminal and staged seed positions must agree by IEEE bits. After only
  the already documented bounded presentation/publication normalization for authenticated redundant
  rectangle aliases and periodic computed-angle representation, design/accepted point coordinates
  and scalar values remain bit-exact. Publication request, solver configuration, activation and
  parameter revisions/digests, external snapshots and their accepted durable payloads must also
  match exactly.
- Terminal parity requires two independently validated Current native authorities. Native
  nodes, ports, reservations, writable leaves, aggregates, persistent identities and allocators
  remain exact; durable topology, feature contact/branch ownership and provenance fail closed.
  Endpoint topology derives only from exact shared-point identity and active Coincident constraints.
  A stale, foreign, ambiguous or mismatched route cannot publish.
- Successful outer publication installs the independently accepted editor before persistence.
  Rejection restores the prior accepted editor and aborts persistence if restoration cannot be
  authenticated; generic or foreign saves preserve a pending pointer route unchanged. The ordinary
  GUI-owned point regression retains its native terminal transaction and one outer checkpoint
  revision with zero semantic rematerialization, unchanged managed source/overlay and exact reload.
- The routing-board browser lifecycle authenticates the exact `serviceRoute.serviceLoop` generated
  address, accepts two history-free native terminals, retains unchanged `sketch.ts`, publishes one
  `GeneratedOverride`, keeps all 64 Fillets Current, and survives Undo twice, Redo twice and exact
  reload. Focused evidence passes accepted-continuation 2/2, exact Typed Panel/Compass terminals
  2/2, the authenticated 23-terminal periodic seam 1/1, delegated/reference boundaries 5/5,
  endpoint topology 6/6, routing-board owner/control/structural cases 3/3, 64-Fillet batched
  composition 1/1, deterministic board headless rendering 1/1, the ten-project ledger 1/1,
  ordinary GUI ownership 1/1 and the complete browser lifecycle 1/1 in 367.68 seconds.

The complete routing-board-amendment dirty-worktree gate was run as:

```bash
TMPDIR=/home/arduano/.cache/geosolve-m87-tmp \
GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true \
nix-shell shell.nix --run \
'TMPDIR=/home/arduano/.cache/geosolve-m87-tmp ./scripts/release-gate.sh'
```

It exited `0`. Metadata, formatting and diff hygiene, warnings-denied workspace Clippy, complete
all-feature workspace tests, 347/347 browser tests, 9/9 headless tests, 3/3 routing-board tests,
managed controls 19/19, endpoint topology 6/6, artifact/source parity, the unchanged
milestone-neutral golden, locked native/WASM parity and actual WASM exports 3/3, TypeScript
40/40 and 14/14 plus fixtures/types/managed sources, warnings-denied Rustdoc, benchmark and release
performance runs, `cargo-deny`, package-content/package verification and final release Trunk
assembly all passed. Because this gate explicitly allowed a dirty worktree, it supplies focused
mechanical qualification only.

## Remaining qualification and UAT sequence

- [x] Complete the generic Code-panel and Inspector manifest adapters.
- [x] Connect the grouped generated-Fillet grip to the outer source transaction.
- [x] Pass the focused crossed Typed Panel, repeated Water Manifold invocation, all-demo,
  DoF/overlay and plain-workspace regressions from `docs/SCENARIOS.md`.
- [x] Run the complete provisional dirty-worktree release gate, including format, warnings-denied
  workspace Clippy, all-feature workspace tests, Rustdoc, locked WASM, both TypeScript packages,
  exact golden `--require-clean`, licence/package checks, performance checks and release Trunk
  assembly.
- [x] Complete and record the additive routing-board focused owner, topology, batched-composition,
  overlay/history/reload, headless, TypeScript fixture and separate-ledger qualification. The prior
  complete provisional gate predates this amendment.
- [x] Delete the complete adaptive-detail/LOD prototype while preserving authority-only retained
  camera recovery, then pass the focused no-LOD resize/failure-recovery browser check.
- [x] Audit the shared graphics boundary, reproduce and repair M87-F002 at its renderer owner, and
  retain the frozen compositor bytes without a speculative z-order rewrite.
- [x] Preserve the supervising user's earlier explicit 2026-08-30 scoped disposition as historical
  acceptance of U1-U8 without claiming those rows were separately replayed. It does not accept the
  later manufacturing visual rows or determine M87's final disposition.
- [x] Record that clean exact-source nomination, immutable no-rebuild freeze, public publication
  and service retirement were not performed. The mutable Tailscale development service remains
  collaboration infrastructure, not release authority.
- [x] Complete I8's CNC locality/generation and Gridfinity profile/Fillet owner regressions.
- [x] Reproduce and repair M87-F003 by replacing CNC's seven unrelated fixed-point locks and
  Gridfinity's unconstrained literal-seed contour with minimal relational datum authority.
- [x] Extend and review the twelve-project ledger, all catalog/headless/browser inventories,
  package parity and deterministic render evidence without changing the 271-row golden.
- [x] Run and record the additive focused manufacturing qualification. That proportional result did
  not by itself claim a complete current twelve-project dirty-worktree gate.
- [x] Run and record the then-current pre-F003 twelve-project dirty-worktree release gate at exit
  `0`. This is historical mechanical qualification only, not F003 qualification, clean nomination
  or human visual acceptance.
- [x] Complete proportional post-F003 mechanical qualification: focused manufacturing 3/3, native
  composition 13/13, all-demo headless/deterministic products 10/10, exact reviewed-ledger 1/1,
  package checks, formatting, diff hygiene and warnings-denied focused Clippy. Generate fresh CNC
  and Gridfinity visual bundles without claiming human acceptance or a complete release gate.
- [x] Preserve the pre-close record that dirty evidence did not establish clean source, freeze or
  publication; qualify exact committed source separately without weakening those distinctions.
- [x] Record M87-U9/U10 as accepted by the supervising user's explicit milestone-level close
  decision without inventing a separately replayed visual session.
- [x] Run the complete clean gate on source `32c7289` / tree `38f7175`, close M87, leave immutable
  freeze/publication/service retirement unclaimed and activate M88.

### Clean closeout qualification — 2026-08-31

Exact committed source `32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`, passed:

```bash
NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The command exited `0` from an empty pre-gate worktree and left an empty post-gate worktree.
Formatting, diff hygiene, warnings-denied workspace Clippy, all-feature workspace tests, the clean
271-row golden, native/WASM parity, both TypeScript packages, Rustdoc, benchmark compilation,
release performance, licences, all package lists, packaged `geosolve-sketch-code` verification and
release Trunk assembly pass. Current focused results include manufacturing 3/3, native composition
13/13, exact reviewed ledger 1/1, headless 10/10 and browser library 344/344. The 256-moving-body
release crossover passes in 158.21 seconds. Existing manifest/dead-code/package metadata warnings
remain nonfatal. This qualifies source only; no immutable no-rebuild freeze or publication is
claimed.

A complete dirty-worktree release-gate rerun was initially attempted after the graphics repair.
While it was running, a separate concurrent workspace session added `CncJoineryFitCoupon` and
`GridfinityBinSection` before updating the exhaustive native inventory and reviewed code-project
ledger. The gate therefore stopped at the first non-exhaustive integration-test compile error
(exit `101`); after that external test was completed, the reviewed ledger still deliberately
rejected the two unreviewed rows. This failed attempt remains historical evidence and is not an I8
qualification. The subsequent additive focused sequence recorded above passed the manufacturing,
twelve-demo native/headless, deterministic-render and reviewed-ledger checks.

After the original exhaustive inventory and reviewed ledger were complete, the then-current pre-
F003 twelve-project dirty-worktree release gate was rerun on 2026-08-30 with the exact command:

```bash
TMPDIR=/home/arduano/.cache/geosolve-m87-tmp GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run 'TMPDIR=/home/arduano/.cache/geosolve-m87-tmp ./scripts/release-gate.sh'
```

It exited `0` for those pre-F003 sources. The post-F003 reviewed twelve-row code-project
ledger has SHA-256
`f6ecd037cef8befc59f9a057fef499a14f0851f8ec5a0d3a1468a69e66a9d1bc`; the historical gate does not
qualify it. This is explicitly dirty mechanical evidence: it does not nominate a clean source/tree,
freeze an immutable artifact, record human visual acceptance, publish a release or retire the
mutable development service.

The following mutable review bundles predate F003 and cannot review the revised designs:

- M87-U9: `/tmp/geosolve-m87-manufacturing-uat.K7YRaV/cnc`;
- M87-U10: `/tmp/geosolve-m87-manufacturing-uat.K7YRaV/gridfinity`.

Neither historical bundle is current, immutable or accepted. Fresh post-F003 mutable replacements
are at `/tmp/geosolve-m87-post-f003.FPHP3b/{cnc,gridfinity}`. The user's 2026-08-31 milestone-level
close decision accepts U9/U10 without a separate replay; exact source `32c7289` passes the complete
clean gate. M87 is closed; M88 followed and is now complete.
