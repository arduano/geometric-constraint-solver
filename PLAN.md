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

## Architectural boundaries

- Keep `geosolve-sketch` and `geosolve-linkage` as separate domain models over `geosolve-core`.
- Keep CAD entities, rigid bodies, joints and branch types out of `geosolve-core`.
- Keep curve definitions closed and serializable while curve evaluation and residual construction become internally generic.
- Do not expose a public generic curve or manifold trait before the built-in families prove the seam.
- Keep branch, span, winding, active-bound and assembly-mode choices as explicit domain state outside differentiable formulas.
- Use local forward automatic differentiation where it reduces fragile analytic code; retain central finite differences as an independent oracle for every residual.
- Preserve pure Rust, GPL-3.0-or-later licensing and the workspace `unsafe_code = "forbid"` policy.
- Keep `geosolve-demo-web` compiling as a compatibility smoke target, but do not let UI concerns shape core APIs or milestone scope.

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

## M10: persistent solve sessions and first-class bounds

Goal: retain compiled structure across edits and represent bounded coordinates mathematically rather than only through post-solve rejection.

- [ ] Separate immutable problem topology from mutable accepted state and source parameters.
- [ ] Add a persistent `SolveSession` with automatic revision and dirty-component tracking.
- [ ] Preserve domain-to-core mappings across non-structural edits.
- [ ] Cache component layouts, accepted states and structural patterns.
- [ ] Add scalar/tangent-coordinate box bounds and an active-set or projected LM policy.
- [ ] Include active bounds in rank, mobility and audit output.
- [ ] Support endpoint-active curve contacts, positive radii and bounded drivers.
- [ ] Make accepted-state commits atomic through a validated patch or clone-and-swap.

Gate: edits solve only affected components, omitted dirty IDs cannot corrupt state, endpoint-active mobility is truthful, and all prior rollback behavior remains transactional.

## M11: manifold geometry and spatial state

Goal: add the mathematically correct state representation needed by 3D rigid-body kinematics.

- [ ] Add validated `SE(2)` composition, inverse, exponential, logarithm, adjoint, retraction and local difference.
- [ ] Add `Vec3` and quaternion-backed `Pose3`/`SE(3)` with ambient dimension 7 and tangent dimension 6.
- [ ] Define one documented body/world transform and tangent convention.
- [ ] Canonicalize quaternion sign without treating it as an assembly branch.
- [ ] Make fixed and alias elimination manifold-aware.
- [ ] Add validated frame and workplane construction plus point/vector transforms.
- [ ] Expose an accepted-state reduced hard linearization and sensitivity solve API.

Gate: manifold property tests, tangent-coordinate finite differences, global-transform equivariance and quaternion-sign invariance pass without core regressions.

## M12: sparse structure, hierarchy and continuation

Goal: scale the shared kernel before production splines and large spatial assemblies expand the graph.

- [ ] Add indexed block COO/triplet assembly from the canonical linearization.
- [ ] Add structural matching and under/well/over-constrained partitions.
- [ ] Convert to `faer` sparse storage and rank-revealing sparse least-squares.
- [ ] Retain dense QR/SVD fallback for small or diagnostically ambiguous components.
- [ ] Cache symbolic ordering/factorization structure.
- [ ] Record and enforce a benchmark-derived dense/sparse crossover policy.
- [ ] Replace large explicit dense nullspaces with sparse-compatible hierarchy operations.
- [ ] Support secondary objectives spanning multiple hard components.
- [ ] Add adaptive predictor-corrector and pseudo-arclength continuation.

Gate: dense and sparse paths agree on independently validated geometry, rank, mobility, diagnostics and branch state; the documented planar toggle crosses only through the explicit pseudo-arclength path.

---

# Domain architecture migration

## M13: generic sketch design graph

Goal: remove the entity-pair compiler fan-out before adding more curve families.

- [ ] Add persistent external IDs separate from runtime generational keys.
- [ ] Add design points and typed design scalars with units and domains.
- [ ] Add one stable `CurveId` store and closed `CurveDefinition` for existing segments, circles and arcs.
- [ ] Add semantic `FeatureRef` values for endpoints, centers, axes, controls and fixed curve locations.
- [ ] Add stable contact slots with numeric domains, periodic winding, active span and neighborhood state.
- [ ] Add generic dependency/reference collection.
- [ ] Add per-source compiled residuals, contact mappings, validators, commit mappings and audit metadata.
- [ ] Replace the central M7 candidate validator and latent-role matches for migrated constraints.
- [ ] Add generic measurements shared by driving and reference dimensions.
- [ ] Add a versioned document envelope and deterministic runtime-ID remapping.

Gate: S1-S3 and the full M5/M7 corpus remain unchanged; existing line/circle/arc constraints use the new graph and no longer require geometry-pair-specific lifecycle plumbing.

## M14: shared planar kinematic architecture

Goal: migrate the planar linkage baseline onto the architecture that spatial assemblies will share.

- [ ] Separate model topology, accepted state and compiled session.
- [ ] Add persistent body, feature and source IDs separate from runtime generational keys.
- [ ] Make local coordinate frames the primary body-feature representation.
- [ ] Distinguish physical grounding from numerical gauge removal.
- [ ] Add explicit gauge policies for floating components.
- [ ] Move velocity solving onto the shared reduced-linearization/rank policy.
- [ ] Preserve `geosolve-linkage` as the public crate and retain the existing planar API as a compatibility facade where practical.

