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
- versioned persistence of topology, geometry, constraints and discrete state.

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

Post-alpha browser policy: the playground is a robust sanity-checking instrument for the supervising user to inspect claims and expose defects, not a production application. Desktop interaction density and layout freedom take priority. Existing mobile behavior may remain, but mobile compatibility is best-effort and is not a gate for future numerical or domain milestones.

## Architectural boundaries

- Keep `geosolve-sketch` and `geosolve-linkage` as separate domain models over `geosolve-core`.
- Keep CAD entities, rigid bodies, joints and branch types out of `geosolve-core`.
- Keep curve definitions closed and serializable while curve evaluation and residual construction become internally generic.
- Do not expose a public generic curve or manifold trait before the built-in families prove the seam.
- Keep branch, span, winding, active-bound and assembly-mode choices as explicit domain state outside differentiable formulas.
- Use local forward automatic differentiation where it reduces fragile analytic code; retain central finite differences as an independent oracle for every residual.
- Preserve pure Rust, GPL-3.0-or-later licensing and the workspace `unsafe_code = "forbid"` policy.
- Keep `geosolve-demo-web` as a separate public-API consumer. M13-M14 may shape embeddable sketch workflows, but web-only interaction and rendering concerns must not enter reusable Rust document/session APIs.

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
8. update this file with checked items and concise completion notes.

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

- [ ] Add validated degree, control identity and nondecreasing knot vectors.
- [ ] Add de Boor evaluation and jets through third derivative.
- [ ] Add clamped and periodic curves.
- [ ] Add stable semantic span identities and one-sided knot evaluation.
- [ ] Restrict residual incidence to the active span's local control support.
- [ ] Add knot insertion with geometry invariance.
- [ ] Add continuity diagnostics from knot multiplicity.

Gate: Bezier equivalence, affine covariance, partition of unity, knot insertion, local support and span-transition tests pass; malformed knots and insufficient continuity reject before success.

## M22: NURBS and advanced CAD constraints

Goal: complete Deliverable 1.

- [ ] Add positive rational weights and homogeneous de Boor jets.
- [ ] Add weight derivatives and an explicit weight-gauge policy.
- [ ] Add rational-denominator and mixed-scale ambiguity diagnostics.
- [ ] Add signed/unsigned curvature and osculating-radius measurements.
- [ ] Add equal-curvature and G2 continuity constraints.
- [ ] Add separately named parametric C2 continuity.
- [ ] Add generic normal/tangent and endpoint continuity constraints.
- [ ] Complete persistence for every curve, feature, dimension, contact, span and branch state.
- [ ] Add sketch fuzz/property, differential-oracle and large sparse performance corpora.

Gate: unit-weight NURBS reproduce B-splines, quadratic NURBS reproduce canonical conics, local support remains bounded by degree, curvature derivatives validate, and the complete 2D CAD acceptance matrix passes.

---

# Kinematic completion

## M23: 2D/3D assembly completion

Goal: complete Deliverable 2 without adding physics.

- [ ] Generalize adaptive and pseudo-arclength continuation to spatial assemblies.
- [ ] Add branch-boundary events, hysteresis and explicit mode-change APIs.
- [ ] Add multiple-driver velocity-level kinematic queries.
- [ ] Distinguish determinate, underdetermined and inconsistent velocity requests.
- [ ] Return body and feature velocities plus optional motion/nullspace bases.
- [ ] Add planar mechanisms embedded in 3D and compare them against planar oracles.
- [ ] Add spatial closed-chain, mixed-scale and large sparse assembly scenarios.
- [ ] Complete persistence for bodies, features, joints, mates, gauges, drivers and assembly modes.
- [ ] Add linkage fuzz/property, differential-oracle and performance corpora.

Gate: planar and spatial assemblies preserve explicit modes, report truthful mobility, validate every accepted configuration and velocity equation, and retain the last accepted state on all failures.

## M24: public API and release hardening

Goal: make both deliverables ready for a stable library release.

- [ ] Review public APIs and remove accidental exposure of compiler/core internals.
- [ ] Finalize versioned serialization and migration policy.
- [ ] Add crate-level documentation and complete examples for both deliverables.
- [ ] Define semantic versioning, changelog and deprecation policy.
- [ ] Complete GPL/licence and attribution audit.
- [ ] Record supported scale/performance envelopes and benchmark baselines.
- [ ] Run malformed-document and degenerate-geometry fuzzing without panic or false success.
- [ ] Keep the disposable WASM playground compiling as a non-authoritative consumer of public APIs.

Gate: all acceptance suites, serialization round trips/migrations, fuzz corpora, documentation tests, performance baselines, native checks and locked WASM smoke builds pass.

## Explicit non-goals

The following are not part of M8-M24:

- solid modeling, B-rep booleans, meshing or a production rendering system;
- global enumeration of every geometric root;
- arbitrary third-party curve or manifold plugins;
- physical contact, collision detection, friction or impact;
- mass properties, loads, reactions, statics, inverse dynamics or forward dynamics;
- time integration.

These require separate product decisions after both library deliverables are complete.
