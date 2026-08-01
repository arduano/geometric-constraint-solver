# GeoSolve core roadmap

Implement milestones in order. Do not begin a milestone until the preceding milestone's tests and acceptance gate pass.

This plan supersedes the original M0-M9 bootstrap plan. Detailed completion history remains in Git and `OVERNIGHT_REPORT.md`; it is not active guidance.

## Product deliverables

### Deliverable 1: production-capable 2D CAD sketches

The library must support independently editable 2D sketch geometry, including:

- points, lines, segments, circles and circular arcs;
- ellipses, elliptical arcs and parametric conics;
- editable polynomial Bezier curves;
- B-splines and NURBS;
- generic point/curve and curve/curve contact;
- line/curve and curve/curve tangency;
- explicit tangent orientation, contact neighborhood, span, winding and branch state;
- curvature, osculating radius, G2 continuity and separately named parametric C2 continuity;
- driving and reference dimensions;
- truthful rank, mobility, redundancy, conflict and invalid-geometry diagnostics;
- versioned persistence of topology, geometry, constraints and discrete state;
- the standard planar CAD relation, dimension and measurement catalog;
- separate retained design intent, attempted candidates and independently accepted
  solved state;
- construction geometry, source activation, typed host parameters and immutable
  external 2D references;
- cancellation, stale-work rejection, stable host diagnostics and a documented
  production-scale envelope;
- companion sketch-operation and production wire/profile APIs suitable for a CAD
  host without importing B-rep or UI concerns into solver state.

M22 completed the built-in curve and generic differential-constraint surface, not
the complete production embedding contract. M33-M45 close the first ordinary-CAD,
host-integration and interaction-consumer cut; M46-M53 perform the cleanup rebase and approved
host-semantics UAT; M54-M62 complete the subsequent functional work, approved advanced UAT and
approved CAD-style authoring, and M63 completes approved geometry-anchored canvas constraint and
dimension presentation. M64 completes the approved editable-sample cleanup and focused UAT cut;
M65 completes approved predictable, bounded projected dragging for those ordinary editable
samples; M66 is the active CAD helper-authoring cut for associative 2D fillets, line offsets and
exact supported-family mirroring.
This deliverable does not include a solid B-rep kernel, meshing or 3D sketch curves.

### Deliverable 2: 2D and 3D rigid-body kinematics

The library must support planar and spatial CAD assemblies and linkage models, including:

- rigid-body configuration and mobility/DOF analysis;
- point, axis, plane and frame features;
- planar and spatial mates/joints;
- explicit assembly modes and branch-preserving driven motion;
- multiple drivers and robust continuation;
- velocity-level kinematic queries;
- floating assemblies with numerical gauge removal distinct from physical grounding;
- versioned persistence of bodies, features, mates, drivers and assembly state.

Kinematics explicitly excludes mass, inertia, forces, reactions, forward dynamics, time integration, collision detection, unilateral contact, friction and impact.

### User-approved next cut: 2D Sketch Playground Alpha

M10-M14 deliver an embeddable 2D sketch library slice and a disposable browser diagnostic playground over it. The alpha is an integration and usability cut, not completion of Deliverable 1.

The alpha geometry scope is exactly:

- point;
- line and polyline;
- rectangle as a command macro over ordinary geometry and constraints;
- circle and circular arc;
- editable quadratic and cubic Bezier curves.

The alpha constraint and dimension scope is exactly:

- fixed, coincident, horizontal, vertical, point-on-curve, parallel and perpendicular;
- equal length, equal radius, midpoint and symmetry;
- distance, length, radius, diameter and oriented-angle dimensions;
- generic line-curve and curve-curve contact and tangency;
- driving and reference dimensions;
- explicit branch, tangent-orientation, contact-neighborhood, parameter-domain, span and winding state where applicable.

The playground interaction scope is select, box-select, multi-select compatible constraints, draw, solver-projected drag, dimension edit, delete/suppress, pan/zoom, undo/redo, JSON import/export, local autosave, diagnostics/conflict/DOF display and retained geometry on failed edits. Prospective coincident/horizontal/vertical inference is only a proposal and requires user confirmation before it changes the document. Desktop and mobile must both be functional.

Reusable Rust APIs own `SketchDocument`, `SketchSession`, commands, history, versioned serialization, curve evaluation and all constraints/equations. Selection, hit testing, tool state, rendering and browser `localStorage` remain web-only. The web crate is non-authoritative, contains no solver or geometry equations and may be replaced without changing document semantics.

The preceding paragraph records the completed M13-M14 alpha boundary. ADR 0029
supersedes its post-alpha interaction ownership: deterministic editor policy moves to
`geosolve-constraint-editor`; only rendering, accessibility, platform events and
browser storage remain presentation-specific.

Post-alpha browser policy: the playground is a robust sanity-checking instrument for
the supervising user to inspect claims and expose defects, not a production
application. M39 and M44 establish its desktop-only CAD-like replacement, and
M46-M50 remove the old playground and its E2E infrastructure after direct-test
replacement. The workbench remains a non-authoritative public-API consumer. Future mobile or
responsive behavior is explicitly outside acceptance and must not consume roadmap
time.

### Production embedding north star

The post-M32 sketch program turns the mathematically broad preview into a planar
engine that a real CAD host can use. The accepted personality is:

- structurally valid but unsolved design intent is retained separately from the
  latest independently validated accepted geometry;
- external CAD geometry is consumed only as immutable revisioned 2D snapshots;
- the host owns formulas, unit conversion, parameter dependency graphs and
  configurations, while GeoSolve consumes typed finite values;
- ordinary user constraints remain hard, compatible redundancy is diagnosed, and
  branch changes remain explicit;
- sketch operations and production topology are companion APIs rather than solver
  equations or B-rep features;
- all semantic interaction intelligence is reusable headless state: tool progression,
  hover/snap memory, inference candidates, guide activation, tolerances, preview
  geometry and commit/cancel policy belong in `geosolve-constraint-editor`, never in a
  particular DOM, canvas or 3D host UI;
- an embedding UI may map a platform pointer or 3D camera ray onto a sketch plane and
  render returned DTOs, but it must not recreate the editor's interaction state machine
  or infer geometric assistance independently;
- currently qualified embedding targets are Rust and WASM;
- the reusable library remains `GPL-3.0-or-later`;
- the web consumer is a desktop demo of sketch-constraint workflows, not a mobile
  product or solid modeller.

## Architectural boundaries

- Keep `geosolve-sketch` and `geosolve-linkage` as separate domain models over `geosolve-core`.
- Keep CAD entities, rigid bodies, joints and branch types out of `geosolve-core`.
- Keep curve definitions closed and serializable while curve evaluation and residual construction become internally generic.
- Do not expose a public generic curve or manifold trait before the built-in families prove the seam.
- Keep branch, span, winding, active-bound and assembly-mode choices as explicit domain state outside differentiable formulas.
- Use local forward automatic differentiation where it reduces fragile analytic code; retain central finite differences as an independent oracle for every residual.
- Preserve pure Rust, GPL-3.0-or-later licensing and the workspace `unsafe_code = "forbid"` policy.
- Keep `geosolve-demo-web` as a separate public-API consumer. M13-M14 may shape embeddable sketch workflows, but web-only interaction and rendering concerns must not enter reusable Rust document/session APIs.
- Keep stateful drafting assistance in `geosolve-constraint-editor`. Remembered hover
  identities, prospective horizontal/vertical/coincident relationships, activation
  boundaries and preview consequences must be deterministic headless inputs/state/
  outputs with native regressions. A UI may display and explicitly confirm them only.
- Do not call host code during residual evaluation. Host parameters and external
  geometry enter one attempt as immutable revisioned values.
- Keep cross-system expressions, B-rep projection, topological naming, feature
  history and application undo outside the sketch solver contract.
- Keep human acceptance records explicit: M40.7, M53 and M61-M64 are complete and approved, and
  every newly scoped milestone from M65 onward ends in its own supervising-human UAT. Every objective correctness, persistence,
  compatibility and presentation-adapter assertion must pass through direct unit or
  integration tests at its owning layer before a human checkpoint begins; old CDP E2E
  suites are not a qualification path.

## Frozen baseline: M0-M7

Status: complete through commit `eb8dbbf` on 2026-07-14.

The following behavior is the permanent regression baseline:

- stable variable, residual and source IDs;
- scalar, `Vec2` and `Pose2` variable blocks;
- normalized residual/Jacobian assembly and finite-difference verification;
- dense LM/Gauss-Newton solving with QR/SVD fallback;
- independent hard-residual validation before success;
- deterministic traces and accepted-state audits;
- incidence decomposition, fixed/alias elimination and component reuse;
- numerical rank, DOF, redundancy and conflict diagnostics;
- strict hard/temporary/preference priority semantics;
- stable-ID 2D sketch model with points, segments, circles and oriented arcs;
- the complete existing sketch constraint, dimension, contact and tangency corpus;
- planar rigid bodies, revolute/prismatic/weld joints, drivers and explicit assembly branches;
- bounded planar continuation and independently validated velocity queries;
- transactional rollback on numerical, geometry, domain or branch failure.

All M1-M7 tests remain mandatory. Refactors may intentionally change internal IDs or implementation details, but not accepted geometry, validation semantics, source ordering, branch behavior or documented diagnostics without an explicit ADR and acceptance update.

## Common milestone gate

Every implementation milestone must:

1. add exact, perturbed-recovery and invalid-state fixtures;
2. add analytic/local-AD versus central finite-difference Jacobian tests with relative error `<= 1e-6`;
3. add translation, rotation and scale metamorphic coverage where applicable;
4. independently validate every success-like result to normalized hard residual `<= 1e-9`;
5. preserve the previous accepted finite state on rejection;
6. keep every discrete branch/domain choice explicit;
7. run formatting, warnings-denied Clippy, workspace tests and relevant locked WASM build checks;
8. keep the desktop web consumer compiling when a supported sketch API changes;
9. update this file with checked items and concise completion notes.

Performance measurements never weaken correctness thresholds.

---

# Shared numerical foundation

## M8: contract rebaseline and representative baselines

Status: complete as of 2026-07-14.

Goal: freeze the semantics and measurements required by both product deliverables before changing numerical infrastructure.

- [x] Update `ARCHITECTURE.md`, `ACCEPTANCE.md` and active ADRs to match this roadmap.
- [x] Specify hard-valid versus secondary-optimum status independently.
- [x] Specify rank thresholds, left/right nullity, near-singular warnings and active-bound mobility.
- [x] Specify diagnostic budget/completeness reporting.
- [x] Add CAD-like and linkage-like benchmark families at approximately 100, 1,000 and 10,000 variables.
- [x] Measure definition construction+compile, linearization assembly, decomposition+solve+diagnostics, and one-component edit/re-solve as four separate Criterion groups.
- [x] Add ADRs for local AD, manifold poses, persistent solve sessions, bounds/active sets, sketch design topology and physical grounding versus numerical gauge.
- [x] Mark stale historical documents as historical and keep active documentation mutually consistent.

Gate: all M1-M7 behavior remains unchanged, benchmark inputs are deterministic, and every new architectural decision has an accepted ADR.

Completion notes: accepted ADRs 0005-0009 assign component-local linearization and
local AD to M9, sessions/bounds to M10, manifold poses to M11, sparse structure to
M12, persistent sketch topology to M13 and physical-ground/gauge separation to
M14. Deterministic CAD-like workloads contain 100/1,000/10,000 tangent variables;
linkage-like workloads contain 99/999/9,999. Criterion exercises 24 exact
family/scale/measurement cases with teardown outside timed windows and validates
every solve report. The gate passes with 79 core tests and 201 workspace tests,
warnings-denied Clippy, locked WASM check, benchmark compilation and all 24
Criterion test-mode cases.

## M9: canonical component-local linearization and local AD

Status: complete as of 2026-07-14.

Goal: create one allocation-conscious derivative path usable by dense, sparse, CAD-curve and spatial-pose residuals.

- [x] Add fused residual/Jacobian linearization into caller-provided storage.
- [x] Build a canonical component-local block linearization IR.
- [x] Remove global-column dense allocation from component solves.
- [x] Add object-safe local forward-AD residual adapters.
- [x] Keep analytic residuals where they are clearer and cheaper.
- [x] Retain central finite differences as the mandatory independent derivative oracle.
- [x] Add structured evaluation errors for degenerate, out-of-domain, nondifferentiable and ambiguous states.
- [x] Report hard validity independently from hard termination and each secondary optimization status.
- [x] Apply the normalized component-local rank threshold with the specified machine floor.
- [x] Report numerical left/right nullity and a distinct near-singular warning band.

Gate: existing accepted geometry and source ordering remain unchanged; representative analytic and AD linearizations agree with finite differences and with each other; the M9 status and numerical-rank contracts pass their acceptance fixtures.

Completion notes: one canonical normalized block IR now backs public dense
assembly and direct component-width hard, priority, rank, conflict and returned-row
linearization. Fixed incidence is evaluated without materialized columns; alias
blocks retain incidence order and accumulate deterministically. Legacy residuals
remain supported while fused evaluators write caller-owned storage. The private
`num-dual` adapter seeds normalized local tangent coordinates and avoids raw
derivative overflow at extreme valid scales; analytic, AD and central-difference
oracles agree through the required scale range. Reports now separate hard
termination, domain-authoritative hard validity and each secondary outcome, and
report machine-floor component rank, left/right nullity and near-singular state.
Structured evaluation categories propagate through attempted audits, while public
audits require Jacobian success before marking rows evaluated. The gate passes
with 98 core tests and 228 workspace tests, warnings-denied Clippy and rustdoc,
benchmark compilation, all 24 Criterion test-mode cases, locked WASM check/test
compilation and a release Trunk build.

Historical allocation note: the milestone labels in the preceding completion record
describe the M8-approved roadmap at that time. The user-approved M10+ rebaseline
below supersedes only those future allocations; it does not change any M8 or M9
completion claim or accepted ADR decision.

## M10: persistent solve sessions and first-class bounds

Status: complete as of 2026-07-15.

Goal: retain compiled structure across edits, represent bounded coordinates mathematically rather than only through post-solve rejection, and prove the lifecycle through a reusable sketch consumer.

- [x] Separate immutable problem topology from mutable accepted state and source parameters.
- [x] Add a persistent `SolveSession` with automatic revision and dirty-component tracking.
- [x] Preserve domain-to-core mappings across non-structural edits.
- [x] Cache component layouts, accepted states and structural patterns.
- [x] Add scalar/tangent-coordinate box bounds and an active-set or projected LM policy.
- [x] Include active bounds in rank, mobility and audit output.
- [x] Add `SketchSession` as the first domain consumer without moving sketch types into `geosolve-core`.
- [x] Support endpoint-active curve contacts and positive radii through the sketch consumer.
- [x] Make diagnostic budgets configurable and report completeness, consumed work and incomplete reasons.
- [x] Make accepted-state commits atomic through a validated patch or clone-and-swap.

Gate: sketch source/state edits automatically solve only affected components, omitted dirty IDs cannot corrupt state, endpoint-active mobility is truthful, and all prior rollback behavior remains transactional. M10 does not yet provide the persistent document or browser playground.

Completion notes: `SolveSession` owns revision-checked source/state/bound
transactions, retained elimination/layout caches and fresh returned-state hard
validation. Deterministic projected/active-set LM uses independent active normals,
KKT release logic and conservative weak critical-cone status; bounds feed rank,
one-sided mobility and structured audit identity. Configurable diagnostics retain
complete/truncated/skipped evidence across reuse. `SketchSession` compiles once,
rebuilds only changed source payloads for non-structural edits, synchronizes
canonical contact state through validated core patches and atomically retains
geometry, branch state, mappings and audit. The gate passes with 27 core M10 and
17 sketch M10 regressions, the complete workspace suite, warnings-denied Clippy,
rustdoc, benchmark compilation, locked WASM check, release Trunk build, format and
diff checks.

## M11: persistent SketchDocument, commands and history

Status: complete as of 2026-07-15.

Goal: pull the generic sketch graph forward and define one reusable, versioned edit model for programmatic consumers and the later playground.

- [x] Add persistent external IDs separate from runtime generational keys.
- [x] Add `SketchDocument` with design points, typed design scalars and one closed, versioned `CurveDefinition` for line/polyline/circle/arc topology.
- [x] Add semantic feature references, stable contact slots and explicit branch/orientation/neighborhood/domain/span/winding state.
- [x] Add generic dependency, compilation, validation, commit and structured-audit mappings for the alpha constraint/dimension corpus applicable to baseline geometry.
- [x] Add a rectangle command macro that emits ordinary document entities and constraints rather than a privileged solver primitive.
- [x] Add typed commands for create, edit, delete, suppress/unsuppress and driving/reference dimension changes.
- [x] Add deterministic undo/redo history over accepted commands; failed commands retain the prior accepted document and do not enter history.
- [x] Add a versioned JSON envelope, deterministic runtime-ID remapping, strict import validation and canonical export.
- [x] Keep history policy reusable but separate from browser selection, tools and storage.

Gate: S1-S3 and the full M5/M7 corpus remain semantically unchanged; commands and undo/redo preserve accepted geometry; JSON round trips preserve persistent IDs and every explicit branch field; malformed or failed edits leave the accepted document unchanged.

Completion notes: `geosolve-sketch` now exposes typed opaque 128-bit persistent
IDs, `SketchDocument`, closed baseline curve/constraint/dimension/contact state,
semantic features, canonical version-one JSON, deterministic runtime remapping and
accepted-state projection. `SketchDocumentSession` provides revision-checked typed
commands, atomic coupled contact/branch edits, accepted-only history, undo/redo and
atomic import; rejected solves retain accepted geometry/mappings while keeping a
separate attempted mapping for diagnostics. Rectangle expansion emits four ordinary
shared-corner lines, fixed/axis sources and width/height dimensions; all equations
remain the existing `Sketch`/`SketchSession` equations. Ten M11 regressions cover
S1-S3 migration, every M5/M7 source/contact role, conflict source remapping,
ID non-reuse, branch history, malformed import retention and canonical round trips.
The complete locked workspace suite, warnings-denied Clippy/rustdoc, WASM check,
benchmark compilation, release Trunk build, format and diff checks passed at the
M11 gate. M12 subsequently supplied Bezier evaluation and geometry-generic
contact/tangency plumbing.

## M12: editable Bezier curves and generic curve constraints

Status: complete as of 2026-07-15.

Goal: complete the reusable Rust API surface needed by the 2D Sketch Playground Alpha and prove that editable curve derivatives and contact/tangency equations are geometry-generic.

- [x] Move immutable curve evaluation into `geosolve-geometry` with typed parameter-domain and regularity outcomes.
- [x] Add position and first-through-third parameter derivatives for alpha curve definitions.
- [x] Add editable quadratic and cubic Bezier entities whose controls are design variables.
- [x] Add generic point-on-curve, line-curve contact/tangency and curve-curve contact/tangency plumbing.
- [x] Use the same generic residual templates for line, polyline segment, circle, arc and Bezier combinations.
- [x] Differentiate every incident control coordinate and latent contact parameter through local AD while keeping branch/span/winding/orientation state outside AD.
- [x] Complete public embeddable sketch APIs for the alpha geometry, constraint, driving/reference dimension, document, session, command, history and JSON scope.
- [x] Reject cusp, zero-speed, escaped-domain, ambiguous-neighborhood and non-finite curve states before success or commit.

Gate: line/circle/arc/Bezier combinations share generic residual plumbing; every control/contact derivative passes finite differences; all alpha library acceptance scenarios pass at model scales `1e-6`, `1` and `1e6`; no web code or browser state is required to construct, edit, solve, audit or serialize a document.

Completion notes: `geosolve-geometry` now exposes finite line, circle, arc,
quadratic-Bezier and cubic-Bezier jets through third derivative with typed domain
and regularity errors. `geosolve-sketch` adds editable Beziers, common local-AD
point/contact/tangency residuals, explicit local contact neighborhoods, persistent
generic curve-pair constraints, arbitrary public curve evaluation and separate
attempted/accepted document solve views. Ten M12 regressions cover A5 edits and
rollback, all six alpha curve families and all 15 unordered pairs, central
finite differences, perturbed recovery and invariant rank/bounds at scales
`1e-6`, `1` and `1e6`, JSON/audit mappings, retained bound replacement,
drag/history/import composition and rejected-attempt diagnostics. Cusp,
zero-speed, escaped or malformed domains, ambiguous root changes and non-finite
states reject before commit. The complete locked workspace, warnings-denied
Clippy/rustdoc, WASM, benchmark, release Trunk, format and diff gates pass.

---

# 2D Sketch Playground Alpha

## M13: disposable browser playground

Status: complete as of 2026-07-16.

Goal: expose the M10-M12 embeddable sketch APIs through a useful but replaceable browser interaction layer.

- [x] Make `geosolve-demo-web` consume only public sketch document/session/command/history/serialization/audit APIs.
- [x] Add select, box-select and multi-select application of compatible constraints.
- [x] Add draw tools for every alpha geometry type, including rectangle as the library command macro.
- [x] Add solver-projected drag, dimension editing, delete and suppress/unsuppress.
- [x] Add pan/zoom and functional pointer/touch interaction for desktop and mobile.
- [x] Add undo/redo, JSON import/export and browser-local autosave.
- [x] Add prospective coincident/horizontal/vertical inference as a visible proposal requiring explicit confirmation.
- [x] Display solve status, retained accepted geometry, rank/DOF, conflicts and structured diagnostics from one accepted result.
- [x] Keep selection, hit testing, tool state, rendering and `localStorage` in the web crate; add no equations or authoritative geometry semantics there.

Gate: a user can construct and edit every alpha entity/constraint workflow on desktop and mobile; failed edits retain visible accepted geometry; page reload restores the last valid local document; replacing the web crate would not require moving any equation or document rule.

Completion notes: `geosolve-demo-web` now installs a document-backed playground
whose authoritative state is `SketchDocumentSession`. It provides all alpha draw
tools, span-aware selection/box/multi-selection, every alpha constraint and
dimension action, independent paired contact branch controls, projected one-step
drag, source suppression, owned-state deletion, pan/zoom, history, canonical JSON,
accepted-only autosave, explicit-confirmation inference, and accepted/attempted
diagnostic separation. Compound library transactions keep circles, arcs, Beziers,
dimensions and contact constraints atomic; public contact construction, semantic
contact ordering, reference measurements and owned-state deletion keep document
rules out of the browser. Fifteen playground regressions and four sketch M13
regressions cover the complete tool/constraint/dimension matrices, branch edits,
selection, drag/history, conflicts, cleanup, escaping, persistence and autosave.
A fresh-profile Chromium release smoke passes desktop and mobile point/line input,
accepted status/audit rendering, sub-two-pixel coordinate checks and byte-identical
autosave reload. The complete locked workspace, warnings-denied Clippy/rustdoc,
WASM, benchmark, release Trunk, format and diff gates pass.

