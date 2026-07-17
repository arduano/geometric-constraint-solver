# Canonical scenarios

These canonical scenarios are shared by domain tests and `geosolve-demo-web`. Their constructors belong in reusable domain test support; the web crate must not duplicate equations.

All lengths below are model units. Each scenario should also run under uniform scales `1e-6` and `1e6` where specified by `ACCEPTANCE.md`.

## S1 — Underconstrained triangle with drag target

Purpose: first end-to-end CAD sketch, local DOF and hard-vs-temporary behavior.

Initial points:

- A = `(0, 0)`;
- B = `(4, 0)`;
- C = `(2.2, 2.0)`.

Hard constraints:

1. A fixed at `(0, 0)`;
2. segment AB horizontal;
3. `length(AB) = 4`;
4. `distance(A, C) = 3`.

Expected state:

- B resolves to `(4, 0)` using the initial/rightward branch;
- C remains on the radius-3 circle around A;
- local DOF is `1`;
- dragging C supplies a temporary target projected onto that circle;
- hard constraints remain within tolerance.

Browser interaction:

- drag C by pointer;
- display the unconstrained-motion cue/tangent if convenient;
- release should preserve the accepted nearby position.

## S2 — Conflicting rectangle

Purpose: source-level conflict diagnosis.

Initial points:

- A = `(0, 0)`;
- B = `(4, 0)`;
- C = `(4, 3)`;
- D = `(0, 3)`.

Hard constraints:

- A fixed;
- AB and CD horizontal;
- BC and DA vertical;
- A/B/C/D connected as a rectangle;
- driving dimension source `width-4`: `length(AB) = 4`;
- driving dimension source `width-5`: `length(AB) = 5`.

Expected state:

- no solution validates;
- the two width source IDs appear as conflict candidates;
- ordinary orientation constraints are not blamed when either width source alone restores solvability.

Redundancy variant:

- replace `width-5` with a second `length(AB) = 4` source;
- geometry converges and the duplicate is classified as redundant, not conflicting.

## S3 — Tangent circles

Purpose: branch-sensitive curve constraints.

Geometry:

- circle A: centre `(0, 0)`, radius `2`, fixed;
- circle B: initial centre `(5, 0.5)`, radius `1`;
- B centre constrained horizontal from A centre;
- explicit external tangency mode.

Expected state:

- B centre resolves to `(3, 0)` on the initial positive-x side;
- switching to internal tangency is an explicit state change, not a solver branch accident;
- internal tangency with A containing B resolves B centre to `(1, 0)` while preserving the positive-x centre-direction branch;
- zero or negative effective radius is rejected as invalid geometry.

Browser interaction:

- switch explicitly between external and internal/A-contains-B modes;
- render the selected containment, shared contact point, centre distance and retained branch state;
- update geometry and the equation audit from the same accepted solve result.

## Frozen M7 auxiliary curve verification fixtures

These browser-only constructions exercise public sketch APIs but are not additional canonical scenarios:

- bounded arc contact: drag a point over an explicit 240-degree counterclockwise arc; accepted targets project onto the active span and genuine endpoint escape retains the prior geometry/contact/audit;
- bounded line-circle tangency: drag a fixed-radius circle along the left side of a finite line segment; the latent line/circle contacts move with it and requests beyond either endpoint retain the prior accepted state.
- free-radius circle-arc tangency: drag a circle center in two dimensions outside a fixed 300-degree arc; no circle radius dimension is present, so the radius and circle/arc contacts solve automatically while requests in the missing span retain the prior accepted state.

These fixtures render contact parameters and branch/domain state from the public sketch result. The web crate does not duplicate their constraint equations.

## 2D Sketch Playground Alpha acceptance scenarios

A1-A10 are M14 gates. Their constructors, command sequences and assertions belong in reusable Rust/domain test support. Browser E2E invokes those public APIs and may duplicate pointer/keyboard/touch actions, but never equations or authoritative expected geometry.

Unless a scenario says otherwise, every success requires `HardValidity::Valid`, maximum normalized hard residual `<= 1e-9`, finite accepted geometry and audit rows from that same accepted revision. Failure retains the complete prior accepted document and visible geometry.

### A1 - Constrained rectangle

