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
the complete production embedding contract. M33-M55 close the ordinary CAD
catalog, host integration, interaction-consumer and release gaps. This deliverable
does not include a solid B-rep kernel, meshing or 3D sketch curves.

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

Post-alpha browser policy: the playground is a robust sanity-checking instrument for
the supervising user to inspect claims and expose defects, not a production
application. M39, M44 and M51 replace it with a desktop-only CAD-like sketch
workbench that remains a non-authoritative public-API consumer. Future mobile or
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
- supported embedding targets are Rust and WASM only through M55;
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
- Do not call host code during residual evaluation. Host parameters and external
  geometry enter one attempt as immutable revisioned values.
- Keep cross-system expressions, B-rep projection, topological naming, feature
  history and application undo outside the sketch solver contract.
- Keep future human acceptance limited to M40, M45, M52 and M54. Every objective
  correctness, persistence, compatibility and browser assertion must pass
  automatically before a human checkpoint begins.

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

Status: active.

Goal: freeze the product and ownership decisions for a host-usable planar sketch
engine before changing persistence or public state semantics.

- [ ] Accept ADRs for design-versus-accepted state, host parameters, immutable external snapshots, cancellation/concurrency, companion APIs and draft-v5 development.
- [ ] Freeze a complete entity/feature/constraint/dimension applicability matrix and identify every unsupported ordinary CAD combination.
- [ ] Define accepted-state identity over design revision, parameter revision, external-snapshot digest, activation state and solver policy.
- [ ] Define the ownership boundaries for host undo, expressions, units, projection, topology and production profiles.
- [ ] Add deterministic representative workloads for connected, disconnected, spline-heavy, parameter-heavy, external-reference and activation-heavy sketches.
- [ ] Record cold compile, warm edit, diagnostics, profile, cancellation-latency and memory measurement boundaries without imposing premature thresholds.

Gate: every new semantic concept has an accepted decision and representative fixture;
M1-M32 behavior and frozen wire languages remain unchanged.

## M34: retained design and accepted solved state

Status: planned.

Goal: retain structurally valid unsolved intent without ever presenting it as
accepted geometry.

- [ ] Separate design, attempted-candidate and independently accepted solved views with distinct revisions.
- [ ] Retain structurally valid conflicts, unavailable references and failed unsuppression in design intent.
- [ ] Continue rejecting malformed, non-finite, resource-invalid or referentially invalid design transactions entirely.
- [ ] Publish optional finite attempted geometry as non-authoritative evidence only.
- [ ] Preserve the last accepted state across topology-changing unsolved edits and identify which design revision it solved.
- [ ] Define persistence and host display rules for design elements that have no accepted solved counterpart yet.

Gate: no unsolved or attempted state can produce a success-like status, accepted
revision or authoritative audit; retained design can be repaired through ordinary
transactions.

## M35: cancellation and operation control

Status: planned.

Goal: make every potentially expensive sketch operation safely interruptible by an
interactive host.

- [ ] Add cooperative cancellation and deterministic work controls to lowering, nonlinear solving, rank, diagnostic trials and profile analysis.
- [ ] Distinguish cancellation, work exhaustion, numerical rejection, invalid geometry and convergence.
- [ ] Add documented cancellation checkpoints and measured maximum latency around non-interruptible kernels.
- [ ] Let hosts implement deadlines through the same cancellation mechanism without making wall time part of correctness.
- [ ] Prove cancellation retains accepted state and cannot commit a partially validated result.
- [ ] Keep native and single-threaded WASM behavior equivalent.

Gate: cancelled work never reports convergence or valid publication, commits nothing
and leaves the prior accepted state bitwise unchanged.

## M36: semantic feature and scalar foundations

Status: planned.

Goal: create closed typed operands for a complete CAD catalog without exposing an
arbitrary curve or residual plugin interface.

- [ ] Introduce capability-specific references for points, centers, endpoints, controls, directions, line supports, curve spans and scalar properties.
- [ ] Add explicit fixed/equal scalar semantics and the units/domains required by signed length, angle, dimensionless, curvature and parameter values.
- [ ] Preserve one persistent semantic source when a relation lowers into several ordinary rows.
- [ ] Make every operand serializable, audit-readable and branch-aware without coordinate inference.
- [ ] Add exhaustive current-schema command/effect/measurement characterization before extending the catalog.

Gate: every semantic operand validates through persistent IDs, lowers deterministically
and has exact, malformed, persistence and audit coverage.

## M37: standard planar constraint catalog

Status: planned.

Goal: cover the ordinary geometric relations expected from a production planar
sketch engine.