## M14: playground hardening and alpha gate

Status: complete as of 2026-07-16.

Goal: harden browser behavior, import failures and representative interaction performance enough to call the playground alpha complete.

- [x] Add browser E2E coverage for the exact alpha scenarios in `docs/SCENARIOS.md` on desktop and mobile viewports.
- [x] Make malformed, unknown-version, duplicate-ID, dangling-reference, non-finite and over-limit JSON imports fail atomically with actionable errors.
- [x] Preserve the current accepted document and visible geometry after invalid command, solve, import or autosave recovery failure.
- [x] Test undo/redo and autosave recovery across create/edit/delete/suppress and branch-bearing curve operations.
- [x] Record separate import, first-solve, incremental edit/solve and render timings for deterministic small and medium alpha documents.
- [x] Set and enforce documented reference-environment interaction budgets without weakening residual, rank or validation policy.
- [x] Verify keyboard, pointer and touch paths, viewport resizing and no-loss JSON download/upload behavior.
- [x] Keep browser diagnostics tied to accepted-state public audit data and prevent stale candidate geometry from appearing authoritative.

Gate: constrained rectangle, underconstrained drag, line-circle tangency, free-radius circle-arc tangency, Bezier tangent line, conflicting dimensions, undo/redo, ID/branch JSON round trip, invalid-edit retention and `1e-6`/`1`/`1e6` scale scenarios pass through public APIs and browser E2E coverage. M14 completes the 2D Sketch Playground Alpha only; it does not complete Deliverable 1.

Completion notes: reusable Rust fixtures and regressions now execute A1-A10 at
scales `1e-6`, `1` and `1e6`, including the complete alpha primitive, constraint
and dimension corpus, finite-difference Jacobians, invariant rank/mobility,
explicit branches, exact conflict sources and transactional failure retention.
Strict imports and browser file/storage paths reject malformed, oversized or stale
input without replacing accepted state; primary/backup autosave recovery retries
failed writes and preserves corrupt input for diagnosis. Dependency-free Chromium
DevTools Protocol E2E passes desktop mouse and mobile touch workflows, keyboard
history, viewport changes, canonical download/upload and A1-A10 recovery paths.
Documented release timing gates enforce separate small/medium import, first-solve,
incremental edit/solve and browser-render budgets without changing correctness
policy. The complete locked workspace, warnings-denied Clippy/rustdoc, WASM,
benchmark, native performance, release Trunk, desktop/mobile Chromium, format and
diff gates pass. This closes the Playground Alpha; Deliverable 1 remains open.
Post-alpha field regressions additionally protect free-line half-plane crossing,
scale-invariant A5 line-end projection and stable unconstrained Bezier controls,
dependency-complete rectangle deletion, free-size drawn rectangles and direct
transactional constraint/dimension deletion from the browser object panel. The
post-alpha playground also stages all supported primitives on pointer release with
recoverable shape-specific previews and includes compass/Bezier-bridge constraint
stress labs plus Bezier-cam roller and full-orbit tangent motion examples built entirely
from public document APIs with explicit tangent orientation and periodic contact state.
The example selector also exposes scale-invariant structural-versus-numerical rank,
fixed-endpoint-versus-active-radius mobility and deterministic redundancy diagnostic labs,
with structural class/backend evidence rendered from public solve reports.
Further public-document motion examples combine rails, midpoint constructions, fixed
coordinates, perpendicular/parallel relations, equal lengths and symmetry into an
elliptic trammel, offset Scotch yoke, rotating square and scissor jack.
Linkage-heavy follow-ups add a synchronized five-stage scissor tower and a
seven-bar Peaucellier-Lipkin cell whose unconstrained output traces an exact line.
The post-alpha WASM drag path coalesces pointer moves to animation frames, renders
straight spans from exact endpoints, reuses one accepted report per frame and defers
heavy object/audit DOM refresh until release. Redundancy diagnostics reuse exact
prefix ranks within each report without changing thresholds or evidence.
The diagnostic-layout follow-up makes the geometry canvas the dominant desktop
surface, keeps editing controls in a sticky inspector, places accepted state beside
the canvas controls and moves detailed solve/audit output into a below-canvas dock.

---

# Deferred production foundations

## M15: manifold geometry and spatial state

Status: complete as of 2026-07-16.

Goal: add the mathematically correct state representation needed by 3D rigid-body kinematics.

- [x] Add validated `SE(2)` composition, inverse, exponential, logarithm, adjoint, retraction and local difference.
- [x] Add `Vec3` and quaternion-backed `Pose3`/`SE(3)` with ambient dimension 7 and tangent dimension 6.
- [x] Define one documented body/world transform and tangent convention.
- [x] Canonicalize quaternion sign without treating it as an assembly branch.
- [x] Make fixed and alias elimination manifold-aware.
- [x] Add validated frame and workplane construction plus point/vector transforms.
- [x] Expose an accepted-state reduced hard linearization and sensitivity solve API.

Gate: manifold property tests, tangent-coordinate finite differences, global-transform equivariance and quaternion-sign invariance pass without core regressions.

Completion notes: `geosolve-geometry` now provides validated `Vec3`, `Pose2`/`Pose3`,
`Frame3` and `PlaneFrame` operations under ADR 0006. Core variable packing,
right-retraction local AD, fixed/alias elimination and planar linkage Jacobians use
the same body-local tangent convention. `SolveSession::accepted_hard_linearization`
returns deterministic revision-stamped reduced hard components, and sensitivity
solves distinguish unique, underdetermined minimum-norm and inconsistent rates
before independently validating both normalized and published raw tangents.

Property, exact/near-half-turn, finite-difference, global-transform, quaternion-sign,
planar branch and velocity-equivariance regressions pass. The locked workspace,
warnings-denied Clippy/rustdoc, WASM check, benchmark compilation, all 24 Criterion
test-mode cases, native M14 performance budgets, release Trunk build, desktop/mobile
Chromium suite, format and diff gates pass. Sensitivity is intentionally limited to
accepted reduced hard equalities in body-local coordinates; bound cones, secondary
objectives and world-frame conversions remain future work.

## M16: sparse structure, hierarchy and continuation

Status: complete as of 2026-07-17.

Goal: scale the shared kernel before production splines and large spatial assemblies expand the graph.

- [x] Add indexed block COO/triplet assembly from the canonical linearization.
- [x] Convert to `faer` CSC storage and validated sparse damped least-squares while retaining dense SVD as the rank-revealing authority under ADR 0012.
- [x] Add structural matching and under/well/over-constrained partitions.
- [x] Retain dense QR/SVD fallback for small or diagnostically ambiguous components.
- [x] Cache symbolic ordering/factorization structure.
- [x] Record and enforce a benchmark-derived dense/sparse crossover policy.
- [x] Replace large explicit dense nullspaces with sparse-compatible hierarchy operations.
- [x] Support secondary objectives spanning multiple hard components.
- [x] Add adaptive predictor-corrector and pseudo-arclength continuation.

2026-07-16 hierarchy slice: cross-hard-component Temporary/Preference groups,
block-local nullspace application, validated projected CGLS, protected-level
reporting, bounds, and session hierarchy invalidation are implemented and
covered by M5/M10/M16 regressions.

Independent review follow-up: returned-state secondary costs/audit are always
fresh, curvature certification is multi-scale and conservative, hierarchy-only
dependency stamps advance, sparse symbolic cache capacity/eviction are fixed,
and parity/crossover/benchmark evidence is tightened. Continuation is implemented
as a planar single-driver slice. ADR 0012 records the caller-approved backend
scope: current faer sparse QR supplies validated damped LM steps, while dense SVD
remains the rank-revealing authority for the M9 report contract.

2026-07-17 bounded hierarchy closure: groups at or above 128 reduced coordinates
now use a deterministic projected-CGLS active set over block-local bound normals,
protected Temporary rows and implicit large-component hard rows. Fixed and active
bounds enter the row-space projector without a global `n x nullity` basis; finite
first events, independent activation, multiplier-sign release, projected KKT,
full-coordinate feasibility, predicted decrease and nonlinear restoration are
validated before acceptance. Large interior/endpoint/release/alias/protection,
dense-oracle, evaluator-domain and constrained-singleton regressions complete
item 413.

2026-07-17 continuation slice: core now provides accepted-threshold augmented
null tangents, deterministic adaptive step control and a manifold-aware audited
pseudo-arclength row. Linkage natural continuation stops transactionally at a
turning point, while explicit pseudo-arclength continuation crosses the
displacement-driven L3 fold at model scales `1e-6`, `1` and `1e6`. Ephemeral
parameter/control rows are never published; every accepted sample is re-solved
and independently validated as an ordinary physical fixed-driver problem.

Independent continuation review follow-up: zero-distance requests now perform
fresh ordinary validation, tangent snapshots must match accepted linkage poses,
pseudo predictors retain explicit tangent orientation, natural predictors are
branch-checked before correction, and absolute/path-relative corrector locality
is required before clone-and-swap commit. Reverse/decreasing
pseudo paths, multi-Pose2 threshold orientation, direct physical endpoint parity
and extreme/nonlocal rollback are covered by M16 regressions.

Second continuation review follow-up: pseudo-row coefficients now resolve only
through `Problem` from authoritative variable scales; accepted physical trials
rejected by tangent policy remain visible; legacy beyond-fold `drive_to` rollback
and full dense/sparse physical endpoint publication parity are regression-tested.
Samples expose only corrector backend/fallback evidence, proving actual sparse QR
without publishing ephemeral equations or rank. Common-left `SE(2)` continuation
equivariance is also covered.
Branch-event claims are limited to predictor-endpoint checks plus locality for the
documented fixtures rather than interval crossing detection.

Completion notes: canonical indexed assembly, deterministic DM partitions,
bounded exact-pattern symbolic reuse, benchmark-routed sparse LM, dense rank
authority/fallback, projected-CGLS hierarchy with bounds and cross-component
strict priorities, and explicit pseudo-arclength continuation all pass their M16
regressions. Every sparse step, hierarchy trial and continuation endpoint is
independently validated before publication. Secondary returned-state costs require
fresh derivative validation, cached Temporary protection retains the originally
attained cost, and operator projectors use the unsquared authoritative rank
threshold. The locked workspace, warnings-denied
Clippy/rustdoc, WASM, benchmark compilation/test mode, native performance, release
Trunk, desktop/mobile Chromium, format and diff gates pass. Spatial/multi-driver
continuation and planar model/session/gauge migration remain assigned to M17+.

Gate: dense and sparse paths agree on independently validated geometry, rank, mobility, diagnostics and branch state; the documented planar toggle crosses only through the explicit pseudo-arclength path.

## M17: shared planar kinematic architecture

Status: complete as of 2026-07-17.

Goal: migrate the planar linkage baseline onto the architecture that spatial assemblies will share.

- [x] Separate model topology, accepted state and compiled session.
- [x] Add persistent body, feature and source IDs separate from runtime generational keys.
- [x] Make local coordinate frames the primary body-feature representation.
- [x] Distinguish physical grounding from numerical gauge removal.
- [x] Add explicit gauge policies for floating components.
- [x] Move velocity solving onto the shared reduced-linearization/rank policy.
- [x] Preserve `geosolve-linkage` as the public crate and retain the existing planar API as a compatibility facade where practical.

Gate: L1-L3 remain unchanged; floating planar assemblies report three world-gauge DOF separately from internal mobility, and alternative gauges preserve all relative geometry and diagnostics.

Completion notes: `PlanarLinkageDocument` separates persistent topology, accepted
continuous state and numerical gauge metadata. Opaque body, body-local point/axis
feature and source IDs round-trip through strict canonical JSON and lower
deterministically to private runtime keys. Domain connectivity includes physical
equations and branch monitors; exhaustive planar source certification distinguishes
physical grounds from common-left-`SE(2)`-invariant relationships. Floating
components solve through private manifold fixed-pose gauges, while a separately
validated ungauged `SolveSession` remains authoritative for public audit, rank,
diagnostics and checked `gauge_dof`/`internal_mobility` reports. Automatic and
explicit persistent gauge references are transactional and never become physical
sources. Persistent and compatibility velocity APIs now consume the same accepted
component-local hard linearization, residual scales and rank thresholds, publish
world-frame body-origin velocities, apply only representative gauge twists and
independently validate every differentiated physical equation. L1-L3, floating
weld/revolute, disconnected, branch-only, scale `1e-6`/`1`/`1e6`, alternative
gauge, persistence and inconsistent-rate regressions pass. The locked workspace,
warnings-denied Clippy/rustdoc, WASM, benchmark compilation/test mode, native
performance, release Trunk, desktop/mobile Chromium, format and diff gates pass.

Post-M17 adversarial hardening adds a separate deterministic public-API corpus for
the architectural seams M18 will reuse. Three-body private-gauge recovery and live
gauge rebuilds keep document, runtime, accepted geometry, audit and accepted core
linearization coherent. Every body reference in a floating welded component is
common-left-`SE(2)` and scale equivariant; a physical ground on a nonlowest body
prevents numerical gauge selection. Disconnected floating/grounded velocity remains
component-local, and alternative velocity gauges differ by exactly one common world
twist. A two-revolute fixture crosses the configured physical rank threshold while
retaining exactly three certified gauge DOF. Multi-component malformed gauge JSON,
current-revision invalid gauge edits and duplicate-weld diagnostics are transactional,
and private gauge rows are absent from every public source, audit, structural, rank,
conflict, redundancy and singularity surface. All eight adversarial tests pass
without a production-code correction.

---

# Parallel product expansion

## M18: spatial kinematics vertical slice

Goal: prove the spatial state, feature and gauge architecture with a minimal useful assembly set.

- [x] Add `SpatialAssembly` within `geosolve-linkage`.
- [x] Add spatial rigid bodies and body-local point/frame features.
- [x] Add physical ground and automatic floating-component gauge policies.
- [x] Add fixed-frame, ball and revolute joints/mates.
- [x] Add source mapping, accepted geometry, audit, rank/mobility and rollback APIs.
- [x] Add transformed/scaled exact and perturbed fixtures.

Gate: every primitive reports expected relative DOF and passes tangent-space Jacobian, gauge, invalid-feature and independent-validation tests.

Completion notes: `SpatialAssembly` owns one quaternion-backed `Pose3` variable
per rigid body plus finite body-local point and checked right-handed frame features.
Physical fixed-pose ground uses trusted core elimination. Certified floating
components solve first with one private six-coordinate manifold gauge and then
publish only a separately solved ungauged physical session; accepted audit, source
mappings, structural/numerical rank and mobility therefore contain no private gauge
rows. Fixed-frame, ball and explicitly directed-axis revolute sources have six,
three and five analytic rows respectively, with right/body-local Jacobians checked
against central retraction differences. Their floating physical right nullities are
six, nine and seven, split into six world-gauge DOF plus zero, three and one internal
DOF; physically grounded reports expose only the internal values. Independent
geometry validation recomputes every equation, enforces the stricter of caller
tolerance and `1e-9`, rejects fixed-frame half-turn roots and wrong revolute parity,
and never accepts non-finite transformed features. Revision-checked pose/feature and
gauge edits rebuild and swap atomically; all-fixed residual-only core components map
back through physical source incidence for truthful rollback reports. Ten public
acceptance tests cover exact and perturbed geometry, scales `1e-6`/`1`/`1e6`, common
left `SE(3)` transforms, all primitive mobility counts, disconnected gauges, audit
and private-row isolation, invalid geometry, loose solver tolerances and complete
rollback. Spatial persistence, drivers, velocity and the broader mate catalog remain
allocated to M20/M23. Locked format/diff, warnings-denied workspace Clippy, full
workspace tests, WASM check, warnings-denied rustdoc, core benchmark compilation and
release Trunk build gates pass.

## M19: ellipses and parametric conics

Goal: cover the major analytic CAD curve family without introducing implicit coefficient gauges.

- [x] Add ellipses and elliptical arcs with explicit axis/orientation state.
- [x] Add rational-quadratic conic segments.
- [x] Add explicit parabola/hyperbola branches and trimmed parameter domains.
- [x] Add center, focus, axis and endpoint features.
- [x] Add ellipse/conic measurements justified by CAD use cases.
- [x] Preserve valid circle-limit geometry while reporting unobservable orientation truthfully.

Gate: analytic jet oracles, affine/similarity transformations, branch retention and rational-pole rejection pass; generic contact/tangency adds no conic-pair equation code.

Completion note (2026-07-18): immutable third-order jets now cover full ellipses,
directed elliptical arcs, homogeneous rational quadratics, trimmed parabolas and
explicit hyperbola branches. The sketch compiler gives homogeneous controls and
shape scalars deterministic active mappings, lowers every contact and tangency
through the existing generic local-AD residuals, independently reconstructs every
candidate conic and caps acceptance at `1e-9`. Persistent version-one curve variants,
semantic features, CAD measurements, commands/history, canonical JSON, deterministic
runtime remapping and accepted-state projection preserve trims, winding, arc sweep and
branch state. Fourteen geometry tests and twenty-four sketch tests cover analytic and
finite-difference jets, required scales, affine/similarity covariance, circle limits,
all-family generic tangent Jacobians, poles, overflow, recovery and rollback. Locked
format/diff, warnings-denied workspace Clippy, full workspace tests, WASM check,
warnings-denied rustdoc, core benchmark compilation and release Trunk build gates pass.
Post-completion playground follow-up exposes domain-projected draggable start/end trim
handles for circular/elliptical arcs, parabolas and hyperbolas plus the rational
homogeneous middle coordinate. Preview and release remain independently validated and
commit as one history step without moving conic equations into the browser.

## M20: spatial mate and joint catalog

Goal: support the common CAD assembly and linkage relationships in three dimensions.

- [x] Add axis and plane features with stable local clocking.
- [x] Add prismatic, cylindrical, planar and universal joints.
- [x] Add distance, angle, axis-alignment and frame-offset mates.
- [x] Add hinge and translation coordinates with position drivers.
- [x] Add explicit axis parity, winding, side and signed-volume branch monitors.
- [x] Add multiple simultaneous drivers and explicit assembly-mode transactions.

Gate: every joint/mate has exact, recovery, tangent-Jacobian, scale, mixed-scale, degeneracy and expected-mobility fixtures; representative shaft/bearing and block/base CAD assemblies pass.

Completion note (2026-07-18): spatial axis and plane features retain complete
body-local `Frame3` clocks. Prismatic, cylindrical, planar and universal joints;
point-distance, interior axis-angle, direction-only axis-alignment and full frame-offset
mates; axial and planar translation plus hinge coordinates; and separate hard position
drivers compile with analytic right-tangent Jacobians and fresh independent domain
validation. Axis parity, winding, side and signed-volume monitors connect domain
components and publish finite mode evaluations without synthetic equality rows.
Revision-checked batch transactions stage multiple driver and mode edits and swap only
one fully validated private-gauge/physical candidate. Spatial IDs carry private assembly
provenance, while M23 retains ownership of serialized persistence. Fifty M20 tests cover
exact/recovery/Jacobian behavior, scales `1e-6`/`1`/`1e6`, true mixed-scale geometry,
rank/mobility, false roots, monitor-only gauges, complete rollback, shaft/bearing and
block/base assemblies. Locked format/diff, warnings-denied workspace Clippy, full
workspace tests, WASM check, warnings-denied rustdoc, core benchmark compilation and
release Trunk build gates pass.

## M21: non-rational B-splines

Goal: add locally supported production spline geometry over the generic curve/contact architecture.

- [x] Add validated degree, control identity and nondecreasing knot vectors.
- [x] Add de Boor evaluation and jets through third derivative.
- [x] Add clamped and periodic curves.
- [x] Add stable semantic span identities and one-sided knot evaluation.
- [x] Restrict residual incidence to the active span's local control support.
- [x] Add knot insertion with geometry invariance.
- [x] Add continuity diagnostics from knot multiplicity.

Gate: Bezier equivalence, affine covariance, partition of unity, knot insertion, local support and span-transition tests pass; malformed knots and insufficient continuity reject before success.

Completion note (2026-07-18): immutable geometry now validates bounded degree,
complete clamped knot vectors and canonical one-period periodic topology, and exposes
span-local basis values plus jets through third derivative with explicit one-sided
knot evaluation. Persistent curves retain ordered control identities, opaque semantic
span IDs, winding and never-reused allocation high-water marks across insertion and
divergent undo history. Generic point contact and tangency differentiate only the
selected span's degree-plus-one controls and latent local parameter. Atomic knot
insertion preserves parameterized geometry and prior control identities, migrates
contacts and neighborhoods, and splits semantic span identity only for a new positive
interval. Nine geometry and nine sketch acceptance tests cover Bezier equivalence,
partition of unity, affine covariance, periodic seams, finite differences, local
support, refinement, continuity, explicit transitions, persistence and rollback; the
web consumer samples all public semantic spans without duplicating spline equations.
Locked format/diff, warnings-denied workspace Clippy, full workspace tests, WASM
check, warnings-denied rustdoc, core benchmark compilation and release Trunk build
gates pass.

## M22: NURBS and advanced CAD constraints

Status: complete as of 2026-07-19.

Goal: complete Deliverable 1.

- [x] Add positive rational weights and homogeneous de Boor jets.
- [x] Add weight derivatives and an explicit weight-gauge policy.
- [x] Add rational-denominator and mixed-scale ambiguity diagnostics.
- [x] Add signed/unsigned curvature and osculating-radius measurements.
- [x] Add equal-curvature and G2 continuity constraints.
- [x] Add separately named parametric C2 continuity.
- [x] Add generic normal/tangent and endpoint continuity constraints.
- [x] Complete persistence for every curve, feature, dimension, contact, span and branch state.
- [x] Add sketch fuzz/property, differential-oracle and large sparse performance corpora.

Gate: unit-weight NURBS reproduce B-splines, quadratic NURBS reproduce canonical conics, local support remains bounded by degree, curvature derivatives validate, and the complete 2D CAD acceptance matrix passes.

Completion note (2026-07-19): immutable and editable clamped/periodic NURBS now
share M21 basis/span topology, use homogeneous refinement, expose only
degree-plus-one local control/weight incidence, and remove the projective weight
null direction through one explicit persisted unit gauge. Reference-translated,
pairwise rational quotient jets and compensated differential geometry reject every
unrepresentable product, denominator, derivative or curvature without turning it
into zero. Signed/unsigned curvature, finite osculating radius, tangent/sided-normal,
signed or branch-explicit magnitude curvature, G0/G1/G2 and rate-explicit
parametric C2 lower through generic differential rows with finite-difference tests,
structured audit and independently recomputed immutable candidate rows.