Construct the rectangle macro from fixed lower-left A = `(0, 0)`, initial width `4` and height `3`, with the positive-x/positive-y orientation branch. The macro must emit four ordinary lines/polyline edges, shared/coincident corners, horizontal top/bottom, vertical left/right, and driving width/height dimensions; it must not emit a privileged rectangle residual.

Edit the driving dimensions to width `6` and height `2.5`.

Expected accepted geometry and report:

- A = `(0, 0)`, B = `(6, 0)`, C = `(6, 2.5)`, D = `(0, 2.5)`;
- a reference diagonal dimension reports `6.5` and adds no equation;
- all four persistent corner IDs and all emitted source IDs survive the dimension edits;
- horizontal/vertical/coincident topology and the positive orientation branch are unchanged.

### A2 - Underconstrained solver-projected drag

Use S1 with A = `(0, 0)` fixed, B constrained to `(4, 0)` by horizontal plus length `4`, and C constrained only by `distance(A, C) = 3`. Disable previous-state preference while checking rank.

Expected interaction and report:

- before drag, local DOF is `1`;
- a temporary drag target at `(0, 3)` resolves C to `(0, 3)` while B remains `(4, 0)`;
- the drag target is temporary, not a driving dimension or persisted hard constraint;
- release removes the temporary target, leaves C at the nearby accepted point and restores local DOF `1` without violating the distance.

### A3 - Line-circle tangency

Geometry and state:

- fixed line segment L from `(-5, 0)` to `(5, 0)`;
- fixed guide point G = `(1, 0)`;
- circle center O initially `(1, 3)`, with `vertical(G, O)` and driving radius `2`;
- generic line-circle tangency with contact on the interior of L, circle on the positive-y side, and the initial same-direction tangent orientation/neighborhood retained.

Expected accepted geometry and state:

- O = `(1, 2)` and shared contact P = `(1, 0)`;
- the normalized line contact coordinate is `0.6` for the directed segment from `(-5, 0)` to `(5, 0)`;
- the circle contact is the negative-y radial location in its retained winding;
- moving the requested contact beyond a segment endpoint fails transactionally rather than extending L or changing branch.

### A4 - Free-radius circle-arc tangency

Geometry and state:

- fixed circular arc centered at `(0, 0)`, radius `5`, start angle `-150 degrees`, counterclockwise sweep `300 degrees`;
- a circle with center initially `(8, 0)`, positive but undimensioned radius, and no fixed center coordinates;
- generic circle-arc external tangency on the positive-x radial branch, with explicit arc span/winding/contact neighborhood;
- a temporary drag target sets the circle center to `(8, 0)`.

Expected accepted geometry and report:

- shared contact P = `(5, 0)` and solved circle radius `3`;
- both latent contact parameters and the radius are solved variables;
- after release, the hard system reports exactly two local DOF for free center motion while retaining the accepted radius/contact state as its warm start;
- a target requiring contact in the omitted 60-degree arc span rejects and retains the previous radius, contacts, branch and geometry.

### A5 - Bezier tangent line

Geometry and state:

- cubic Bezier controls P0 = `(0, 0)`, P1 = `(1, 0)`, P2 = `(2, 1)`, P3 = `(3, 1)`;
- a line from fixed endpoint A = `(0, 0)` to B = `(2, 0)` with driving length `2`;
- generic line-Bezier contact/tangency at line endpoint A and Bezier parameter `t = 0`, with same tangent orientation.

Edit P1 to `(1, 0.5)` while retaining the contact parameter and orientation branch.

Expected accepted geometry and report:

- A and P0 remain coincident at `(0, 0)`;
- B resolves to `(4 / sqrt(5), 2 / sqrt(5))`, the length-2 same-direction tangent at P0;
- derivative incidence includes B, every incident Bezier control and the active contact parameter, with local AD agreeing with central finite differences;
- attempting P1 = P0 creates a zero-speed endpoint jet, rejects with a typed regularity/degeneracy error and retains the prior accepted line and curve.

### A6 - Conflicting dimensions

Use S2 with accepted rectangle geometry A = `(0, 0)`, B = `(4, 0)`, C = `(4, 3)`, D = `(0, 3)` and driving source `width-4`. Submit a command adding source `width-5` to the same width.

Expected failure and diagnostics:

- hard validation is invalid and the command does not enter accepted history;
- conflict candidates name both `width-4` and `width-5` under a `Complete` diagnostic result for this bounded case;
- removing either width source restores a valid width under the corresponding remaining source;
- the prior 4-by-3 accepted geometry remains visible after the rejected command.

