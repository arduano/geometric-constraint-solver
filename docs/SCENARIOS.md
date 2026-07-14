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

## M7 auxiliary curve verification fixtures

These browser-only constructions exercise public sketch APIs but are not additional canonical scenarios:

- bounded arc contact: drag a point over an explicit 240-degree counterclockwise arc; accepted targets project onto the active span and genuine endpoint escape retains the prior geometry/contact/audit;
- bounded line-circle tangency: drag a fixed-radius circle along the left side of a finite line segment; the latent line/circle contacts move with it and requests beyond either endpoint retain the prior accepted state.
- free-radius circle-arc tangency: drag a circle center in two dimensions outside a fixed 300-degree arc; no circle radius dimension is present, so the radius and circle/arc contacts solve automatically while requests in the missing span retain the prior accepted state.

Both fixtures render contact parameters and branch/domain state from the public sketch result. The web crate does not duplicate their constraint equations.

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

## Near-singular fixtures

Add only after ordinary scenes pass:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures must test truthful singularity/rank reporting and finite state retention. They must not demand arbitrary global branch selection.