Persistent weights, gauge identity, semantic spans, winding, knot side,
neighborhood, normal side, curvature relation, endpoint order and C2 rates round
trip canonically; refinement, transition, re-gauging, deletion and solved-weight
commits are atomic. Property/differential corpora include 48 generated cases,
required scales, malformed and mixed-scale inputs, cancellation/underflow
regressions and a 1,000-control/128-contact sparse-locality case. Thirteen geometry
and seventeen public sketch M22 tests plus private candidate/AD and web sampling
regressions pass. Locked format/diff, warnings-denied workspace Clippy, full
workspace tests, WASM, rustdoc, core benchmark compilation and release Trunk
gates pass.

---

# Kinematic completion

## M23: 2D/3D assembly completion

Goal: complete Deliverable 2 without adding physics.

- [x] Generalize adaptive and pseudo-arclength continuation to spatial assemblies.
- [x] Add branch-boundary events, hysteresis and explicit mode-change APIs.
- [x] Add multiple-driver velocity-level kinematic queries.
- [x] Distinguish determinate, underdetermined and inconsistent velocity requests.
- [x] Return body and feature velocities plus optional motion/nullspace bases.
- [x] Add planar mechanisms embedded in 3D and compare them against planar oracles.
- [x] Add spatial closed-chain, mixed-scale and large sparse assembly scenarios.
- [x] Complete persistence for bodies, features, joints, mates, gauges, drivers and assembly modes.
- [x] Add linkage fuzz/property, differential-oracle and performance corpora.

2026-07-19 spatial continuation slice: `SpatialAssemblySession` now provides
revision-checked adaptive natural and explicit pseudo-arclength continuation for
one selected hinge or translation position driver under ADR 0016. Active scalar
drivers reuse the executable spatial equations and pass central differences;
private `Pose3` gauges and pseudo control rows remain ephemeral, while every
published sample is a separately solved and independently validated ordinary
physical session. Monotone shaft/bearing motion, grounded and floating gauges,
the embedded spatial slider-crank fold, reverse orientation, exact-fold rank,
zero/tiny paths, nonlocal rollback, common-left `SE(3)` and forced dense/sparse
parity pass at the required scales. Typed branch-boundary events, hysteresis and
explicit mode changes remained the next ordered item and were not claimed by
that first slice.

2026-07-19 spatial branch-event slice: accepted spatial solves now publish typed
finite clearances for source parity, prismatic clock, fixed/frame-offset
half-turn roots, hinge driver/cut roots and explicit axis/side/volume monitors.
Continuation uses normalized `2e-3` enter and `4e-3` leave thresholds, reports
predictor versus corrected endpoints, retains hysteresis latches only through
accepted ordinary solves and turns attempted canonical hinge-cut crossings into
typed stops. Revision-checked mode transactions atomically change parity, side,
orientation or hinge winding/cut state, require the changed branch to leave the
band and roll back every accepted view on failure. Endpoint observation remains
explicitly narrower than interval-global event tracing. Multi-driver spatial
velocity is the next ordered item.

2026-07-19 spatial velocity slice: `SpatialAssemblySession::velocity` and
`velocity_with_options` accept revision-checked source-keyed hinge/translation
rates, treat unlisted drivers as zero-rate rows and solve each accepted physical
component with its existing dense-authoritative rank threshold. Executable
parameterized driver columns preserve the configuration-dependent hinge
derivative and translation scaling. Outcomes distinguish determinate modulo
certified world gauge, underdetermined internal motion and inconsistent affine
rates without publishing a least-squares body field. Gauge-selected body-origin
world velocities, concrete point/frame/axis/plane derivatives and coordinate
rates pass independent differentiated source validation and central position
oracles. Optional deterministic normalized physical nullspace bases reuse the
accepted rank, retain floating world actions and publish raw body/feature fields
without private-gauge removal under ADR 0017. Embedded-planar oracle parity is
the next ordered item.

2026-07-19 embedded-planar parity slice: one reusable displacement-driven
spatial slider-crank fixture now shares the exact L3 dimensions, positive-X mode,
driver and fold geometry of the planar oracle under an arbitrary static `SE(3)`
embedding. At scales `1e-6`, `1` and `1e6`, natural continuation endpoints agree
after lifting every body pose and representative closure point. Spatial rank 18
and planar rank 9 both have zero physical nullity/internal mobility at the driven
regular endpoint. Compatibility and persistent planar velocity agree with
spatial body-origin, angular, point, hinge and translation rates under the
embedding basis, while every domain independently retains its `1e-9` validation.
Spatial closed-chain, mixed-scale and large sparse scenarios are next.

2026-07-19 spatial scenario slice: a non-planar four-universal closed ring runs at
all required scales with explicit positive signed-volume chirality, rank `16`,
right nullity/internal mobility `2` and rollback on a mirrored mode edit. A
three-body planar-stage/tool stack combines `1e6s` and `2e-6s` coordinates plus a
micro frame offset, remains finite/full-rank with retained winding/side modes and
passes at `s=1e-6,1,1e6`. A 43-moving-body fixed-frame chain has 258 active
coordinates/rows, structural nnz `3060`, accepted rank `258`, zero mobility and a
real `SparseQr` step under `SparsePreferred`; dense SVD remains authoritative for
rank. The exact connected-chain `Auto` density boundary is deferred to the
explicit release performance corpus rather than adding a long dense-SVD debug
test.

2026-07-19 spatial persistence slice: ADR 0018 adds a version-1
`SpatialAssemblyDocument` with canonical strict JSON, fixed persistent IDs,
separate topology/accepted state, source-order preservation and deterministic
fresh runtime remapping. Every body and concrete feature/source/coordinate/mode
family, accepted pose, driver target, winding, gauge reference and boundary
hysteresis latch round-trips. `SpatialAssemblyDocumentSession` captures accepted
sessions, independently solves imports and atomically retains all prior views on
failed replacement. Six persistence tests cover the full catalog, required scales,
explicit gauges, hysteresis, malformed fields/references/state and rollback.

2026-07-19 final linkage corpus slice: 32-case generated scale/common-left `SE(3)`
and persistence properties plus 32 single-byte document mutations retain finite
independently validated success only. A 36-case analytic slider-crank oracle checks
body position, body/feature velocity and coordinate rates over six phases, two
embeddings and all required scales. The normal performance corpus fixes 43/255/256
moving-body shapes; the explicit release gate proves the true 256-moving-body,
1536-active-column `Auto` density boundary selects `SparseQr`, preserves dense rank
1536 and validates residuals within its 180-second budget. Locked format/diff,
warnings-denied workspace Clippy, full workspace tests, WASM, rustdoc, core
benchmark compilation and release Trunk gates pass.

Gate: planar and spatial assemblies preserve explicit modes, report truthful mobility, validate every accepted configuration and velocity equation, and retain the last accepted state on all failures.

## M24: sketch extension and embedding foundation

Goal: establish additive identity, metadata and serialization seams before adding
new persisted advanced-sketch definitions.

- [x] Freeze sketch JSON version 1 behind a private wire DTO and explicit version-dispatch boundary without changing canonical output.
- [x] Add one persistent document-element identity enum covering the document, geometry, constraints, dimensions and audit sources.
- [x] Add persistent source-owner/source lookup APIs so embedders do not join application state through runtime/core IDs.
- [x] Add a generic `SketchAttributes<T>` sidecar bound to one document identity, with live/orphan inspection and explicit cleanup.
- [x] Preserve dormant sidecar values across delete/undo/redo while rejecting foreign-document and wrong-kind targets.
- [x] Keep application attributes outside sketch lowering, solver state, equation audit and canonical sketch JSON.
- [x] Document application-owned attribute history/serialization and provide a native typed-attribute example.

Gate: legacy version-1 JSON remains byte-canonical, migration dispatch is strict,
typed sidecars require no generic solver/document/session propagation, metadata
cannot dirty or change accepted geometry/audit, and native plus locked WASM public
consumers compile.

2026-07-20 completion: ADR 0019 separates the public in-memory sketch graph from
its private frozen version-1 wire DTO and routes imports through explicit strict
version dispatch without changing canonical bytes. `DocumentElementId`, raw-ID
resolution and source-order `DocumentSourceRef`/`DocumentSourceOwner` views expose
stable semantic joins without runtime/core IDs. Generic `SketchAttributes<T>` is
bound to one `DocumentId`, rejects foreign/missing/wrong-kind targets, retains
dormant values across real delete/undo/redo and requires explicit orphan cleanup;
it has no serde, lowering, solver or audit coupling. Five M24 tests and the native
`typed_attributes` example cover every element/source kind, history, exact v1 JSON
and solver isolation. Locked format/diff, warnings-denied workspace Clippy, full
workspace tests, WASM, rustdoc, core benchmark compilation and release Trunk gates
pass.

## M25: associative linear constructions

Goal: add line offsets and point-defined entity mirroring by composing ordinary
geometry, dimensions and explicit constraints.

- [x] Add signed line-offset driving/reference dimensions with explicit side and endpoint orientation.
- [x] Keep supporting-line offset and exact translated-segment offset as separately named modes with truthful remaining DOF.
- [x] Add one accepted entity-mirror construction for lines, polylines, quadratic/cubic Beziers and non-rational B-splines over existing point symmetry equations.
- [x] Make topology-changing mirrored spline refinement an explicit coordinated transaction rather than silently losing association.
- [x] Complete public command/example/demo coverage for the existing oriented line-angle dimension without adding a duplicate equation.
- [x] Add sketch document version 2 and deterministic version-1 migration for the new persisted definitions.
- [x] Add audit, derivative, branch-cut, transformation, scale, persistence, history and rollback corpora.

Gate: both offset modes report correct rank/DOF and independently validated rows;
mirrored control geometry remains associative under accepted point edits; oriented
angles preserve explicit direction; and all three workflows round-trip and roll
back atomically at scales `1e-6`, `1` and `1e6`.

2026-07-20 completion: ADR 0020 adds distinct `SupportingLineOffset` and
`ExactTranslatedSegmentOffset` runtime/document dimensions with positive targets,
explicit left/right side and same/reversed endpoint orientation. The supporting
form contributes one normalized parallel row plus one signed-distance row and
retains target axial-slide/length DOF; the exact form contributes four translated
endpoint rows and preserves correspondence/length. Local-AD Jacobians, structured
audit and independent candidate row/branch validation cover both. Point-defined
line/polyline/quadratic/cubic/B-spline mirrors expand to reflected points, ordinary
same-family geometry and existing `SymmetricAboutLine` constraints; branch vectors
are reflected explicitly. Public document commands make mirror creation and
coordinated paired B-spline knot insertion atomic and undoable. Canonical sketch
JSON is v2, while a private dimension-specific v1 DTO migrates accepted legacy
definitions deterministically and rejects v2 offset syntax labeled as v1. The
existing oriented-angle equation is exercised through its public document command,
playground tool and `associative_linear` native example, including branch-cut and
wound-target history. Nine M25 tests cover derivative, audit, rank/DOF, invalid
branch/input, all required scales, rigid transformations, v1/v2 persistence,
associative edits, topology refinement, history and rollback. A generated M23
property seed also exposed and now guards bitwise-idempotent accepted `Pose3`
quaternion canonicalization. `cargo fmt --all -- --check`, `git diff --check`,
warnings-denied locked workspace Clippy, full locked workspace tests, locked WASM,
warnings-denied rustdoc, core benchmark compilation, the native example and release
Trunk through `nix-shell ../../shell.nix` pass. Mirror construction intentionally
excludes rational/conic families; generic visual profile analysis starts in M26.

## M26: visual line-profile detection

Goal: expose reusable read-only bounded-face analysis without creating usable CAD
regions or implying solid/B-rep semantics.

- [x] Analyze accepted line and polyline spans through public sketch APIs only.
- [x] Weld shared point identities and active coincidence constraints without undocumented proximity snapping.
- [x] Split proper crossings and T-junctions ephemerally, then extract deterministic bounded faces through a half-edge walk.
- [x] Reject collinear overlaps and numerically ambiguous affected components instead of guessing topology.
- [x] Publish source-span provenance, ordered contours, orientation, visual area and explicit `Complete`/`Truncated`/`Skipped` status.
- [x] Enforce deterministic candidate/fragment budgets and never publish partial analysis as complete.
- [x] Render pointer-transparent visual-only overlays in the WASM demo without IDs, selection, persistence, history or autosave state.
- [x] Add exact loops, open chains, diagonals, crossings, T-junctions, bow-ties, nesting, ambiguity, transformation, scale and budget tests.

Gate: accepted line arrangements produce deterministic finite visual faces, all
unsupported/ambiguous cases are typed, canonical documents are unchanged by
analysis, and no detected boundary becomes a sketch entity or solver equation.

Completion record (2026-07-20): ADR 0021 adds read-only line/polyline arrangement
analysis with explicit identity/coincidence topology, ephemeral crossing and
T-junction splits, iterative bridge/half-edge face extraction, nesting, source
interval provenance and typed completeness. Collinear overlap, uncertainty and
unsolved inconsistent coincidence classes fail closed; overflow-safe candidate,
fragment, containment and face budgets bound publication. Eleven M26 scenarios,
three internal depth/count regressions and the pointer-through overlay browser
check pass. Format/diff, warnings-denied locked workspace Clippy, full locked
workspace tests, locked WASM, warnings-denied rustdoc, all benchmark compilation,
release Trunk and desktop/mobile Chromium E2E pass.

## M27: associative line fillet foundation

Goal: establish one truthful line-line fillet vertical slice before broadening the
generic curve matrix.

- [x] Accept an ADR for fillet ownership, radius, parent contacts, normal sides, endpoint order, sweep and later trimming semantics.
- [x] Add a persistent line-line fillet whose center/contact equations reuse ordinary line jets and explicit side state.
- [x] Derive accepted arc endpoints from solved parent contacts rather than retaining stale fixed trim angles.
- [x] Support driving/reference radius and explicit clockwise/counterclockwise sweep.
- [x] Keep parent lines untrimmed in this foundation milestone and state that limitation in public APIs.
- [x] Independently validate center-normal, contact, tangent, radius, side and sweep data before publication.
- [x] Add finite-difference, recovery, convex/concave, invalid-domain, transform, scale, persistence and rollback tests.

Gate: regular line-line fillets remain associative and independently valid through
accepted edits while parallel, collinear, escaped, ambiguous, zero-radius and
non-finite candidates never publish success or mutate retained state.

Completion record (2026-07-20): ADR 0022 defines one persistent association over
an ordinary output arc, two strict-interior line contacts, explicit normal sides,
endpoint order, sweep and driving/reference radius. Four common local-AD line-jet
rows solve the center and contacts; accepted endpoint angles are derived before
independent center-normal, contact, tangent, radius, side, order and sweep
validation. Sketch JSON v3 persists the construction behind frozen v1/v2 DTOs.
Seven M27 scenarios plus a private sweep-corruption regression cover Jacobians,
audit, every side/order/sweep combination at all required scales, similarity
transforms, DOF, associative parent edits, persistence, history, suppression,
explode/ownership behavior, rollback and invalid geometry. Format/diff,
warnings-denied locked workspace Clippy, full locked workspace tests, locked WASM,
warnings-denied rustdoc, all benchmark compilation, release Trunk and
desktop/mobile Chromium E2E pass. Parent trim views and truthful executable
incidence on associated output arcs remain M28 work.

## M28: generic fillets and parent trimming

Status: complete as of 2026-07-21.

Goal: generalize associative fillets over the existing immutable curve-jet seam
and add explicit visible trim topology.

- [x] Add persistent trim views over underlying curve support instead of destructively approximating control geometry.
- [x] Make rendering, hit testing, contact validation, deletion, history and persistence respect visible trim intervals.
- [x] Generalize fillet incidence to regular line, circle/arc, ellipse/conic, Bezier, B-spline and NURBS spans without pair-specific residual equations.
- [x] Update both parent trim endpoints atomically from independently accepted fillet contacts.
- [x] Preserve explicit span, side, winding, endpoint-order and sweep state through edits and migrations.
- [x] Reject zero speed, cusps, poles, escaped spans, ambiguous roots and offset singularities such as `1 +/- r*kappa = 0`.
- [x] Add full family-pair derivative, transformation, scale, trimming, persistence, malformed-input and rollback corpora.

Gate: every supported regular curve family produces finite branch-explicit fillets
and trim views through common residual plumbing, no singular/ambiguous candidate
becomes success-like, and the complete Deliverable 1 corpus remains unchanged.

Completion record (2026-07-21): ADR 0023 keeps immutable support geometry and
adds one equation-free persistent visible interval per stable curve span. Generic
version-4 fillets use six common local-AD rows: four center/normal-offset rows and
two radial endpoint-alignment rows. Their ordinary output-arc endpoint angles are
solver coordinates, so point, contact, tangency, curvature and continuity consumers
retain complete incidence. Accepted contacts atomically update owned boundaries;
suppression freezes them, while explosion converts them to fixed boundaries and
retains the ordinary arc. Frozen v1-v3 DTOs migrate deterministically, with legacy
v3 line fillets remaining visibly untrimmed.

The 14-family, 105-unordered-pair corpus covers finite differences, every branch
code, required scales and transforms, active spline/NURBS support, persistence,
periodic winding, malformed ownership, singular offsets, root escape, suppression,
history and rollback. Public visible-interval APIs drive rendering, hit testing,
selection and line-profile analysis; desktop/mobile browser automation covers
trim interaction, persistence and explosion. Format/diff, warnings-denied locked
workspace Clippy, full locked workspace tests, release WASM, warnings-denied
rustdoc, all benchmark compilation, release Trunk and Chromium E2E pass. M28
intentionally supports one visible interval per support span rather than arbitrary
multi-fragment trim topology.

## M29: public API and release hardening

Status: complete as of 2026-07-21.

Goal: make both deliverables ready for a stable library release.

- [x] Review public APIs and remove accidental exposure of compiler/core internals.
- [x] Finalize versioned serialization and migration policy.
- [x] Add crate-level documentation and complete examples for both deliverables.
- [x] Define semantic versioning, changelog and deprecation policy.
- [x] Complete GPL/licence and attribution audit.
- [x] Record supported scale/performance envelopes and benchmark baselines.
- [x] Run malformed-document and degenerate-geometry fuzzing without panic or false success.
- [x] Keep the disposable WASM playground compiling as a non-authoritative consumer of public APIs.

Gate: all acceptance suites, serialization round trips/migrations, fuzz corpora,
documentation tests, performance baselines, native checks and locked WASM smoke
builds pass.

Completion record (2026-07-21): version `0.1.0` is the first supported preview.
`docs/API_COMPATIBILITY.md` defines lockstep crate SemVer, MSRV, API tiers,
deprecation, schema retention and dependency-order publication. Sketch reads v1-v4
and writes v4; planar and spatial linkage retain frozen v1 languages. Public API
review removed unused scenario enums, made evolving domain errors non-exhaustive
and explicitly classifies retained compiler/runtime/core report views as unstable
advanced diagnostics rather than persisted application identity.

All library crates now carry package descriptions, registry-versioned internal
dependencies, docs.rs metadata, packaged README/GPL files and expanded crate guides.
Complete persistent sketch, planar linkage and spatial assembly examples compile and
run. `CHANGELOG.md`, `THIRD_PARTY_LICENSES.md`, `deny.toml` and the M29 scale and
performance envelope establish release, attribution and rebaseline policy; the WASM
distribution includes visible legal/schema/performance links and an E2E-checked M28
UAT guide. Deterministic mutation tests exercise sketch v1-v4 plus planar/spatial v1
under panic guards and require every surviving solve to be finite, canonical,
independently hard-valid and at most `1e-9` normalized residual.

`scripts/release-gate.sh` enforces format/diff, warnings-denied Clippy/rustdoc, full
locked workspace tests, locked WASM, benchmark compilation, mutation tests, package
contents, dependency licences, native interaction budgets, the ignored 1,536-
coordinate spatial release case, release Trunk and desktop/mobile Chromium. The
M29 run recorded small/medium first-solve p95 `1.849/51.550ms`, incremental p95
`4.558/111.778ms`, browser p95 `9.300/61.400ms`, and spatial release `88.29s`, all
inside their gates. Registry publication remains a separate maintainer action after
a repository URL, clean release commit and tag exist.

## M30: interactive construction and NURBS UAT

Status: complete as of 2026-07-21.

Goal: expose the completed M25-M28 construction and NURBS behavior through
genuinely movable public-document browser labs and focused authoring controls.

- [x] Add separate supporting-line and exact translated-segment offset labs with their truthful two-DOF and one-DOF motion.
- [x] Add an entity-mirror lab that exercises the public mirror macro rather than only a point-symmetry residual.
- [x] Add a directed-angle branch-cut lab with editable orientation, target and driving/reference mode.
- [x] Add interactive M27 line-line and M28 line-circle, line-Bezier and NURBS-line fillet labs.
- [x] Add NURBS weight/gauge, local-support/knot-insertion, periodic span/winding and differential-continuity labs.
- [x] Add focused offset, mirror, angle, fillet and NURBS browser controls using only public document/session transactions.
- [x] Give every normal lab at least one documented initial DOF, one named primary drag and a reset action.
- [x] Prove projected drags change accepted associated geometry, not merely that examples render.
- [x] Add native persistence/history/rollback tests plus desktop interaction and mobile-load browser coverage.

Gate: every advertised lab starts independently hard-valid, reports its documented
DOF, changes accepted geometry through its primary interaction, retains explicit
branch state and canonical persistence, and contains no browser equation.

Completion record (2026-07-21): twelve reusable public scenarios cover supporting
and exact offsets, entity mirror, directed angle, M27/M28 line/circle/Bezier/NURBS
fillets and four NURBS interaction families. `AlphaScenarioUat` publishes expected
equality/bounded DOF, instructions and a named primary drag. Focused browser controls
create offsets/mirrors and edit angle orientation, fillet branch/radius, NURBS
weights/gauge/knots and periodic transitions through public document transactions.

Seven M30 native tests prove exact initial DOF, geometry-changing projected drags,
association motion, canonical persistence, history and invalid-edit retention.
All 81 native web tests pass. Release Trunk plus focused Chromium proves desktop
construction/fillet/NURBS interactions and responsive loading of all twelve labs on
mobile. Format/diff and warnings-denied focused Clippy pass. Aggressive one-step
NURBS-line fillet drags may truthfully reject at iteration limit; documented smaller
continuation moves remain accepted and no failed drag republishes geometry.

## M31: all-family visual profile analysis

Status: complete as of 2026-07-21.

Goal: make visual-only boundary detection truthful for every built-in planar curve
family rather than silently omitting non-linear spans.