### A7 - Undo/redo command history

Starting from an empty document, perform these accepted commands in order:

1. create the A1 4-by-3 rectangle;
2. edit its width dimension from `4` to `6`;
3. suppress its height dimension;
4. create point E = `(9, 9)`;
5. delete E.

Expected history behavior:

- undo delete restores E with the same persistent ID;
- undo create removes E; undo suppress reactivates the same height source; undo width restores width `4`; undo rectangle returns the empty document;
- redoing all five commands returns the exact accepted post-delete document with width `6`, suppressed height source and E absent;
- every step reproduces deterministic accepted geometry and source ordering; failed commands add no history entry and clear no redo entry.

### A8 - JSON round trip of IDs and branches

Build one document containing A1, A3, A4 and A5 with exactly the positive rectangle orientation, A3 positive-y side/same-direction tangent/interior line neighborhood/winding zero, A4 positive-x external branch/opposite tangent orientation/300-degree active arc span/winding zero, and A5 same-direction endpoint neighborhood specified above. Export canonical versioned JSON, import it into a fresh process/session, and export again.

Expected persistence behavior:

- canonical re-export is byte-for-byte identical;
- every document/entity/point/scalar/curve/constraint/dimension/contact/source persistent ID is identical, while runtime generational keys may differ;
- every branch, span, winding, tangent-orientation, contact-neighborhood, suppression and driving/reference field is identical;
- solving the imported document preserves accepted geometry, rank/DOF, source ordering and branch state.

### A9 - Invalid edit and import retention

Start from the accepted A1 4-by-3 document with a non-empty undo history. First submit a negative width edit. Then attempt an import that duplicates one persistent point ID and leaves one constraint reference dangling.

Expected behavior for each rejection:

- return a typed, actionable command/import error before any success-like status;
- preserve canonical document JSON, accepted geometry, accepted revision, history cursor, redo entries, branch state and accepted audit/diagnostics exactly;
- keep the 4-by-3 rectangle visible; no candidate or partially imported geometry becomes authoritative;
- local autosave remains the last valid document and is not overwritten by rejected input.

### A10 - Scale corpus

Run A1-A9 and the reusable alpha geometry/constraint corpus at uniform model scales `s` in exactly `1e-6`, `1` and `1e6`. Multiply every length, coordinate and drag target by `s`; leave angles, normalized curve parameters and persistent/discrete state unchanged.

Expected behavior:

- accepted normalized hard residual remains `<= 1e-9` and Jacobian oracle error remains `<= 1e-6` away from excluded singular/nondifferentiable states;
- topology, persistent IDs, source ordering, branch/span/winding/orientation/neighborhood state, rank/mobility and conflict diagnosis are invariant;
- reference dimensions scale by `s`, while angles and normalized contact parameters do not;
- desktop and mobile E2E can pan/zoom to and edit the accepted geometry at all three scales without changing document semantics.

### M14 field regressions

- R1: an otherwise free line endpoint can cross the stored direction half-plane without an `opposite branch` error. The inactive persisted branch remains unchanged; adding both an axis constraint and driving length makes that branch enforceable again and an opposite crossing rejects until an explicit branch transition.
- R2: dragging A5 line endpoint B follows successive transient targets at scales `1e-6`, `1` and `1e6`, projects to the nearest length-2 tangent configuration, retains the fixed contact/orientation state and commits the final accepted preview as exactly one history entry. Rank-deficient secondary solves must satisfy their KKT check rather than report `NumericalFailure` or accept `Stalled` as success.
- R3: deleting all visible A1 points and edges atomically removes dependent constraints, dimensions, contacts and private scalars while retaining disconnected geometry. Undo restores the same persistent IDs and source order.
- R4: the canonical A1 rectangle remains anchored and dimensioned. A rectangle created by the playground removes the macro anchor and generated width/height dimensions, retains hard horizontal/vertical topology, reports four local DOF and changes size under projected corner drag.
- R5: rotating the A5 tangent line through its endpoint applies a transient stability target to the opposite cubic Bezier handle. The opposite handle and endpoint remain stable while the constrained handle and line satisfy contact, tangent orientation and driving length.
- R6: every constraint and dimension row in the playground object panel exposes a typed transactional delete action. Deletion removes owned hidden state, enters history only when accepted and restores the same persistent IDs on undo.
- R7: every supported draw tool stages points on pointer release, exposes its exact next step, renders the prospective primitive, and commits once. Pointer cancellation changes no draft/document state, invalid completion retains staged points, and Undo point/Cancel never mutate accepted history.
- R8: the tangent-orbit satellite traverses all four quadrants and returns to its start under projected drag. Opposed tangent orientation and periodic contact state retain external tangency without imposing a fixed center-direction half-plane or switching to internal tangency.