Gate: L1-L3 remain unchanged; floating planar assemblies report three world-gauge DOF separately from internal mobility, and alternative gauges preserve all relative geometry and diagnostics.

---

# Parallel product expansion

## M15: editable Bezier curves and generic contact

Goal: prove that editable design derivatives and contact equations are curve-generic before implementing splines.

- [ ] Move immutable curve-jet evaluation into `geosolve-geometry`.
- [ ] Add position and first through third parameter derivatives with typed regularity/domain metadata.
- [ ] Add editable quadratic and cubic Bezier entities whose controls are design variables.
- [ ] Add generic point-on-curve, curve/curve contact and tangent residual templates.
- [ ] Add endpoint tangency and explicit same/opposite tangent orientation.
- [ ] Differentiate every incident control coordinate and contact parameter through local AD.
- [ ] Keep span and branch selection outside AD.

Gate: line/circle/arc/Bezier combinations use the same generic residual templates; every control and contact derivative passes finite differences; cusps and zero-speed contacts cannot report success.

## M16: spatial kinematics vertical slice

Goal: prove the spatial state, feature and gauge architecture with a minimal useful assembly set.

- [ ] Add `SpatialAssembly` within `geosolve-linkage`.
- [ ] Add spatial rigid bodies and body-local point/frame features.
- [ ] Add physical ground and automatic floating-component gauge policies.
- [ ] Add fixed-frame, ball and revolute joints/mates.
- [ ] Add source mapping, accepted geometry, audit, rank/mobility and rollback APIs.
- [ ] Add transformed/scaled exact and perturbed fixtures.

Gate: every primitive reports expected relative DOF and passes tangent-space Jacobian, gauge, invalid-feature and independent-validation tests.

## M17: ellipses and parametric conics

Goal: cover the major analytic CAD curve family without introducing implicit coefficient gauges.

- [ ] Add ellipses and elliptical arcs with explicit axis/orientation state.
- [ ] Add rational-quadratic conic segments.
- [ ] Add explicit parabola/hyperbola branches and trimmed parameter domains.
- [ ] Add center, focus, axis and endpoint features.
- [ ] Add ellipse/conic measurements justified by CAD use cases.
- [ ] Preserve valid circle-limit geometry while reporting unobservable orientation truthfully.

Gate: analytic jet oracles, affine/similarity transformations, branch retention and rational-pole rejection pass; generic contact/tangency adds no conic-pair equation code.

## M18: spatial mate and joint catalog

Goal: support the common CAD assembly and linkage relationships in three dimensions.

- [ ] Add axis and plane features with stable local clocking.
- [ ] Add prismatic, cylindrical, planar and universal joints.
- [ ] Add distance, angle, axis-alignment and frame-offset mates.
- [ ] Add hinge and translation coordinates with position drivers.
- [ ] Add explicit axis parity, winding, side and signed-volume branch monitors.
- [ ] Add multiple simultaneous drivers and explicit assembly-mode transactions.

Gate: every joint/mate has exact, recovery, tangent-Jacobian, scale, mixed-scale, degeneracy and expected-mobility fixtures; representative shaft/bearing and block/base CAD assemblies pass.

## M19: non-rational B-splines

Goal: add locally supported production spline geometry over the generic curve/contact architecture.

- [ ] Add validated degree, control identity and nondecreasing knot vectors.
- [ ] Add de Boor evaluation and jets through third derivative.
- [ ] Add clamped and periodic curves.
- [ ] Add stable semantic span identities and one-sided knot evaluation.
- [ ] Restrict residual incidence to the active span's local control support.
- [ ] Add knot insertion with geometry invariance.
- [ ] Add continuity diagnostics from knot multiplicity.

Gate: Bezier equivalence, affine covariance, partition of unity, knot insertion, local support and span-transition tests pass; malformed knots and insufficient continuity reject before success.

## M20: NURBS and advanced CAD constraints

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

## M21: 2D/3D assembly completion

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

## M22: public API and release hardening

Goal: make both deliverables ready for a stable library release.

- [ ] Review public APIs and remove accidental exposure of compiler/core internals.
- [ ] Finalize versioned serialization and migration policy.
- [ ] Add crate-level documentation and complete examples for both deliverables.
- [ ] Define semantic versioning, changelog and deprecation policy.
- [ ] Complete GPL/licence and attribution audit.
- [ ] Record supported scale/performance envelopes and benchmark baselines.
- [ ] Run malformed-document and degenerate-geometry fuzzing without panic or false success.
- [ ] Keep the WASM crate compiling as a non-authoritative smoke consumer of public APIs.

Gate: all acceptance suites, serialization round trips/migrations, fuzz corpora, documentation tests, performance baselines, native checks and locked WASM smoke builds pass.

## Explicit non-goals

The following are not part of M8-M22:

- solid modeling, B-rep booleans, meshing or rendering;
- global enumeration of every geometric root;
- arbitrary third-party curve or manifold plugins;
- physical contact, collision detection, friction or impact;
- mass properties, loads, reactions, statics, inverse dynamics or forward dynamics;
- time integration.

These require separate product decisions after both library deliverables are complete.