- [x] Accept an ADR for family-specific bounded curve pieces, certified intersection isolation, curved half-edges, area/containment bounds and incomplete-result policy.
- [x] Support line/polyline, circle/arc, ellipse/elliptical arc, rational conic, parabola/hyperbola, Bezier, B-spline and NURBS visible intervals.
- [x] Isolate transverse pair and self intersections without coordinate snapping or missed-root `Complete` claims.
- [x] Treat tangency, overlap, poles, unresolved multiple roots and exhausted subdivision as typed incomplete component outcomes.
- [x] Sort half-edges by actual outgoing curve tangent and preserve exact source-span parameter provenance in traversal order.
- [x] Compute analytic Green-area terms where available and bounded interval integration otherwise; publish orientation only when the area-sign enclosure is resolved.
- [x] Perform bounded curve-aware containment and include exact circular/elliptic extrema in contour bounds.
- [x] Join fillet-owned trim boundaries to output-arc endpoints through explicit ownership, never proximity inference.
- [x] Expose analysis scope, status, issues and consumed budgets to public consumers while keeping output equation-free and non-persistent.
- [x] Add an all-family pair/self-intersection, transform, scale, ambiguity, budget and persistence-neutrality corpus.
- [x] Render returned curved source intervals through public curve evaluation and expose focused browser UAT scenes and diagnostics.

Gate: every built-in family can participate in a complete finite visual face only
when all relevant roots, local rotations, area signs and containment decisions are
resolved; ambiguous or over-budget components never publish false complete faces.

Completion record (2026-07-21): ADR 0024 extends the read-only M26 arrangement
through family-specific linear, circular, analytic-conic, polynomial and homogeneous
rational pieces. Pure-Rust outward interval arithmetic, certified transcendental and
angle kernels, interval-Newton/Krawczyk isolation and bounded integration make root,
tangent-order, area and containment publication fail closed. Periodic winding,
source-parameter provenance, exact endpoint joins, same-carrier overlap and explicit
fillet ownership remain structural state; coordinate proximity and render samples
never establish topology. Component-local ambiguity removes affected faces while
retaining provably disjoint clean components under an overall incomplete status.

The 31-test M31 corpus covers all 120 family-pair fixtures, self-intersections,
required scales and transforms, periodic seams, nesting, malformed geometry,
overlap/tangency, local and global budgets, fillet ownership and canonical-JSON
neutrality. Six focused browser scenes expose complete curved topology, movable
fillet trims, editable NURBS self-intersections, typed incompleteness and budget
exhaustion using public curve evaluation.

Post-completion UAT regressions make active endpoint-to-curve interior contacts
structural source splits after fresh accepted-residual validation, so movable fillet
closures no longer depend on the sign of a tiny geometric residual. Self-isolation
retries transverse roots on artificial parameter boundaries in a larger source-domain
box and merges duplicate certified parameter boxes only after a fresh uniqueness
proof. Required-scale/reflection and knot-refinement tests preserve complete NURBS
self-roots away from semantic boundaries; a root exactly on an inserted semantic
knot boundary remains a typed incomplete result rather than false `Complete`.
Cycle-area integration apportions the unchanged scale-relative display uncertainty
target across directed fragments before independently checking the summed interval
against the original target. A captured NURBS capsule regression sweeps nearby
control-point perturbations while preserving four certified roots, eleven fragments,
four faces and matching endpoint topology without weakening fail-closed acceptance.
`cargo fmt --all -- --check`, `git diff --check`, warnings-denied locked workspace
Clippy, full locked workspace tests, locked WASM, release Trunk and focused
desktop/mobile Chromium E2E pass.

## M32: post-expansion UAT and release hardening

Status: complete as of 2026-07-22.

Goal: re-run the release discipline after M30-M31 broaden the public browser and
analysis surfaces.

- [x] Consolidate construction, NURBS, fillet and all-family profile UAT instructions in the browser.
- [x] Add desktop E2E for every new interaction and retained-state failure path.
- [x] Extend malformed/extreme-value mutation coverage to the new commands and profile analysis.
- [x] Record updated native/browser performance and resource envelopes.
- [x] Re-run package, licence, documentation, native, locked WASM and browser release gates.
- [x] Update compatibility, changelog and public release records for the next preview.

2026-07-21 UAT handoff slice: the browser can copy a deterministic
`GEOSOLVE_SCENE_V1` capsule containing canonical sketch JSON, exact profile budgets,
active-example/status metadata and a checksum. A private pure-Rust LZSS/base64url
codec keeps text compact without a new dependency. The existing Import text action
recognizes capsules, independently solves the document and restores profile options;
malformed, corrupt, oversized or over-budget capsules retain the accepted scene.
Native codec/round-trip/atomicity tests and focused Chromium copy/import coverage
pass. Capsules are diagnostic exchange text, not a new sketch persistence schema.
The same focused layout gate assigns every below-canvas diagnostic section an
explicit desktop grid area and enforces usable solver/profile/audit/release heights;
long bodies scroll locally instead of collapsing sibling rows, while narrow screens
retain the natural stacked document flow.

Completion record (2026-07-22): `0.2.0` is the post-expansion supported preview.
The desktop M32 suite covers reset, both offset constructors, previous NURBS span,
generic-fillet branch/radius, NURBS weight/knot edits and exact retained state for
invalid operations plus corrupt, oversized and over-budget capsules. The two-test
M32 mutation corpus exercises new command payloads and every profile family/option
under panic guards while requiring finite independently valid accepted output and
bounded fail-closed analysis.

`docs/M32_SCALE_PERFORMANCE.md` records six native and four browser timing/resource
classes. On clean candidate `8d6f648`, native p95 was at most `0.506 ms` for the
construction edit, `0.340 ms` for NURBS knot insertion, `24.796 ms` for all-family
profiles and `16.486 ms` for NURBS self-analysis; browser p95 was `4.3/6.2/81.4/35.3
ms` for the corresponding scene classes. Deterministic profile work remained 1,445
candidate pairs/31 roots/113 fragments/30 faces and 1/1/4/1 respectively.

The clean `scripts/release-gate.sh` run at `8d6f648` passed formatting/diff,
warnings-denied Clippy/rustdoc, full locked workspace tests, locked WASM, benchmark
compilation, M29/M32 mutation suites, M14/M32 timing gates, the 1,536-coordinate
spatial release case in `87.56 s`, dependency licences, package contents, release
Trunk and the complete Chromium suite on its first invocation. The completion-status
commit containing this record must pass the same clean command before M33 work begins.

Gate: all M1-M31 acceptance, mutation, documentation, package, native, WASM,
performance and browser suites pass from one release-gate command.

## M33: CAD engine contract and baselines

Status: complete as of 2026-07-23.

Goal: freeze the product and ownership decisions for a host-usable planar sketch
engine before changing persistence or public state semantics.

- [x] Accept ADRs for design-versus-accepted state, host parameters, immutable external snapshots, cancellation/concurrency, companion APIs and draft-v5 development.
- [x] Freeze a complete entity/feature/constraint/dimension applicability matrix and identify every unsupported ordinary CAD combination.
- [x] Define accepted-state identity over design revision, parameter revision, external-snapshot digest, activation state and solver policy.
- [x] Define the ownership boundaries for host undo, expressions, units, projection, topology and production profiles.
- [x] Add deterministic representative workloads for connected, disconnected, spline-heavy, parameter-heavy, external-reference and activation-heavy sketches.
- [x] Record cold compile, warm edit, diagnostics, profile, cancellation-latency and memory measurement boundaries without imposing premature thresholds.

Completion record (2026-07-23): accepted ADRs 0025-0028 define separate design,
attempt and accepted identities with complete immutable input stamps; host-owned
parameters, expressions, units, projection and history; cooperative operation control
and prepared-work concurrency; draft-v5 development; and one-way operations/topology
companion boundaries. The machine-checked capability matrix freezes 15 curve
families, 38 relations, 37 dimensions/measurements and explicit unsupported or
conditional combinations without adding a target-only API or changing frozen v1-v4.

Six deterministic current-v4 workloads freeze exact document, solve, audit and
profile signatures. Criterion validates 24 cold-compile, warm-edit/solve,
solve/diagnostic and visual-profile cases; peak RSS remains observational, and
cancellation latency is explicitly unavailable until M35. The full gate exposed one
saved transformed spatial property where repeated quaternion normalization changed an
accepted snapshot by a few ULPs. Machine-roundoff-unit `Pose3` reconstruction is now
bitwise idempotent while the public norm-validation band and exact velocity/
continuation snapshot checks remain unchanged; the saved seed and zero-distance
continuation are regressions.

Clean candidate `5cd7cb6` passes `scripts/release-gate.sh`: format/diff,
warnings-denied workspace Clippy/rustdoc, full locked workspace tests, locked WASM,
benchmark compilation, mutation and native timing gates, the 1,536-coordinate spatial
release case in `90.08 s`, licences, package contents, release Trunk and complete
Chromium. The focused M33 tests and all 24 Criterion test-mode cases also pass.

Gate: every new semantic concept has an accepted decision and representative fixture;
M1-M32 behavior and frozen wire languages remain unchanged.

## M34: retained design and accepted solved state

Status: complete as of 2026-07-23.

Goal: retain structurally valid unsolved intent without ever presenting it as
accepted geometry.

- [x] Separate design, attempted-candidate and independently accepted solved views with distinct revisions.
- [x] Retain structurally valid conflicts, unavailable references and failed unsuppression in design intent.
- [x] Continue rejecting malformed, non-finite, resource-invalid or referentially invalid design transactions entirely.
- [x] Publish optional finite attempted geometry as non-authoritative evidence only.
- [x] Preserve the last accepted state across topology-changing unsolved edits and identify which design revision it solved.
- [x] Define persistence and host display rules for design elements that have no accepted solved counterpart yet.

Completion record (2026-07-23): `RetainedSketchDocumentSession` adds typed,
non-interchangeable design, attempt and accepted-state identities above the unchanged
accepted-only `SketchDocumentSession`. Valid conflicting and failed-unsuppression edits
advance retained design and attempt revisions, ordinary follow-up edits repair that
same graph, and malformed/non-finite/referential/resource-invalid input advances
nothing. Attempts record exact candidate/publication requests and effective solver
policy, use persistent-ID parent warm starts, carry attempt-local mappings and expose
only complete finite optional candidate geometry. Accepted document, runtime, audit,
measurements and mappings remain one coherent older view until fresh independent
validation publishes a new accepted revision.

Frozen sketch v1-v4 remains unchanged: design and accepted graphs serialize
separately, while three integer revision high-water values belong to a host-owned
sidecar and prevent identity reuse on restore. Design-only and accepted-only elements
join only by persistent identity; no draft-v5 language, host input, cancellation or
prepared-job behavior was introduced. Thirteen focused M34 regressions cover initial
conflict, all-family finite candidates, conflict/repair, failed unsuppression,
topology-separated mappings, invalid-input atomicity, underconstrained parents, exact
attempt inputs and persistence/high-water restoration. The locked workspace tests,
warnings-denied Clippy/rustdoc, locked WASM check, release Trunk build and complete
release gate pass.

Gate: no unsolved or attempted state can produce a success-like status, accepted
revision or authoritative audit; retained design can be repaired through ordinary
transactions.

## M35: cancellation and operation control

Status: complete as of 2026-07-24.

Goal: make every potentially expensive sketch operation safely interruptible by an
interactive host.

- [x] Add cooperative cancellation and deterministic work controls to lowering, nonlinear solving, rank, diagnostic trials and profile analysis.
- [x] Distinguish cancellation, work exhaustion, numerical rejection, invalid geometry and convergence.
- [x] Add documented cancellation checkpoints and measured maximum latency around non-interruptible kernels.
- [x] Let hosts implement deadlines through the same cancellation mechanism without making wall time part of correctness.
- [x] Prove cancellation retains accepted state and cannot commit a partially validated result.
- [x] Keep native and single-threaded WASM behavior equivalent.

Completion record (2026-07-24): additive core operation-control APIs provide a
monotonic host cancellation handle/token, deterministic overflow-safe work limits,
typed operation outcomes and exact checkpoint/work reports. Controlled sketch
lowering, compilation, solving, rank/diagnostic work, profiles and accepted/retained
session mutations operate on scratch state and check `BeforeCommit` immediately before
clone-and-swap publication. Cancellation and work exhaustion therefore remain distinct
from invalid geometry, numerical rejection and independently validated convergence;
neither can publish partial state or advance accepted revisions.

Controlled dense factorization and rank kernels authorize exact counters and enforce a
256-row/256-column M35 input bound before entering their non-interruptible regions.
`docs/M35_CANCELLATION_LATENCY.md` records 20-run release maxima of `12.623086 ms`
for profile cancellation, `2.687691 ms` for the bounded QR window and `7.323588 ms`
for the bounded rank-SVD window. Nineteen focused M35 regressions cover cancellation,
work exhaustion, checkpoint placement, rollback, constructor/compile/session parity,
dense-cap boundaries and operation reports. Frozen sketch v1-v4 remains unchanged;
M56 prepared jobs and compare-and-swap concurrency remain deferred. Formatting,
warnings-denied locked workspace Clippy/rustdoc, full locked workspace tests, locked
WASM, release Trunk and the complete release gate pass.

Gate: cancelled work never reports convergence or valid publication, commits nothing
and leaves the prior accepted state bitwise unchanged.

## M36: semantic feature and scalar foundations

Status: complete as of 2026-07-25.

Goal: create closed typed operands for a complete CAD catalog without exposing an
arbitrary curve or residual plugin interface.

- [x] Introduce capability-specific references for points, centers, endpoints, controls, directions, line supports, curve spans and scalar properties.
- [x] Add explicit fixed/equal scalar semantics and the units/domains required by signed length, angle, dimensionless, curvature and parameter values.
- [x] Preserve one persistent semantic source when a relation lowers into several ordinary rows.
- [x] Make every operand serializable, audit-readable and branch-aware without coordinate inference.
- [x] Add exhaustive current-schema command/effect/measurement characterization before extending the catalog.

Gate: every semantic operand validates through persistent IDs, lowers deterministically
and has exact, malformed, persistence and audit coverage.

Completion record (2026-07-25): capability-specific persistent operands now cover
point, center, endpoint, persistent owning-curve control identity (including
B-spline/NURBS controls across knot insertion), directed
axis/tangent/normal, directed line support and stable curve spans with explicit
winding. Closed scalar-property operands distinguish length, angle, dimensionless,
inverse-length curvature and curve parameters while retaining explicit signed-length,
orientation, side, winding and neighborhood provenance. Fixed/equal scalar sources
live in one separately serialized document-bound catalog whose deterministic
allocation reserves the document high-water ID and rejects duplicate/out-of-order
sibling identities. They validate exact unit/domain/support/neighborhood compatibility,
lower deterministically to clearly separated raw and normalized hard-row evaluation
and Jacobians, pass normalized central finite differences at all required scales and
require independent `1e-9` validation of recomputed values and public evidence
structure. Controlled lowering reports cancellation/work exhaustion without mutation.

The new strict semantic-source catalog serde is deliberately separate from frozen sketch v1-v4;
canonical v4 bytes and strict older readers remain unchanged. Exhaustive stable kind
tables characterize all 30 current commands, 28 accepted effects and 16 current
dimension/differential/conic measurements. Thirteen focused M36 regressions cover
exact, malformed, persistence, refinement-stable control identity, complete parameter
branch invariants, scale, cancellation, reserved identity collision, audit corruption,
normalized finite differences and accepted-row validation behavior. M38 persistent
measurement/path-length behavior remains unimplemented.

## M37: standard planar constraint catalog

Status: complete as of 2026-07-25.

Goal: cover the ordinary geometric relations expected from a production planar
sketch engine.

- [x] Add first-class concentric and collinear relations.
- [x] Add horizontal and vertical relations between arbitrary point features.
- [x] Add grouped whole-entity block/fix and point symmetry about a center.
- [x] Generalize line/entity symmetry and equal circular radius across compatible circles and arcs.
- [x] Add equal scalar, distance and angle relations where the applicability matrix permits them.
- [x] Add high-level contact and tangent constructors that allocate and validate explicit latent branch state for hosts.
- [x] Preserve explicit same/opposite direction, side, neighborhood and containment choices for every multi-root relation.

Gate: the frozen standard relation matrix has no undocumented family gaps; every new
row passes derivative, transform, scale, persistence, branch, cancellation and
rollback gates.

Completion record (2026-07-25): the separately serialized semantic-source catalog now
owns persistent concentric, collinear, arbitrary point-pair horizontal/vertical,
grouped block, center-point and explicitly corresponded line/entity symmetry, equal
circular radius, equal point-pair distance and explicitly oriented/unwrapped equal
angle relations. High-level contact and tangent constructors allocate complete
domain/span/winding/neighborhood/orientation/side or containment state atomically and
reuse the established common-jet and specialized tangent equations. New collinear,
equal-distance and equal-angle residuals have analytic AD Jacobians, mixed-unit row
scaling, structured per-row audit and independent accepted-residual validation.

Fourteen focused M37 regressions cover all frozen relation rows, circle/arc radius family
pairs, typed point features, exact and transformed finite differences at model scales
`1e-6`, `1` and `1e6`, canonical persistence with branch state, malformed and
tautological no-mutation rejection, constructor atomicity, cancellation, grouped
source audit completeness and retained-session rollback. Frozen sketch v1-v4 remains
byte/schema compatible. Formatting, warnings-denied locked workspace Clippy, full
locked workspace tests and the locked WASM consumer check pass.

## M38: dimensions and persistent measurements

Status: complete.

Goal: complete the normal dimensional vocabulary and make measurement semantics
available without UI formulas.

- [x] Add signed relative horizontal/vertical and absolute datum-coordinate dimensions.
- [x] Add signed point-to-line distance and parallel-line separation.
- [x] Add two-line and three-point angle, circular sweep and circular arc-length dimensions.
- [x] Add driving/reference ellipse-axis and supported conic-property dimensions.
- [x] Persist curvature, osculating-radius and generic measurement definitions with typed units and provenance.
- [x] Add equation-free bounded length measurement for every regular bounded curve.
- [x] Add driving and equal path-length constraints only with bounded value/derivative evaluation and typed work exhaustion.
- [x] Remove the misleading line-only public meaning of `CurveLength` through a migration-safe replacement.

Gate: every driving and reference form agrees on the same independently evaluated
measurement; no integral dimension succeeds outside its certified work and derivative
contract.

Completion notes: the separate M38 catalog provides signed coordinates and spacing,
explicitly unwrapped angles, circular and conic dimensions, bounded path/equal-path
length, and typed persistent measurements with accepted-lifecycle provenance. The new
`SegmentLength` and `PathLength` definitions carry the expanded semantics; the old
line-only `DocumentDimensionDefinition::CurveLength` spelling remains only as required
frozen sketch-v1-v4 compatibility. Eleven focused M38 regressions cover central finite
differences, driving/reference agreement, positive and negative angle winding, work
limits, audit evidence, stale/foreign provenance and no-mutation rejection. Formatting,
warnings-denied locked workspace Clippy, full locked workspace tests and the locked
WASM consumer check pass.

## M39: CAD workbench foundation and core authoring

Status: complete.

Goal: begin the desktop-only rewrite early enough to test the ordinary CAD interaction
model before host and advanced workflows build on it.

- [x] Split application state, domain controller, tools, selection, scene, panels, persistence and browser platform code into explicit modules.
- [x] Make one domain adapter the sole owner of sketch sessions and unstable core-report translation.
- [x] Add explicit accepted, design-unsolved, solving, solved-preview and rejected-attempt application states.
- [x] Build a CAD-like desktop shell with command bar, tool palette, sketch tree, full-height canvas, property inspector, status bar and Problems drawer.
- [x] Add retained scene layers, adaptive public-jet tessellation and revision-keyed geometry/profile caches.
- [x] Implement point, line/polyline, rectangle, circle/arc, core constraints, dimensions, drag, delete, undo and redo through public commands.
- [x] Draw selectable persistent constraint glyphs and driving/reference dimensions without recomputing equations in the browser.
- [x] Keep the old advanced lab available only through an explicit temporary developer route during migration.

Gate: automated desktop E2E proves every core workflow, accepted-state retention and
canvas/tree/inspector synchronization before human UAT begins. Mobile support is not
implemented or tested.

Completion notes: `geosolve-demo-web::workbench` separates application, domain,
selection/tool, retained-scene, panel, persistence and platform responsibilities. Its
versioned application-owned snapshot stores frozen design JSON, optional accepted JSON
and lifecycle revision high-water metadata; reload independently restores accepted
state and truthfully replays retained rejected intent. Canvas dimensions consume
accepted domain values, and the web crate contains no solver equations. The release
Trunk build and focused desktop M39 E2E pass, including core authoring, synchronized
selection, accepted-state reload, driving/reference values, rejected-attempt reload,
retained accepted geometry and the isolated `#/dev/lab` route. Mobile behavior remains
outside implementation and acceptance.

## M40 pivot: mechanically qualified constraint editing

Status: complete as of 2026-07-26. UAT-C1-F4 and UAT-C1-F5 are confirmed fixed,
automated requalification is complete and the supervising human approved M40.7.

Goal: make every deterministic constraint-editing transition mechanically testable in
pure Rust before asking a human to assess usability.

Historical prequalification note (2026-07-26, superseded by the approved F4/F5
rechecks): `cargo test --locked --workspace
--all-features`, locked WASM check, release Trunk build and isolated `e2e/m40.mjs`
all pass. The browser suite covers staged-design unsolved state, accepted edits,
projected constrained drag/preview, dimensions, accepted redundancy, rejected conflict,
history, reload and independently checksummed finding downloads. `docs/M40_UAT.md`
preserves the prepared scorecard; M40 remained open at that time for
supervising-human findings and explicit approval. This was the
prequalification state before the findings below invalidated it.

UAT finding UAT-C1-F1 (2026-07-26): the first human pass was stopped because staged
geometry had no live preview, polyline completion was unclear and SVG letterboxing
offset exact endpoint clicks. The targeted fix adds live previews for every core draft,
an in-canvas polyline completion guide with button/double-click/Enter paths, correct
screen-to-SVG conversion and endpoint-ID snapping for lines/polylines. `e2e/m40.mjs`
regresses preview, completion, letterboxed coordinates and topology reuse. The human
targeted recheck was required at that stage and subsequently passed.

UAT finding UAT-C1-F2 (2026-07-26): point dragging showed `Sketch unsolved` and did
not move accepted geometry until pointer release. The targeted fix rebuilds each pointer
move as an isolated retained-session drag request, renders only independently accepted
projected candidates, retains the last valid preview on rejection and commits one normal
design/history edit on release. Escape and pointer cancellation discard the preview.
`e2e/m40.mjs` now checks unconstrained and horizontal-constrained positions before
release, including live projection onto the constraint manifold. A human targeted
recheck was required at that stage and subsequently passed.