### Advanced UI stress examples

- `stress-compass`: a fixed 30-degree bisector carries two symmetric equal-length arms, a reference 60-degree oriented angle, and reference arm/chord dimensions. It loads with one rotational DOF so either tip drives the symmetric mechanism; switching the angle to driving locks the compass at zero DOF and exposes one intentionally redundant hard row.
- `stress-bridge`: two cubic Beziers meet through explicit End/Start contacts and aligned generic curve-curve tangency. The equal seam-handle source loads suppressed, exposing one bounded seam-sliding DOF; restoring it locks the C1 seam. A drag toward a collapsed handle projects to valid geometry, while an exact edit/import collapse rejects as degenerate and retains the accepted bridge.
- `motion-cam`: two equal-radius circles have independent generic tangencies to a fixed quadratic Bezier cam. The document loads with two DOF; dragging either center makes that roller follow the cam's normal-offset path while a transient stability target leaves the other roller stationary.
- `motion-orbit`: a radius-1 satellite circle is externally tangent to a fixed radius-3 circle through generic curve contact with explicit opposed tangent orientation and periodic contact state. It loads with one orbital DOF; center drag follows the complete radius-4 locus while retaining the external-tangency branch.
- `motion-trammel`: the ends of a length-5 bar slide on perpendicular bounded rails. Two nested midpoint constraints place a tracer one quarter of the way from the vertical slider, so projected drag reveals an exact ellipse without an ellipse primitive or equation in the browser.
- `motion-scotch-yoke`: a length-5 crank rotates about a fixed center while a vertical slot shares its pin and its opposite end is restricted to a horizontal guide. Crank rotation therefore emerges as sinusoidal slider travel from only distance, vertical and fixed-coordinate constraints.
- `motion-rotating-square`: four ordinary lines become a rigid square through one driving side length, adjacent perpendicular/equal-length relations and opposite parallel relations. The assembly retains one rotational DOF about its fixed corner even though no rectangle or square primitive is used.
- `motion-scissor`: equal upper arms meet between a fixed anchor and horizontal base slider, while a symmetry constraint reflects the upper joint into a lower joint across the moving base. Dragging the slider opens and closes the mirrored jack with one DOF.
- `motion-scissor-tower`: five stacked X stages use twelve level pivots, ten equal diagonal bars and six equal-width horizontal platforms. One fixed base pivot and one horizontal base slider leave 24 point coordinates under 23 independent hard rows, so moving the base synchronously raises or lowers the entire sixteen-member tower with one DOF.
- `motion-peaucellier`: two equal length-5 links and a four-sided length-3 rhombus form a Peaucellier-Lipkin inversor, driven by a length-4 input crank whose fixed circle passes through the origin. Seven bars and eleven independent hard rows leave one DOF; circular input motion maps to an exact vertical output line even though the output point has no line or coordinate constraint.
- `diagnostic-rank-drop`: two fixed radius-2 circles are tangent at a free point constrained to distance 2 from both centers. The declared block envelope is structurally well-constrained, while the accepted numerical Jacobian has left/right nullity `(1, 1)` at the dependent-gradient configuration.
- `diagnostic-endpoint-bound`: a fixed `t = 1` line contact is shown beside a circle radius at its positive lower domain. Equality mobility is two, the fixed endpoint and active radius remove bidirectional mobility, and only the radius contributes one-sided feasible motion.
- `diagnostic-redundancy`: a fixed-origin horizontal arm has two independent driving length-4 sources. Geometry remains valid and locked, structural/numerical left nullity is one, and the duplicate source receives deterministic complete redundancy evidence.

These examples are loadable interaction/audit stress labs, not additional canonical A1-A10 gates. They compose existing public document constraints and add no browser equations or new curve family.

## M15 manifold and accepted-sensitivity fixtures

The manifold regression corpus applies ADR 0006 directly rather than relying on
one end-to-end assembly:

- `Pose2` and `Pose3` identity, composition, inverse, exponential, logarithm,
  adjoint, right retraction and local difference round-trip at ordinary, tiny and
  near-half-turn increments;
- exact `+pi` and `-pi` rotations canonicalize to identical quaternion bits, while
  values immediately on either side of the tie band retain the principal log;
- checked point/vector transforms reject non-finite input and finite overflow;
  validated `Frame3`/`PlaneFrame` round-trip and reject invalid axes or off-plane
  inverse requests;
- scalar, `Vec2`, `Vec3`, `Pose2` and `Pose3` packing preserves distinct ambient
  and tangent dimensions, and pose fixed/alias Jacobians match tangent-coordinate
  finite differences away from the principal-log cut;
- accepted hard linearization preserves deterministic component, row and reduced
  root/member ordering with session revisions; sensitivity matches a central target
  perturbation oracle and distinguishes unique, underdetermined minimum-norm,
  inconsistent and numerical-failure outcomes;
- L1/L2 geometry, explicit branch signs, rank and source order are invariant under
  a common left `SE(2)` transform; L3 published world-frame body-origin velocity
  transforms equivariantly and matches continued position solves.

These fixtures do not claim pose-coordinate box bounds, active-bound sensitivity,
secondary-objective sensitivity, world-frame sensitivity conversion or spatial
joints. Those contracts remain assigned to later milestones.

## M17 persistent planar gauge and velocity fixtures

The planar migration corpus applies ADR 0009 through persistent domain sessions:

- one floating two-body weld at scales `1e-6`, `1` and `1e6` has physical equality
  right nullity three, `gauge_dof = 3`, `internal_mobility = 0`; automatic
  lowest-ID and explicit alternate body references preserve relative `SE(2)`
  geometry, physical rank, structural diagnostics, source order and public audit;
- one floating revolute has one internal rotational mobility after the three world
  gauge DOF are separated; adding a relative-angle driver removes that internal
  mobility, and the selected numerical reference has zero representative velocity;
- two disconnected floating welded pairs contribute six gauge DOF, while a
  disconnected physically grounded body contributes none and retains its physical
  ground source in audit;
- a branch monitor joining otherwise equality-disconnected bodies forms one domain
  component: floating it reports three world gauge plus three internal DOF, while
  grounding one body reports zero gauge plus three internal DOF;
- explicit gauge policy JSON requires exactly one reference per floating component,
  no references in grounded components and transactional revision changes;
- persistent L3 velocity and the compatibility facade use the same accepted hard
  component ranks, thresholds, row scales and independently validated physical
  differentiated equations.

Private numerical gauge rows never appear in physical source order, audit,
conflict/redundancy candidates or published rank. A certified floating component
whose physical right nullity is below three is an error, not a saturating mobility
subtraction.

The post-M17 adversarial corpus additionally protects:

- a perturbed three-body welded chain whose private gauge candidate, ungauged
  physical session, persistent document, runtime geometry, accepted result and audit
  remain one coherent state before and after an explicit live gauge rebuild;
- every valid body reference in that floating chain under common-left `SE(2)`
  transforms at scales `1e-6`, `1` and `1e6`;
- a physically grounded nonlowest-ID body, proving automatic policy never selects a
  numerical reference in a grounded domain component;
- selected-driver velocity across driven, unselected welded, isolated floating and
  isolated grounded components, with zero cross-component motion;
- alternative offset-revolute velocity gauges related by exactly one common rigid
  world twist, including the angular lever arm at each body origin;
- two centered revolute closures with normalized separation above and below the
  accepted component rank threshold, changing internal mobility while retaining
  exactly three world gauge DOF;
- multi-component missing, duplicate, grounded, unknown and private gauge JSON
  references plus current-revision transactional rejection;
- duplicate physical weld diagnostics, where every public row and candidate belongs
  to a persistent physical source and no private gauge identity leaks.

## M9 sketch dependent-gradient fixture

Purpose: distinguish an accepted finite hard state from configuration-dependent numerical rank loss in a domain-compiled sketch.

For each uniform model scale `s` in `1e-6`, `1` and `1e6`:

- fixed first centre C0 = `(0, 0)`;
- fixed second centre C1 = `(2s, 0)`;
- free point P starts at `(s, 0)`;
- driving distance source D0 imposes `distance(C0, P) = s`;
- driving distance source D1 imposes `distance(C1, P) = s`;
- previous-state preferences are disabled.