- [ ] Add first-class concentric and collinear relations.
- [ ] Add horizontal and vertical relations between arbitrary point features.
- [ ] Add grouped whole-entity block/fix and point symmetry about a center.
- [ ] Generalize line/entity symmetry and equal circular radius across compatible circles and arcs.
- [ ] Add equal scalar, distance and angle relations where the applicability matrix permits them.
- [ ] Add high-level contact and tangent constructors that allocate and validate explicit latent branch state for hosts.
- [ ] Preserve explicit same/opposite direction, side, neighborhood and containment choices for every multi-root relation.

Gate: the frozen standard relation matrix has no undocumented family gaps; every new
row passes derivative, transform, scale, persistence, branch, cancellation and
rollback gates.

## M38: dimensions and persistent measurements

Status: planned.

Goal: complete the normal dimensional vocabulary and make measurement semantics
available without UI formulas.

- [ ] Add signed relative horizontal/vertical and absolute datum-coordinate dimensions.
- [ ] Add signed point-to-line distance and parallel-line separation.
- [ ] Add two-line and three-point angle, circular sweep and circular arc-length dimensions.
- [ ] Add driving/reference ellipse-axis and supported conic-property dimensions.
- [ ] Persist curvature, osculating-radius and generic measurement definitions with typed units and provenance.
- [ ] Add equation-free bounded length measurement for every regular bounded curve.
- [ ] Add driving and equal path-length constraints only with bounded value/derivative evaluation and typed work exhaustion.
- [ ] Remove the misleading line-only public meaning of `CurveLength` through a migration-safe replacement.

Gate: every driving and reference form agrees on the same independently evaluated
measurement; no integral dimension succeeds outside its certified work and derivative
contract.

## M39: CAD workbench foundation and core authoring

Status: planned.

Goal: begin the desktop-only rewrite early enough to test the ordinary CAD interaction
model before host and advanced workflows build on it.

- [ ] Split application state, domain controller, tools, selection, scene, panels, persistence and browser platform code into explicit modules.
- [ ] Make one domain adapter the sole owner of sketch sessions and unstable core-report translation.
- [ ] Add explicit accepted, design-unsolved, solving, solved-preview and rejected-attempt application states.
- [ ] Build a CAD-like desktop shell with command bar, tool palette, sketch tree, full-height canvas, property inspector, status bar and Problems drawer.
- [ ] Add retained scene layers, adaptive public-jet tessellation and revision-keyed geometry/profile caches.
- [ ] Implement point, line/polyline, rectangle, circle/arc, core constraints, dimensions, drag, delete, undo and redo through public commands.
- [ ] Draw selectable persistent constraint glyphs and driving/reference dimensions without recomputing equations in the browser.
- [ ] Keep the old advanced lab available only through an explicit temporary developer route during migration.

Gate: automated desktop E2E proves every core workflow, accepted-state retention and
canvas/tree/inspector synchronization before human UAT begins. Mobile support is not
implemented or tested.

## M40: human UAT 1 - core sketch interaction

Status: planned; human approval required.

Goal: retire the risk that the basic creation, selection, constraint and dimension
interaction model is mathematically correct but unsuitable for CAD authoring.

- [ ] Prequalify all objective core workflows through native, WASM and isolated desktop browser automation.
- [ ] Provide one URL, deterministic resets and a 30-45 minute core drafting script requiring no build or numeric comparison.
- [ ] Exercise geometry creation, constraints, driving/reference dimensions, constrained drag, conflict/redundancy, delete and history.
- [ ] Assess whether accepted, preview, unsolved and rejected states are unmistakable.
- [ ] Capture every finding as a scene capsule, screenshot, action transcript and accepted/attempted diagnostic bundle.
- [ ] Convert objective findings into native and browser regressions and complete only the necessary targeted human rechecks.

Gate: the supervising human explicitly approves the core interaction scorecard and no
correctness, data-loss, misleading-state or basic-usability blocker remains.

## M41: construction roles and activation

Status: planned.

Goal: represent construction and configuration semantics as explicit domain state.

- [ ] Add persisted regular/profile and construction geometry roles.
- [ ] Keep construction geometry fully constrainable while excluding it from production profiles by default.
- [ ] Generalize activation over entities, constraints, dimensions and associations.
- [ ] Distinguish user suppression, host-configuration inactivity, unavailable dependency and unavailable external reference.
- [ ] Validate the effective active dependency closure and report every inactivity reason.
- [ ] Preserve branch, span, winding, contact and ownership state while inactive and across reactivation.