UAT finding UAT-C1-F3 (2026-07-26): the next pass found broken canvas multiselection,
impractically narrow line selection, point clicks being submitted as tiny drags, and an
arc preview whose sweep and moving endpoint disagreed with accepted geometry. The fix
makes point selection stable at pointer start, supports Shift/Ctrl/Meta extension, adds
14 px invisible curve hit targets, requires 3 px of motion before drag preview or commit,
and derives preview and commit from the same model-space counterclockwise arc endpoint
projection. Browser regressions create point- and line-based constraints from canvas
multiselection, prove click-only geometry is unchanged, and compare projected arc preview
with accepted placement. A subsequent human recheck still could not select lines for
constraints. This invalidates the prior automated-prequalification claim and triggers
the headless-editor pivot below rather than another targeted DOM patch.

### Post-M40 headless interaction ownership audit (2026-07-26)

#### Requirements

- The architecture rule assigns every behavior that changes an editing gesture's meaning
  or progression—including remembered identities, candidate ranking, tolerance-boundary
  transitions, guides/previews and confirmation—to `geosolve-constraint-editor` with
  native replay. Hosts may normalize platform input, render DTOs, store state and register
  events, but may not recreate assistance (`ARCHITECTURE.md`, *Headless
  interaction-intelligence rule*).
- The reusable acceptance rule is prospective: it does not reopen completed M40.1-M40.6
  or create an M40 gate. M40.7 required supervising-human approval
  (`ACCEPTANCE.md`, *Post-M40 reusable interaction acceptance rule*).

#### Evidence and source pointers

- **Default workbench (the default route): allowed platform/render/storage ownership.**
  `workbench::wasm::install_canvas`, `install_keyboard` and `pointer_input`
  (`crates/geosolve-demo-web/src/workbench/mod.rs`) register DOM events and normalize
  letterboxed browser coordinates into `PointerInput`; `scene.rs` renders headless
  scene/preview/selection DTOs and accepted sketch state, while `panels.rs` renders public
  sketch-document data and editor-provided interaction state; `persistence.rs` serializes
  coordinator checkpoints; `platform.rs` supplies `Window`. `routing::parse` selects `Workbench`
  unless the hash is exactly `#/dev/lab`.
- **Default workbench: headless semantic-policy ownership.** `ConstraintEditor` owns
  deterministic scene/picking, selection, tool/draft progression, endpoint snap,
  drag threshold, projected-preview request/result and construction effects
  (`crates/geosolve-constraint-editor/src/lib.rs`: `EditorScene::hit_test`,
  `ConstraintEditor::{pointer_down,pointer_move,pointer_up,projected_drag_result,
  complete_draft}`). `RetainedEditorCoordinator` owns projected-solve acceptance,
  applicability/disabled reasons, mutations, history, lifecycle and replay
  (`coordinator.rs`: `resolve_projected_point_move`, `actions`,
  `apply_editor_effect`, `undo`, `redo`, `replay`). The default adapter dispatches
  those effects rather than deriving candidates or geometry (`workbench/mod.rs`:
  `dispatch_effects`, `perform_action`).
- **Evidenced default-workbench semantic-policy exception (one narrow case).**
  `workbench/effect_adapter.rs::dispatch_construction_effect` retains a construction
  preview after a failed commit by maintaining `failed_commit` and suppressing the
  following `ClearConstructionPreview`. That failure-dependent preview-lifecycle
  decision is semantic interaction state outside the headless crate, even though it
  preserves the intended retry behavior. No other default-workbench exception was
  evidenced in the inspected allowed sources.
- **Legacy developer lab: isolated, non-default, nonconforming legacy ownership.**
  `lib.rs::wasm::start` installs `playground::wasm` only for the explicit
  `#/dev/lab` route and otherwise installs the workbench; `workbench/routing.rs` has a
  regression proving that default hashes select `Workbench`. The lab's
  `playground.rs::PlaygroundState` is a parallel web-owned interaction state machine:
  `PointerGesture`, `InferenceProposal`, `set_tool`, `draw_click`, `create_point`,
  `select_at`/`hit_test`, `begin_*`, `update_gesture`, `end_gesture`, `apply_constraint`,
  `apply_dimension`, `apply_branch_state`, `confirm_inference`, and
  `undo`/`redo` own gesture progression, hit ranking, tolerance/drag behavior,
  inference, branch choices, previews and commits. It therefore does not satisfy the
  reusable headless rule, but is deliberately isolated from the default M40 workbench.

#### Decisions / inferred constraints

- **Default-workbench adherence: high, approximately 85--95% by ownership area, not a
  measured line/action percentage.** The audit counted eight meaningful semantic areas
  (scene/picking/selection; tools/drafts/snapping; drag threshold; projected preview;
  commit/cancel; action applicability; retained lifecycle/history; replay). Seven are
  owned by the editor/coordinator; the failed-construction-clear disposition is the one
  remaining adapter-owned area. This band communicates the narrow exception without
  claiming false precision.
- **Developer-lab adherence: nonconforming legacy tier (effectively no reuse of the
  headless interaction state machine for its own policies).** Its isolation and explicit
  route mean this is not evidence that the default workbench changes behavior by host.
- Smallest future migration: extend the headless construction-effect protocol with a
  commit acknowledgement/failure disposition so `ConstraintEditor` decides whether a
  terminal preview clear is consumed; then make `effect_adapter` a literal renderer of
  that decision. Do this as prospective interaction cleanup, without reopening M40.1--
  M40.6 or adding an M40 gate.

#### Open questions

- None within the permitted source area. The later supervising-human M40.7 approval
  closes the milestone without changing this audit's prospective ownership finding.

#### Out of scope

- No claim is made about uninspected routes, future assistance, or a migration of the
  intentionally isolated `#/dev/lab` playground.

### M40.1: headless editor contract and transition inventory

Status: complete as of 2026-07-26.

- [x] Accept ADR 0029 and add `geosolve-constraint-editor` as a one-way public consumer
  over `geosolve-sketch`.
- [x] Assign normalized input, viewport, scene, hit testing, selection, drafting,
  gestures, applicability, typed effects, lifecycle and replay to the headless crate.
- [x] Keep rendering, widgets, accessibility, storage and platform event registration
  outside the crate; keep equations and accepted-state authority in `geosolve-sketch`.
- [x] Replace the original M40 gate with ordered M40.1-M40.7 qualification.

Gate: crate ownership and dependency direction are explicit; every current web-owned
interaction responsibility has one planned headless or presentation owner.

### M40.2: accepted scene, picking and selection foundation

Status: complete as of 2026-07-26.

- [x] Build deterministic screen-space points and semantic curve spans from accepted
  documents using only public immutable curve evaluation.
- [x] Add validated viewport transforms and configurable pixel-space pick tolerances.
- [x] Resolve hits by point priority, distance and persistent identity without DOM or
  CSS hit geometry.
- [x] Add ordered replace/toggle selection with Shift/Ctrl/Command semantics.
- [x] Add a 3 px point click-versus-drag state transition with typed preview, commit
  and cancellation effects.
- [x] Expose compatible fixed, coincident, horizontal, vertical, parallel,
  perpendicular and equal-length actions as ordinary public `DocumentEdit`s.

Gate: native tests select a line 6.5 px off its centerline, prioritize endpoints,
multiselect two spans into a parallel edit, prove sub-threshold clicks emit no geometry
work and validate viewport/error boundaries without a browser.

Completion notes: `geosolve-constraint-editor` now provides `EditorScene`, `Viewport`,
`ConstraintEditor`, persistent point/span selection, normalized pointer transitions,
core action applicability and typed host effects. Five native tests pass. This does not
qualify the existing browser, which still owns duplicate interaction logic.

### M40.3: headless drafting, snapping and projection gestures

Status: complete as of 2026-07-26.

- [x] Add tool activation and complete point, line/polyline, rectangle, circle and arc
  draft state machines with exact completion/cancel transitions.
- [x] Add deterministic endpoint snapping and shared preview/commit construction
  proposals without duplicating sketch equations.
- [x] Add point-drag preview request/result transitions with last-valid-preview,
  release and cancellation semantics.
- [x] Exhaustively test every tool stage, threshold boundary, modifier, pointer ID,
  invalid input and interruption sequence natively.

Gate: generated transition sequences cannot mutate accepted state from an incomplete or
cancelled draft, cannot commit a click as a drag and cannot disagree between preview
and commit operands.

Completion notes: typed `ConstructionProposal`s lower through public atomic document
transactions; all core draft tools, persistent endpoint snapping and correlated
projected-drag request/result transitions are headless. Seventeen native tests cover
tool stages, exact gesture/snap boundaries, modifiers, pointer interruption, non-finite
input, invalid-draft recovery, zero-sweep arcs and preview/commit operand identity.
Focused tests and warnings-denied Clippy pass.

### M40.4: headless edit lifecycle, history and diagnostics

Status: complete as of 2026-07-26.

- [x] Coordinate retained design, attempted candidate and accepted state through
  public `RetainedSketchDocumentSession` APIs.
- [x] Add constraints, dimensions, delete, suppression, undo/redo and stale-revision
  transitions with exact action applicability.
- [x] Publish presentation DTOs for lifecycle, problems, selection, action enablement,
  accepted measurements and audit identity without core-report interpretation in UIs.
- [x] Add deterministic replay/model tests for conflicts, redundancy, rejected edits,
  retained accepted geometry, history and reload inputs.

Gate: native state-machine scenarios cover every objective core scorecard workflow and
prove accepted/design/attempt identities, rollback, history and available actions.

Completion notes: `RetainedEditorCoordinator` owns retained lifecycle coordination,
revision-bound effects, opaque checkpoint history, action applicability, core
dimensions, cascading delete/suppression, replay, coherent problem/audit identities and
provenance-checked measurements. Thirty-two editor tests cover accepted/rejected intent,
stale effects, preview provenance, history/reload revision non-reuse and transcript
replay. Accepted redundancy remains deliberately unavailable until sketch provides a
stable domain DTO; the editor does not inspect core reports to fabricate one.

### M40.5: thin desktop web adapter

Status: complete as of 2026-07-26.

- [x] Replace workbench selection, tools, drafts, gesture thresholds, constraint
  compatibility, history orchestration and lifecycle inference with editor inputs and
  returned state/effects.
- [x] Render editor scene primitives and persistent identities without authoritative
  DOM/CSS hit geometry.
- [x] Keep only browser event translation, SVG presentation, accessibility, storage,
  files and evidence capture in `geosolve-demo-web`.
- [x] Delete superseded duplicate workbench state rather than retaining fallback paths.

Gate: dependency and source checks prove the web adapter does not reimplement headless
policy; native editor tests and focused fresh-profile browser adapter tests pass.

Completion notes: the workbench now owns one `RetainedEditorCoordinator`, translates
DOM pointer/modifier/widget input into editor inputs, dispatches every typed editor
effect through the coordinator, and renders editor scene/selection/lifecycle/action and
audit DTOs. The browser retains only SVG/HTML formatting, event registration,
`localStorage`, routing, downloads and environment evidence. The duplicate
`app_state.rs`, `domain_adapter.rs`, `selection.rs` and `tools.rs` policy modules,
index-based selection and CSS-authoritative curve hit paths are deleted. Thirty-two
native editor tests, the locked WASM check, release Trunk build, source-policy checks and
fresh-profile `e2e/m40.mjs` adapter qualification pass. Full scorecard-action and
native/WASM parity coverage remains M40.6; strict workspace Clippy currently exposes
unrelated legacy-playground warnings that must be cleared before M40.6 closes.

### M40.6: automated core-interaction qualification

Status: complete as of 2026-07-26.

- [x] Add generated/model-based transition coverage, deterministic replay corpus and
  native/WASM parity for all core tools and actions.
- [x] Run exact boundary, overlapping-hit, scale/viewport, cancellation, malformed
  input, persistence and accepted-retention matrices.
- [x] Keep browser automation focused on platform wiring, rendered identity,
  accessibility, storage and downloadable evidence.
- [x] Produce a machine-readable coverage matrix linking every UAT action to native
  state-machine and browser-adapter evidence.

Gate: all objective M40 workflows pass the primary native oracle, locked WASM and thin
browser adapter without retries; no scorecard action relies only on browser E2E or UAT.

Completion notes (2026-07-26): a checked-in transition corpus and seeded bounded model
execute all frozen creation, snapping/picking/selection, constraint, dimension,
projected-drag, delete/suppression/history, conflict/repair, lifecycle, redundancy,
persistence, malformed-input and boundary classes. Native tests validate the corpus,
the all-covered machine matrix and canonical golden report; the release WASM exports
the same runner and Chromium compares its report byte-for-byte. Dimension-family
selection is coordinator-owned, and accepted redundancy is a provenance-bearing
`geosolve-sketch` DTO rather than editor/browser report interpretation. The thin
browser suite covers only platform normalization, rendered persistent identity,
accessibility, storage, lifecycle and checksummed evidence downloads and passes 14/14.
Formatting/diff checks, warnings-denied locked workspace Clippy, full locked workspace
tests, locked WASM check and the supported release Trunk build all pass.

### M40.7: human UAT 1 - core sketch interaction

Status: complete as of 2026-07-26; UAT-C1-F4 and UAT-C1-F5 are confirmed fixed and
the supervising human explicitly approved the milestone.

UAT finding UAT-C1-F4 (2026-07-26): while manipulating constrained geometry, live
projected previews remained valid and followed the intended nearby solution, but
pointer release sometimes re-solved onto a different valid configuration. Release must
commit the exact accepted preview branch rather than reconstructing the edit from the
raw pointer target or another warm start. The review is stopped until a deterministic
headless regression covers preview-to-commit geometry/branch continuity and the
targeted release path is requalified.

Remediation note (2026-07-26): the coordinator now privately retains the exact
same-design independently accepted preview session and uses its complete continuous
state as the final solve warm start while applying only the point-position design edit.
Commit is dispatched before preview clearing; rejected provenance/identity checks do
not mutate the retained session and preserve a valid preview for retry. The canonical
two-link regression proves that the former cold release selects materially different
geometry, while the preview-seeded release preserves every point within `1e-10`, keeps
explicit branches and adds exactly one Undo checkpoint. Focused sketch/editor tests,
formatting, warnings-denied locked workspace Clippy, full locked workspace tests, the
locked WASM check, supported release Trunk build and isolated `e2e/m40.mjs` all pass;
the release-browser suite reports 14/14. The targeted human recheck was then reopened
and subsequently passed.

UAT finding UAT-C1-F5 (2026-07-26): after confirming the constrained release no longer
jumped, human review found four related drafting-preview defects: open polyline
previews showed implicit filled area, circle previews omitted their center, arc drafts
showed no center/radius guidance before a complete three-point arc existed, and Finish
could leave the last unplaced polyline segment visible. The shared cause was using only
a complete committable `ConstructionProposal` as preview state, styling every draft SVG
shape as fillable, and duplicating terminal commit/clear effect construction.

Remediation note (2026-07-26): `ConstructionPreview` is now a distinct typed,
non-committable DTO with complete-proposal, retained-anchor and arc-radius-guide stages.
Circle and arc centers render explicitly, arc center-to-start guidance exists before
the complete normalized arc preview, and every provisional construction is wire-only;
area fill remains accepted-profile presentation. One shared terminal helper emits
commit before clear for pointer completion, Finish, Enter and double-click routes.
Native editor tests pass 45/45, the web library tests pass 91/91, the locked WASM check
and supported release Trunk build pass, and `e2e/m40.mjs` passes 14/14 with focused
render/lifecycle assertions. The targeted UAT-C1-F5 recheck was then reopened and
subsequently passed.

Second-pass consistency note (2026-07-26): complete previews now retain resolved
accepted operand positions alongside persistent retained-design point IDs. Construction
apply derives line/polyline branches and arc scalar seeds from those exact snapshots,
so a rejected retained-design point edit cannot make the visible accepted preview and
the eventual commit disagree. Removed retained IDs remain visible/pickable from the
accepted scene but cannot snap. Browser arc serialization has direct minor/major CCW
flag coverage, and adapter dispatch now retains a complete preview when construction
commit fails while successful commit still clears it. Formatting/diff checks,
warnings-denied locked workspace Clippy, full locked workspace tests, locked WASM
check, release Trunk build and `e2e/m40.mjs` 14/14 pass. The supervising human
subsequently passed the targeted recheck and approved M40.7.

- [x] Provide one URL, deterministic resets and a 30-45 minute core drafting script.
- [x] Restrict human review to discoverability, manipulation intent and clarity of
  accepted, preview, unsolved and rejected states.
- [x] Convert any objective finding into a headless regression first, then perform only
  the necessary targeted human recheck.
- [x] Obtain explicit supervising-human approval.

Gate: the supervising human approves the core interaction scorecard and no correctness,
data-loss, misleading-state or basic-usability blocker remains.

Completion note (2026-07-26): after the mechanically requalified F4/F5 remediations,
the supervising human passed M40.7. No unresolved correctness, data-loss,
misleading-state or basic-interaction blocker remained, so M41 began.

## M41: construction roles and activation

Status: complete as of 2026-07-27.

Goal: represent construction and configuration semantics as explicit domain state.

- [x] Add persisted regular/profile and construction geometry roles.
- [x] Keep construction geometry fully constrainable while excluding it from production profiles by default.
- [x] Generalize activation over entities, constraints, dimensions and associations.
- [x] Distinguish user suppression, host-configuration inactivity, unavailable dependency and unavailable external reference.
- [x] Validate the effective active dependency closure and report every inactivity reason.
- [x] Preserve branch, span, winding, contact and ownership state while inactive and across reactivation.

Gate: activation changes are atomic, never evaluate dangling dependencies and never
infer a new branch from retained coordinates.

Completion note (2026-07-27): M41 added closed profile/construction roles, immutable
revisioned host activation, one deterministic typed dependency closure, activity-aware
lowering/profile/branch/ownership consumers and activation revision/digest lifecycle
stamps. Frozen v1-v4 bytes remain unchanged for representable state; supported v4
encoding rejects non-default M41 state and the draft-v5 codec remains explicitly
unsupported. Focused M41 tests, independent verification, locked all-feature workspace
tests, warnings-denied workspace Clippy, the WASM check and the release Trunk build all
passed. M42 and M43 subsequently completed.

## M42: typed host parameters

Status: complete (2026-07-27).

Goal: let host-owned expression/configuration systems drive sketches through finite
typed values without becoming solver variables or a second expression language.

- [x] Add persistent parameter identities and typed length, angle, dimensionless and activation input bindings.
- [x] Accept immutable revisioned parameter batches and map dependencies to affected sources/components.
- [x] Allow one parameter to drive multiple dimensions without adding an artificial unknown.
- [x] Return declared reference measurements as revision-stamped output proposals with units and provenance.
- [x] Reject input/output ownership cycles and stale parameter commits atomically.
- [x] Keep expression parsing, unit display and configuration dependency graphs host-owned.

Gate: identical design and parameter bytes reproduce identical accepted geometry and
diagnostics; rejection changes no accepted input or output revision.

Completion note (2026-07-27): M42 added persistent typed parameter declarations,
bindings and reference-output proposals; canonical immutable revision/digest batches;
activation-first resolution; fixed-coefficient driving dimensions; and deliberately
declared dimensionless fixed-scalar targets with exact runtime/audit provenance and no
parameter solver unknown. Domain, unit, branch, ownership, duplicate-supplier, stale,
missing, cancellation and rejection paths retain accepted input/output truth atomically.
Draft-v5 round trips M42 state while supported v1-v4 bytes remain frozen and v4 export
rejects non-default state. Focused M36/M42 tests, independent review, locked all-feature
workspace tests, warnings-denied workspace Clippy, the WASM check and release Trunk build
all passed. M43 subsequently completed.

## M43: immutable external 2D references

Status: complete as of 2026-07-27.

Goal: constrain native sketch geometry against other sketches or model geometry
without callbacks, hidden fixed copies or coordinate-based repair.

- [x] Persist stable local external-binding identity and expected feature kind.
- [x] Accept immutable finite 2D point/curve snapshots carrying revision, digest, domain, orientation, scale and resource evidence.
- [x] Integrate external features into the same typed operand and audit system as native geometry without adding solver variables.
- [x] Require explicit rebinding/remapping for family, span or topology changes.
- [x] Report missing, stale, duplicate, wrong-kind, non-finite, oversized and incompatible snapshots as typed unsolved-design outcomes.
- [x] Keep arbitrary host/PDM keys and 3D projection computation outside sketch equations and canonical sketch state.
- [x] Let diagnostic capsules bundle design, parameter and snapshot inputs for reproducibility without making stored status authoritative.

Gate: one attempt validates against exactly one immutable snapshot set and records its
revision/digest; no host callback or proximity inference participates in solving.

Completion record (2026-07-27): M43 added monotone document-local external bindings
and a closed v1 snapshot vocabulary for points and directed single-span line segments.
Canonical bounded snapshot sets validate exact revision/digest, topology, orientation,
domain, scale and resource evidence before lowering. Native point coincidence and line
collinearity consume external geometry only as immutable fixed coefficients, publish
complete binding/snapshot audit provenance and pass independent residual validation;
external geometry never becomes a solver unknown or hidden native copy. Missing, stale,
duplicate, wrong-kind, malformed and topology-incompatible inputs remain typed attempts,
propagate the M41 unavailable-dependency closure and retain prior accepted bytes and
input stamps atomically. Explicit rebinding is the only topology recovery path.

Draft-v5 and the unstable diagnostic capsule reproduce exact design, parameter,
activation and snapshot evidence without importing stored status or geometry authority;
frozen sketch v1-v4 remains unchanged. Ten focused M43 regressions cover canonical
validation, solving, stamps, activity precedence, rollback, rebinding, audit and finite
differences. Independent review, formatting/diff checks, warnings-denied locked workspace
Clippy, full locked workspace tests, locked WASM, release Trunk and browser E2E passed.
M44 subsequently completed; at that time M45 became active. The M45 status below
supersedes this dated handoff sentence.

## M44: host-state workbench integration

Status: complete as of 2026-07-27. Implementation and focused M44 qualification pass;
the supervising user explicitly removed the costly legacy full-M14 carry-forward run
from the M45 preparation gate rather than treating its incomplete runs as passing evidence.

Goal: expose construction, activation, parameters, references and dual-state behavior
coherently through the CAD-like desktop consumer.

- [x] Add construction styling and explicit profile participation controls.
- [x] Add activation/suppression editors distinct from driving/reference dimension mode.
- [x] Add parameter inputs, bindings and output proposals without exposing internal design scalars indiscriminately.
- [x] Add external-reference tree entries, styling, revision/digest status and explicit rebind workflows.
- [x] Display design, attempted and accepted revisions together whenever they differ.
- [x] Prove atomic batch updates, stale/missing inputs and accepted-state retention in browser automation.

Gate: every host-state workflow is objectively qualified before the second human UAT
checkpoint; the browser still contains no equation or host callback.