The two circles are externally tangent at P. After fixed-centre elimination the normalized active hard Jacobian with columns `(P.x, P.y)` is exactly:

```text
[  1  0 ]
[ -1  0 ]
```

Expected state and report:

- P remains finite at `(s, 0)` and both normalized residuals are exactly zero;
- `HardValidity::Valid`, numerical rank `1`, left nullity `1`, right nullity/local DOF `1`;
- the active `2 x 2` component is singular but not in the distinct near-singular warning band;
- `sigma_max = sqrt(2)`, with finite component-local machine and final rank thresholds identical across all three model scales;
- each D0/D1 domain dimension maps to one hard core source and one evaluated audit row with the matching residual ID.

## L1 — Four-bar, open assembly

Purpose: first rigid-body closed loop and driver continuation.

Ground pivots:

- O2 = `(0, 0)`;
- O4 = `(4, 0)`.

Link lengths:

- input crank O2-A = `1.5`;
- coupler A-B = `3.0`;
- output rocker B-O4 = `2.5`;
- ground O2-O4 = `4.0`.

Initial driver/input angle: `60 degrees`.

Assembly mode:

- choose the circle-intersection root with B above the directed line A→O4;
- serialize this as `Open` plus the initial orientation sign used by tests.

Safe demonstration sweep:

- initially use `25..135 degrees` in increments no larger than `2 degrees`;
- warm-start each sample;
- if analysis finds a singularity inside this interval, narrow the ordinary safe sweep and add a separate near-toggle fixture rather than permitting a silent branch change.

Expected state:

- both revolute closure points coincide within tolerance;
- link lengths are intrinsic body geometry, not extra distance constraints;
- orientation/assembly sign remains constant over the safe sweep.

## L2 — Four-bar, crossed assembly

Same dimensions and initial driver as L1, but select the opposite A/O4 circle-intersection root and serialize `Crossed`.

Expected state:

- closure validates;
- orientation sign is opposite L1;
- a sweep does not drift into L1.

## L3 — Slider-crank

Purpose: revolute plus prismatic joints and linear continuation.

Geometry:

- ground crank pivot O = `(0, 0)`;
- crank length = `1.25`;
- connecting rod length = `3.5`;
- slider guide is world/local x-axis (`y = 0`);
- initial crank angle = `45 degrees`;
- choose the slider solution on positive x.

Safe driver sweep:

- `15..165 degrees`, increments no larger than `2 degrees`;
- preserve positive-x assembly choice.

Expected state:

- crank/rod revolute anchor coincidence validates;
- slider anchor remains on the guide;
- slider orientation remains aligned with the guide;
- velocity solve satisfies differentiated constraints for a unit input angular velocity.

M9 near-aligned acceptance fixture:

- start from canonical L3 and continue the angular driver from `45 degrees` to `+1e-6 rad`, then exactly `0 rad`, then `-1e-6 rad`, using the existing maximum step of `2 degrees`;
- at exact zero the crank pin is `(1.25, 0)`, the positive-x slider pin is `(4.75, 0)`, and the connecting rod is horizontal;
- all three targets are accepted with finite geometry, `HardValidity::Valid`, active position rank `9`, and numerical left/right nullity `(0, 0)`;
- the active normalized position component is `9 x 9`, has `sigma_max ~= 3.79714252615743`, smallest retained singular value `~= 0.445041867912`, and within-component ratio `~= 0.117204414858`;
- the relative rank threshold is `~= 3.797142526e-10`, above the machine floor `~= 7.588215109e-15`; the smallest retained-to-threshold ratio is about `1.172e9`, so neither M9 `near_singular` nor the linkage conditioning warning is raised;
- this is intentional: the crank-angle driver coordinate keeps the position equality system well-conditioned at geometric dead centre;
- the compatibility unit-rate velocity query is likewise rank `9`, has zero local DOF, the same finite spectrum, and independently validates its differentiated residual;
- after the finite forward crossing, adding a grounded blocker pin at `(100, 0)` and an incompatible revolute closure to the slider pin reaches the bounded solver iteration limit with `HardValidity::Invalid`; it must retain the accepted crank, rod and slider poses bitwise and keep all returned geometry finite.

M16 displacement-driven fold fixture:

- use the same `1.25s` crank, `3.5s` rod, x-axis guide and positive-X slider branch at each model scale `s` in `1e-6`, `1` and `1e6`;
- start at crank angle `0.05 rad` with linear slider-displacement target `4.747880210234948s`;
- the analytic turning point is crank angle `0` and displacement `4.75s`;
- natural adaptive continuation toward `4.751s` retains an accepted prefix and stops with `PseudoArclengthRequired` at every required scale, without committing the negative-angle side or switching modes;
- explicit increasing-parameter pseudo-arclength continuation for normalized path length `0.2` crosses to a negative crank angle while retaining the positive-X assembly monitor;
- a second explicitly oriented increasing-parameter path from that negative endpoint crosses back to positive crank angle, while explicit decreasing-parameter orientation from the original positive endpoint moves away from the maximum deterministically;
- correctors exceeding either the absolute or path-step-relative normalized locality limit are rejected and retried before any state mutation;
- an ordinary physical corrector rejected only by post-corrector tangent policy remains visible in `rejected_attempts`, while the accepted prefix stays committed;
- legacy bounded-step `drive_to` toward the impossible displacement `4.751s` rejects its first beyond-fold sample and retains its entry target and geometry exactly;
- every published accepted sample is an ordinary fixed-displacement physical solve with finite geometry, independently valid hard residuals and no pseudo parameter/control source in its rank, audit or diagnostics;
- forced dense and sparse-preferred physical endpoints agree on geometry, final driver target, rank/mobility, diagnostics, audit structure and positive-X branch state.

## S1-S3 — Spatial vertical slice

Purpose: prove one-pose-per-body spatial assembly state, local feature transforms,
six-coordinate gauge separation and minimal useful joint mobility before the larger
M20 mate catalog.

Shared construction:

- each body stores `T_WB` as a checked quaternion-backed `Pose3` and receives
  right/body-local increments `[v_x, v_y, v_z, omega_x, omega_y, omega_z]`;
- local point and right-handed frame features transform through their owning pose;
- exact fixtures use arbitrary non-axis-aligned body poses and offset local features;
- perturbed fixtures right-retract the second body before solving;
- every fixture is repeated at model scales `1e-6`, `1` and `1e6`, with both
  scale-proportional and mixed common-left `SE(3)` transforms.

Expected physical equality counts:

| Fixture | Scalar rows/rank | Floating right nullity | Gauge DOF | Internal mobility | Grounded right nullity |
| --- | ---: | ---: | ---: | ---: | ---: |
| S1 ball | 3 | 9 | 6 | 3 | 3 |
| S2 fixed frame | 6 | 6 | 6 | 0 | 0 |
| S3 revolute | 5 | 7 | 6 | 1 | 1 |

Gauge and validation policy:

- each certified floating connected component selects the lowest body ID by default
  or exactly one explicit reference; grounded components select none;
- private manifold fixed-pose gauges are used only by the scratch solve, while the
  published physical source mapping, audit, rank and accepted linearization come
  from a separately solved ungauged session;
- fixed-frame executable rows use origin coincidence plus three independent
  off-diagonal orientation rows; independent positive diagonal-axis checks reject
  all half-turn false roots;
- revolute rows use origin coincidence plus two directed z-axis alignment rows;
  explicit aligned/opposed parity is independently checked and cannot flip silently;
- independent physical acceptance uses `min(caller tolerance, 1e-9)` and freshly
  rebuilds every transformed point and frame.

Rollback fixture:

- two physically grounded bodies begin with coincident ball points;
- moving one local point makes the all-fixed source impossible;
- the failed revision-checked patch retains the prior revision, geometry, audit,
  source mappings, gauge report, core report and accepted hard linearization exactly;
- the residual-only all-fixed core component is mapped through physical source
  incidence rather than being mistaken for a missing spatial body component.

## Frozen near-singular fixtures

The regression corpus includes:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures test truthful singularity/rank reporting and finite state retention. They do not demand arbitrary global branch selection. M9 makes the machine-floor numerical rank contract and distinct near-singular warning band mandatory.

The detailed L3 fixture above demonstrates that geometric alignment does not itself justify an M9 warning when the selected driver makes the reported position/velocity matrices full-rank and well-conditioned. The detailed sketch fixture demonstrates actual dependent gradients and therefore does report numerical singularity.
