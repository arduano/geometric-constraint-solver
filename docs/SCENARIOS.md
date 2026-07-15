# Canonical scenarios

These hardcoded scenarios are shared by domain tests and `geosolve-demo-web`. Their constructors belong in the domain crates; the web crate must not duplicate equations.

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

## Frozen near-singular fixtures

The regression corpus includes:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures test truthful singularity/rank reporting and finite state retention. They do not demand arbitrary global branch selection. M9 makes the machine-floor numerical rank contract and distinct near-singular warning band mandatory.

The detailed L3 fixture above demonstrates that geometric alignment does not itself justify an M9 warning when the selected driver makes the reported position/velocity matrices full-rank and well-conditioned. The detailed sketch fixture demonstrates actual dependent gradients and therefore does report numerical singularity.