Qualification note (2026-07-27): narrow coordinator wrappers and deterministic
workbench fixture sidecars expose canonical M41 roles/activity, complete immutable M42
parameter batches/proposals, M43 snapshot status/rebinding and concurrent
design/attempt/accepted stamps without browser equations, host callbacks or canonical
workspace pollution. Focused coordinator tests (26), the web suite (103), format/diff,
warnings-denied locked workspace Clippy, full locked workspace tests, locked all-feature
WASM check, release Trunk, preserved M40 browser qualification (14/14) and fresh-profile
M44 browser qualification (6/6) passed. Three standalone carry-forward M14 runs exposed
two tower burst-drag overruns (`130 ms` and `118 ms` against the unchanged `100 ms`
budget) and one 30-second CDP mouse-event timeout. Isolated comparison localized the
overrun to unconditional M41 dependency-closure traversal in the all-active case;
`SketchDocument` now skips that traversal when there are no direct inactivity reasons.
Five isolated post-correction tower runs measured `62`, `42`, `37`, `39` and `44 ms`,
comparable to clean-HEAD measurements of `66`, `38`, `43`, `41` and `38 ms`. Focused
M41 tests, the release native tower test, formatting and release Trunk build passed after
the correction. A full post-correction M14 run was stopped by the supervising user after
desktop layout and has no final-pass result. No threshold was weakened. Per explicit
supervising-user direction at that time, further flaky full-M14 work was deferred until
the M45 UAT preparation window; M44 remained active pending a later scope decision and
explicit supervising-human participation.

Completion addendum (2026-07-27): during M45 preparation the release WASM build and the
focused 103-test web suite passed, and fresh-profile M44 browser qualification passed all
six frozen host-state groups, including the M45 finding package. A full M14 attempt first
found that `chromium` was unavailable in the copied shell and, with an explicit Chrome
binary, was then stopped after desktop layout. It still has no final-pass result. The
supervising user explicitly authorized avoiding these costly legacy E2E runs and
fast-tracking the already-focused host-state candidate into M45 UAT. This is a scope
decision, not M14 passing evidence: no threshold or assertion changed, and the historical
suite remains available for later retirement or targeted replacement. M44 is complete on
its focused native/WASM/browser evidence; at that time M45 entered pre-UAT cleanup. The
M45 completion record and M46-M53 cleanup sequence below supersede that dated handoff.

## M45: cleanup investigation and UAT-point capture

Status: complete as of 2026-07-27. This milestone does **not** record human UAT approval.
The previously prepared host-semantics review was archived and relocated to post-cleanup
M53 so cleanup can replace its broad fixture and obsolete browser infrastructure first.

Goal: establish the source-backed cleanup boundary without losing intended host-semantics
coverage.

- [x] Preserve ten host-semantics verification points independently of the temporary fixture.
- [x] Establish that the workbench is the default application while `#/dev/lab` remains a separate legacy runtime.
- [x] Classify all 92 legacy inline consumer tests and every M14/M40/M44 E2E group.
- [x] Define the direct-test replacement layers and explicit-retirement policy.
- [x] Record the incomplete M14 history without calling it passing evidence.

Gate: the archived M45 capture now consolidated in `docs/M53_UAT.md`, plus
`docs/M45_CLEANUP_PLAN.md`,
`docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md`,
`docs/M45_UI_CLEANUP_INVESTIGATION.md`, `docs/M46_DIRECT_TEST_REPLACEMENT.md` and
`docs/M46_REBASE_INVENTORY.md` preserve the evidence and make no human-approval claim.

Independent review found the inventory sufficient to guide implementation. The focused
demo-web suite passed 103/103 and `git diff --check` passed; no browser suite was run for
the investigation. Workspace-wide Clippy reproduced the disclosed linkage lint during
M45; the M46 follow-up cleared it without changing behavior.

## Post-cleanup numbering record

M46-M53 are the completed cleanup and host-semantics UAT sequence. M54-M60 subsequently completed
the functional sequence, M61 completed the advanced human UAT gate and M62 completed approved
CAD-style authoring; M63 completed approved canvas-constraint presentation. On 2026-07-29 the
supervising user withdrew the previously forecast M62-M64 hardening sequence so that additional
UI, cleanup and product milestones can be scoped one at a time. M64 now owns only the editable
sample-library cleanup defined below and inherits no withdrawn hardening scope.

## Pre-cleanup phase

### M46: direct-test ownership freeze

Status: complete as of 2026-07-27.

Goal: give every retained assertion from the old E2E/demo stack one authoritative direct
unit or integration-test owner before deletion begins.

- [x] Inventory all M14, M40 and M44 E2E groups, static scans and legacy inline tests.
- [x] Freeze one deletion ledger marking each assertion as domain, editor, workbench presentation, persistence, WASM-adapter or explicit retirement.
- [x] Ban new CDP, served-page, DOM-scraping, screenshot-diff and wall-clock browser tests.
- [x] Decide deterministic finding capture as cleanup/UAT test infrastructure over public APIs, not a stable product API.
- [x] Add named regression targets for every ledger entry that lacks a direct owner.
- [x] Clear the workspace Clippy failure before implementation milestones claim completion.

Gate: every old E2E assertion has a named direct owner or a reviewed explicit-retirement
reason; no replacement relies on a browser process, HTTP server, CDP or substring scan.

Completion notes: `docs/M46_DIRECT_TEST_REPLACEMENT.md` freezes unconditional direct-owner
or reviewed-retirement dispositions for all M14/M40/M44 E2E groups, static scans and the
92 legacy inline consumer tests. Finding capture is test/UAT-only infrastructure; proposed
replacement tests remain assigned to M47-M49, and M46 deletes nothing. A behavior-preserving
match-guard rewrite cleared the workspace linkage Clippy lint. Formatting, diff and shell
syntax checks, warnings-denied workspace Clippy, the complete locked all-feature workspace
suite and the locked WASM check pass. Independent read-only review found no ownership or
deletion-gate blocker. No browser E2E was run or used as current evidence.

### M47: focused host-state replacement and M44 purge

Status: complete as of 2026-07-28.

Goal: replace the broad temporary M44 fixture with small direct fixtures and remove its
browser qualification stack.

- [x] Add focused role/profile/activity presentation tests.
- [x] Add parameter/binding/proposal stamp and invalid/stale recovery tests.
- [x] Add external snapshot/rebind and retained-evidence tests.
- [x] Add lifecycle design/attempt/accepted identity tests.
- [x] Add deterministic typed finding-package tests over public domain/audit APIs.
- [x] Delete the broad host fixture, fixture-only controls, `e2e/m44.mjs` and its CDP/profile/server infrastructure.

Gate: all six former M44 groups and all ten preserved UAT points have passing direct
owners; no M44 browser fixture or E2E artifact remains.

Completion notes: five direct Rust fixture groups in workbench panels, accepted-scene
rendering and evidence serialization now cover role/profile/activity, atomic parameter
bindings and proposal provenance, external snapshot retention and explicit rebind,
design/attempt/accepted lifecycle separation, and checksummed typed host finding content.
They preserve all six former M44 groups and all ten M45 UAT points without a reusable or
persisted aggregate host fixture. `workbench/host_state.rs`, fixture-only actions/markup,
`e2e/m44.mjs` and its M44 CDP/server/profile machinery are deleted; no equations or
canonical persistence behavior changed. The focused M41-M43 suites, 101-test all-feature
demo-web suite, complete locked all-feature workspace suite, formatting, diff,
warnings-denied workspace Clippy and locked all-feature WASM check pass. No browser E2E
was run. M40/M14 infrastructure remains only for its assigned M48/M50 deletion gates.

### M48: direct workbench qualification and M40 purge

Status: complete (2026-07-28).

Goal: replace root-workbench browser qualification with direct editor, presentation,
persistence and WASM-adapter tests.

- [x] Keep the native M40 transition corpus and byte-identical golden oracle.
- [x] Directly test creation effects, coordinate conversion, selection identity, glyph/dimension DTOs, lifecycle, redundancy and conflict retention.
- [x] Directly test persistence codecs/fallbacks, semantic accessibility markup and evidence-package serialization.
- [x] Replace source substring scans with executable typed-API use and module ownership.
- [x] Explicitly retire browser-only keyboard/focus delivery, DOM layout, download/blob and reload mechanics.
- [x] Delete `e2e/m40.mjs`, `scripts/serve-m40.sh` and all M40 CDP/profile/server/download infrastructure.

Gate: every retained M40 contract passes at its direct owner and no M40 browser E2E or
source-policy scan remains.

Completion notes: the unchanged 53-test editor suite retains the native M40 corpus,
canonical report bytes and completeness oracle. Pure workbench tests now own letterboxed
client-coordinate normalization, construction-effect dispatch, persistent scene/tree
selection identity, unique constraint glyphs, dimension mode/value DTOs, lifecycle/problem/
redundancy semantics, the exact production persistence codec and malformed fallback, semantic
markup, deterministic evidence serialization and a test-only adapter comparison with the
authoritative M40 report/checksum. Browser-only focus/keyboard delivery, DOM/layout/reload,
download/blob observation and source-substring scans are retired rather than imitated.
`e2e/m40.mjs`, `scripts/serve-m40.sh` and M40-only runtime markers/actions are deleted;
`e2e/m14.mjs`, `scripts/serve-m45.sh`, the legacy playground and its shared platform
dependencies remain for M49-M50. The 111-test all-feature demo-web suite, complete locked
all-feature workspace suite, formatting, diff, warnings-denied workspace Clippy, locked
all-feature WASM check and release Trunk build pass. No browser E2E was run.

### M49: legacy semantic extraction

Status: complete (2026-07-28).

Goal: move useful assertions out of the old playground before deleting the application.

- [x] Move all class-A domain/transaction/branch/history/sampling/profile assertions into sketch, linkage, editor, persistence or focused presentation tests.
- [x] Confirm the three class-C duplicates against their cited native tests.
- [x] Explicitly retire class-B/D legacy delivery/layout assertions and class-E capabilities not present in the cleanup-era product scope.
- [x] Preserve advanced mathematical behavior in native domain tests without preserving its old demo controls.
- [x] Replace any retained capsule/file semantics with direct codec tests; retire file-picker/download delivery.
- [x] Produce a zero-unowned-assertion ledger for `e2e/m14.mjs` and the 92 legacy inline tests.

Gate: every retained semantic claim passes outside the legacy runtime and every retired
claim has an explicit rationale; the old UI can be deleted without coverage ambiguity.

Completion notes: `docs/M49_IMPLEMENTATION.md` reconciles all 13 M14 browser groups and
all 92 legacy inline tests with zero unowned claims. Direct sketch, linkage, editor and
focused workbench owners preserve retained geometry, continuation, transaction, inference,
diagnostic and accepted-snapshot semantics; delivery/layout/browser claims and private
adaptive-render policy are explicitly retired. The complete locked all-feature workspace
suite, formatting, diff, warnings-denied workspace Clippy, all-feature WASM check and release
Trunk build pass. Independent read-only verification passed after confirming that M49 leaves
`e2e/m14.mjs`, the playground, route, CSS, serving and release-gate infrastructure unchanged.
No browser E2E was run.

## Cleanup cut

### M50: old E2E and legacy application purge

Status: complete (2026-07-28).

Goal: remove the old application and every old E2E mechanism in one reviewable deletion
after M46-M49 establish direct coverage.

- [x] Delete `e2e/m14.mjs` and the remaining `crates/geosolve-demo-web/e2e/` directory.
- [x] Delete Chromium/CDP launch code, HTTP test servers, profiles, timing/retry helpers and download interception.
- [x] Delete `#/dev/lab`, `src/playground.rs`, legacy/frozen application branches, hidden DOM, obsolete CSS and legacy persistence/capsule glue.
- [x] Delete legacy-only inline tests, selectors, fixture markers and stale serving/UAT scripts.
- [x] Remove dead dependencies, features, generated artifacts and release-gate E2E invocations.
- [x] Update every repository document to describe the single-workbench/direct-test architecture.

Gate: reviewed source/script search finds no executable or current instruction for old E2E,
CDP, the legacy route/application, broad fixtures or obsolete serving; historical evidence is
explicitly archival. Direct native/WASM tests, formatting and warnings-denied Clippy pass.

Completion notes: the final M14 Node/CDP suite, its `e2e/` directory, the separately routed
playground runtime, hidden DOM/CSS, legacy inline tests, serving scripts and release-gate
browser invocation are deleted. Demo-web now starts exactly one non-authoritative workbench
through public sketch/editor APIs. Dead geometry/linkage/`js-sys` dependencies and obsolete
`web-sys` features were pruned while release Trunk remains in the gate. Reviewed searches find
no executable legacy route, E2E, Chromium/CDP, profile, server, download or environment
infrastructure; historical records remain explicitly archival.

The 58-test editor suite, 22-test all-feature demo-web suite, complete locked all-feature
workspace suite, formatting, diff, warnings-denied workspace Clippy, all-feature WASM check
and release Trunk build pass. Independent read-only verification passed after one stale-ledger
correction. No browser automation or serving was run.

## Post-cleanup phase

### M51: single-workbench consolidation and hardening

Status: complete (2026-07-28).

Goal: simplify the surviving consumer and make direct tests the only automated
qualification path.

- [x] Consolidate routing, persistence, evidence, presentation DTOs and test fixtures around one workbench.
- [x] Remove compatibility shims and dependencies made dead by M50.
- [x] Keep browser/platform glue minimal and isolate it from testable pure transformations.
- [x] Add direct regressions for every cleanup defect or lost contract found during purge review.
- [x] Verify native and `wasm32-unknown-unknown` consumers without serving or launching a browser.
- [x] Publish the post-cleanup module/test ownership map.

Gate: one workbench remains, all automated qualification is direct, and no cleanup-era
compatibility shim or dead test infrastructure remains.

Completion notes: the survivor now restores only its complete workspace snapshot. The obsolete
design-only local-storage migration, duplicate demo wrapper around the editor's M40 qualification
report and M40 JSON/SVG evidence package are deleted. The retained typed host-evidence serializer,
workspace codec, scene/panels and effect adapter have focused direct owners documented in
`docs/M51_IMPLEMENTATION.md`; the stale M32 distribution copy is removed, while ordinary Trunk
developer configuration and all live dependencies remain.

The 58-test editor suite, 19-test all-feature demo-web suite, complete locked all-feature
workspace suite, formatting/diff, warnings-denied workspace Clippy, all-feature WASM check and
release Trunk build pass. Independent read-only verification found no functional loss and passed
after the active inventory/status wording was corrected. No browser automation or serving was run.

### M52: post-cleanup host-semantics UAT candidate

Status: complete as of 2026-07-28.

Goal: build a minimal disposable UAT composition after cleanup rather than preserving a
fixture as product state.

- [x] Compose small role/activity, parameter/proposal, external/rebind and lifecycle fixtures only for UAT setup.
- [x] Generate deterministic instructions and finding evidence from public domain/audit APIs.
- [x] Requalify all ten preserved host-semantics points through direct automated tests.
- [x] Build the release WASM candidate and verify its public adapter exports without E2E automation.
- [x] Rewrite the archived scorecard against the minimal post-cleanup composition.

Gate: the candidate is reproducible, all objective claims pass directly, and the only
remaining work is supervising-human judgment.

Completion notes: the sole workbench now hosts one explicitly labelled in-memory UAT sidecar
composed from four fixed-identity fixtures. Ten deterministic instructions and typed actions cover
role/activity, parameters/proposals, external snapshots/rebind and retained lifecycle evidence.
M52 finding evidence deliberately omits canonical fixture documents, and the production-used
sidecar state directly proves ordinary-action/save isolation, unchanged exit state and persistence
codec reload. No public API, schema, dependency or `wasm_bindgen` export was added.

The five focused M52 tests, 24-test all-feature demo-web suite, complete locked all-feature
workspace suite, formatting/diff, warnings-denied workspace Clippy, all-feature WASM check and
release Trunk build pass. Independent read-only verification passed after converting canonical
evidence leakage and indirect save/exit checks into regressions over the production state path. No
browser automation, serving or source-substring scan was used. Human clarity, trust and approval
remain exclusively M53 work.

### M53: human UAT 2 - CAD host semantics

Status: complete as of 2026-07-28; supervising-human approval recorded.

Goal: retire trust and comprehension risks around construction, activation, parameters,
external references and retained unsolved intent on the post-cleanup workbench.

- [x] Replace the one-off M52 launcher/action wall with six typed scenario definitions, a nested
  selector and a contextual guide while preserving P1-P10 and ordinary-workspace isolation.
- [x] Requalify the integrated demo-web/WASM/release candidate and record its new identity before
  targeted human review.
- [x] Replace nested per-group disclosures with recursive right-expanding hover/focus flyouts,
  retaining an inline narrow-screen fallback and the same stable scenario definitions.
- [x] Requalify the flyout selector and record M53-S4 before targeted human review.
- [x] Publish current-attempt error attribution through the headless editor in persistent identities,
  using a global scope whenever the domain evidence cannot identify individual elements cleanly.
- [x] Render targeted canvas icons/highlights and global fallback, and add two reusable scenarios
  demonstrating attributed conflict and unattributable input failure with recovery.
- [x] Requalify canvas error attribution and record M53-S5 before targeted human review.
- [x] Assess role/profile and suppression/mode distinctions.
- [x] Assess parameter ownership, proposal provenance and invalid/stale recovery.
- [x] Assess missing/stale/topology/rebind external-reference recovery.
- [x] Assess design/latest-attempt/accepted-state clarity and natural-use coherence.
- [x] Capture findings, convert objective defects into direct regressions and perform only targeted rechecks.
- [x] Record explicit supervising-human approval and disposition of nonblocking concerns.

Gate: the supervising human approves the host-semantics scorecard and no state-trust,
recovery or ownership blocker remains.

Execution record (2026-07-28): `docs/M53_UAT.md` is the consolidated durable M53 session, finding,
change-request and retest ledger. Every observation or UI request must be recorded and classified
before implementation. Objective defects require direct regressions and targeted requalification;
clarity/layout changes require an identified candidate rebuild and affected human retest; future
scope remains in a subsequently approved milestone or an explicit open question. One request did not shadow or silently
close another development concern, question or plan. Candidate identities and any temporary
human-only access details belong exclusively in the M53 ledger; they are not automated gates,
retained server infrastructure or product routes.

M53-P011 owns the selector change: the typed catalog, native regressions and clean integrated
release qualification pass, and M53-S3 records build-source commit `17a4a25` plus its frozen
distribution manifest. The targeted human discoverability/guidance retest passed in the final
M53-S5 approval. The
change presents the already qualified M52 fixture behavior and adds no new host semantic.

M53-P012 supersedes only the S3 selector navigation presentation before human ratings. Recursive
plain-list branches now open immediately to the right on hover or focus, with an inline narrow
fallback. Focused tests and exploratory browser interaction pass; the complete clean release gate
passed from `49ddcb8`, and M53-S4 records distribution manifest
`d2d91ff200a7e55d0e04bb90e863d9c771f10325cb286b5147790bdb8e192b33`. The final M53-S5
supervising-human review rated the flyout navigation Pass.

M53-P013 supersedes S4 before ratings. It owns structured current-error metadata at the public
headless-editor/UI seam, owner-and-operand canvas attribution, global fallback, accessible
non-mutating tooltips and two reusable demonstration scenarios under **Error attribution**. The
implementation and direct native/WASM regressions are complete: conflict targets come only from
attempt mappings/core sources and document dependencies, wrong-kind input remains global, recovery
clears metadata, and accepted geometry remains the only rendered geometry. Clean qualification and
M53-S5 identity are recorded below; the targeted human review passed.

M53-S5 completion note (2026-07-28): the complete clean release gate passes from build-source
commit `f72116b`, including editor 60/60, demo-web 31/31, the 127.85s release-only 256-moving-body
sparse crossover, all-feature WASM, package/licence checks and the Trunk 0.21.14 release build.
`docs/M53_UAT.md` records distribution manifest
`1a96ebe29b5eaa8973b9f726d062be74428f2545763a108a19688913ccaaeadc`; every served file matches
its local SHA-256. The supervising human subsequently rated every scorecard area Pass, reported no
concern or blocker and explicitly approved M53 on 2026-07-28.

## Post-M53 functional and release sequence

### M54: stable diagnostics and mobility evidence

Status: complete.

Scope: publish stable sketch-owned diagnostics, revision identities, structural/numerical
rank and mobility evidence, bounded conflict/repair information, and move direct core
reports behind an unstable seam.

M54 completion note (2026-07-29): `SketchDiagnosticSnapshot` now publishes exact
accepted/attempt provenance and input identities; stable solve, source, component, dependency,
activation, parameter, external-reference and bound evidence; separate structural/numerical rank
and equality/bidirectional/one-sided mobility; completeness-aware conflict/redundancy searches; and
typed non-mutating repair suggestions using persistent document identities. Raw core reports and
bound reports are available only through explicitly named unstable compatibility seams. The
headless editor attributes current problems through stable persistent conflict candidates, and the
sole workbench renders the stable DTO without reconstructing core audit semantics. Nine focused M54
regressions, editor 60/60, workbench 31/31, warnings-denied workspace Clippy, the complete
all-feature workspace test suite, all-feature WASM check and Trunk 0.21.14 release build pass.
`docs/M54_IMPLEMENTATION.md` records the exact API, acceptance and qualification evidence.

### M55: alpha constraint, dimension and branch-action parity

Status: complete as of 2026-07-29.

Goal: expose the complete preserved M13-M14 alpha constraint, dimension and explicit branch-action
surface early through reusable headless-editor policy and the sole visible workbench, without
restoring the deleted playground or moving equations into presentation code.

- [x] Freeze one applicability/action matrix covering the existing core actions plus point-on-curve,
  equal-radius, midpoint, symmetry, generic contact and generic tangency.
- [x] Expose distance, segment-length, radius, diameter and oriented-angle dimensions in both
  driving and reference modes wherever the public document applicability rules permit them.
- [x] Add typed editor actions for every required tangent orientation, contact neighborhood,
  parameter-domain, span and winding choice; never infer a discrete branch from coordinates.
- [x] Render the returned actions, disabled reasons, glyphs, annotations and branch controls in the
  workbench without browser-owned applicability, equations or audit interpretation.
- [x] Add deterministic reusable scenarios for ordinary line/point relations, circular dimensions,
  midpoint/symmetry, point-on-curve and generic contact/tangency with branch editing and rejection
  recovery.
- [x] Directly qualify the complete matrix through native editor/coordinator tests, the WASM adapter
  and focused presentation tests; do not restore old browser E2E, CDP, `/#/dev/lab` or legacy
  harnesses.

Gate: every preserved alpha constraint, dimension and branch action is discoverable and executable
through the headless editor and sole workbench using public sketch APIs, with accepted-state
retention and typed diagnostics on rejection. This is action-surface parity, not restoration of the
old application, mobile behavior or browser-owned solver semantics.