Gate: activation changes are atomic, never evaluate dangling dependencies and never
infer a new branch from retained coordinates.

## M42: typed host parameters

Status: planned.

Goal: let host-owned expression/configuration systems drive sketches through finite
typed values without becoming solver variables or a second expression language.

- [ ] Add persistent parameter identities and typed length, angle, dimensionless and activation input bindings.
- [ ] Accept immutable revisioned parameter batches and map dependencies to affected sources/components.
- [ ] Allow one parameter to drive multiple dimensions without adding an artificial unknown.
- [ ] Return declared reference measurements as revision-stamped output proposals with units and provenance.
- [ ] Reject input/output ownership cycles and stale parameter commits atomically.
- [ ] Keep expression parsing, unit display and configuration dependency graphs host-owned.

Gate: identical design and parameter bytes reproduce identical accepted geometry and
diagnostics; rejection changes no accepted input or output revision.

## M43: immutable external 2D references

Status: planned.

Goal: constrain native sketch geometry against other sketches or model geometry
without callbacks, hidden fixed copies or coordinate-based repair.

- [ ] Persist stable local external-binding identity and expected feature kind.
- [ ] Accept immutable finite 2D point/curve snapshots carrying revision, digest, domain, orientation, scale and resource evidence.
- [ ] Integrate external features into the same typed operand and audit system as native geometry without adding solver variables.
- [ ] Require explicit rebinding/remapping for family, span or topology changes.
- [ ] Report missing, stale, duplicate, wrong-kind, non-finite, oversized and incompatible snapshots as typed unsolved-design outcomes.
- [ ] Keep arbitrary host/PDM keys and 3D projection computation outside sketch equations and canonical sketch state.
- [ ] Let diagnostic capsules bundle design, parameter and snapshot inputs for reproducibility without making stored status authoritative.

Gate: one attempt validates against exactly one immutable snapshot set and records its
revision/digest; no host callback or proximity inference participates in solving.

## M44: host-state workbench integration

Status: planned.

Goal: expose construction, activation, parameters, references and dual-state behavior
coherently through the CAD-like desktop consumer.

- [ ] Add construction styling and explicit profile participation controls.
- [ ] Add activation/suppression editors distinct from driving/reference dimension mode.
- [ ] Add parameter inputs, bindings and output proposals without exposing internal design scalars indiscriminately.
- [ ] Add external-reference tree entries, styling, revision/digest status and explicit rebind workflows.
- [ ] Display design, attempted and accepted revisions together whenever they differ.
- [ ] Prove atomic batch updates, stale/missing inputs and accepted-state retention in browser automation.

Gate: every host-state workflow is objectively qualified before the second human UAT
checkpoint; the browser still contains no equation or host callback.

## M45: human UAT 2 - CAD host semantics

Status: planned; human approval required.

Goal: retire trust and comprehension risks around construction, activation,
parameters, external references and retained unsolved intent.

- [ ] Prequalify all state/revision/digest/atomicity behavior automatically.
- [ ] Provide a 30-45 minute prepared script for role conversion, suppression/reactivation, shared parameter updates, invalid configurations and external-reference loss/recovery.
- [ ] Assess whether stale external data, solver rejection and unsolved design are visibly distinct.
- [ ] Assess whether parameter ownership, output proposals and activation reasons are understandable.
- [ ] Capture findings automatically and convert objective defects into regressions.
- [ ] Require only targeted human rechecks unless the host-state workflow changes materially.

Gate: the supervising human approves the host-semantics scorecard and no state-trust,
recovery or ownership blocker remains.

## M46: stable diagnostics and mobility evidence

Status: planned.

Goal: let CAD hosts explain and repair sketches without consuming unstable core
reports or parsing display strings.

- [ ] Publish stable sketch-owned solve, source, component, dependency, activation, parameter and external-reference diagnostic DTOs.
- [ ] Keep design, attempt and accepted revisions explicit in every relevant diagnostic.
- [ ] Report structural and numerical rank, equality/bounded/one-sided DOF and diagnostic completeness separately.
- [ ] Add budgeted mobility witnesses mapped to persistent point/scalar identities.
- [ ] Add bounded conflict cores or ranked repair candidates without claiming global minimality.
- [ ] Publish stable machine-readable action suggestions that create ordinary explicit transactions.
- [ ] Move direct core reports behind a clearly unstable advanced-diagnostics seam.

Gate: a host can render every supported success, failure, incompleteness and repair
path from persistent domain IDs and stable codes alone.

## M47: prepared jobs and concurrency contract

Status: planned.