Completion notes (2026-07-29): `geosolve-constraint-editor` now owns one closed, ordered matrix
for 13 alpha relations and five dimension identities in driving/reference modes, including typed
wrong-arity/wrong-kind reasons, explicit contact construction choices and selection-scoped contact
and oriented-angle branch edits. `geosolve-sketch` adds an atomic persistent-ID-preserving complete
contact-branch transaction over semantic span, domain/value, winding, neighborhood and tangent
orientation; accepted-parent seeding restores solver-owned contact scalar values without routing
them through an ordinary scalar edit.

The sole workbench renders and dispatches only those typed actions, choices and reasons, publishes
semantic glyph/dimension attributes and adds the reusable `alpha-parity-catalog` and
`alpha-branch-recovery` leaves under **M55 Action parity**. Seven focused M55 editor tests cover
the complete relation/dimension matrices, driving/reference execution, accepted span/domain
transitions, retained rejected orientation/winding candidates, stable identities, accepted-state
retention and bounded undo recovery. Editor, sketch, workbench, full workspace, all-feature WASM
and release Trunk qualification pass; `docs/M55_IMPLEMENTATION.md` records exact commands and
limitations. No residual equation, schema version, legacy route/harness or mobile claim was added.

#### M55 contextual-authoring follow-up during M61 remediation

Status: complete as of 2026-07-29; approved by the supervising caller on 2026-07-29.

Goal: preserve M55 mathematical coverage while replacing equation-shaped workbench actions with
selection-sensitive authoring intents owned by the reusable headless editor.

- [x] Replace Point-on-curve/Generic-contact with one contextual Coincident intent.
- [x] Replace Equal-length/Equal-radius with one contextual Equal intent and expose existing
  branch-explicit equal curvature where applicable.
- [x] Replace Generic-tangency with Tangent; retain Parallel for line pairs and resolve a
  line-plus-circle/arc Perpendicular / Normal intent to explicit radial centre-on-line incidence.
- [x] Expose existing ordered endpoint Continuity with explicit G0/G1/G2/parametric-C2 choices.
- [x] Publish resolved underlying definition metadata and typed disabled reasons at the headless
  boundary; the browser owns labels and controls only.
- [x] Add direct matrix/lifecycle/presentation/WASM regressions and reusable UAT demonstrations
  without restoring `/#/dev/lab`, browser E2E or a legacy harness.

`docs/M55_CONTEXTUAL_AUTHORING.md` freezes the approved dispatch matrix and explicit branch rules.
M36/M37 semantic-catalog-only relations and a new arbitrary curve-pair angle residual remain
outside this follow-up until their retained lifecycle/schema or mathematical contracts are
deliberately implemented in milestone order.

Completion notes (2026-07-29): the reusable headless editor now publishes eleven
`ConstraintIntent` identities and the selection-resolved `ResolvedConstraintKind` plus explicit
`ConstraintRelationChoice`. Curve picking retains the selected parameter for contact seeding, and
endpoint continuity resolves Start/End to exact bounded endpoint parameters. The workbench exposes
only the compact contextual vocabulary and renders the resolved relation, choice progression and
typed disabled reason returned by the editor.

The existing `alpha-parity-catalog` scenario demonstrates equal curvature and endpoint continuity;
`alpha-branch-recovery` creates its impossible contact through Coincident; and
`circle-tangent-normal` contrasts true contact-bearing tangency with radial centre-on-line normal
incidence. Direct qualification includes 13 focused M55 integration tests and 49 workbench tests,
plus the complete warnings-denied workspace Clippy, locked
all-feature workspace test suite, all-feature demo-web WASM check and Trunk 0.21.14 release build.
No residual equation, persistence schema, old route/harness or mobile scope was added.

### M56: prepared jobs and concurrency contract

Status: complete as of 2026-07-29.

Scope: immutable accepted snapshots, exact-revision prepared jobs, non-mutating candidate
patches, compare-and-swap commit, host-managed scheduling and safe Rust `Send`/`Sync`
contracts for native and single-threaded WASM consumers.

- [x] Capture the exact retained design, latest attempt, accepted/high-water state, solve policy,
  activation, parameter and external-snapshot identities in one immutable prepared snapshot.
- [x] Execute typed design edits, reattempts, parameter batches and external snapshot replacements
  against scratch session state and return only a non-mutating candidate patch.
- [x] Publish a completed patch only through exact-input compare-and-swap; stale, out-of-order,
  cancelled and work-exhausted jobs cannot mutate the owning session.
- [x] Document and directly qualify the safe ownership contract: session-bearing snapshots/jobs/
  patches move as `Send` single-owner values, immutable DTOs are `Send + Sync`, native hosts may
  use workers, and single-threaded WASM uses the same synchronous boundary without `unsafe`.

Completion notes (2026-07-29): `RetainedSketchDocumentSession::prepared_snapshot` now captures a
complete `PreparedSketchInput` stamp and returns a read-only snapshot. `PreparedSketchOperation`
covers ordinary typed edits, reattempts, parameter-batch changes and external-snapshot changes.
`PreparedSketchJob::execute` performs controlled work only on its captured clone and yields no
patch on cancellation or work exhaustion. `commit_prepared_patch` compares the complete captured
stamp against the live owner before one atomic session replacement; stale patches return a typed
`StalePreparedPatch` error without mutation.

The native regression moves a prepared job through `std::thread::spawn`, proves the live session
unchanged before commit and verifies exact CAS publication. Separate regressions cover out-of-order
stale rejection, pre-cancelled parameter work, complete lifecycle/activation/parameter/external
stamps and same API behavior under the all-feature WASM build. Solver caches remain safe
single-owner interior state: session-bearing values are `Send`, not promised `Sync`; immutable
input/operation/commit DTOs are `Send + Sync`. Full workspace tests, warnings-denied Clippy,
all-feature WASM and release Trunk pass. `docs/M56_IMPLEMENTATION.md` records the exact gate.

### M57: incremental solving and production scale

Status: complete as of 2026-07-29.

Scope: persistent runtime mappings, dependency-closure rebuilds, indexed/history storage,
profile caches, workload envelopes, sparse-rank evaluation and full fresh validation on
every optimized return path.

- [x] Retain compatible document/runtime/core identities and derive dirty variables/sources from
  persistent document dependencies rather than caller-supplied runtime IDs.
- [x] Prove local geometry, parameter, external-reference and same-shape activation updates reuse
  only clean components while topology/source-shape changes take an explicit full-rebuild path.
- [x] Index persistent-to-runtime point/curve/source/contact mappings and keep application history
  outside the retained solver lifecycle; accepted-only command history remains directly indexed.
- [x] Cache bounded visual profiles only inside one accepted revision and invalidate the cache on
  every newly accepted state.
- [x] Publish execution-path/fresh-validation evidence and an honest production rank assessment:
  sparse steps are supported, while numerical rank remains dense-SVD authoritative within the
  256-row/256-tangent connected-component envelope.
- [x] Qualify cold/warm, profile, storage and cancellation behavior with deterministic direct
  tests, fresh-rebuild parity and an observed release workload record.

Completion notes (2026-07-29): compatible retained attempts now lower one scratch compatibility
oracle, preserve `DocumentRuntimeMap`, `CompiledSketch` and `SolveSession` identities, and submit
only changed shape variables plus the transitive persistent source closure through the existing
core `SessionPatch`. Parameter and external-reference updates bypass the former full attempted
solve when request shape is unchanged. Local point edits preserve the temporary-drag semantics,
then publish through the same retained runtime boundary. Every optimized return still executes
`finalize_solved_candidate_controlled`, fresh hard-row/Jacobian/rank evaluation, document
projection and atomic publication.

`SketchSessionExecutionSummary` distinguishes initial, incremental and full-rebuild execution;
`SketchProductionScaleAssessment` reports bounded dense-SVD rank authority without pretending
sparse steps imply sparse rank certification. Runtime maps have persistent-ID indexes, accepted
profile caches are revision-local, and host/application history remains outside
`RetainedSketchDocumentSession`. The ten-case M57 corpus covers two- and sixteen-component
workloads, fresh parity, local geometry, parameters, immutable external references, activation,
topology and changed-incidence fallback, profile invalidation and deterministic work exhaustion.
Release execution of the complete M57 corpus took 0.21 s and 66,496 KiB maximum RSS on the
recorded development host;
these are observations, not correctness tolerances. `docs/M57_IMPLEMENTATION.md` records the gate.

### M58: sketch operations companion

Status: complete as of 2026-07-29.

Goal: add a separate deterministic, equation-free transaction companion without giving
drafting operations solver, residual, session-publication or B-rep ownership.

- [x] Add `geosolve-sketch-ops` with one-way direct dependencies only on public sketch and
  immutable geometry APIs, plus native/WASM-safe controlled prepared work.
- [x] Capture the complete retained input stamp and matching accepted-state identity when
  geometry is required; apply only through exact-input compare-and-swap and the ordinary retained
  sketch transaction path.
- [x] Implement typed split, break, trim, line extension, exact supported-family mirror, line
  chamfer, existing associative-fillet integration, rectangle, regular-polygon, slot and bounded
  linear-pattern requests.
- [x] Publish deterministic retained/replaced/split/proposed identity mappings and typed
  unsupported/incomplete outcomes; never approximate an unsupported exact family.
- [x] Generalize equation-free visible topology to ordered non-overlapping multi-interval
  supports, exact fixed/contact boundary identity and atomic constraint-owned boundary freezing.
- [x] Preserve frozen canonical sketch v4: M58-only state is rejected by v4 import/export and
  round-trips only through the explicitly unsupported draft-v5 codec.
- [x] Directly qualify stale application, cancellation/work exhaustion, foreign accepted state,
  interval validation, profile adjacency, persistence rejection, operation expansion and
  dependency boundaries under native, workspace and WASM gates.

Gate: complete. The companion owns no residual or solver state, every proposal is deterministic
for one exact input, stale/cancelled/exhausted work cannot mutate the live session, and all
operation results pass ordinary sketch validation and independent accepted-state publication.

Completion notes (2026-07-29): `geosolve-sketch-ops` exposes an immutable
`SketchOperationSnapshot`, worker-movable `PreparedSketchOperation`, closed request/result
surface, deterministic `SketchOperationProposal` and exact-CAS `apply`. Geometry-dependent
requests require the accepted state for the same retained design. Mirror and linear pattern
currently require accepted geometry to equal design geometry exactly; unsupported curve families
and ambiguous/incomplete geometric cases return typed outcomes rather than sampled approximations
or guesses.

`geosolve-sketch` now supports several ordered visible intervals per immutable support, ordinary
point-on-curve-owned trim boundaries, exact semantic profile endpoint keys and a read-only current
prepared-input stamp. Existing generic fillets remain public sketch associations; chamfer emits
ordinary contacts, point-on-curve constraints and driving dimensions. Canonical v4 remains frozen,
while the hidden draft-v5 codec preserves M58 state pending a future schema-freeze decision.

The 18-case M58 suite covers every request family plus deterministic mappings, stale/cancelled/
exhausted atomicity, foreign accepted geometry, bounds/non-finite input, interval overlap/order,
constraint deletion freezing, exact split profile closure, draft-v5 round-trip and strict v4
rejection. Focused M28/M31/M34/M57/editor compatibility, warnings-denied Clippy, the complete
all-feature workspace suite, demo-web and operations WASM checks and the Trunk release build pass.
`docs/M58_IMPLEMENTATION.md` records APIs, mathematical behavior, exact commands and limitations.

### M59: production topology companion

Status: complete as of 2026-07-29.

Goal: add a separate read-only production-topology companion without converting bounded visual
analysis directly into a B-rep claim or giving the companion solver/publication ownership.

- [x] Add `geosolve-sketch-topology` with direct dependencies only on public sketch and immutable
  geometry APIs, plus native/WASM-safe controlled prepared queries.
- [x] Capture the complete current retained input and matching independently accepted state;
  reject retained accepted geometry from any older design, parameter, activation, external,
  request or solver-policy input.
- [x] Require explicit native profile/construction and external line scope, and publish exact
  eligible-source evidence including ignored external point entries.
- [x] Publish typed bounded policy for tangency, overlap, touching contours, T-junctions and
  self-intersections, with separate complete/truncated/skipped evidence.
- [x] Independently validate source coverage, parameter enclosures, fresh endpoints, wire closure,
  certified orientation/area, outer/hole nesting and output limits before exposing a production
  profile.
- [x] Publish exact native visible-interval and immutable external line revision/digest/domain
  provenance, plus exact-input validation before host consumption.
- [x] Directly qualify determinism, safe worker ownership, stale/cancelled/exhausted atomicity,
  ambiguity policies, construction/external filtering, M58 multi-interval compatibility and
  dependency boundaries under native, workspace and WASM gates.

Gate: complete. Only a current `Complete` result carries consumable production wires and regions.
The companion owns no residual, live session, accepted publication path or B-rep entity.

Completion notes (2026-07-29): `TopologySnapshot` captures one complete
`PreparedSketchInput` only when the accepted state was independently published for that exact
design and host input. `PreparedTopologyQuery` is worker-movable and returns outer
cancelled/work-exhausted outcomes separately from `TopologyCompleteness`. `TopologyRequest`
records native construction and immutable external-line scope, all ambiguity policies and
deterministic analysis/output limits.

Visual-profile analysis supplies bounded candidate evidence only. The companion independently
checks complete eligible-source coverage, exact interval/domain provenance, parameter enclosures,
fresh curve endpoint evaluation, closure, certified signed-area orientation and output counts.
Only `TopologyCompleteness::Complete` constructs `TopologyProductionProfile`; every live consumer
must pass exact-stamp `validate_current`. Native multi-interval sources retain exact boundary
provenance. External line snapshots retain binding, revision, digest and domain evidence but do
not proximity-weld to native endpoints because M43 has no persistent cross-owner endpoint join.

The 15-case M59 suite covers square and nested-hole production output, construction filtering,
external line/point scope, open/overlap/tangent/T-junction/self-intersection failures, deterministic
output truncation and replay, safe worker movement, M58 multi-interval closure, cancellation/
exhaustion/stale atomicity, older design/host-input rejection and manifest isolation. Focused
M26/M28/M31/M34/M57 and M58 compatibility, warnings-denied Clippy, the complete workspace,
topology/demo WASM and release Trunk gates pass. `docs/M59_IMPLEMENTATION.md` records exact APIs,
commands and limitations.

### M60: advanced workbench completion

Status: complete as of 2026-07-29.

Goal: build on the completed M55 action surface by exposing advanced curves/branches, companion
operations, production profiles, stable diagnostics, cancellation/stale presentation and a
versioned desktop workspace envelope in the already-clean single workbench.

- [x] Preserve all 13 relation/five dimension M55 actions and the ten stable M53/M55 scenario
  identities.
- [x] Add deterministic accepted all-family and periodic NURBS scenarios with explicit
  span/winding transition and knot insertion through public document edits.
- [x] Add associative-fillet plus split, exact-mirror and bounded-pattern workflows through public
  `geosolve-sketch-ops` prepared proposals and ordinary retained-session publication.
- [x] Present `geosolve-sketch-topology` complete wires/regions and typed incomplete, cancelled,
  exhausted and unavailable outcomes without promoting candidate evidence.
- [x] Add four stable M61 leaves under one right-expanding advanced/topology selector subtree,
  deterministic guidance/evidence and unchanged ordinary-workspace isolation.
- [x] Advance the desktop workspace envelope to version 2 with explicit canonical-v4/draft-v5
  document encodings and deterministic version-1 migration.
- [x] Directly qualify editor, presentation, persistence, scenario, operations and topology
  behavior under native, WASM and release Trunk gates without restoring browser E2E, CDP,
  `/#/dev/lab` or mobile scope.

Completion notes (2026-07-29): `geosolve-demo-web` now consumes the public operations and topology
companions directly. The inspector publishes complete production wires/regions only from
`TopologyProductionProfile`; open support, cancellation, exhaustion, staleness and unavailable
accepted input remain non-consumable typed states. The scenario catalog grows from ten to fourteen
stable leaves with `advanced-all-families`, `nurbs-branch-topology`,
`associative-companion-operations` and `production-topology-trust`; old IDs, M55 action coverage,
reset/evidence behavior and persistence isolation are directly frozen.

Workspace v2 records whether each checkpoint payload is canonical v4 or the explicitly unstable
draft v5, restores M58 multi-interval documents exactly and migrates legacy workspace v1. The
focused gate passes editor 60 unit plus 7 M55 integration tests, demo-web 40/40, operations 18/18
and topology 15/15, warnings-denied Clippy, all three relevant WASM checks and Trunk 0.21.14
release build. The complete workspace gate also passes. `docs/M60_IMPLEMENTATION.md` records exact
behavior and commands; `docs/M61_UAT.md` is the prepared human scorecard.

### M61: human UAT 3 - advanced geometry and topology

Status: complete as of 2026-07-29; the first candidate was withdrawn after five human blockers,
the remediated candidate was directly qualified, and the supervising human explicitly approved
M61 for its recorded scope.

Scope: advanced authoring, operations, topology, branch clarity and interaction-performance
review after objective direct qualification.

- [x] Preserve and expose ten representative nonzero-mobility alpha mechanisms, including both
  scissor fixtures, with a preselected persistent driver, active-scenario projected drag, exact
  reset and ordinary-workspace isolation.
- [x] Repair recursive desktop flyouts so third-level descendants expand to the right without
  becoming children of a clipping scroll container.
- [x] Add cursor-anchored wheel zoom, middle-drag pan, explicit zoom/Fit controls and direct camera
  math/large-scene containment tests.
- [x] Add reusable headless construction proposals, draft/previews and sole-workbench controls for
  quadratic/cubic Bezier, ellipse, elliptical arc, rational quadratic conic, parabola, hyperbola
  and clamped/periodic gauge-separated NURBS.
- [x] Reject non-finite/out-of-domain conic state and invalid NURBS degree/count/weight/gauge
  topology atomically; sample advanced previews only through public sketch curve evaluation.
- [x] Revoke the original scorecard candidate and replace its instructions with the expanded
  mechanism, camera, authoring, navigation, branch, operation and topology review.
- [x] Resolve `M61-F001` by routing the twin-roller cam's active/passive persistent identities
  through headless stabilized projection; directly regress repeated drags in both directions with
  the passive center stationary.
- [x] Resolve `M61-F002` by retaining a dynamic branch-selector value only while it remains among
  the current headless choices; otherwise select the first published default so point-on-circle
  Coincident authoring cannot submit an empty stale domain.
- [x] Resolve `M61-F003` by coalescing projected pointer moves to the latest sample per animation
  frame and flushing at most that latest sample before pointer-up, while retaining the solver's
  truthful ambiguous-contact rejection for the supplied pathological workspace.
- [x] Resolve `M61-F004` by removing obsolete full visual-profile analysis from the synchronous
  host-state render path. Retain cheap accepted geometry-role declarations and use the separately
  qualified production-topology companion as the sole consumability authority.
- [x] Resolve `M61-F005` by removing direction-only line/curve Parallel and Perpendicular from
  compact authoring, retaining true generic Tangent contact, and lowering circle/arc Normal to
  radial centre-on-line incidence with a dedicated reusable UAT scenario.

Remediation notes (2026-07-29): the M61 subtree now contains an **Interactive mechanisms** branch
with **Compact mechanisms** and **Linkage mechanisms** grandchildren plus the four preserved
advanced/topology leaves. Ten new stable leaves cover compass, Bezier bridge, twin-roller cam,
tangent orbit, trammel, Scotch yoke, rotating square, scissor jack, five-stage scissor tower and
Peaucellier linkage. The active scenario owns all selection/projected-drag effects while scenario
persistence remains disabled. The editor/workbench construct every requested advanced family
through public document/session APIs; no equation, deleted playground, `/#/dev/lab` or browser
harness returned. `docs/M61_UAT.md` records the replacement scorecard and final scoped approval.
Targeted repair `1c314e9` preserves the motion-cam fixture's intended independent freedoms while
holding the non-dragged roller at its accepted position through an interaction-only stability
target. `M61-F002` confirms the exact authored line-endpoint plus circle selection resolves and
publishes a periodic point-on-curve contact, while the workbench now discards invalid stale/empty
dynamic option values before dispatch. Neither repair adds an equation or persistent constraint;
the final scoped M61 approval accepts the requalified behavior. `M61-F003` reconstructs the supplied retained workspace
with five points, two circles, two lines, one quadratic Bezier, four contacts, three constraints
and two driving line-length dimensions. Native replay retains
`AmbiguousContactNeighborhood` rather than laundering the rejected design into convergence, and a
small projected retry recovers. The WASM adapter no longer synchronously solves every queued raw
pointer sample: one scheduled frame owns only the latest pending sample, pointer-up drains it once,
and stale frames are invalidated. No solver equation, solver policy or persistence schema changed.
Targeted replay then isolated `M61-F004`: the exact WASM restore takes about 171 ms, but the legacy
host-state panel recomputed accepted visual-profile analysis on every render. The supplied accepted
graph cost about 2.3 seconds per panel render even in optimized native code. Replacing that
duplicate analysis with declared accepted geometry roles reduces the same panel generation to
about 0.12 ms; the independently validated production-topology card remains authoritative.
`M61-F005` then identified that direction-only curve relations made Tangent/Normal authoring
appear non-geometric, especially on a full circle whose free contact parameter can satisfy any
requested direction. Compact Tangent remains generic shared-contact plus tangent alignment;
Perpendicular / Normal on a line plus circle or arc now reuses point-on-curve to constrain the
circular centre onto the line. Parallel is line-pair only, arbitrary nonlinear direction-only
dispatch is disabled, and `circle-tangent-normal` plus direct mathematical regressions freeze the
distinction. The public domain-level `CurveDirection` definition is retained.

Approval record (2026-07-29): after the five original blockers and findings `M61-F001` through
`M61-F005` were remediated and mechanically requalified, the supervising human closed M61 as
approved for the scope recorded above. This approval does not freeze future UI scope or imply
release hardening.

### M62: CAD-style constraint and dimension authoring

Status: complete and explicitly approved by the supervising human on 2026-07-29.

Goal: replace inspector-driven relation creation with a compact CAD authoring workflow while
making the headless editor, rather than the browser, own operand collection and contextual
dispatch.

- [x] Add a selection-independent headless authoring state machine with explicit immutable
  selection snapshots and picked operands, typed expected-operand guidance, warnings and outcomes.
- [x] Preserve the eleven compact contextual constraint intents and five dimension tools without
  adding a residual, equation-shaped alias or browser-owned compatibility rule.
- [x] Apply a compatible non-empty host selection once; reject an incompatible selection without
  mutation; enter persistent repeated authoring mode from an empty selection.
- [x] Support one-operand repetition, two-pick repetition, ordered continuity/angle operands,
  normalized role-distinct operands, and point/point/axis Symmetric collection.
- [x] Keep pending operands separate from ordinary selection, reconcile stale operands after
  topology changes, retain operands after pre-transaction input errors, and clear completed
  operands after retained accepted or rejected mutations.
- [x] Provide two-stage Escape: clear pending operands first, then exit authoring mode.
- [x] Replace inspector creation dropdowns with a wider two-column left palette containing
  geometry, constraint and dimension tools plus non-persisted option flyouts.
- [x] Route canvas and tree picks through the same authoring operand API, suppress ordinary
  selection and point drag while authoring, and render pending operands plus concise canvas
  guidance.
- [x] Preserve explicit defaults in memory only: aligned tangency, signed curvature, G1
  continuity, parametric-C2 rates 1/1, driving dimensions and counter-clockwise angles.
- [x] Add retained selected-dimension target editing in the inspector using the owned target
  scalar and ordinary history/replay.
- [x] Directly qualify headless transitions, contextual dispatch, rejected-state retention,
  history, stale reconciliation, palette presentation and WASM adaptation.
- [x] Run formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature WASM
  check and release Trunk build.
- [x] Complete and explicitly approve `docs/M62_UAT.md` in the ordinary workspace.

Gate: every visible creation tool is driven by public headless authoring metadata and emits only
ordinary retained document edits. Scenario mode remains non-editable, no scenario is added, and no
legacy harness or `/#/dev/lab` route returns.

Candidate notes (2026-07-29): commits `0ec560b` and `53e7867` add the public operand collector,
explicit retained coordinator entry points, two-column palette, flyout options, shared canvas/tree
picking, pending highlights and target editing. Direct editor/workbench tests, the locked complete
workspace gate and release WASM bundle pass. Follow-ups `M62-F001` through `M62-F005` were
mechanically requalified before final approval.

UAT follow-up `M62-F001` corrects angle creation before approval. Dimension creation now measures
the exact independently accepted document rather than potentially divergent retained design
coordinates, so adding an angle cannot move geometry merely because its seed came from another
lifecycle state. The headless target metadata presents the acute supporting-line intersection
angle in degrees while retaining the persisted directed-radian quadrant and winding branch.
Degree edits map back through that same branch, and retained-rejected authoring/target edits are
reported distinctly from accepted publication. Direct regressions cover accepted/design
divergence, reversed endpoint direction, all four directed quadrants, no-move creation, a 45-to-60
degree edit and acute canvas annotation. No residual, equation or persistence schema changed.

UAT follow-up `M62-F002` corrects repeated constraint collection before approval. Canvas
authoring previously consumed one physical click first through the parameter-preserving
pointer-down path and then again through the bubbled generic item-click path. That duplicated the
first operand: single-item tools attempted an immediate duplicate transaction, while pair tools
could fill both slots with the same line and remain wedged at full arity after the coordinator
refused it. Canvas pointer-down now exclusively owns canvas picks, tree clicks retain their one
click path, and the headless collector is re-armed after every terminal application attempt,
including coordinator errors. A direct workbench regression exercises the exact pointer-down plus
click sequence for Horizontal and Normal/Perpendicular. No solver, equation, branch, persistence or
scenario behavior changed.

UAT follow-up `M62-F003` corrects simple curve-constraint execution before approval. The headless
authoring adapter generated contact-domain, parameter and neighborhood choices for every selected
curve, even when the resolved definition was a simple Horizontal, Vertical, Parallel,
Perpendicular, Equal Length or Equal Radius relation that accepts no contact state. The retained
coordinator therefore rejected the complete application before creating a document constraint.
Contact choices are now generated only for point-on-curve, curve contact/tangency, equal
curvature, endpoint continuity and radial circle/arc Normal. A direct coordinator regression
authors Horizontal and line-line Perpendicular on skew free lines, verifies accepted publication
and inspects the exact persistent definitions. No residual, equation, branch, schema or scenario
changed.

UAT follow-up `M62-F004` completes a closed-path audit before approval. All sixteen
`ResolvedConstraintKind` families now pass both an exhaustive metadata-ownership regression and
the complete `AuthoringState -> AuthoringApplication -> RetainedEditorCoordinator -> accepted
persistent constraint` path. The audit found two further contact translation defects: repeated
picks on one semantic curve span were recovered by identity and both inherited the first picked
parameter, while endpoint continuity preserved an End parameter but always chose the Start
neighborhood. Contact operands now preserve occurrence order, radial Normal still selects only its
line operand, and endpoint choices put the parameter-matching neighborhood first. The five
dimension families pass a separate complete authoring-to-accepted-transaction matrix and do not
pass through contact metadata. No residual, equation, branch definition, schema, scenario or
browser rule changed.

UAT follow-up `M62-F005` converts the remaining pre-closure review list into direct owning-layer
tests. The sixteen-relation and five-dimension integration matrices now prove identical
applications from compatible preselection and repeated operand collection, including terminal
re-arming. Representative line, circle, quadratic-Bezier and NURBS point-on-curve authoring
preserves the picked parameter through accepted persistence. Endpoint continuity passes in both
End/Start and Start/End orders, retained-rejected contact authoring can be undone and retried
without leaving its active tool, dimension target history passes Undo and Redo, and headless
options survive tool re-entry while a fresh process state returns to defaults. This work found
one last metadata variant: ordinary bounded point-on-curve picks at parameter endpoints defaulted
to `Interior`. Bounded contact choices now put `Start` or `End` first when the picked parameter is
the corresponding endpoint. No residual, equation, branch definition, schema, scenario or
browser-owned compatibility rule changed.

Approval record (2026-07-29): after findings `M62-F001` through `M62-F005` were remediated and
mechanically requalified, the supervising human explicitly approved M62. This closes the
milestone for the scope recorded above without assigning any scope to M63.

### M63: canvas constraint visualization and interaction

Status: complete and explicitly approved by the supervising human on 2026-07-30.

Goal: make constraints and dimensions understandable and directly selectable at their accepted
geometry without turning the canvas into a permanent icon cloud.

- [x] Publish typed, finite, geometry-derived constraint/dimension annotations from
  `geosolve-constraint-editor`, including persistent identity, semantic kind, direct operands,
  visibility policy, selectable hit geometry and deterministic fan-out.
- [x] Keep every angle and every driving dimension visible at its accepted geometry; keep
  non-angle reference dimensions and ordinary constraint glyphs contextual.
- [x] Give headless hover, selection and pointer hit testing ownership of annotation interaction.
  Exact icon-occurrence proximity is separate from the geometry context that keeps a related set
  revealed; visible annotation hits precede geometry in Select mode, while constraint authoring
  remains geometry-only.
- [x] Render accessible CAD-like SVG symbols, angle arcs, witness lines, leaders, values,
  selected/problem states and related-operand emphasis in the sole workbench.
- [x] Cover the complete persistent constraint/dimension catalog through direct headless tests,
  including contextual visibility, reference-angle override, fan-out, pointer selection and
  public advanced-scenario projection.
- [x] Add three focused reusable scenario leaves under **M63 Canvas constraints** for angle and
  dimension presentation, contextual relation discovery and crowded annotation fan-out.
- [x] Pass the common clean format, warnings-denied Clippy, workspace-test, WASM and release Trunk
  gates from the final candidate.
- [x] Complete and explicitly approve `docs/M63_UAT.md`.

No solver equation, residual, persistent document schema, branch rule, mobile claim, legacy E2E
harness or `/#/dev/lab` route is added.

UAT finding `M63-F001` (2026-07-30) corrected radial annotation jitter caused by choosing among
mathematically tied adaptive-tessellation samples. Full circles now use canonical parameter zero
and circular arcs use their semantic midpoint through public accepted-curve evaluation. Direct
regression and mechanical requalification pass; the focused human retest is accepted.

UAT finding `M63-F002` (2026-07-30) corrected overlapping crowded constraint symbols. Headless
fan-out now collision-checks deterministic concentric candidates and requires 22 px separation
between every final glyph center while retaining semantic leaders. The actual rotating-square
fixture directly verifies all marker pairs; the focused human retest is accepted.

UAT finding `M63-F003` (2026-07-30) made visible leaders contextual hover corridors, but human
retest showed that correction was insufficient for paths beginning elsewhere on related geometry.
`M63-F004` supersedes it by retaining the actual last geometry-hover position and constructing
bounded corridors from there to directly related annotations, selecting the nearest overlapping
corridor and clearing outside all corridors. The regression begins outside both geometry and
leader hit tolerances; the superseding focused interaction review is accepted.

UAT finding `M63-F005` (2026-07-30) showed that `M63-F004` still conflated geometry reveal
context, corridor transit and persistent-annotation hover. Crossing one icon could replace the
context owner and hide its siblings, leaders counted as icon hits, and one multi-marker
constraint highlighted every occurrence at once. The headless editor now publishes separate
typed proximity and context-owner state, identifies glyph occurrences by deterministic marker
index, treats leaders and inter-icon links as transit only, and maps occurrence clicks back to
the persistent constraint. The renderer applies hover only to the matching glyph child. Direct
headless and workbench regressions pass; the focused human retest is accepted.

UAT refinement `M63-F006` (2026-07-30) replaces the unrelated Unicode/letter authoring
placeholders and inconsistent canvas drawings with one text-free CAD vector catalog. Ten shared
constraint concepts use the same symbol in the palette and accepted canvas; the canvas retains
distinct symbols for point-on-curve, collinear, equal-length, equal-radius, generic contact,
curve direction, curve normal, equal curvature and fillet. All five dimension authoring actions
also receive geometry-representative vector icons, and the contextual operation label is now
explicitly **Perp / normal**. Direct catalog, shared-language, palette-host and scene regressions
pass; the focused human review is accepted.

UAT refinement `M63-F007` (2026-07-30) moves line-relation glyphs from an accidental endpoint
sample to the geometric midpoint of each related line. Horizontal, vertical, parallel,
perpendicular, collinear and equal-length presentation now share this stable interior rule, while
contact- and curve-specific annotations retain their existing semantic anchors. A direct
multi-line regression requires both parallel markers to occupy their respective line midpoints;
the focused human review is accepted.

UAT refinement `M63-F008` (2026-07-30) gives line-line perpendicularity angle-like geometric
presentation instead of two generic midpoint glyphs. The headless scene now publishes a typed
selectable `RightAngle` square at the finite on-screen supporting-line intersection, choosing the
quadrant that enters endpoint-adjacent spans and reserving its corner during dense glyph fan-out.
An off-screen intersection retains the compact midpoint fallback rather than inventing a false
corner, and curve-contact Normal keeps its distinct contact-local symbol. Direct headless,
renderer and rotating-square density regressions pass; the focused human review is accepted.

UAT refinement `M63-F009` (2026-07-30) completes the workbench icon audit beyond constraint
annotations. All fifteen geometry tools now use distinct text-free CAD vector symbols instead of
letters or punctuation, their tool keys and runtime catalog have one owner, sketch-tree rows
distinguish points, curves, constraints, dimensions and external bindings, and canvas problem
markers use a vector alert mark rather than SVG text. Constraint/dimension SVG containers are
non-semantic icon hosts; only genuine Enter/Esc key hints and camera controls retain textual
symbols. Direct catalog, palette-host, tree and problem-marker regressions pass, the release
palette is visually legible at its actual desktop size, and the focused human review is accepted.

Approval record (2026-07-30): after findings `M63-F001` through `M63-F009` were remediated,
mechanically requalified and reviewed through the focused canvas-constraint UAT, the supervising
human explicitly approved M63. This closes the milestone for the scope recorded above without
assigning any scope to M64.

### M64: editable sample library and scenario-harness cleanup

Status: complete and supervising-human approved as of 2026-07-30.

Goal: make every demonstration an ordinary editable save-like workspace, organize the sample
library by purpose, and add representative multi-freedom mechanisms without retaining the guided
review harness.

- [x] Replace milestone-owned recursive scenario definitions with exactly three one-level purpose
  groups: mechanisms, constraints and dimensions, and curves and constructions.
- [x] Remove guide copy, scripted actions, verification points, transcripts, evidence capture,
  reset/exit controls, hidden ordinary-workspace isolation and all alternate-coordinator routing.
- [x] Make opening a sample replace the ordinary coordinator, reset editor history, fit the
  camera, and participate in normal workspace autosave.
- [x] Keep ordinary geometry/constraint/dimension authoring, branch editing, selection, Delete,
  Undo/Redo, zoom/pan and projected dragging available after a sample opens.
- [x] Add public four-bar coupler, pantograph and three-link drawing-arm fixtures with independently
  validated 1/2/3-DOF behavior and scale-invariant persistent IDs at `1e-6`, `1` and `1e6`.
- [x] Generalize passive-freedom drag stabilization in the retained headless coordinator so the
  twin-roller sample needs no browser/sample-specific driver metadata.
- [x] Directly qualify the 22 unique sample keys, accepted construction, workspace round-trip,
  fresh history, fixed-constraint Delete/Undo and complete absence of guided harness markup.
- [x] Pass formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature
  WASM check and release Trunk build.
- [x] Complete and explicitly approve `docs/M64_UAT.md`.

Gate: every catalog leaf constructs through public domain APIs and opens as the sole ordinary
editable workspace; every sample round-trips through `WorkspaceSnapshot`; the new mechanisms
publish hard-valid residuals `<= 1e-9` with advertised mobility at all three scales; no guided
scenario/action/evidence state, browser E2E, `/#/dev/lab` route or browser-owned solver rule
returns. M64 closes only after the focused human sample UAT is explicitly approved.

Mechanical qualification note (2026-07-30): `cargo fmt --all -- --check`,
warnings-denied locked workspace Clippy, the locked all-feature workspace test suite,
the `wasm32-unknown-unknown` demo-web check, the release Trunk bundle and `git diff --check`
all pass. The host exports `NO_COLOR=1`, which Trunk 0.21.14 does not accept as a boolean;
the successful release invocation therefore used `env NO_COLOR=true trunk build --release`.

Approval record (2026-07-30): the supervising human reviewed the mechanically qualified candidate,
reported satisfaction with the result and explicitly asked to close M64. All five focused
scorecard areas are recorded Pass in `docs/M64_UAT.md`; no M64 finding remains open.

### M65: predictable bounded projected dragging

Status: complete and explicitly approved by the supervising human on 2026-08-01.

Goal: make projected dragging predictable and synchronously bounded for the existing editable
mechanism samples. Stability and local UX take priority; bounded work must not weaken mathematical
validation or introduce sample-specific behavior.

- [x] Derive one opaque locality plan at gesture start from the independently accepted hard
  nullspace. Measure the active point rank, cover only passive mobility, and choose the smallest
  deterministic anchor set by rank gain, then lower anchor mobility rank, then compile order.
- [x] Capture anchor targets from the gesture-start accepted visible geometry. Compile only the
  cursor as a Temporary target and only locality anchors as PreviousState Preferences; do not
  reintroduce all-point stabilization, persistent-ID retry order or sample-owned driver metadata.
- [x] Continue from the complete last independently accepted preview and execute exactly one
  retained attempt per pointer sample. Rejected or exhausted work keeps that preview intact, a
  later valid sample may recover in the same gesture, and stale/out-of-order request IDs are
  deterministic no-ops.
- [x] Preserve semantic circle-center dragging with the initial circumference-to-center pointer
  offset. Release publishes only an independently validated exact preview, Cancel mutates no
  history, and release/cancel/Undo/Redo remain ordinary editor lifecycle operations.
- [x] Harden core priority publication: Hard rows must validate independently. On the
  single-component dense path, independently capture and revalidate the complete positive
  Temporary residual vector; Preference work may publish only a candidate that preserves that
  vector row-by-row within
  `max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`.
  This reproducibility floor does not relax Hard validation or Temporary attainment. Coupled-
  priority solving retains its existing scalar attained-level semantics.
  Accepted/no-motion reports must reject invalid-geometry or numerical-failure termination and
  require evaluable audit rows; truthfully non-optimal secondary termination remains compatible
  with independently valid Hard geometry.
- [x] Enforce one documented projected-sample operation envelope: `16,384` each for document
  validation, dependency and lowering items; `256` each for nonlinear iterations,
  factorizations and rank kernels; `512` rejected trials; `1,024` component linearizations;
  dense kernels no larger than `256 × 256`; `512` diagnostic candidates; and `1,024`
  diagnostic trials.
- [x] Add compact table-driven regressions for both twin rollers across horizontal, vertical,
  diagonal and reversal paths; real center/circumference pointer overlaps; passive-center movement
  `<= 1e-8`; bounded rejection followed by same-gesture recovery; pantograph
  input/guide/output/center plus natural off-manifold guide projection; Scotch-yoke guide deletion;
  scissor jack/tower; circle-handle offset; release/cancel/Undo/Redo; and stale queued results.
- [x] Pass formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature
  WASM check, release Trunk build and `git diff --check` on one final source state.
- [x] Complete and explicitly approve the focused M65 Tailscale UAT.

Gate: ordinary projected drag follows the selected control locally, leaves independent passive
controls stationary, never changes assembly branch implicitly, preserves the complete last valid
preview on rejection or exhaustion, and cannot synchronously exceed the documented operation
envelope. Hard/Temporary success remains independently validated. M65 adds no alternate-branch
UI/search/sample, new residual family, relaxed tolerance, weighted-priority substitute,
sample-ID policy, worker architecture or global root enumeration. It closes only after fresh
mechanical qualification and explicit supervising-human UAT approval. See
`docs/M65_IMPLEMENTATION.md` and `docs/M65_UAT.md`.

Mechanical qualification record (2026-08-01): direct locality ordering and objective-inventory
tests, exact release exhaustion/retry tests, representative mechanism paths, strict core
publication regressions and the integrated authoring/workspace/editability lifecycle all pass.
Formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests, the
all-feature WASM check, the release Trunk bundle and `git diff --check` pass on replacement code
source `b6433d1`. `M65-F004` makes twin-roller geometry reachable through overlapping dimension
leaders without hiding offset labels; `M65-F005` certifies the rank-one `2 x 2` cursor projection
and restores natural off-manifold guide motion without relaxing rank, KKT, hierarchy or work
limits. The supervising human subsequently approved the focused U2/U3 retests against the
replacement candidate. M65 is closed; the historical candidate endpoint was
`http://100.94.63.83:8080/`.

### M66: CAD helper-operation authoring

Status: active. M65 is approved, ADR 0030 is accepted and implementation is in progress.

Goal: expose intuitive reusable authoring for associative 2D fillets, line offsets and exact
supported-family mirrors while keeping branch synthesis, applicability, scratch preview and
publication in the headless editor/operations layers rather than the browser.

- [x] Accept ADR 0030, extending the allowed dependency direction so
  `geosolve-constraint-editor` may consume public `geosolve-sketch-ops` proposals without moving
  equations, solver state or accepted authority into either the editor or web crate.
- [ ] Add a separate `OperationAuthoringState` with Fillet, Line offset and Mirror tools, finite
  model-space picks, typed pending stages/guidance, explicit options, warnings and terminal
  outcomes; do not overload M62 `AuthoringState`.
- [ ] Support compatible preselection and empty-selection persistent repeated mode, Apply/Enter,
  two-stage Escape, stale-operand reconciliation and terminal re-arming while preserving ordinary
  pan/zoom navigation.
- [ ] Make the retained coordinator capture immutable operation snapshots, execute proposals on
  scratch retained state, expose preview only from an independently accepted exact-input result,
  and publish the exact proposal through compare-and-swap as one normal history edit.
- [ ] Synthesize a complete branch-explicit M28 fillet request from two distinct local curve-span
  picks, including parameter, neighborhood, winding, normal sides, retained endpoints, periodic
  anchors, endpoint order, sweep and positive driving radius.
- [ ] Default fillets to `0.1 * document.model_scale()`, the locally picked retained portions and a
  minor arc; expose flip-first-side, flip-second-side and alternate-arc corrections and remember
  the radius only for the current process.
- [ ] Reject ambiguous/unresolved local fillet roots, same supports, already-trimmed parents,
  singular/parallel offsets, zero-speed/pole/cusp geometry and escaped spans without a global root
  search or partial mutation.
- [ ] Add one atomic `geosolve-sketch-ops` line-offset request for a line or polyline span that
  creates target endpoints, target line, positive scalar and the existing driving offset
  dimension with explicit side and `Same` orientation.
- [ ] Default line offset to exact translated segment and expose supporting-line offset as the
  truthful alternate mode; do not approximate general curve, chain, conic, spline or NURBS
  offsets.
- [ ] Author one source then one line axis for exact mirror, retaining existing supported
  line/polyline/Bezier/non-rational-B-spline behavior and typed unsupported outcomes for circle,
  arc, conic, rational and NURBS families. Multi-source mirror remains deferred.
- [ ] Add a **Modify** section to the left palette with distinct text-free Fillet, Line offset and
  Mirror icons, headless pending/warning/options presentation, accepted scratch preview and
  primary-created-curve selection after commit.
- [ ] Add ordinary editable **2D fillet workshop**, **Associative line offsets** and
  **Mirror construction workshop** leaves under **Curves & constructions**, with no guide,
  protection, scripted action or alternate coordinator.
- [ ] Directly qualify operation expansion, state transitions, branch/request synthesis,
  preview/commit rejection, stale/exhausted work, Undo/Redo, workspace round-trip and sample
  editability. Retain M25/M27/M28 as the derivative and independent-validation owners because M66
  adds no residual.
- [ ] Pass formatting, warnings-denied Clippy, locked all-feature workspace tests, all-feature
  demo-web WASM check, release Trunk build and `git diff --check` from one final source state.
- [ ] Complete and explicitly approve `docs/M66_UAT.md`.

Gate: every visible helper tool is driven by public headless operation-authoring metadata and
commits only an independently accepted public operation proposal through the ordinary retained
history path. Every branch/side/span/winding choice is explicit before publication. Unsupported or
ambiguous geometry is typed and mutation-free; no browser equation or applicability matrix,
general curve offset approximation, global fillet-root enumeration, persistent feature tree,
legacy harness, `/#/dev/lab`, browser E2E or mobile claim is added. M66 closes only after direct
qualification and explicit supervising-human UAT approval.

## Explicit non-goals

The following are not part of the currently approved roadmap:

- solid modeling, B-rep booleans, meshing or a production rendering system;
- 3D sketch curves or a unified 2D/3D sketch entity model;
- global enumeration of every geometric root;
- arbitrary third-party curve or manifold plugins;
- user-authored residual equations or soft ordinary constraints;
- an internal expression/configuration language or host B-rep projection callbacks;
- C/C++ ABI bindings or an approved `unsafe` exception;
- responsive, tablet or mobile support for the desktop demo workbench;
- physical contact, collision detection, friction or impact;
- mass properties, loads, reactions, statics, inverse dynamics or forward dynamics;
- time integration.

These require separate product decisions in a future milestone.