Goal: let native and WASM hosts schedule sketch work safely without an internal async
runtime or thread pool.

- [ ] Make accepted snapshots immutable and shareable under a documented Rust ownership contract.
- [ ] Prepare solve/profile jobs against exact design, parameter, external, activation and policy revisions.
- [ ] Return attempted diagnostics and a candidate patch without mutating session state.
- [ ] Add compare-and-swap commit that rejects every stale input revision.
- [ ] Specify synchronous single-writer sessions and host-managed worker scheduling.
- [ ] Prove deterministic publication independent of scheduling order and cancellation timing.
- [ ] Add compile-time and runtime tests for the intended `Send`/`Sync` surface without adding `unsafe` code.

Gate: stale or cancelled work remains inspectable but can never overwrite a newer
accepted state; native and single-threaded WASM consumers share one semantic contract.

## M48: incremental solving and production scale

Status: planned.

Goal: stop rebuilding the complete document for ordinary edits and publish an honest
supported production envelope.

- [ ] Retain persistent-to-runtime mappings and patch nonstructural edits through the existing session machinery.
- [ ] Rebuild only affected topology/dependency closures for structural, parameter, reference and activation changes.
- [ ] Add indexed stores and structural-sharing history without changing canonical order or ID high-water semantics.
- [ ] Add profile broad-phase indexing and revision-keyed immutable piece/bounds caches while preserving certified narrow-phase decisions.
- [ ] Benchmark cold import, warm edit, drag, diagnostics, parameter/reference updates, cancellation latency, profiles and memory separately.
- [ ] Evaluate a pure-Rust rank-revealing sparse authority against dense SVD or publish an explicit connected-component size limit if parity cannot be proved.
- [ ] Preserve complete fresh validation and diagnostic evidence on every optimized return path.

Gate: incremental and full rebuild paths agree on accepted geometry, status, rank,
branch and source diagnostics; all documented workload envelopes pass.

## M49: sketch operations companion

Status: planned.

Goal: provide reusable drafting operations without moving construction algorithms or
private equations into the numerical solver.

- [ ] Establish a separate companion API/crate that produces public sketch transactions and owns no residual formulas.
- [ ] Add general split, break, trim and extend with identity-preserving result mappings.
- [ ] Generalize visible topology to multiple explicit intervals per immutable support.
- [ ] Add family-complete mirror where exact parameter transformation is available.
- [ ] Add chamfer and preserve explicit ownership for existing fillet workflows.
- [ ] Add rectangle, polygon, slot and pattern expansion into ordinary geometry and grouped sources.
- [ ] Keep general spline/conic offset approximation and persistent pattern-object personality outside the M55 gate.

Gate: every operation is deterministic, transactional, dependency-aware and
replaceable by an equivalent host transaction without changing solver semantics.

## M50: production topology companion

Status: planned.

Goal: turn accepted visible geometry into trustworthy wire/profile input for a CAD
feature system without creating B-rep or solver state.

- [ ] Establish a separate production topology API/crate over accepted sketch snapshots and exact input digests.
- [ ] Publish revision-stamped wires, orientation, nesting, holes, source-span provenance and bounded region boundaries.
- [ ] Define explicit production policies for tangency, overlap, touching contours, T-junctions and self-intersections.
- [ ] Filter construction and external geometry only through declared query scope.
- [ ] Keep production results ephemeral and distinct from visual-analysis faces, persistent sketch entities and solver sources.
- [ ] Preserve `Complete`, `Truncated`, `Skipped` and cancellation evidence with consumed budgets.

Gate: downstream CAD features may consume only `Complete` output for the exact
accepted-state/input digest; ambiguous, stale or incomplete topology is unusable.

## M51: advanced workbench completion and automated qualification

Status: planned.

Goal: complete the CAD-like desktop demo over advanced geometry, operations,
diagnostics and topology, then remove the old playground.

- [ ] Add conic, Bezier, B-spline and NURBS controls, weights, knots, gauges and periodic transitions.
- [ ] Add explicit editors for sweep, side, orientation, winding, neighborhood and other branch state.
- [ ] Add companion fillet, trim, split, extend, mirror, chamfer, pattern, polygon and slot workflows.
- [ ] Add production profile inspection, source navigation and truthful incomplete-state presentation.
- [ ] Add conflict navigation, redundancy display, mobility evidence, cancellation and stale-result presentation.
- [ ] Add a versioned desktop workspace envelope for annotations, panel state, attributes, parameters and external descriptors around canonical sketch inputs.
- [ ] Split browser automation into isolated core, host-state, advanced, persistence, profile and performance suites.
- [ ] Remove the old playground state, legacy application, hidden DOM and obsolete CSS after automated parity.
- [ ] Generate deterministic UAT scenes, capsules, instructions and automatic finding evidence.

Gate: every advertised workflow passes native, WASM and fresh-profile desktop browser
qualification without retries; no legacy alternate application or browser equation
remains.

## M52: human UAT 3 - advanced geometry and topology

Status: planned; human approval required.

Goal: retire perceptual branch, advanced-authoring, topology-trust and interaction-
performance risks after objective qualification is complete.

- [ ] Provide a 45-60 minute prepared script covering conics, splines/NURBS, weights/knots, periodic transitions and explicit branch controls.
- [ ] Exercise fillets, trims, mirrors, patterns and other companion operations.
- [ ] Inspect valid profiles, holes, self-intersections and intentionally incomplete topology.
- [ ] Exercise rapid edits, cancellation and one representative medium sketch.
- [ ] Assess local predictability, branch clarity, coherent associated motion, topology trust and interactive responsiveness.
- [ ] Capture findings automatically, add regressions and perform targeted human rechecks.

Gate: the supervising human approves the advanced/topology scorecard and no wrong-
branch, misleading-profile, interaction or responsiveness blocker remains.

## M53: API and schema release-candidate freeze

Status: planned.

Goal: make one deliberate compatibility cut only after ordinary, host and advanced
workflows have passed their phase UAT.

- [ ] Freeze one final sketch v5 language with deterministic direct migration from frozen v1-v4.
- [ ] Freeze separate versioned parameter, external-snapshot and desktop-workspace envelopes.
- [ ] Freeze the supported Rust/WASM facade, request builders, diagnostics, capability queries, cancellation and threading contracts.
- [ ] Remove fixture/scenario and unstable compiler/runtime types from the primary production surface.
- [ ] Pass schema goldens, downstream compile fixtures, SemVer checks, packaged-crate examples and migration mutation tests.
- [ ] Record the candidate commit, toolchains, performance/resource envelopes and complete automated evidence bundle.
- [ ] Revoke and requalify the candidate after any API, schema, persistence or major workflow change.

Gate: the exact release candidate passes every native, WASM, browser, fuzz, mutation,
performance, package, documentation and licence gate with no known correctness or
trust blocker.

## M54: human UAT 4 - integrated release candidate

Status: planned; human approval required.

Goal: validate end-to-end trust and coherence on the frozen candidate rather than
repeat an exhaustive feature matrix.

- [ ] Provide one 45-60 minute integrated workflow from empty sketch through ordinary/construction geometry, constraints, parameters, external references, advanced curves and an associative operation.
- [ ] Introduce and repair a conflict, inspect a production profile and exercise save/reload/history/capsule recovery.
- [ ] Include a short unscripted exploratory authoring period.
- [ ] Assess whether normal work is unobtrusive, failures are trustworthy and advanced diagnostics remain available without dominating the workflow.
- [ ] Capture and regress every objective finding; request only targeted rechecks unless the candidate changes materially.
- [ ] Record explicit human sign-off and disposition of nonblocking polish findings.

Gate: the supervising human ratifies the release-candidate scorecard and no integrated
correctness, data-loss, topology, persistence or trust blocker remains.

## M55: production embedding release gate

Status: planned.

Goal: prove a real CAD host can build on the frozen contract without duplicating
equations and publish a CAD-engine-ready release candidate.

- [ ] Add a mock host that owns expressions, parameter values, external keys, attributes, cross-system history and worker scheduling.
- [ ] Exercise retained unsolved intent, immutable snapshots, parameters, activation, cancellation, stale jobs, stable diagnostics and production topology through public APIs only.
- [ ] Add coverage-guided fuzz targets and resource limits for every document/input/transaction/profile envelope.
- [ ] Gate Linux, Windows, macOS and `wasm32-unknown-unknown` Rust consumers without adding a C ABI.
- [ ] Build examples from packaged archives and complete MSRV, dependency, advisory, licence and documentation checks.
- [ ] Publish compatibility, migration, performance, security and resource records.
- [ ] Run one reproducible release command covering all M1-M54 automated acceptance and recorded human approvals.

Gate: no input can panic, exceed its declared interruption/resource policy, publish
non-finite data, falsely report success/complete topology or commit stale work. The
result is a production embedding release candidate; a `1.0` label waits for at least
one real downstream CAD integration.

## Explicit non-goals

The following are not part of M8-M55:

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

These require separate product decisions after the production embedding gate.
