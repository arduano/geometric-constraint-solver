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
- direct native scale owners preserve accepted geometry and document semantics at all three
  scales; the retired desktop/mobile browser delivery is not a post-cleanup gate.

### M14 field regressions

- R1: an otherwise free line endpoint can cross the stored direction half-plane without an `opposite branch` error. The inactive persisted branch remains unchanged; adding both an axis constraint and driving length makes that branch enforceable again and an opposite crossing rejects until an explicit branch transition.
- R2: dragging A5 line endpoint B follows successive transient targets at scales `1e-6`, `1` and `1e6`, projects to the nearest length-2 tangent configuration, retains the fixed contact/orientation state and commits the final accepted preview as exactly one history entry. Rank-deficient secondary solves must satisfy their KKT check rather than report `NumericalFailure` or accept `Stalled` as success.
- R3: deleting all visible A1 points and edges atomically removes dependent constraints, dimensions, contacts and private scalars while retaining disconnected geometry. Undo restores the same persistent IDs and source order.
- R4: the canonical A1 rectangle remains anchored and dimensioned. An ordinary rectangle
  transaction removes the macro anchor and generated width/height dimensions, retains hard
  horizontal/vertical topology, reports four local DOF and changes size under a projected
  corner edit.
- R5: rotating the A5 tangent line through its endpoint keeps independent curve freedom local
  while the constrained handle and line satisfy contact, tangent orientation and driving length.
  M65 supersedes the former second Temporary stability target with deterministic frozen
  PreviousState anchors derived from the accepted hard nullspace.
- R6: every supported constraint and dimension exposes a typed transactional editor delete
  action. Deletion removes owned hidden state, enters history only when accepted and restores
  the same persistent IDs on undo.
- R7: every supported draw tool stages points on pointer release, exposes its exact next step, renders the prospective primitive, and commits once. Pointer cancellation changes no draft/document state, invalid completion retains staged points, and Undo point/Cancel never mutate accepted history.
- R8: the tangent-orbit satellite traverses all four quadrants and returns to its start under projected drag. Opposed tangent orientation and periodic contact state retain external tangency without imposing a fixed center-direction half-plane or switching to internal tangency.

### Advanced UI stress examples

- `stress-compass`: a fixed 30-degree bisector carries two symmetric equal-length arms, a reference 60-degree oriented angle, and reference arm/chord dimensions. It loads with one rotational DOF so either tip drives the symmetric mechanism; switching the angle to driving locks the compass at zero DOF and exposes one intentionally redundant hard row.
- `stress-bridge`: two cubic Beziers meet through explicit End/Start contacts and aligned generic curve-curve tangency. The equal seam-handle source loads suppressed, exposing one bounded seam-sliding DOF; restoring it locks the C1 seam. A drag toward a collapsed handle projects to valid geometry, while an exact edit/import collapse rejects as degenerate and retains the accepted bridge.
- `motion-cam`: two equal-radius circles have independent generic tangencies to a fixed quadratic
  Bezier cam. The document loads with two DOF; dragging either center makes that roller follow the
  cam's normal-offset path while M65 locality planning leaves the other roller stationary.
  Symmetric headless and editor regressions cover both directions without a scenario-owned driver,
  passive-point ID or second Temporary target.
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

These are historical interaction/audit stress compositions, not additional canonical A1-A10
gates or promised post-cleanup UI fixtures. Their retained mathematical behavior is owned by
direct domain/editor tests; they compose existing public document constraints and add no
browser equations or new curve family.

## Historical M39 desktop workbench qualification

M39-W1 is a desktop-browser interaction fixture, not a new mathematical
scenario. It composes ordinary public document edits and retained-session views.

### M39-W1 - Core authoring and retained-state synchronization

- at the M39 snapshot the default route opened the CAD workbench while a temporary advanced
  playground remained separately routed; M50 later removed that route and runtime;
- point, line/polyline, rectangle, circle and circular-arc tools retain complete
  public document transactions, with incomplete drafts changing no document;
- canvas and sketch-tree selection identify the same persistent points, curves,
  constraints and dimensions, and the inspector applies only compatible public
  edits;
- fixed, coincident, horizontal, vertical, parallel, perpendicular and equal-
  length constraints are selectable persistent glyph objects;
- point-distance and line-length dimensions remain selectable persistent
  driving/reference objects whose values and equations are evaluated by
  `geosolve-sketch`, never by browser formulas;
- point drag submits a public retained-design point edit, delete removes the
  selected persistent object, and application undo/redo restores document
  snapshots through `RetainedSketchDocumentSession`;
- rendering reads the accepted document only, tessellates public immutable curve
  jets adaptively and keys its retained cache by accepted revision;
- every rejected attempt keeps the prior accepted canvas visible while the tree
  shows retained unsolved design intent and Problems names the exact latest
  attempt;
- lifecycle badges expose `Accepted`, `Design unsolved`, `Solving`, `Solved
  preview` and `Rejected attempt` as distinct application states.

Automation uses a fixed desktop viewport. Responsive, tablet and mobile behavior
is neither tested nor claimed.

## M21 non-rational B-spline fixtures

The B-spline corpus exercises immutable geometry and the persistent generic-curve
path without adding a curve-pair equation.

### M21-B1 - Clamped local-support cubic

- degree `3`, seven distinct persistent controls and complete clamped knots
  `[0,0,0,0, 0.25,0.6,0.8, 1,1,1,1]`;
- every positive knot interval has a stable semantic span ID unrelated to its
  knot-array index;
- a point-on-curve and line-tangency source select the second span with local
  parameter `0.37` and a strict local neighborhood;
- only the selected span's four controls plus the latent parameter enter curve
  incidence; controls outside that support enter neither residual incidence nor
  its Jacobian;
- controls are fixed and the point begins `0.15s` off the curve at model scales
  `s = 1e-6, 1, 1e6`; recovery must be hard-valid with normalized residual
  `<= 1e-9`, and local AD must agree with central differences to `<= 1e-6`;
- distinct control identities at coincident positions are definition-valid, but
  a selected zero-speed span rejects before a source or solve can succeed.

### M21-B2 - Periodic topology and refinement

- degree `2`, five unique cyclic controls and one-period knots
  `[0,1,2,3,4,5]`; no seam control is duplicated in persistence;
- all five semantic spans evaluate with local `[0,1]` coordinates, while winding
  remains separate discrete contact state;
- left and right seam jets satisfy the multiplicity-derived continuity guarantee,
  and evaluating at parameters separated by an integer period gives identical
  position and derivatives;
- inserting native knot `2.4` splits only its selected semantic span: the left
  child retains its ID, the right child receives one fresh never-reused ID, every
  old control ID survives and one fresh control ID is allocated;
- inserting at the existing seam `0` raises multiplicity without allocating a
  span ID; dense pre/post samples preserve parameterized geometry;
- contacts on a split span retain world position and migrate atomically to a child
  span/local coordinate. An exact inserted-knot contact selects the retained left
  span at local `1`.

### M21-B3 - Explicit one-sided transition and continuity

- a contact at one clamped span end transitions only through an explicit adjacent
  span command and becomes the next span start at the same world point;
- crossing the periodic last/first seam increments winding by one; the reverse
  transition decrements it;
- point contact may cross a `C0` knot, while a tangent-bearing contact requires a
  guaranteed `C1` knot and rejects transactionally otherwise;
- malformed degree/count/knot order/clamping/multiplicity/control/span identity,
  unavailable endpoint side, escaped local parameter and insertion beyond maximum
  connected multiplicity all return typed failures and retain accepted state;
- canonical JSON, deterministic lowering, accepted-state projection, insertion
  undo/redo and the public document sampler preserve control IDs, span IDs and
  periodic winding.

## M22 NURBS and advanced CAD fixtures

The M22 corpus completes the reusable 2D CAD surface. Every success is checked
both through compiled local AD and independently reconstructed immutable jets.

### M22-N1 - Rational equivalence, gauge and local support

- unit weights reproduce clamped and periodic M21 B-spline jets through third
  order; canonical degree-two weights `[1, 1/sqrt(2), 1]` reproduce the rational
  quarter circle and its curvature;
- one explicit persisted weight is exactly one and absent from solver incidence;
  every other active weight and exactly `degree + 1` controls enter a selected
  span residual, while inactive controls/weights enter neither component nor
  Jacobian;
- explicit re-gauging divides every weight by the selected new gauge, preserves
  parameterized geometry and makes the old gauge editable; direct selected-gauge
  edits reject;
- homogeneous knot insertion retains all old control/weight identities, creates
  one fresh pair, preserves the gauge identity and parameterized geometry, and
  normalizes only local refinement stencils;
- raw and lifecycle-aware NURBS deletion remove all owned weights but retain
  independently owned controls.

### M22-N2 - Rational conditioning and cancellation

- active normalized weights, pairwise weight products, weighted control
  differences, homogeneous outputs, denominator condition scale and all returned
  derivatives must be finite and representable or return a typed mixed-scale or
  denominator failure;
- controls translated near `1e15` with one-ULP separation retain the correct
  positive rational tangent; weights `[1, 1e16]` retain the representable
  `4e-16` derivative rather than cancellation-corrupting its sign;
- a tiny basis value, weight `1e-12` and control `1e308` retain their representable
  product by multiplying the weighted control difference before the basis term;
- distant extreme weights cannot reject insertion on a locally conditioned
  degree-one span; truly unrepresentable active ratios/products reject before
  source success or commit;
- all solved NURBS weights commit as one clone-and-swap transaction, never as an
  invalid old/new hybrid, and failed candidates retain prior points and weights.

### M22-D1 - Differential geometry and direction

- circles, directed arcs and canonical NURBS report signed/unsigned curvature,
  curvature vector and finite osculating radius at scales `1e-6`, `1` and `1e6`;
  straight curves report zero curvature and typed undefined osculating radius;
- parameter reversal flips tangent, left normal and signed curvature while
  preserving unsigned curvature and curvature vector; reflection flips sign and
  positive similarity divides curvature by scale;
- compensated raw determinant, unscaled normal projection and scaled normal
  projection regressions cover near-parallel cancellation, subnormal products,
  overflowing tangential acceleration and representable mixed-scale curvature;
- tangent and explicit left/right normal constraints use generic curve jets,
  preserve their direction branch, pass central differences and independently
  validate the normalized row rather than only its sign.

### M22-D2 - Curvature and endpoint continuity

- signed equal curvature uses `k1-k2`; magnitude equality stores explicit same- or
  opposite-sign state and uses a smooth signed equation, with zero magnitude
  treated as branch-ambiguous;
- ordered endpoint G0 compares position, G1 adds aligned path tangent, and G2 adds
  path-oriented signed curvature invariant under positive reparameterization;
- separately named parametric C2 stores positive fixed rates and compares rate-
  adjusted first and second derivatives using sequential scaling that avoids
  premature rate-squared overflow/underflow;
- candidate validation independently recomputes every normalized G0/G1/G2/C2,
  direction and curvature row from immutable solved geometry at the effective
  tolerance; branch-only agreement cannot produce success;
- C2 consumers require guaranteed C2 span transitions, while one-sided endpoint
  measurements remain valid without claiming cross-knot continuity.

### M22-P1 - Persistence, properties and sparse locality

- canonical JSON preserves weights, gauge, semantic spans, winding, knot side,
  neighborhoods, normal side, curvature relation, endpoint order and C2 rates;
- malformed IDs, gauges, weights, rates, endpoints and transition continuity
  reject atomically through commands, history and import;
- 48 generated valid/malformed cases cover refinement invariance, differential
  oracles and retained-state failure behavior with reproducible seeds;
- a deterministic 1,000-control NURBS with 128 contacts proves every residual
  remains degree-local and does not create a global weight-gauge component;
- the web consumer samples public spans only through document APIs; one failed
  sample suppresses the complete span path and publishes a separate accessible
  sampling diagnostic instead of connecting across missing geometry.

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

## M20 spatial mate catalog and driven assembly fixtures

Axis and plane features store complete checked body-local frames. Their directed
`z` axis is the axis direction or plane normal, while `x/y` are persistent clocks.
Every feature is transformed and independently validated even when no source uses
it. Feature, relation and coordinate conventions follow ADR 0013.

Primitive equality counts are:

| Fixture | Scalar rows/rank | Floating right nullity | Gauge DOF | Internal mobility | Grounded right nullity |
| --- | ---: | ---: | ---: | ---: | ---: |
| M20-J1 prismatic | 5 | 7 | 6 | 1 | 1 |
| M20-J2 cylindrical | 4 | 8 | 6 | 2 | 2 |
| M20-J3 planar | 3 | 9 | 6 | 3 | 3 |
| M20-J4 universal | 4 | 8 | 6 | 2 | 2 |
| M20-M1 point distance | 1 | 11 | 6 | 5 | 5 |
| M20-M2 interior axis angle | 1 | 11 | 6 | 5 | 5 |
| M20-M3 direction-only axis alignment | 2 | 10 | 6 | 4 | 4 |
| M20-M4 frame offset | 6 | 6 | 6 | 0 | 0 |

Each primitive has exact, perturbed, right-tangent Jacobian, common-left `SE(3)`,
uniform-scale `1e-6`/`1`/`1e6`, mixed-scale, invalid-geometry, branch-retention,
audit and rollback fixtures. Expected rank applies to the documented regular
configuration; a special rank is reported truthfully rather than forced to match
the table.

The literal mixed-scale fixtures use nominal model scale `1` and place
approximately `1e-6` feature offsets together with approximately `1e6` body,
feature or target offsets in the actual relative geometry. They preserve finite
accepted geometry/audit, source-local row scales, rank, gauge/internal mobility
and branch state. Central differences run at the literal span where resolvable;
universal/frame-offset cancellation columns use the documented `1e4` span, and
the planar microscopic-transverse driver oracle uses `1e-3..1`, rather than
weakening the `1e-6` Jacobian tolerance.

The `A-SB` shaft/bearing fixture uses one grounded bearing and one shaft connected
by a cylindrical joint. Undriven internal mobility is two; a hinge or translation
driver leaves one; both simultaneous drivers leave zero. Hinge winding, axis parity
and translation side are explicit mode state. The translation side is evaluated
by a row-free plane/point monitor. A failed combined hinge-target,
translation-target and mode transaction retains both prior targets, the mode and
every accepted publication.

The `A-BB` block/base fixture uses one grounded base and a three-DOF planar joint.
Its coordinates are a planar-parent hinge plus explicit plane-X and plane-Y
translations constructed with `SpatialPlanarTranslationAxis::{X,Y}`. It reports
internal mobility `3/2/1/0` with zero/one/two/three drivers. One successful
three-target transaction commits once; incompatible duplicate targets or an
invalid mode edit roll all three targets and complete accepted state back. A full
frame-offset variant has rank six and zero internal mobility. Directed normal
parity, witness side and ordered signed volume reject mirrored roots without
adding fake equality rows.

All public spatial IDs also run a same-local-ordinal foreign-assembly corpus.
Private assembly provenance makes constructor, gauge, coordinate, monitor and
transaction use return typed `Unknown*` rather than aliasing a local object;
`as_u64` and deterministic audit text remain local-ordinal compatible.

Position transactions only are in M20. Spatial continuation, event hysteresis,
multi-driver velocity and complete spatial persistence were assigned to M23 and
are covered below.

## M23 spatial continuation fixtures

The first M23 slice applies ADR 0011 continuation semantics to the M20 spatial
position-driver and gauge architecture under ADR 0016. Every accepted sample is
an ordinary fixed-driver `SpatialAssemblySession`; active parameters, private
gauges and pseudo-arclength rows are absent from its source mappings, audit and
physical rank.

### M23-C1 - Shaft/bearing natural paths

- use the M20 `A-SB` grounded cylindrical shaft/bearing at scales `1e-6`, `1`
  and `1e6`;
- natural axial continuation moves `1.9s` to `2.4s` while retaining hinge phase
  `0.48`, winding `2`, aligned parity and positive translation side;
- natural hinge continuation moves phase `0.48` to `0.82` while retaining axial
  translation `1.9s` and winding `2`;
- the same coordinate equations run on a floating cylindrical pair with one
  private six-DOF gauge; public gauge/internal mobility remains `6/0`, and no
  private source is published;
- a zero-distance request performs fresh ordinary validation, publishes no
  sample and consumes no revision. Tiny positive pseudo paths either publish a
  representably changed physical sample or stop without success.

### M23-C2 - Embedded spatial slider-crank fold

Use four `Pose3` bodies constrained to one embedded mechanism plane:

- crank radius `1.25s`, connecting-rod length `3.5s`, initial crank phase
  `0.05`, and initial positive-X slider displacement
  `4.747880210234948s`;
- ground/crank use one aligned revolute, each rod end uses a ball joint, the
  slider uses one aligned prismatic, and a regular `pi/2` axis-angle row fixes
  rod roll without redundant planar closure rows;
- one winding-zero hinge coordinate measures crank phase, while the selected
  axial translation driver measures slider displacement;
- winding zero, aligned rod normal and positive-X slider side are explicit mode
  monitors.

The analytic fold is crank phase `0` and displacement `4.75s`. Natural
continuation toward `4.751s` retains a positive-phase accepted prefix and stops
with `PseudoArclengthRequired`. Explicit increasing-parameter pseudo-arclength
for normalized path length `0.2` crosses to negative crank phase; a second
explicitly oriented path crosses back, while decreasing orientation moves away
from the maximum. The corpus runs at all required scales, under a common-left
`SE(3)` transform and forced dense/sparse correctors. Physical endpoint geometry,
rank/nullity, structural class, gauge split and retained modes agree within the
documented normalized tolerances.

Correctors outside either locality limit retry without mutation. Monitor-only
connections that leave mobility outside the selected physical hard component
reject independently of numerical gauge reference. A fixed-driver null direction
inside the selected component is left to the augmented SVD test, rather than
being rejected from ordinary nullity alone.

### M23-C3 - Typed boundary events and mode changes

- every accepted spatial solve evaluates source parity, prismatic clock,
  fixed/frame-offset half-turn, hinge-driver/cut and explicit monitor boundaries;
- normalized clearances enter at `2e-3` and leave at `4e-3`; an accepted sample in
  the deadband inherits its prior latch without a duplicate event;
- a one-revolute fixture approaches the positive principal cut from a clear
  endpoint, accepts exactly one corrected `Entered` event, moves within the
  deadband without chatter and emits one `Left` event after clearing it;
- a coarse predictor that reaches the strict `1e-3` margin stops with a typed
  predictor event and publishes no invalid endpoint;
- pseudo-arclength prediction through the canonical cut reports
  `CrossingAttempted` rather than wrapping the hinge or changing winding;
- an explicit positive-to-negative cut updates coordinate, driver and winding
  monitor once, while the wrong direction rolls back all accepted state;
- a plane-side mode change plus its translation seed commits once, while an
  incompatible parity change rolls back.

These fixtures observe predictor endpoints, ordinary corrected endpoints and the
known scalar hinge cut. They do not claim interval-global boundary tracing.

### M23-V1 - Multi-driver spatial velocity and fields

- grounded shaft/bearing fixtures at scales `1e-6`, `1` and `1e6` prescribe
  simultaneous hinge and axial rates in both request orders and compare body and
  point fields against central ordinary-position transactions;
- all prescribed coordinates reproduce their raw rates while unlisted position
  drivers have zero rate; executable active parameter columns retain the hinge
  trigonometric derivative and translation model scale;
- a fully driven floating cylinder is determinate modulo its certified six-DOF
  world gauge and leaves the selected reference stationary; changing reference
  changes the representative by exactly one common world twist;
- one driven coordinate on a grounded cylinder reports one remaining internal
  motion; equal duplicate translation rates are consistent, unequal or omitted
  duplicate rates are an inconsistent outcome with no body field;
- block/base simultaneous hinge, plane-X and plane-Y rates publish every body,
  point, frame, clocked axis, clocked plane and topology-coordinate derivative;
- a static common-left `SE(3)` transform rotates body-origin, angular and feature
  velocities without a translation lever-arm term;
- optional motion bases have exactly accepted physical right-nullity vectors,
  are deterministic and normalized in accepted tangent coordinates, satisfy all
  independently differentiated source rows and retain all six floating world
  actions rather than leaking the private gauge.

### M23-P1 - Embedded-planar L3 oracle parity

At scales `1e-6`, `1` and `1e6`, place the displacement-driven L3 in a static
non-axis-aligned `SE(3)` frame with scale-proportional translation. The planar
workplane and spatial assembly use the same frame and exact crank `1.25s`, rod
`3.5s`, initial phase `0.05`, positive-X mode and displacement target.

- natural continuation away from the fold to `4.70s` completes in both domains;
- lifted planar ground/crank/rod/slider poses and four representative closure
  points match the independently accepted spatial geometry;
- the driven regular endpoint has planar rank `9`, spatial rank `18`, zero right
  nullity and zero internal mobility in both domains;
- compatibility and persistent planar velocity agree exactly after remapping;
- the embedding basis maps planar body-origin linear and scalar angular rates to
  spatial body/point fields, while spatial hinge and translation coordinate rates
  match the planar relative crank and driver rates;
- each domain independently retains hard residual validation at `1e-9`; parity
  tolerances do not substitute for either acceptance check.

### M23-SC1 - Non-planar universal closed ring

Four bodies form a non-coplanar ring through four universal joints, with one
physical ground and one positive signed-volume monitor over four joint witnesses.
At `s=1e-6,1,1e6` it has 16 active rows, rank `16`, left nullity `0`, right
nullity/internal mobility `2`, structural nnz `144` and no numerical gauge. The
chirality metric remains above `0.2`; selecting its mirrored sign rejects and
retains every accepted view. One scale-1 fixture also rechecks all existing
universal residual Jacobians by central differences.

### M23-MS1 - Macro/micro stage and rigid tool

A grounded base and driven planar stage use phase `0.41`, winding `-2`, plane-X
translation `1e6s` and plane-Y translation `2e-6s`. A third tool body is attached
by a frame-offset mate with translation `(3e-6,-4e-6,5e-6)s` and a regular
three-axis rotation. Winding and positive-side monitors are row-free. At every
required scale the 12 active coordinates/rows have rank `12`, no nullity or
gauge, structural nnz `108`, finite audit data and independent residual at most
`1e-9`.

### M23-LS1 - Connected sparse fixed-frame chain

One ground plus 43 moving `Pose3` bodies form a connected chain of 43 fixed-frame
sources. The reduced hard system has 258 rows/coordinates, rank `258`, no
nullity, structural nnz `3060` and no gauge. A finite perturbation of the final
body converges with `SparseQr` and no fallback under `SparsePreferred`; the final
ordinary report remains independently valid and dense SVD remains authoritative
for rank. This bounded debug fixture completes the large connected scenario gate.
The exact fixed-frame-chain `Auto` density boundary belongs to the explicit
release performance corpus because its authoritative debug SVD is intentionally
too expensive for normal correctness tests.

### M23-PS1 - Spatial document persistence

Shaft/bearing documents round-trip at `s=1e-6,1,1e6`; block/base covers hinge and
both planar translation coordinates. One combined fixture covers every ground,
joint and mate variant plus signed-volume state. Canonical JSON preserves fixed
document-local IDs, semantic source order, accepted poses, targets, winding,
parity/side/orientation, explicit gauge references and boundary hysteresis while
fresh lowering changes every runtime namespace. Unsupported versions, unknown
fields/references, duplicate IDs, wrong driver target kinds and incomplete
boundary state reject. Failed replacement retains document, mapping, accepted
geometry, audit and revision.

### M23-PR1 - Generated and differential corpora

- 32 generated slider-crank cases span required scale exponents, safe positive
  and negative phases and arbitrary static common-left `SE(3)` transforms;
  accepted position, velocity, mode and canonical persistence remain equivariant;
- saved transformed slider-crank seeds require accepted `Pose3` quaternion
  canonicalization to be bitwise idempotent, so a private velocity snapshot cannot
  diverge by one ULP merely by reconstructing an already accepted pose;
- 32 single-byte mutations of accepted JSON may reject at parse, structural or
  solve validation, but cannot panic or publish non-finite/unvalidated success;
- 36 analytic slider-crank cases span six phases, two embeddings and all required
  scales, independently checking body poses, crank/rod/slider velocities, one
  feature velocity and hinge/translation coordinate rates;
- normal performance tests fix 43, 255 and 256 moving-body compile shapes. The
  explicit release-only 256-moving-body chain has 1536 active columns, selects
  `SparseQr` under `Auto`, preserves dense-authoritative rank 1536 and validates
  ordinary hard residuals within a 180-second reference budget.

## M24 sketch extension and embedding fixtures

### M24-E1 - Persistent element and source joins

The complete A8 document enumerates its document, point, scalar, curve, contact,
constraint, dimension and source identities through `DocumentElementId`. Resolving
each raw persistent ID returns the same typed element. `DocumentSourceRef` follows
semantic source order and maps every source to its exact constraint/dimension
owner, label and suppression state without runtime/core IDs.

### M24-A1 - Typed host attributes

A non-serializable host attribute type attaches to accepted geometry and sources
through `SketchAttributes<T>`. A foreign document, missing target or same raw ID
with the wrong semantic kind rejects. Deleting an attributed dimension and source
makes both values dormant; undo restores liveness, redo returns dormancy and only
explicit cleanup destroys them. Attribute changes leave accepted geometry,
runtime state, revision, audit and canonical JSON byte-identical.

### M24-J1 - Frozen version-one JSON

At the M24 boundary an empty fixed-ID document had one exact golden version-1
payload. Export used the private frozen DTO, import dispatched explicitly by
version and reproduced the same bytes. Unknown versions and injected metadata
fields rejected; application attributes required an application-owned workspace
envelope. M25-J1 supersedes current export behavior with canonical v2 while
retaining that strict frozen v1 input language.

## M25 associative linear construction fixtures

### M25-O1 - Supporting-line offset

A fixed source segment and a same- or reverse-oriented target segment use an
explicit left/right offset at scales `1e-6`, `1` and `1e6`. The two analytic rows
match finite differences, independent validation retains the selected side and
orientation, and the target reports exactly two local DOF: axial slide and
length. An algebraically parallel antiparallel root is rejected by the explicit
orientation branch.

### M25-O2 - Exact translated-segment offset

The same source and branch matrix uses four endpoint-translation rows. With the
source fixed, rank is four and local DOF is zero. Same/reversed endpoint
correspondence and left/right side round-trip through sketch JSON v2. Reference
mode has no core source or residual and reports the selected signed distance.

### M25-M1 - Point-defined mirrors

Line, open polyline, quadratic/cubic Bezier and clamped non-rational B-spline
sources are reflected across a directed line. Every mirrored control has an
ordinary `SymmetricAboutLine` source with finite-difference-checked Jacobians;
line/polyline branch directions are reflected too. An accepted source-point edit
moves its associated mirror point, and construction undo/redo restores the same
persistent IDs.

### M25-M2 - Coordinated mirrored B-spline refinement

Two equal-topology clamped B-splines with active control-pair symmetry sources
receive the same interior knot. Both gain one control and compatible span
topology, and the new pair gains one ordinary symmetry source in a single
accepted command. Undo removes both controls and the new source; redo restores
the accepted refined JSON. Missing pair associations reject before mutation.

### M25-A1 - Directed angle branch cut

Two fixed directed lines straddle `-pi`/`pi` under rotation, translation and all
three required scales. Their counterclockwise angle is `2 degrees`; editing the
target to `2*pi + 2 degrees` remains on the same explicit unwrapped branch.
Persistence and undo/redo preserve it, while an incompatible fixed `pi/2` edit
rejects and retains document/history state.

### M25-J1 - Frozen v1 migration to v2

A nonempty v1 document containing a legacy curve-length dimension parses through
the private v1 dimension DTO and re-emits deterministic canonical v2. The same
version relabel applied to a v2 offset payload rejects, proving that the frozen
v1 dimension language did not silently expand.

## M26 visual line-profile fixtures

### M26-L1 - Exact loops and explicit topology

A shared-identity square publishes one complete counterclockwise contour with
area `16`. Four coordinate-equal but identity-distinct line endpoints publish no
face until four active `Coincident` constraints explicitly weld the corners.
Moving one endpoint without solving makes its coincidence class exceed the
default hard-residual tolerance and returns `InconsistentCoincidence` with no
faces rather than silently teleporting the endpoint.
Open chains publish a complete empty result. Analysis leaves canonical JSON
byte-identical.

### M26-X1 - Diagonals, crossings and T-junctions

A square diagonal produces two area-`2` faces. A line whose distinct endpoints
lie exactly in the interiors of opposite square edges creates two ephemeral
T-junctions and the same two faces. A closed bow-tie splits its proper crossing
ephemerally and publishes two area-`1` lobes. Every contour edge retains its
source span and parameter interval.

### M26-N1 - Nested contours

A disconnected area-`4` square inside an area-`16` square publishes an area-`12`
annulus with one clockwise hole plus the independent area-`4` inner face. No
overlapping area-`16` face is published.

### M26-A1 - Overlap and numerical ambiguity

Two positively overlapping collinear segments skip their connected component
with `CollinearOverlap`; a disconnected clean square still publishes under
overall `Truncated` status. Near-collinear spans inside the determinant uncertainty
band skip with `NumericalAmbiguity` rather than being snapped or intersected.

### M26-B1 - Deterministic budgets and transforms

Candidate, fragment and cross-component containment limits return `Skipped` with
no partial faces; candidate counts divide before multiplication and fail closed
on `usize` overflow. Two large separated face components are bounded by component
boxes instead of entering all cycle-pair polygon tests. A one-face
limit over a two-face arrangement returns one deterministic face with `Truncated`
status. Rotated/translated square-diagonal arrangements at scales `1e-6`, `1` and
`1e6` preserve normalized area and reproduce exactly after canonical JSON
round-trip.

### M26-W1 - Pointer-transparent overlay

The browser renders accepted rectangle faces as even-odd SVG paths. The paths
have `pointer-events: none`, no data/object identity attributes and no effect on
selection, command history or exported JSON when the filled interior is clicked.

## M27 associative line-fillet fixtures

### M27-F1 - Audited line-jet equations and derived arc

Two fixed perpendicular bounded lines and a perturbed ordinary circular arc use
two explicit left-side strict-interior contacts, first-then-second endpoint order
and counterclockwise sweep. The four center/contact rows match finite differences
within `2e-6`, publish four structured `left_normal` audit rows and recover center
`(3, 1)`, radius `1` and contacts `(3, 0)`/`(4, 1)` with zero local DOF. The
accepted arc endpoints are derived from those contacts, and equation-free
curvature remains measurable. A pre-used or already-associated output arc and a
new executable consumer on an active output arc reject.

### M27-F2 - Explicit branch matrix and radius mobility

At scales `1e-6`, `1` and `1e6`, rotated and translated perpendicular parents run
all two-by-two normal-side, two endpoint-order and two sweep combinations. Every
driving-radius case retains its explicit branch, has zero local DOF and matches
the transformed analytic contacts. Replacing the driving dimension with a
reference radius adds no equation, leaves exactly one local DOF and reports the
accepted radius through the ordinary reference-dimension API.

### M27-F3 - Association, history and ownership

Editing a parent endpoint re-solves both contacts and re-derives the output arc
while both parents remain ordinary untrimmed lines. Atomic creation and branch
edits survive undo/redo with stable persistent IDs. Active derived angle scalars
and trim handles reject direct edits; suppression freezes the ordinary arc and
permits angle editing but retains output ownership. Direct, cascading and
indirect output deletion returns `ObjectInUse`, including while suppressed.
Deleting the association explicitly explodes it: owned contacts disappear and
the last accepted ordinary arc remains; undo/redo restores the same semantics.

### M27-J1 - Frozen v1/v2 migration to v3

Canonical version-3 JSON persists the association, two contacts, ordinary arc,
normal sides, endpoint order, sweep and radius dimension, and round-trips
byte-identically. Relabeling that payload as version 1 or 2 rejects, proving that
neither frozen older constraint language silently accepts fillet syntax.

### M27-I1 - Invalid geometry and transactional rollback

An accepted radius edit whose strict-interior contacts would escape rejects and
leaves canonical JSON and command history unchanged. Construction with an escaped
radius, exact or numerically unresolved near-parallel parents, zero radius, NaN or
infinity rejects before allocating persistent objects. Independent validation
also recomputes endpoint-order and canonical sweep data, so corrupted derived arc
state cannot become success-like.

## M28 generic-fillet and persistent-trim fixtures

### M28-F1 - Common family matrix and differentiable output arc

Fourteen regular support roles cover line/polyline, circle and arc, ellipse and
elliptical arc, rational conics, quadratic/cubic Beziers, and clamped/periodic
B-spline and NURBS spans. All 105 unordered pairs lower through one six-row
generic fillet residual: four center/normal-offset rows and two radial endpoint
alignment rows. Local AD includes only active spline controls/weights, excludes
the NURBS gauge and agrees with central differences under the documented mixed
relative/absolute policy. Point, curve-contact, tangency, curvature and continuity
consumers on the associated ordinary arc include both solved endpoint angles.

### M28-T1 - Persistent visible intervals and associative edits

A line-circle fixture owns one visible endpoint on each parent while preserving
immutable support geometry. The bounded line retains its explicit opposite native
endpoint; the full circle uses an explicit fixed periodic anchor and winding.
Editing a parent re-solves both contacts and atomically updates both visible
intervals and the output arc. Rendering, hit testing, selection and line-profile
analysis consume public interval queries, so hidden support cannot be selected or
used as a new contact seed. Contact-derived markers are pointer-transparent.

### M28-B1 - Branches, scales and periodic winding

Every normal-side, parent-order and sweep code runs under rotations/translations
at scales `1e-6`, `1` and `1e6`. Span, local neighborhood, endpoint ownership,
periodic winding and fixed-anchor winding remain explicit. A periodic B-spline
contact with nonzero winding round-trips and projects accepted trim boundaries
without being collapsed to its principal period.

### M28-L1 - Suppression, explosion and spline lifecycle

Suppression disables the six association rows, freezes both contacts, the ordinary
arc and visible intervals, and retains output ownership. Deleting the association
explicitly explodes it: owned contacts disappear, contact-derived boundaries become
fixed at their last accepted parameters, and the ordinary arc plus visible parent
views remain. Undo/redo restores IDs and branch state. Refinement of a spline span
with a trim view rejects unless an atomic semantic-span remap exists; an unowned
fixed view can be cleared explicitly.

### M28-J1 - Version-4 persistence and frozen migrations

Canonical version-4 JSON persists generic parents, trim endpoint ownership,
periodic anchors, winding, neighborhoods, sides, endpoint order and sweep and
round-trips byte-identically. Frozen versions 1 through 3 reject version-4 syntax.
Version-3 `LineLineFillet` migrates as an explicitly untrimmed legacy association
because its wire format contains no retained parent-side choice.

### M28-I1 - Invalid roots, singular offsets and rollback

Zero-speed jets, cusps, rational poles, escaped spans, non-finite seeds, ambiguous
local roots, parallel offset intersections and non-finite or unresolved
`1 - side*radius*curvature` reject before success or partial state. A second view,
conflicting owner, missing fixed opposite boundary or malformed boundary winding
rejects atomically. Accepted edits that escape their local root retain canonical
JSON, visible intervals, geometry, history and audit.

## Historical M30 interactive construction and NURBS UAT fixtures

At the M30 checkpoint, every focused lab started accepted, published its expected
equality/bounded DOF and named one primary projected drag. The retired browser reset
action reconstructed the same canonical public scenario rather than restoring
private UI geometry. The public scenarios and direct domain assertions remain;
M50 removed the lab and browser delivery.

### M30-C1 - Offsets, mirror and directed angle

- `construction-supporting-offset` fixes the source support and leaves target axial position and length free. Dragging either target endpoint preserves a left/same-direction distance of two and reports equality/bounded DOF `2/2`.
- `construction-exact-offset` anchors one source endpoint and fixes source length. Dragging the free source endpoint rotates both segments while exact endpoint translation remains equal; DOF is `1/1`.
- `construction-entity-mirror` creates the reflected line through `add_mirrored_curve`. Dragging either free endpoint projects its ordinary symmetry counterpart across the fixed axis; DOF is `1/1`.
- `construction-directed-angle` begins as a reference angle with one rotational DOF. Dragging crosses the principal cut without changing explicit orientation; editing orientation/target and switching to driving locks the intended branch transactionally.

### M30-F1 - Interactive line and generic fillets

- `fillet-line-line-reference` retains M27's visibly untrimmed parents and one reference-radius DOF.
- `fillet-line-circle`, `fillet-line-bezier` and `fillet-nurbs-line` expose M28 parent trim views and movable accepted contact/output state.
- Primary drags must move the ordinary output arc and accepted parent contacts; M28 views update atomically. A rejected aggressive root escape retains the previous arc, intervals, history and audit.

### M30-N1 - NURBS interaction labs

- `nurbs-quarter-circle` exposes a positive non-gauge weight, explicit unit gauge and draggable controls while preserving a finite rational arc.
- `nurbs-local-support` shows the selected semantic span and supports transactional homogeneous knot insertion with stable old IDs and contact migration.
- `nurbs-periodic` exposes explicit previous/next span transition, winding and knot-side state; crossing the seam changes winding only through the command.
- `nurbs-differential` exposes the existing tangent/normal/curvature and endpoint continuity audit on movable NURBS geometry.

Focused controls submitted only public `SketchDocumentSession` commands. The M30
browser tests compared accepted geometry before/after every advertised drag or
editor action; loading an example without motion was not M30 acceptance. Those
durable geometry/edit claims now remain at their direct Rust owners rather than in
a browser gate.

## M31 all-family visual-profile fixtures

The M31 corpus applies ADR 0024 to every accepted visible curve interval. It includes
standalone circular/elliptic disks, arc-line caps, overlapping circular/conic lenses,
Bezier loops, mixed analytic/polynomial contours, periodic B-spline/NURBS contours,
fillet-owned joins, nested curved holes and a clean component beside each typed
tangent/overlap/pole/budget ambiguity.

The focused `profile-fillet-trim` lifecycle moves its explicit circle-interior
closure contact across the accepted local neighborhood and requires a complete
line/circle/output-arc face after every accepted preview and release. The analyzer
splits the contacted source from active contact identity and fresh validation, never
from coordinate proximity.

The `profile-nurbs-self-intersection` lab exposes one deterministic rational cubic
loop, ordered control-point X/Y targets, non-gauge weights, knot insertion and
certified self-root parameter/position enclosures. Native regressions cover roots on
recursive partition boundaries under required scales, reflection and translation,
plus geometry-preserving knot insertion away from the root. A root placed exactly on
an inserted semantic knot boundary must remain typed incomplete until a one-sided
cross-span certificate is available; it must never disappear under `Complete`.

A captured diagnostic-capsule regression sweeps nearby positions of one local NURBS
control and requires four certified self-roots, eleven fragments, four faces and
matching endpoint topology throughout. Cycle-area integration apportions the
unchanged scale-relative display uncertainty target across its directed fragments,
then independently checks the summed interval against that original target. This
removes a shape-sensitive work-allocation failure without relaxing publication.

All supported family pairs and eligible self-pairs require deterministic parameter-
interval provenance, resolved outgoing tangent order and independently bounded area
sign before `Complete`. Required scales, rotations, reflections and large translations
preserve topology and scale area by `s^2`. Canonical JSON, history and selection remain
unchanged by analysis and pointer-transparent rendering.

### Historical text failure-case handoff

The removed M31 playground exposed a `GEOSOLVE_SCENE_V1` compressed diagnostic
capsule containing canonical v4 sketch JSON, exact profile budgets, metadata, byte
count and checksum. Import independently solved and validated the document before
replacing accepted state; malformed input retained the previous accepted scene.
M49 classified the retained canonical/import semantics and M50 retired the private
capsule UI and codec. This paragraph is historical evidence, not an import
instruction or a supported persistence format.

## Post-M32 CAD embedding and human UAT scenarios

M33-M44 add the current production-embedding fixtures without replacing the frozen
scenarios above. Cleanup M46-M53 preserves their durable behavior through direct tests and
approved post-cleanup UAT. M54-M59 complete stable diagnostics, early alpha action parity,
prepared concurrency, incremental scale and the separate operations/production-topology
companions; M60 completes the advanced workbench and M61 completes its approved advanced UAT.
M62 completes approved CAD-style constraint/dimension authoring, and M63 completes approved
geometry-anchored canvas constraint/dimension presentation, M64 completes the approved editable
purpose-based sample library and M65 completes approved predictable, bounded projected dragging.
M66 completes the explicitly approved computed-feature cut for ordinary multi-corner 2D Fillets;
M67 completed the approved cleanup cut and added no new scenario fixture. Its focused UAT used the
ordinary editable Samples catalog to prove the surviving workbench after removal of developer-only
cards and frozen harnesses. M68 completed and received supervising-human approval under ADR 0032
for the Fillet direct-manipulation scenarios below: branch-preserving radius rails, explicit local
branch/contact/
retention actions, Current-only interaction history, pointer capture and separate friendly/fold
specimens. Their implementation, focused direct qualification, clean full release gate and human
UAT are complete. M69 reuses the ordinary Construction/reference and 2D Fillet playground leaves
for the Profile/construction scenarios below; it adds no scenario-mode state. Its direct/release
qualification and focused human UAT are complete. M70 completed ADR 0034 and adds one
ordinary editable auto-constraint drafting playground; implementation and focused direct
qualification, integrated release qualification, frozen replacement-candidate publication and
served-byte verification are complete, and the scoped human UAT was approved on 2026-08-10. M70B
is the completed bounded reproduction-capsule cut. It adds a workbench-global copy/paste overlay rather
than a protected sample fixture; F001/F002 replacement qualification/publication pass and the
test-only H1 authoring/scene survey, complete release gate and fresh byte-verified publication are
historically clean. H2 preserves those exact 193 passing rows under milestone-neutral names.
Test-only H3 historically added four reviewed `feature.fillet` rows without changing the original
bytes: two F003 Coincident-closure authoring routes and two F004 same-cell line-circle evaluation
branches.
That pre-repair 197-row checklist contained 193 `PASS` plus four `DEFECT`; `--check` passed while
`--require-clean` intentionally failed. H3 changed no production behavior or release bytes.
Authorized production repairs now make the same four stable rows pass without changing their input
fingerprints. The F003/F004 repair checkpoint was 197/197 `PASS`, SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. F005 appends one exact
source-rotation evaluation row at `input-04658a77db2dc779` while preserving those 197 records
byte-for-byte; the M70B closing fixture is 198/198 `PASS`, SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`. Focused F005 owner/golden,
aggregate golden, formatting and focused warnings-denied Clippy qualification pass. Prior F003/F004
source `0ef60ef47035e8b1fb1eece2c38d05ccdfdc4abf` passes
`env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` and is retained as
historical release evidence. Clean F005 source `d400c4a8201f6afc531f5b504424d6430dbf3937`
passes that complete gate, including its 198-row clean oracle and 152.49-second 256-moving-body
sparse crossover. Its immutable seven-file snapshot `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` was
served at `http://100.94.63.83:8080/` for M70B; every file and `/` byte-matched, with ordered-
manifest aggregate `3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`.
That server has since retired. The focused F005
movement behavior was subsequently reported fixed by the supervising human, who requested sign-off
once the closing regressions were satisfactory. Clean closing source `48e3cc3` passes the complete
release gate with the two-previously-Current transaction and CircularArc transport/domain
regressions; the golden and release bytes remain unchanged. M70B is closed under that scoped
approval. M71 is complete under ADR 0035 and adds one ordinary editable **Retained drafting
relations** playground over its mechanically qualified document/editor implementation. The
pre-F003 contribution from its original four reviewed nine-row relation families extends the
current canonical fixture to 234/234 `PASS`, SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`. M71-F003's midpoint-axis
correction, M71-F004's endpoint-axis/direction composition, M71-F005's distinct-reference
orthogonal point-axis intersection and M71-F006's tighter default capture envelope remain in
focused owner regressions rather than adding systemic golden dimensions. Clean post-F005/F006
qualification and byte-verified replacement publication pass; F003/F004 evidence remains
historical. The supervising human accepted the scoped U1-U5 review and explicitly closed M71 on
2026-08-14.
Every new fixture must name its exact design, parameter, external-snapshot, activation and accepted-state
revisions. The workbench remains a desktop-only public-API consumer; no mobile scenario is
required.

Objective geometry, residual, derivative, rank, branch, persistence, migration, resource,
cancellation, presentation-adapter and topology assertions are directly automated at their
owning Rust/WASM layer. Old browser E2E is not a qualification path. Human acceptance dispositions
are recorded at completed M40.7, M53 and M61-M74. M72's scoped UAT and exact final public-artifact
verification complete its direct automated qualification. Completed M73 qualified its F001-F003
construction-stage, contextual-authoring and candidate-trace consolidation plus F004 live
world-axis span precedence, passed the clean replacement release gate, published a byte-verified
immutable Tailscale UAT snapshot, received focused supervising-human approval and exact-verified
the final GitHub Pages artifact. M73 adds no new editable sample or browser scenario mode. M74 has
explicit scoped closure approval on its clean-qualified, byte-verified F001 replacement. Its
hands-on intrinsic-datum and desktop-polish scorecard is intentionally deferred into the next
bug-fixing/UAT follow-up milestone rather than claimed as completed human evidence. Exact final
M74 Pages publication passes, and the next milestone remains unstarted.

### M69-PC1 - Explicit construction remains solver-active but interaction-distinct

Use one closed Profile rectangle, a Construction diagonal sharing two rectangle points and a
separate Construction guide exactly overlapping one Profile edge. The diagonal remains constrained
through its shared native points, while default profile analysis still sees only the closed
rectangle. In `All` picking, the Profile edge wins the exact overlap; `Construction` scope selects
the guide. Batch role conversion, Undo/Redo and workspace reload preserve every curve ID and change
no accepted coordinate, branch, residual, rank or DOF.

Direct tests own the role/edit/profile facts. Human review uses **Samples → Curves & constructions
→ Construction and reference geometry** for discoverability, overlap priority, scope/visibility,
whole-curve conversion and dashed selection presentation.

### M69-PC2 - Fillet-discarded geometry is implicit construction

Apply one start trim, one end trim and two opposite-end Fillets to bounded/open native supports.
Every materially discarded source complement is finite, contained in the prior visible interval
and published outside effective computed edges with exact source, corner and endpoint provenance.
Failed, suppressed, conflicting and full-period parent cases publish no implicit fragment.

At the editor boundary, clicking a discarded portion returns the existing native `CurveSpan` and
its parameter. The retained and discarded occurrences highlight as one complete source; no new
tree row, persistent ID or constraint operand appears. Human review uses **Samples → Curves &
constructions → 2D Fillet playground** and verifies the distinct implicit dash, Profile overlap
priority, Construction-only access and unchanged full circles/ellipses.

### M40-ES1 - Headless persistent line selection and relation action

Construct one accepted document containing two separate line segments and map it
through a finite viewport. A pointer click 6.5 px from each centerline is within the
7 px curve tolerance. The first click replaces selection; Shift/Ctrl/Command on the
second extends the ordered persistent span selection. Point endpoints win overlapping
hits. Applying **Parallel** emits one ordinary public `DocumentEdit`; no DOM target,
CSS hit stroke, renderer or browser event is part of the oracle. A 2.9 px point motion
remains a click and emits no geometry edit, while exactly 3 px starts typed drag
preview. M40.2 native tests own this regression.

### UAT-C1 - Core sketch interaction at M40.7

One ordinary mechanical profile covers geometry creation, canvas/tree/inspector
selection, standard constraints, driving/reference dimensions, projected drag,
redundancy, conflict, deletion and history. The prepared 30-45 minute review judges
discoverability, manipulation intent and whether accepted, solved-preview, unsolved
and rejected states are unmistakable. Automation proves all numerical facts.
The supervising human approved this gate on 2026-07-26 after the mechanically
requalified UAT-C1-F4 and UAT-C1-F5 targeted rechecks.

### UAT-C1-F4 - Constrained release preserves the accepted preview branch

Construct a two-link underdetermined arm with a fixed base, equal fixed link lengths,
and explicit branch directions. Drag its end back to the base through a separately
solved retained preview. Replaying the former cold release from the pre-drag accepted
state must deterministically choose an elbow position more than `0.5` model units from
that preview, proving the regression exercises the original seam. Pointer release must
instead consume the exact accepted preview session: every accepted point remains within
`1e-10` model units of the last preview, both explicit line branches are unchanged, and
no clear/cancellation effect may discard the seed before commit. The release adds
exactly one retained history checkpoint, and one Undo restores the pre-drag accepted
geometry.

### UAT-C1-F5 - Staged construction previews are wire-only and terminal

Construction previews are distinct from complete committable proposals. An open line
or polyline preview is always an unfilled wire and cannot imply profile area. After the
circle center click, its retained center marker remains visible while the radius is
positioned. A counterclockwise arc publishes its center marker after click one, a
center-to-start radius guide while placing and after clicking the start point, and only
then the normalized complete arc preview. Pointer completion, Finish, Enter and
double-click emit the same ordered terminal effects: commit the complete proposal, then
clear all provisional geometry. In particular, Finish commits only placed polyline
vertices and removes any last pointer-following unplaced segment immediately.

### M70-AI1 - Remembered reference inference is headless

This scenario was originally recorded as `Future-HI1`; ADR 0034 assigns its implemented target to
M70 without making it an M40 completion requirement. Start a line draft, hover an eligible
persistent point, semantic line midpoint or native affine span, then move away without placing the
endpoint. The editor—not the UI—retains that bounded stage-local reference. A later sample may
publish a ranked Horizontal/Vertical, Parallel/Perpendicular or midpoint-normal candidate, guide
and adjusted preview. Hysteresis owns stable entry/leave behavior and the placement click is the
explicit confirmation.

Bare-point horizontal/vertical guidance is `TrackingOnly` in M70 and does not adjust or create a
durable source. Existing persistent points are reused by identity; native curve positions create
explicit PointOnCurve metadata; midpoint outranks generic curve contact; and new real line/polyline
spans may receive H/V or a remembered affine Parallel/Perpendicular relation. Exact semantic ties
remain Ambiguous. Suppression clears the current latch/reference and a suppressed click places the
raw sample.

Point identity lowers structurally into another construction's existing-point operand rather than
creating a Coincident relation. A standalone Point-tool click on that already-existing identity is
therefore a history-neutral no-op. Candidate enumeration stops at the first unique bundle proving
its configured bound insufficient; candidate or scene exhaustion returns typed incomplete
evidence, raw coordinates and no partial semantic prefix.

Circle authoring treats its circumference click as a radius sample rather than a point operand. At
an existing persistent point, including a line endpoint, the headless proposal is **Circle through
point** and the atomic plan creates PointOnCurve(existing point, created circle). It creates no
hidden rim point. A semantic midpoint or arbitrary line interior is not eligible and cannot silently
become line contact or tangency. `M70-F001` passed direct regressions, replacement publication,
served-byte verification and its targeted human recheck.

The replay uses persistent identities and normalized 2D editor inputs and must produce identical
transitions natively and through WASM. A browser can map Shift to semantic suppression and render
the returned guide; a 3D CAD host can first map its camera ray onto the active sketch plane.
Neither may generate anchors, remember references, calculate tolerances, rank candidates, adjust
the preview or compose the inferred edit. Cancellation, stage completion, mutation, Undo/Redo,
reload and viewport/policy changes clear memory deterministically.

The publication replay must originate from the retained session's exact current accepted input.
A compatibility/render-only scene built from caller-supplied document, revision or detached stamp
may show identical inference, but cannot emit or authorize the inferred plan. Direct coordinator
regressions own this distinction; browser behavior is not the authority. The exact private seal
covers the accepted revision, design identity, viewport, native inference curves and construction
snap anchors: changing them before binding rejects authentication, while changing them after
binding revokes plan publication without disabling detached presentation.

`crates/geosolve-constraint-editor/tests/m70_transition_parity.rs` and its
`tests/fixtures/m70_transition_parity.golden.txt` bytes are the shared native/WASM transition
oracle. Focused native tests separately own exact candidate, reference, anchor and chord limits.

### M70-AI2 - Editable auto-constraint drafting playground

The **Samples → Constraints & dimensions → Auto-constraint drafting playground** is an ordinary
editable save-like leaf. It increases the post-M66 current sample catalog from 23 to 24 leaves
without changing the historical M64 22-leaf freeze. The sample contains spaced Profile and
explicit Construction reference points, lines and polyline midpoints; native circle, Bezier and
NURBS targets; parallel/perpendicular reference spans; a midpoint-normal area; and one deliberately
ambiguous overlap. Separate Profile and Construction point markers expose role/scope behavior. A
prepared Construction line over two labelled rejection-marker centres already owns Horizontal;
drawing a new line between those same identities deterministically rejects the duplicate inferred
Horizontal while preserving the draft for an off-axis retry. No specimen is computed Fillet
output.

The sample owns no guide text, scripted action, protected geometry, preselection, alternate
coordinator or read-only state. Normal drawing, selection, constraints, roles/scopes, Delete,
dragging, Undo/Redo, camera and workspace persistence remain available. Opening it clears any
prior ephemeral reference memory just like ordinary reload.

Direct headless tests, not sample coordinates, own point identity reuse, all-family contact
metadata, ranking, hysteresis, suppression, resource limits and atomic commit. Human M70 UAT uses
the leaf to assess discoverability and predictability for H/V, point reuse, PointOnCurve,
midpoint-normal, remembered Parallel/Perpendicular, ambiguity, suppression, zoom/scope,
Undo/Redo and reload. `docs/M70_UAT.md` records the approved replacement-candidate scorecard and
resolved `M70-F001` Circle-authoring recheck.

Application-workspace v5 round trips the field-opaque persistent-object and spline-span allocator
high-water needed for never-reuse after Undo/divergent history and process reload. Frozen workspace
v1-v4 fixtures migrate by deriving graph-visible maxima, while malformed, foreign or trailing
cursors reject. This checkpoint metadata is distinct from inference wake/reference state, which
remains ephemeral and is never serialized.

### M71-R1 - Retained drafting relations

The **Samples → Constraints & dimensions → Retained drafting relations** leaf is one ordinary
editable workspace containing stored-point Horizontal/Vertical, semantic-center Concentric and
native-support Collinear specimens. M71-F003 additionally owns point-to-native-line/polyline-
midpoint Horizontal/Vertical definitions. Each relation is one retained constraint/source with normal
selection, suppression, deletion, history, dragging, persistence and diagnostic behavior; the
sample adds no protected state, guide script or alternate coordinator.

Contextual Horizontal/Vertical accepts either one affine span or two stored points. Explicit
Concentric and Collinear remain distinct from Coincident and Parallel. M70 drafting intelligence
may persist stored-point H/V, exact semantic-center Concentric and certified native supporting-line
Collinear, including beyond a finite endpoint. A remembered accepted native line/polyline midpoint
may create `HorizontalPointToMidpoint` and/or `VerticalPointToMidpoint`; the point follows the live
endpoint average as the support changes. Fillet-discarded and nonlinear midpoint occurrences
remain tracking-only. A point/native-midpoint axis may compose with the complementary exact
Cartesian direction of a new line/polyline span, producing one exact-intersection preview and one
atomic two-relation plan. Same-axis, oblique, ambiguous, stale, unsupported or exhausted evidence
fails closed. One construction may create its geometry and a relation to that prospective curve
atomically without exposing an uncommitted ID.

F005 additionally permits two distinct remembered stored-point references to contribute
orthogonal axes to the same endpoint: Horizontal supplies Y, Vertical supplies X, and one candidate
owns the exact Cartesian intersection, both references, two constraint-backed guides and an atomic
two-relation plan. An exact semantic tie remains Ambiguous, one reference cannot supply both axes,
and a resource limit publishes no prefix. F006 narrows the current default inclusive capture
envelope to `6/9 px` for points/midpoints, `8/12 px` for curves and `3/5 degrees` for directions;
explicitly configured valid policies keep their caller-supplied values and hysteresis semantics.

Direct sketch/editor/native-WASM tests own 1/1/1/1/2/2 lowering, finite hard residuals, rank/DOF,
commutative operands, retained parent edits, lifecycle, draft-v5 round trips, frozen-v4 rejection,
inference ranking and exact publication authority. Human review follows `docs/M71_UAT.md` for
discoverability, annotation clarity, predictable authoring/inference and recovery. The clean,
byte-verified F005/F006 replacement is the approved M71 closing product; the scoped review and
explicit supervising-human approval pass.

### M71-F003 - Native midpoint axis alignment is durable

On clean source `5b29744f445f458cffabd176c123861f39392d12`, draw or load one accepted native
line, hover its exact midpoint to wake the semantic reference, move horizontally or vertically and
place a new point. The obsolete behavior published a tracking-only guide and committed geometry
without any retained relation because `DraftInferenceEngine::point_tracking_candidates` made only
`PersistentPoint` references durable.

The corrected public `EditorScene → ConstraintEditor → RetainedEditorCoordinator` transition must
atomically create the point plus `HorizontalPointToMidpoint` for Y alignment or
`VerticalPointToMidpoint` for X alignment. Each source owns one hard row
`P[c] - (A[c] + B[c]) / 2`; both axes may coexist and keep the point at the live midpoint after
either endpoint moves. Accepted geometry and normalized hard residuals are independently checked,
and rejection retains prior accepted authority. Only accepted native line/polyline spans qualify:
fillet-discarded and nonlinear midpoint occurrences remain tracking-only. Ambiguity, suppression,
hysteresis, stale preference and candidate bounds remain fail-closed.

The focused owner regression is
`crates/geosolve-constraint-editor/tests/m71_f003_midpoint_axis.rs`. Sketch owner proofs cover the
Jacobian, audit metadata, scales, endpoint aliases, both axes, lifecycle, deletion, invalid
operands and prepared CAS. Native transition and web DTO tests prove adapter parity without browser
equations. This is a focused defect correction, not a new systemic golden dimension. The pre-F003
publication is withdrawn; the later clean, byte-verified F005/F006 replacement is current
authority for human retest.

### M71-F004 - Endpoint point-axis and span-direction inference compose

On clean source `603194947a642917b9e44359326708de37f1a1d2`, start a line at `[0, 0]`, hover a
stored point at `[-4, 4]`, then approach `[0, 4]`. The obsolete behavior generates singleton
`Vertical` and singleton `HorizontalPoints` candidates. Their exact tie is `Ambiguous`; a biased
sample selects only one, so one coordinate remains unsnapped and only one relation is retained.

The corrected `DraftInferenceEngine` must publish one candidate at `[0, 4]` whose relations are
ordered `HorizontalPoints` then `Vertical` and whose two constraint-backed guides terminate at the
same endpoint. The symmetric `VerticalPoints + Horizontal` case must work for line and polyline
authoring. One placement lowers the exact displayed bundle into one commit plan/history step; the
accepted endpoint is independently checked against both equations and the normalized hard residual
must be finite and `<= 1e-9`. Later compatible edits retain both relations.

Only complementary exact Cartesian directions compose. World H/V and remembered
Parallel/Perpendicular/Collinear sources whose original vector is exactly axis-aligned qualify;
normalization cannot turn a finite non-Cartesian source into an axis. Same-axis relations and
oblique directions remain alternatives, distinct operands remain ambiguous, stale singleton IDs
cannot alias bundle semantics, both latches retain through the exit band, and candidate overflow
publishes no prefix.

The focused owner regression is
`crates/geosolve-constraint-editor/tests/m71_f004_axis_bundle.rs`; inference unit tests own the
composition/ranking/identity/resource matrix and `m71_transition_parity` owns native/WASM adapter
parity. No equation, Jacobian, solver priority, branch or persistence format changes. The canonical
234-row authoring/scene oracle does not exercise inference bundles and remains unchanged. Clean
F004 qualification/publication remains exact historical evidence, but F005/F006 withdraw that
snapshot from current UAT; the clean, byte-verified F005/F006 replacement is current authority.

### M71-F005 - Distinct remembered point axes compose at one intersection

Starting from the qualified F004 behavior, wake two distinct stored-point references: one supplies
a Horizontal endpoint axis and the other a Vertical endpoint axis. Approach their Cartesian
intersection while authoring a line or polyline. The pre-F005 candidate key and confirmed-reference
handoff represented only one point-tracking component, so the endpoint could not publish and retain
both point-axis relations as one semantic candidate.

The corrected headless owner publishes exactly `[vertical.x, horizontal.y]`, ordered
`HorizontalPoints` then `VerticalPoints`, two terminating constraint-backed guides and both
remembered references under one stable candidate identity. Placement commits the displayed line or
polyline plus both relations in one transaction/history step; accepted coordinates are finite,
both endpoint equations independently hold at `<= 1e-9`, and later compatible edits preserve both
relations. Polyline stage handoff retains both references rather than only the first.

Two axes from the same semantic reference do not compose because that would disguise point
identity as redundant H/V relations. Distinct pairs with exactly tied semantic/ranking evidence
remain Ambiguous until explicitly preferred, both tracking components retain only through their
shared exit band, and the first candidate-limit overflow returns raw coordinates with no candidate
or guide prefix. F004 point-axis plus complementary live-span direction remains an explicit valid
alternative. Focused owner and public coordinator coverage lives in `inference.rs` and
`crates/geosolve-constraint-editor/tests/m71_f005_cross_axis.rs`; no solver equation, Jacobian,
branch or persistence format changes.

### M71-F006 - Default capture tolerances are tighter

The historical accepted M70 candidate used inclusive `8/12 px` point/midpoint, `10/14 px` curve
and `4/6 degree` direction enter/leave defaults. F006 does not reinterpret that record. For the
current default engine only, the corresponding thresholds are `6/9 px`, `8/12 px` and
`3/5 degrees`. Thus a fresh default engine rejects old-only entry samples such as a seven-pixel
point, nine-pixel curve or 3.5-degree direction while retaining inclusive entry and latched-exit
behavior at the new boundaries.

`DraftInferencePolicy` remains explicit and authoritative: caller-supplied valid tolerance values,
validation, resource limits, suppression and hysteresis transitions are unchanged. Focused default
and boundary tests own the change; it adds no constraint, residual, branch, persistence or browser
policy. Because F006 changes interaction capture behavior after the qualified F004 publication,
that publication is historical. The clean, byte-verified post-F005/F006 replacement is current
UAT authority.

### M71-F001 - Rejected design intent remains visible over retained accepted geometry

Create a design whose accepted document contains two fixed points, then attempt a point-pair
Horizontal relation that is structurally retained but solver-rejected. Build the ordinary
historical scene through `EditorScene::from_accepted_for_design`, using the retained accepted
document for geometry and the newer design document for intent.

The rejected relation must appear in the scene's constraint-entry list with its stable
constraint/source semantics, while no accepted annotation geometry is invented for it. The
retained accepted identity and document remain unchanged, and the detached historical scene cannot
be rebound as current publication authority. `M71-F001` is resolved by the exact headless scene
owner regression
`annotations::tests::m71_f001_rejected_design_entry_is_published_without_unaccepted_annotation_geometry`.
The thin-adapter regression
`workbench::tests::rejected_constraint_keeps_a_detached_accepted_canvas_scene` also requires the
ordinary composed scene to carry the design-only entry while publishing no annotation for it.
It was independently classified `DEFECT` against source
`95d54581748292ecf2d1fb3687387b2a2a7805f8`; the pre-fix exact regression fails and the repaired
regression passes 1/1. The later M71-F003 correction withdraws the former candidate, so the clean
F005/F006 replacement qualification and publication recorded in `docs/M71_UAT.md` now supply
approved M71 closing product authority.

### M71-F002 - Direct relation applicability rejects missing selections

Select a valid stored point together with a foreign persistent point ID through the compatibility
`ConstraintEditor` API, then select two center-bearing curves where one selected span occurrence
does not exist. Direct availability must advertise no relation and direct edit construction must
fail with its compatibility error; it must not disagree with contextual authoring's
`MissingObject` classification by manufacturing a point-pair or curve-only definition.

`M71-F002` is resolved by sharing the contextual owner's exact selection-existence predicate at
the direct availability boundary. The focused regression
`tests::m71_f002_direct_relation_availability_rejects_missing_objects_and_invalid_spans` preserves
both public authoring surfaces without expanding M71 into a broader applicability refactor.
It was independently classified `DEFECT` against source
`95d54581748292ecf2d1fb3687387b2a2a7805f8`; the pre-fix exact regression fails and the repaired
regression passes 1/1. The later M71-F003 correction withdraws the former candidate, so the clean
F005/F006 replacement qualification and publication recorded in `docs/M71_UAT.md` now supply
approved M71 closing product authority.

M73-F002 later retired the compatibility surface described above: `ConstraintKind`,
`ConstraintEditor::{available_constraints, constraint_edit}` and
`EditorError::IncompatibleConstraint`. This does not reinterpret M71-F002's historical defect or
acceptance record. Current contextual ownership is
`m71_f002_contextual_relation_availability_rejects_missing_objects_and_invalid_spans`, with
empty-selection and all-family coverage owned by
`complete_relation_and_dimension_matrix_is_headless_and_selection_scoped` and
`every_resolved_relation_executes_through_the_authoring_adapter`.

### M73-R1 - One retained authoring meaning across stage, action and candidate handoff

For every construction tool and relevant stage, derive its point/center/circumference or
coordinate-only role and optional created line/polyline span slot from one private description.
Line stage one and every post-first polyline stage must retain the same segment identity through
directional inference, created-span lowering and confirmed-reference handoff. Invalid completed
stages produce no semantic descriptor. The existing point-identity precedence and prospective
curve/segment indices remain exact.

Exercise all 20 `ResolvedConstraintKind` families through `AuthoringState` and the retained
coordinator. Preserve accepted operand reversals, intentionally unsupported ordering, explicit
contact/branch choices and typed failures for empty, wrong-arity, wrong-kind, foreign/missing,
invalid-span and stale selections. The unreleased `ConstraintKind` and
`ConstraintEditor::{available_constraints, constraint_edit}` compatibility surface plus
`EditorError::IncompatibleConstraint` is removed rather than kept as a second applicability
oracle. Its public direct methods have no non-test caller; the coordinator's internal simple-
lowering use moves to the contextual owner. This does not remove lower-level sketch builders or
any persistent relation.

Confirm ordinary line/polyline, circle-through-point, centered Concentric and M71-F004/F005
compound-axis inference. The private confirmation retains the winning candidate ID, and its
candidate-owned guides, relations, references and lowered plan must agree. Ambiguous, stale,
resource-limited and rejected commits remain mutation-free. Direct tests own these facts; focused
human UAT checks that ordinary construction and contextual authoring feel unchanged. The 234-row
golden remains byte-identical and no new sample, residual, branch or persistence scenario is added.

F001-F003 status (2026-08-15): mechanically implemented and directly qualified. The stage table is
owned by `construction_stage_semantics_table_covers_every_editor_tool`; the 20-family path by
`every_resolved_relation_executes_through_the_authoring_adapter`; exact terminal provenance by
`compound_candidate_guides_confirmation_and_commit_plan_keep_one_identity`; and prospective/stale
publication by `contextual_authoring_resolution_is_prospective_until_one_coordinator_apply`.
Clean implementation source `b1b2162` passes the unchanged 234/234 golden and complete release
gate. The later byte-verified F001-F003 candidate is historical after M73-F004 and is withdrawn
from current UAT.

### M73-F004 - Live world-axis span direction owns same-axis tracking

Against the nominated F001-F003 product source
`efde645345577f44e0d6b691f7ca27eb587c4b53`, start or extend a line/polyline while same-axis
remembered point or native-midpoint tracking is awake and the live span also qualifies for a world
Horizontal or Vertical relation. On that withdrawn candidate both meanings could survive candidate
enumeration even though they adjust the same endpoint coordinate, producing redundant alternatives
or guides.

For a live world Horizontal direction whose active inference behavior both adjusts coordinates and
persists the constraint, commit `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88` retains Horizontal
and suppresses competing durable same-axis `HorizontalPoints` and `HorizontalPointToMidpoint`
tracking. Apply the symmetric rule to live world Vertical versus durable `VerticalPoints` and
`VerticalPointToMidpoint`. Follow-up `0153fc0` performs this suppression before guide publication,
candidate accounting, latch acquisition or cross-axis pairing. Follow-up `89e409a` limits it to
durable trackers: generic tracking-only cues remain visible and awake without contributing a
competing retained relation. A durable point/native-midpoint tracker on the orthogonal axis must
still compose with the live world-axis direction into the existing two-relation bundle.

This precedence is deliberately limited to live world Horizontal/Vertical span directions.
Remembered Parallel, Perpendicular and Collinear directions keep their existing alternatives even
when their stored support is Cartesian, and retained authoring still relies on the solver to reject
actual redundancy. No residual, Jacobian, solver priority, branch, persistence, golden or browser
policy changes. M73-F004 narrowly supersedes M71-F004's same-axis-alternative rule for this eligible
live world-axis case; the M71-F004 wording above remains exact historical evidence.

Focused/proportional status (2026-08-15): final focused source
`89e409a6ebe12c640ae2f313f95de67430dfa8d0` passes public regression
`m73_f004_span_axis_precedence` 3/3 with finite accepted geometry/residual and exact history. The
`inference.rs` owner matrix passes Horizontal/Vertical point and native-midpoint precedence,
suppression before guide publication/candidate budget/latch/cross-axis pairing, orthogonal bundles,
generic tracking-only cues and remembered Parallel/Perpendicular/Collinear controls. The full
editor suite passes 325 unit tests plus every integration suite; M71 F003/F004/F005 and transition
parity, warnings-denied Clippy, and unchanged 234/234 golden survey/check/clean gate pass. Clean
replacement release qualification/publication and focused UAT remained pending at that focused
checkpoint.

Regression-hardening source `f41e398d00b7a7ca1e12a12a285408a0b7bd3566` makes all four durable
point/native-midpoint by Horizontal/Vertical rows part of the focused `same_axis_span` run and
checks the exact published world-axis guide plus empty durable tracker latch. Its retained-editor
midpoint case first proves the native midpoint wake, then proves one final candidate and no leaked
same-axis top-level guide. The focused owner run passes 5/5 and the public target remains 3/3.

Clean replacement qualification/publication (2026-08-15): exact product source
`4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`, tree
`fe9897153baa974b3c5c06e7a3bf5eee76e920f2`, passes the complete clean release gate with editor
325/325 plus integrations, public M73 3/3, unchanged golden 234/234, native/WASM parity, the full
workspace/documentation/package/performance matrix, 135.18-second 256-body sparse crossover and
Trunk 0.21.14 success. Its exact seven-file read-only snapshot
`/tmp/geosolve-m73-uat.JKAWtJ`, aggregate
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`, is served only on
Tailscale at `http://100.94.63.83:8080/` and byte-verifies file-by-file plus `/`. It is current UAT
authority. The old server exited while its historical snapshot remains. M73 remained open only for
focused human UAT and explicit approval at that nomination checkpoint.

Scoped closure (2026-08-15): the supervising caller confirmed that the focused behavior works and
requested milestone closure. M73-U1 through M73-U4 are accepted for the recorded scope; direct
owner regressions remain authoritative for semantic permutations not manually replayed. M73 is
complete.

Final public publication (2026-08-15): accepted product source
`4c93ac5dd102fd52c78665a75997bcaf3d1d6f99`, tree
`fe9897153baa974b3c5c06e7a3bf5eee76e920f2`, is deployed from documentation-only approval
descendant `ef7b90feb17bfba62c45f9463ceb934fc34e6f4d` by successful Pages run `31878139709` as
artifact `9245585021`. Its downloaded inner tar SHA-256 is
`d6c210b50aa9bb7e257555f931016551402fb7a8faa5d4ccfe267c68c44ceb56` and C-locale seven-file
manifest aggregate is `4e562280bc0656f9bd7358057d62739ba02e74a5f76b0328c5e45bf18640031c`.
The public root and all seven paths return HTTP 200 and match the artifact byte-for-byte; `/`
equals `index.html`, application URLs use `/geometric-constraint-solver/`, and JavaScript/WASM/CSS
media types are correct. The separate Tailscale bytes remain UAT-snapshot evidence, not the public
publication authority.

### M74-RG1 - Intrinsic datum relations remain identity-free

Start from an empty sketch and assert intrinsic `Origin`, `XAxis` and `YAxis` scene operands exist
without adding a persistent element, variable, allocator value, history entry, geometry count or
Fit bound. Select each datum directly and in a mixed native selection. Picking and contextual
relation authoring are legal; drag, Delete, suppress/unsuppress, role conversion, Unconstrain and
Lock reject the whole action with `ProtectedDatum` and leave design/accepted identities and history
unchanged.

Exercise point/Origin, point/X axis, point/Y axis and affine line/each-axis in both operand orders.
Independently validate finite accepted residuals at several document scales, central
finite-difference Jacobians and datum-labelled audit rows. Suppress, reactivate, dependency-delete,
Undo/Redo and delete the ordinary relation while proving the intrinsic operand remains. Parallel
and Perpendicular with an axis must lower to existing Horizontal/Vertical. Draft-v5 side records
round-trip exactly; canonical-v4 encoding of any datum relation rejects with
`UnsupportedM74State` and never emits a partial payload.

### M74-I1 - Datum inference has bounded priority and compatible coordinates

For point-bearing Line, Polyline and centred-geometry stages, Origin enters at exactly `6 px`
Euclidean distance and remains active through `9 px`; each axis enters at `4 px` perpendicular
distance and remains active through `7 px`. Repeat across viewport scales. A native point/curve
candidate wins before any datum; Origin wins the shared Origin/axis location and emits one
`CoincidentWithOrigin` relation. Datums do not become remembered native references. Shift,
Reference visibility off, cancellation, stage/camera/policy/document change and Circle
circumference placement produce raw or non-datum results and clear the datum latch.

For both Line and Polyline, a live Horizontal span suppresses same-coordinate X-axis inference
before guide publication, candidate budget, latch or pairing; live Vertical symmetrically
suppresses Y axis. Horizontal still composes with Y axis and Vertical with X axis into one terminal
candidate containing the expected two guides and two retained relations. Candidate authentication,
commit, Undo/Redo and rejection are atomic and independently preserve finite accepted geometry.

### M74-F001 - Symmetric uses intrinsic X/Y axes directly

Create two otherwise-free stored points and apply Symmetric with X axis, then Y axis. For X,
independently verify the accepted point midpoint has zero Y and both X coordinates agree; for Y,
verify zero midpoint X and equal Y. Repeat at model scales `1e-6`, `1` and `1e6` and with reversed
point order. The two normalized hard rows are finite and independently within `1e-9`, their
analytic Jacobian matches central finite differences, and the accepted rank/right-nullity are
exactly two/two.

Active collection remains point → point → line/reference axis. Complete preselection accepts all
six point/point/axis permutations; a repeated point reports `SameSemanticOperand`, and Origin as
the third operand reports `WrongOperandKind` while preserving the two valid pending points and all
design/history/checkpoint bytes. Application creates exactly one accepted history mutation.

The retained definition has no synthetic curve or datum identity. Its tree entry and accepted
annotation reuse the Symmetry glyph and paired-point midpoint anchor while including the selected
datum for related highlighting. Suppress/reactivate/delete/dependency/Undo/Redo and draft-v5
checkpoint reload follow the ordinary constraint lifecycle; canonical export returns
`UnsupportedM74State`. Existing line-backed symmetry stays green. Focused sketch 9/9, editor
native/WASM 5/5 and nine append-only golden rows own this scenario; the earlier M74 candidate is
not UAT authority after this product change.

### M74-W1 - Reference presentation is polished but equation-free

At `1440x900` and approximately `1024x720`, inspect the dedicated Reference tree group, Origin/X/Y
labels, protected inspector and normal/hover/selected/related styling. Native geometry must paint
and pick above overlapping datums. References and Grid toggle independently. The adaptive SVG grid
stays aligned to Origin and uses `1–2–5 × 10^n` major spacing across zoom, but has no editor-item,
snap, relation, history or persistence semantics. Origin recentres without changing zoom; Fit uses
only native accepted geometry and an empty Fit restores the canonical camera.

The coordinate HUD normally reports raw model coordinates and uses the headless adjusted coordinate
during inference while retaining raw coordinates in explanatory text. Select, drawing, relation,
Fillet and active pan states expose distinct contextual cursors. `Ctrl/Cmd+Z`,
`Ctrl/Cmd+Shift+Z` and `Ctrl+Y` perform exactly one appropriate history action outside editable or
dialog-owned targets; Ctrl+Command, Alt-modified and editing chords do nothing to sketch history.
Pointer start, hover, double-click and wheel input in SVG letterbox bands are inert, while input in
the mapped sketch plane and existing captured-gesture completion remain unchanged. Direct Rust/WASM
presentation tests own translation; `docs/M74_UAT.md` preserves the hands-on feel scorecard. Clean
qualification and immutable Tailscale nomination pass. The supervising caller approved scoped M74
closure on 2026-08-16 without a separate hands-on pass; that scorecard and any findings are
deferred to the next bug-fixing/UAT follow-up milestone, which remains unstarted.

Final public publication (2026-08-16): accepted product source
`55693372bea4759c9a67eee14f1af3d6a9e0690c`, tree
`866fbf8b58ec19e72cbe6936e06f3615dba2f692`, is deployed from documentation-only approval
descendant `b6b1d62b49466ea06522dbdd3f5444a324d36584` by successful Pages run `31923806117` as
artifact `9257602997` through deployment `5927348343`. Its downloaded inner tar SHA-256 is
`14ef2ae52b641620f958fb9df66bb40570f0b26911da695e632ac747bb7a9985` and seven-file manifest
aggregate is `df421cc0050c31008e5cb5620092c4d05e91191fd1eccaaf020ca437ce97e725`. The public root
and all seven paths return HTTP 200 and match the artifact byte-for-byte; `/` equals `index.html`,
application URLs are repository-prefixed, media types are correct, and both public two-size
Chromium checks pass. The separate Tailscale bytes remain frozen-candidate evidence, not public
publication authority. U1-U8 and future findings remain deferred to the unstarted follow-up.

### M72-R1 - Recoverable public workbench bulk fixes

In the ordinary editable workspace, create an incompatible driving dimension that publishes a
retained rejection and computed-feature problem. Undo must restore the prior accepted geometry and
clear both native and computed problem text without reload. Redo must publish the genuine rejection
again; another Undo or an accepted repair clears it. Closing Problems hides only that exact rendered
set while its canvas/tree evidence remains; a changed failure opens automatically.

Draw an ordinary rectangle through the interactive tool. It retains four shared corners, four
directed line edges and four H/V sources, but no generated anchor, dimensions or target scalars.
Translate and resize it while independently validating finite geometry, residuals and four DOF;
Undo/Redo treats construction as one history step. Direct uses of the constrained rectangle macro
continue to produce A1 semantics.

At `1440x900` and approximately `1024x720`, activate Equal, Tangent, Continuity, every dimension,
Fillet, each Conic-family tool, NURBS and Construction display. One bottom-left canvas overlay opens
from each centered main palette button without a separate chevron, shows only relevant controls,
remains contained and remembers valid values until refresh. Re-invoking the same family is
idempotent. Blur, outside/canvas clicks, zoom and ordinary controls leave the overlay open;
switching tools closes or replaces it. Escape and the overlay `×` close it, activate Select and
focus Select. Invalid inactive-family fields cannot block an unrelated tool.

After the clean release gate, the same qualified workbench is deployed from `main` to
`https://arduano.github.io/geometric-constraint-solver/`. Its stylesheet, JavaScript and WASM use
the repository prefix, every expected file returns successfully with the WASM media type, and an
ordinary browser-local workspace survives reload. Human review and closure are recorded in the M72
UAT record. The accepted `b700313` follow-up is deployed by run `31862218764` as artifact
`9241248173`; all seven public files byte-match that artifact with the expected media types, and
the two-size public Chromium contract passes. M72-R1 is complete under the scoped 2026-08-15 human
approval.

### M70B-R1 - Complete workspace reproduction payload

Create or open any ordinary editable workspace containing representative persistent sketch state,
at least one computed Fillet, constraints/dimensions and a Construction curve. **Copy repro**
serializes the current retained coordinator freshly through `WorkspaceSnapshot` v5 and shows one
single-line `GEOSOLVE_REPRO_V1` value in a visible overlay. The capsule contains design
and accepted document payloads, accepted-current provenance, feature intent, allocator high-water
and lifecycle revisions already owned by workspace v5. It does not contain the current tool,
selection/hover/pointer state, camera, sample identity, guide text or command-history cursor.

The transport uses one deterministic zlib stream, strict unpadded base64url, exact decoded length
and an FNV-1a accidental-corruption checksum. The complete text, compressed body and decoded
workspace have separate 16 MiB, 12 MiB and 64 MiB limits. Loading first validates that transport,
then the ordinary strict workspace envelope, then reconstructs a complete retained coordinator.
Only the fully reconstructed value replaces the live workspace. Corruption, truncation, trailing
data, oversize input, invalid workspace semantics or coordinator reconstruction failure leaves the
current scene unchanged.

Human M70B UAT assesses discoverability, copy/manual-copy fallback, text handoff, exact visible
restore and recoverable error presentation through `docs/M70B_UAT.md`. Direct native Rust tests
own canonical bytes, resource limits, computed-feature/high-water fidelity and atomicity, while
the same codec path must compile for WASM. The native `geosolve-repro` stdin decoder lets a
recipient inspect decoded workspace JSON without
granting it publication authority. The scenario revives neither `/#/dev/lab` nor browser
E2E/file/download/raw-`localStorage` exchange.

### M70B-F001 - Open contact-neighbourhood drag boundary

The first payload handed off through M70B has envelope identity
`8446:ea81c82137d5b13c`. Its minimal public-document regression retains the exact accepted
geometry and branch metadata while using test-local persistent IDs:

- one otherwise-free line start; the line end has a periodic point-on-circle contact;
- one ellipse whose major-axis point has a bounded point-on-line contact;
- no fixed source, dimension, trim view or computed feature; and
- the line contact was picked at parameter `0.37362649353483557` with explicit Local
  neighbourhood `[0.17362649353483556, 0.5736264935348356]`. Its restored accepted parameter is
  already `0.5268478331756027`.

The accepted graph has numerical rank `4`, equality and bidirectional bounded mobility `10`, no
near-singular warning and no initially active bound. Drag-locality planning for the free line start
owns five passive freedoms through three deterministic point anchors. Before the correction,
larger horizontal/vertical samples converged at one edge of the Local interval and then failed
independent validation with `AmbiguousContactNeighborhood`; this was neither rank loss nor a
nonlinear convergence failure.

Local contact neighbourhoods are semantically open, while core coordinate bounds are closed. The
sketch compiler therefore represents a Local interval by its nearest closed representable
interior, `[lower.next_up(), upper.next_down()]`. Independent validation keeps the original strict
branch test, and the persisted contact neighbourhood is neither widened nor rewritten.

The headless regression moves the free line start by `+/-0.5` horizontally, `+/-0.5` vertically
and two larger diagonal reversals in one continued gesture. Every sample uses exactly one bounded
attempt, reaches the requested point within `1e-8`, remains independently hard-valid at normalized
residual `<= 1e-9`, preserves all ten equality freedoms and keeps the contact parameter strictly
inside the same persisted Local interval. A direct sketch test separately verifies that the core
bound lies strictly inside both persisted Local edges and that accepted branch metadata is
unchanged. No tolerance, rank rule, drag retry, payload migration or browser policy changes.

### M70B-F002 - Radial Normal support and rejected-scene retention

The second payload handed off through M70B has envelope identity
`6037:eecc886c0e61208f`. Its accepted parent contains one circle and one line whose end point is
coincident with the circle perimeter. The retained rejected design adds radial Normal as a second
point-on-curve source from the circle centre to the line:

- circle centre `(0.9830076032045713, 2.569500433739858)`, radius
  `1.7643099377746696` and periodic perimeter contact `3.7647919835238595`;
- directed line start `(-2.2974945144665004, -0.32237077103638284)`, end
  `(-0.4496391860811665, 1.5397855539407332)` and picked Normal parameter
  `0.5237281588081177`; and
- no fixed source, dimension, trim view or computed feature.

Before the correction, generic contact defaults persisted the radial relation as bounded
`[0,1]`/Interior. That silently changed “circle centre lies on the line support” into “circle
centre lies inside the finite segment.” The unique projection of the accepted centre onto this
line is about `1.6632787580742947`, beyond the end. Starting at the unrelated click parameter led
secondary optimization toward the degenerate zero-radius branch; the radius reached about
`1.39e-17`, termination stalled after 17 iterations and the maximum normalized hard residual
remained about `1.53e-2`. This is a satisfiable underconstrained graph, not a genuine conflict.

Radial Normal authoring now publishes exactly one SupportingLine/Interior contact, winding zero,
no tangent orientation or side branch, and seeds its affine parameter from the circle/arc centre's
unique projection in compatible retained accepted geometry. It never reads newer rejected design
coordinates. Direct bounded/local radial requests fail before retained mutation. The
payload-derived application accepts with independently validated normalized hard residual at most
`1e-9`; a fixed external segment `(2,0)->(3,0)` with centre `(0,0)` verifies parameter `-2` for
circle and arc supports in both operand orders. A rejected design with centre `(100,0)` separately
freezes the historical accepted seed at `-2`. The relation remains radial centre-on-support
incidence, not contact-bearing tangency/normality at the selected circumference point.

The payload also freezes the presentation failure: its design revision is newer than its retained
accepted revision. That historical accepted document remains the only authoritative visible
geometry. The workbench composes and paints it as a detached scene while
`accepted_state_for_current_input()` is absent; `with_retained_session` must still reject it, so
the stale scene cannot emit inferred construction. No attempted or invalid geometry is painted,
and no retained-session authority rule is weakened. The companion current-computed row freezes the
opposite invariant: exact-stamped Fillet output remains composite and authenticated, and a failed
current composition cannot silently fall back to an authoritative native scene.

### M70B-F003 - Coincident-closed triangle Fillet authoring (resolved)

Disposition: resolved by an authorized production repair. It was independently reproduced and
classified `DEFECT` against source `63845836d3245eccc7ab7f820ac60ba2d562f7e1`.

Draw one open three-span triangle polyline with four persistent points, then add an ordinary
Coincident constraint between its distinct first and last points. The first and last points begin
at different finite coordinates. Their accepted coordinates agree, all four accepted points remain
finite and independent hard validation reports normalized residual at most `1e-9`. The two ordinary
interior corners form a valid two-corner computed-Fillet preview.

At the historical test-only checkpoint, the closure corner was not authorable. Selecting either
coincident endpoint as a point returned `WrongOperandKind`. Selecting the last and first spans
explicitly collected the first support, then returned `DuplicateSupport` with the message that
same-curve Fillet parents must be adjacent spans of one open polyline. Both paths retained the exact
prior authoring/preview state and published no computed feature, so this was a headless topology/
authoring defect rather than a solver-convergence or browser-layout failure.

The root cause was direct comparison of persistent point IDs in Fillet topology. The distinct first
and last IDs were geometrically solved together by an active explicit Coincident constraint but
were not recognized as one semantic join. `SketchDocument::point_coincidence_representatives` now
deterministically computes transitive active-Coincident components. Suppressed constraints do not
join their points, and exact or near coordinate overlap never implies coincidence. Headless point-
to-corner incidence, same-polyline pair eligibility and retained-endpoint hints consume those
representatives while preserving the original persistent IDs.

The focused public-boundary regression is now positive:
`m70b_f003_coincident_triangle_closure_is_filletable_by_point_or_curve_pair` in
`crates/geosolve-constraint-editor/tests/m70b_closed_triangle_fillet.rs`. It proves either
Coincident closure endpoint and both first/last span orders produce the same closure corner, one
three-corner preview and one Current feature containing three Fillet arcs.

The historical H1/H2 golden passed all 193 rows because that matrix covered the complete retained
constraint/dimension authoring inventory and four scene-authority states, but did not execute
`FeatureAuthoringTool::Fillet`, point-to-corner incidence or curve-pair collection. H3 recorded
the systemic gap as two isolated reviewed rows without replacing the focused owner regression:

- `feature.fillet.authoring.coincident-closure.point`; and
- `feature.fillet.authoring.coincident-closure.curve-pair`.

At H3 both rows were reviewed `DEFECT` with finding `M70B-F003`, without any production correction.
They now retain the same case identities and input fingerprints while passing:

- point: `input-4ba571059db7afff`; and
- curve-pair: `input-d04adbf29c08b9bd`.

### M70B-F004 - Persisted line-circle Fillet same-branch traversal (resolved)

Disposition: resolved by an authorized production repair. It was independently reproduced and
classified `DEFECT` against source `b10bc6b2de478239472b08fe71727ccbb49d67ab` from payload
identities `4752:daa87c91c75abf9f` and `4750:beda1885b15e38b5`.

Both application-workspace v5 payloads restore through the ordinary bounded decoder and retained
coordinator. Their accepted sketches are finite, independently hard-valid at normalized residual
below `1e-9`, rank one and six-DOF. They share the same circle centre
`(-0.9640476565370273, 2.537115794695225)`, circle radius `1.1815315903695374` and persistent
radius-1 Fillet. The accepted horizontal line height is `0.079969938399629` in the first case and
`2.043335287688455` in the second; the right endpoint and resulting line extent also differ.

The persistent branch is identical in both cases: circle Right/End with picked parameter
`6.010678569256539`, Local cell `[4.712388980384694, 7.853981633974479]` and periodic anchor
`2.869085915666746`; line Left/End/Interior; FirstThenSecond endpoint order; and counter-clockwise
sweep. At the historical test-only checkpoint, persisted evaluation returned
`ComputedFeatureFailure::NoLocalRoot` and published no generated arc or partial source fragments.

Public contact reseeding through the computed-feature authoring snapshot finds finite,
independently validated roots on that same explicit branch. Their circle contacts are
`5.551739581930468` and
`6.517367674350060`, both strictly inside the stored Local cell, with unchanged normal sides,
retained endpoints, endpoint order and sweep. The latter crosses the periodic parameter seam and
is represented with winding one without leaving the total-parameter cell. Their displacement from
the stored seed is respectively about `0.458939` and `0.506689`. The historical persisted non-
affine evaluation narrowed the certified cell to 12.5% of its width around the old seed—about
`0.392699` here—so both viable roots were excluded and misclassified as absent. These payloads
therefore exposed one
source-edit locality defect, not two findings or missing normal-side branches.

The root cause was the persisted-evaluation path applying the generic 12.5%-of-cell seed-connected
window to constant-curvature circular offsets. For a Circle or CircularArc paired with affine
support, a nonsingular fixed-radius offset cannot fold within one certified tangent-orientation
cell. Persisted evaluation therefore now searches that complete explicit cell. General nonlinear
curves retain the narrower seed-connected guard because their offset regularity can change inside a
cell, and radius continuation retains its fold and remote-root guards.

The positive owner regression lives in `geosolve-sketch-features`. It preserves both payload
fingerprints, independently hard-valid accepted state, source and span identities, normal sides,
retained endpoints, endpoint order, sweep, Local cell and winding. Both exact persisted evaluations
are now Current and publish finite independently validated arcs without an implicit branch change.

The historical H1/H2 193-row golden remained green because its only computed-Fillet row presented
an unchanged precomposed Current scene and exercised neither native source edits nor traversal of a
persistent nonlinear branch cell. H3 recorded the systemic branch dimension as two compact,
isolated reviewed rows while retaining the exact feature-owner regression and payload evidence:

- `feature.fillet.evaluation.line-circle.same-cell-lower` freezes the lower same-cell root with
  winding zero; and
- `feature.fillet.evaluation.line-circle.same-cell-seam` freezes the periodic-seam root with
  winding one.

At H3 both rows were reviewed `DEFECT` with finding `M70B-F004` and independently validated the
viable branch rather than treating an evaluation status as their geometric oracle. They now retain
the same case identities and input fingerprints while passing:

- lower same-cell: `input-f9920c3cf170130d`; and
- periodic seam: `input-2da21ef04cfb4246`.

### M70B-F005 - Persistent line-circle Fillet source-rotation continuity (qualified/published)

The complete loadable capsule is preserved at
`crates/geosolve-demo-web/tests/fixtures/m70b_f005_repro.txt`; its identity
`4228:0823d31f269300af` restores through the ordinary decoder/workspace/coordinator path to an
unconstrained, finite, independently hard-valid accepted sketch at rank zero and seven DOF. It
contains a circle at
`(0.16002449354493023, 1.9065418176251467)` with radius `2.201783656372145`, an affine line from
`(-2.6404041434913528, 2.0437056692350866)` to
`(1.371638516099403, 4.855564627238864)`, and one persistent radius-1 Fillet. The circle parent is
Right/End with total seed `6.299486624551188`, Local witness
`[4.959571177211237, 7.857323073392596]`, winding one and an explicit periodic anchor; the line is
Left/Start/Interior. Endpoint order is FirstThenSecond and sweep is counter-clockwise.

After the line rotated, persisted evaluation reported `NoLocalRoot`. The intended contact is total
circle parameter `7.909322804062922` (principal `1.626137496883336`, winding one), line parameter
`0.796915905159832`, with centre `(-0.017075528971715, 5.103423761681947)`. Independent incidence,
radius, tangency and signed-side residuals are approximately machine precision and normalized
transversality is about `0.527757`. The contact lies only `0.051999730670326` above the stale stored
upper certificate. Fresh interval certificates around the stored seed and intended root overlap,
so the apparent cardinal/90-degree break is not a geometric fold. The alternative contact at
about `9.021239181530` lies across the real tangent-orientation barrier and remains rejected.

The repair keeps ordinary persisted evaluation as the fast path. Only a circular-plus-affine
`NoLocalRoot` may search the complete retained circular support. Each candidate must have a fresh
cell connected through strict stored-to-seed and seed-to-candidate certificate overlap; zero or
multiple material roots, a fold, offset singularity, invalid geometry or true barrier fail closed.
The same proof is reconstructed before publication. Standalone evaluation remains read-only;
after an accepted native edit, the retained coordinator may atomically promote only the exact
contact-frame refresh derived from that evaluation. It refreshes contact parameters, winding,
periodic anchor and Local certificate while preserving radius, sources, sides, retained endpoints,
endpoint order and sweep. Generic nonlinear parents and radius continuation are unchanged.

The movement regression drives the actual headless point gesture through the stale `90.19°`
certificate edge and the true `90°` cardinal point while the finite line contact remains interior.
It then covers a genuine out-of-segment sample. Harmless crossings publish continuously and may
accumulate a full periodic winding; a genuine limit retains the last complete native-plus-Fillet
preview and its release coordinate, exposes a targeted corner/two-parent cue, and recovers on a
valid reverse sample without changing roots. A first-sample limit commits nothing; a terminal
invalid sample commits only the previous valid preview. Mixed Current plus unrelated Failed sets,
exact-edit replay binding, Undo/Redo, cold restore and stale/detached scene authority are also
frozen. Direct document edits remain free to leave a computed feature visibly Failed.

The final closing audit additionally requires one focused retained-coordinator sequence with two
distinct features that both begin `Current` and only one becoming invalid during projected
dragging. It must withhold the entire candidate rather than paint a partial/native-only scene,
retain the paired last-valid scene and release coordinate, attribute only the failing feature,
recover in reverse and release only the last valid sample. This is an owning-layer transaction
regression, not a sixth static Fillet golden row.

The same closing cut directly covers the finite CircularArc member of the circular/affine transport
path. Both parent orders move one regular contact beyond a stale Local witness while preserving
explicit sides, retained endpoints, endpoint order and sweep; finite incidence, radius, tangency,
signed side and bounded-domain state are independently checked. A same-orientation root on the
complete supporting circle but beyond the finite arc endpoint remains Failed with no partial edge.
This primitive permutation is likewise focused owner coverage rather than another golden row.

Mouse-up is staged rather than repaired after publication: the candidate native session, exact
contact-frame-only sidecar, cold no-hint computed output, allocator, checkpoint, history and replay
transition must all succeed before live authority changes. A forced allocator exhaustion between
continued evaluation and the cold durability proof leaves design/accepted JSON, feature JSON,
computed snapshots, allocator high-water, history, transcript and the held solved/computed preview
unchanged. Replay rejects the authentic transition after a parameter-input revision changes even
though design and edit identities still match. A separate ordinary constraint-action regression
proves non-`Edit` actions cold-evaluate without persisting an unrecorded feature revision.

Owner regression
`m70b_f005_line_circle_source_rotation_transports_persisted_branch_cell` freezes the exact sketch
and feature JSON, rank/DOF, metadata, intended and alternate roots, full-circle non-trimming and
read-only state. The systemic golden row is
`feature.fillet.evaluation.line-circle.source-rotation.retained-start` at fingerprint
`input-04658a77db2dc779`.

The sequence-level owner suite is
`crates/geosolve-constraint-editor/tests/m70b_f005_retained_movement.rs`; it is deliberately
focused rather than appended as nine near-duplicate static golden rows.

### M70B-H1 - Continue-through-failure authoring and scene oracle

This test-only survey turns the complete UI-exposed authoring family inventory into a repeatable
defect checklist without adding a sample, browser harness or runtime behavior. Every one of the
sixteen `ResolvedConstraintKind` families and five `DimensionKind` families has one exact
deterministic witness plus eight variants derived from fixed base seed
`aa6ab88cc8aa4878c51d78db3d1b993355406fce8c6c42353a850c05696c2edd`.

The eight indices explicitly schedule span reversal, compatible operand-order reversal and
perturbed recovery while seeded values vary finite translation, scale, rotation and contact
parameter. Tangency exercises both orientations, Equal-curvature cycles every relation choice and
endpoint continuity cycles G0/G1/G2/rate-explicit parametric C2, including path-oriented signed G2
curvature and both pre-satisfied and displaced unequal-rate C2 witnesses. Dimension cases create
one Driving dimension, edit its display target, Undo and Redo while independently comparing the
accepted measurement, persisted target and ModelUnits/AcuteDegrees metadata. A passing accepted row
requires the exact resolved/stored definition and branch metadata, finite current publication,
independent hard validation at normalized residual at most `1e-9` and a public geometric
postcondition.

Four additional rows cover the actual scene-authority state space: current empty computed output,
current computed Fillet output, current native fallback under Withheld computed output and detached
historical accepted presentation beneath rejected design. A fresh coordinator reports a current
empty computed snapshot, so the oracle does not fabricate an unreachable native `Absent` row.

At H1, every authoring and scene row ran in a separate bounded process. Semantic defects, panics,
timeout/hard-kill exits and harness errors were written to the stable six-column TSV while later
rows continued. The driver rejected nonzero child exits even if a TSV existed, verified the exact
193 case/family pairs and froze each authoring PASS row's effective scheduled-input fingerprint.
The H1 `--check` compared those rows with `golden_authoring_scene_oracle.golden.tsv`, while
`--require-clean` additionally rejected any non-PASS row. H3 retains that contract over the
expanded inventory described below.

The initial 2026-08-11 survey was clean: all 193 rows passed and opened no finding at survey time.
Later human UAT opened `M70B-F003` outside the matrix's constraint/dimension and scene-authority
scope; no reproduction payload was needed because the exact topology is compactly constructed
through public Rust APIs. `docs/M70B_HARDENING.md` owns the full readable checklist,
commands, seed and limitations. This does not replace the exact M70B-F001/F002 payload regressions,
the broader M55/M62 family-by-primitive coverage or the supervising-human M70B close record. Clean
source `dd645d99e705e56c80ab2a4a136f7a4d03baafbf` also passes the complete release gate and its fresh
seven-file Tailscale snapshot is byte-verified.

### M70B-H2 - Canonical golden defect workflow

H2 keeps H1's exact 193-row family/scene inventory, fixed seed, input fingerprints, TSV schema and
golden bytes while moving the test, fixture, process-isolated driver, environment variables and
scene survey to milestone-neutral names. The golden remains the broad compatibility matrix rather
than the sole home for defects. A reproduced finding first gets the smallest public owning-layer
regression; the matrix expands only when it exposes a systemic family, branch, transform,
operand-order, lifecycle or authority-state gap.

On the clean H2 source, the release gate invoked
`scripts/golden-authoring-scene-oracle.sh --require-clean` and passed. Reviewed finding IDs may
belong to the active milestone rather than M70B alone. The repository-local
`.agents/skills/geosolve-harden-defect/` workflow owns intake, payload preservation, reproduction,
layer routing, independent invariants, matrix-expansion decisions and proportional qualification.
It excludes pure browser/CSS defects unless evidence crosses a Rust headless or scene-authority
contract. H2 adds no residual, solver behavior, persistent schema, browser behavior or UAT scene.

### M70B-H3 - Reviewed computed-Fillet golden expansion

H3 adds the two systemic computed-Fillet dimensions exposed only after F003 and F004 had focused
owner-layer characterizations. `crates/geosolve-constraint-editor/tests/golden_fillet_oracle.rs`
drives five public-boundary cases, each in its own bounded process through the existing aggregate
driver:

- `feature.fillet.authoring.coincident-closure.point` — `M70B-F003`;
- `feature.fillet.authoring.coincident-closure.curve-pair` — `M70B-F003`;
- `feature.fillet.evaluation.line-circle.same-cell-lower` — `M70B-F004`, winding zero; and
- `feature.fillet.evaluation.line-circle.same-cell-seam` — `M70B-F004`, winding one; and
- `feature.fillet.evaluation.line-circle.source-rotation.retained-start` — `M70B-F005`, moved
  affine source with overlapping fresh certificates.

The first pair uses the public headless feature-authoring/coordinator path. The next pair uses
the public computed-feature evaluation boundary and independently checks finite accepted geometry,
hard validity, source/contact incidence, radius, tangency, signed normal side, source/span
identity, contact parameter, winding and membership in the unchanged Local cell. Public contact
reseeding is evidence that the branch remains viable, not a substitute production path. The F005
row uses that same public evaluation boundary while varying the affine source and requiring fresh
certificate overlap rather than membership inside a stale numeric interval.

At the historical test-only H3 checkpoint, all original H1/H2 row records remained byte-identical
and the inventory was 197 rows: 193 `PASS` plus four reviewed `DEFECT`. That checked golden
SHA-256 was
`a7fa99c3e7668c023a05c1bdeb7d2b794116f6f60b1d186e8115eff4bad117ec`.
`scripts/golden-authoring-scene-oracle.sh --check` passed; `--require-clean` intentionally failed
on exactly the four rows above. H3 changed no residual, solver, feature-authoring, feature-
evaluation, persistent schema, browser or release behavior.

After authorized F003/F004 production repairs, the same four case IDs retain the exact input
fingerprints listed above and transition `DEFECT` to `PASS`. The original 193 row records remain
byte-identical, so the F003/F004 repair checkpoint was 197/197 `PASS` at SHA-256
`035a72ddb611997be285bfc623d52b0dc3e6fe99eaec625d527c611fd31fd190`. Its focused and complete
workspace qualification passed. Clean source
`0ef60ef47035e8b1fb1eece2c38d05ccdfdc4abf` passes the complete release gate. Its immutable
seven-file snapshot `/tmp/geosolve-m70b-f003-f004-uat.lKC2xY` was served at
`http://100.94.63.83:8080/` for that historical checkpoint; every file and `/` byte-matched the
snapshot, whose ordered-manifest aggregate was
`96cc64dec998074ede56e3e38fb919a4854d0e0dbb8030138393e01a3d0844d3`. F005 superseded that
publication.

F005 preserves all 197 records and appends the source-rotation row at fingerprint
`input-04658a77db2dc779`. The M70B closing fixture is 198/198 `PASS` at SHA-256
`bd2e550b94924f173da09943ba5b8451341348aa6937c9f211b3cca1534b980b`; its focused owner/golden,
aggregate golden, formatting, warnings-denied all-workspace Clippy, locked all-feature workspace
tests and the relevant WASM check pass. Clean source
`d400c4a8201f6afc531f5b504424d6430dbf3937` passes the complete release gate. Its immutable
seven-file snapshot `/tmp/geosolve-m70b-f005-uat.Q5c9Wi` was served and byte-verified at
`http://100.94.63.83:8080/` for M70B, with ordered-manifest aggregate
`3173fa529fa14fab5783cf4cb4733b17db5e6850ff5d6c63022fe712a0be4c7f`; that server has since
retired. The supervising human later
reported the F005 movement behavior fixed and requested sign-off once the closing regressions were
satisfactory. Clean source `48e3cc3` passes the complete release gate after adding the focused
two-previously-Current retained-coordinator transaction and finite CircularArc/affine transport/
domain scenarios. The 198/198 golden remains byte-identical, and the generated seven-file build
matches the immutable F005 snapshot at the same aggregate. The scoped decision accepts the recorded
M70B scope without claiming an unrecorded exhaustive UAT replay. M70B is closed.

### M41-A1 - Construction geometry remains solver-active but profile-ineligible

A closed square initially publishes one complete visual profile. Mark its curve as
construction and constrain one of its persistent points to a different fixed position.
The accepted solve must move that point to the constraint target and retain a runtime
curve mapping, while default visual-profile analysis publishes no face. Undo and redo
of the role edit preserve the persistent curve identity.

### M41-A2 - Typed transitive inactivity and exact reactivation

Suppress one persistent operand of a trimmed-fillet design through user state, then
reactivate it; repeat with newer immutable host-configuration revisions. The direct
operand reports the requested typed reason and dependents report the unavailable
dependency identity before lowering. A rejected conflicting reactivation retains the
previous accepted input stamp. Successful reactivation restores draft bytes exactly,
including branch, span/sweep, winding, contacts, trim associations and output ownership,
without selecting from coordinates.

### UAT-C2 - CAD host semantics at M53

Historical/superseded UI record: this M53-M55 section describes the typed scenario selector and
guided sidecar at their approved checkpoints. M64 later flattened retained fixtures into the
ordinary editable **Samples** catalog and removed scenario mode, reset/exit controls, guidance,
transcript and evidence capture. The domain/error behavior and direct tests below remain relevant;
the selector paths and interaction instructions do not describe the current workbench.

M52 directly qualified four fixed fixture families and ten objective points without
recording human approval. Completed M53 presents that same behavior as six typed scenarios and adds
two M53-P013 error-presentation scenarios under the stable root `m53-host-semantics`
(**M53 Host semantics**):

| Selector group | Stable scenario ID | Scenario title | Objective points |
| --- | --- | --- | --- |
| `geometry-intent` (**Geometry intent**) | `role-profile-participation` | Role & profile participation | P1 |
| `geometry-intent` (**Geometry intent**) | `activation-dimension-mode` | Activation & dimension mode | P2-P3 |
| `host-owned-inputs` (**Host-owned inputs**) | `shared-parameter-proposal` | Shared parameter & proposal | P4 |
| `host-owned-inputs` (**Host-owned inputs**) | `invalid-stale-parameter-recovery` | Invalid/stale parameter recovery | P5 |
| `host-owned-inputs` (**Host-owned inputs**) | `external-loss-explicit-recovery` | External loss & explicit recovery | P6-P7 |
| `truth-evidence` (**Truth & evidence**) | `lifecycle-evidence-natural-pass` | Lifecycle, evidence & natural pass | P8-P10 |
| `error-attribution` (**Error attribution**) | `attributed-canvas-error` | Attributed canvas error | P11 |
| `error-attribution` (**Error attribution**) | `global-canvas-error` | Global canvas error | P12 |

Here P1-P10 identify the preserved M52 objective verification points and P11-P12 identify the
M53-P013 canvas-presentation checks in `docs/M53_UAT.md`,
not its `M53-Pxxx` finding and process identifiers. Selecting a scenario constructs and
activates its deterministic ephemeral candidate; switching selects a fresh candidate and
**Reset scenario** reconstructs the selected one. Global **Capture typed evidence** remains
available across scenarios, while **Exit scenario** discards all scenario state and restores
the unchanged ordinary workspace.

Inside the top dropdown, group branches are recursive right-expanding flyouts: hover or keyboard
focus exposes the next level immediately, while narrow layouts render the same branch inline. The
flyout state is ephemeral navigation presentation and never becomes scenario or workspace state.

The guide sidebar publishes the selected scenario's description, objective points, human
questions, typed steps, expected outcome and recent transcript/evidence. Stable definitions
may group and present typed candidate actions, but the browser does not derive equations,
accepted state, revisions, digests or recovery semantics. Those remain products of the
direct-qualified fixture transitions and public domain/audit APIs. The completed human review
judged discoverability, state, ownership and recovery clarity as Pass.

`attributed-canvas-error` begins with an accepted fixed two-point line and a reference
line-length dimension whose stored target is intentionally incompatible with the fixed geometry.
Changing the dimension to driving creates a retained rejected design. The headless-editor problem
metadata targets the attempted dimension owner and persistent visible operands through attempted
source mappings and the document dependency graph. The renderer keeps the prior accepted line
authoritative while highlighting resolvable points, curve, constraints and dimension annotations;
returning the dimension to reference mode accepts and clears the current problem.

`global-canvas-error` submits an angle value to a length parameter. The typed input failure has no
defensible individual canvas target, so its metadata scope is global and the canvas presents one
top-right marker without highlighting unrelated geometry. A subsequent valid length batch advances
accepted state and clears the marker. Both examples expose the same message in non-mutating
hover/focus tooltips and the canonical Problems panel.

### M55-AP1 - Alpha action-surface parity matrix

At the M55 checkpoint, the now-retired headless qualification corpus and reusable workbench
scenario catalog jointly covered every preserved M13-M14 constraint, dimension and explicit branch
action without restoring the old application. M67 mapped the retained corpus claims to current
direct tests in `docs/M67_M40_OWNERSHIP.md`. The historical matrix includes:

- one-point, two-point, point-curve, line-line, circle/arc and generic curve-pair applicability;
- fixed, coincident, horizontal, vertical, point-on-curve, parallel, perpendicular, equal-length,
  equal-radius, midpoint, symmetry, contact and tangency actions;
- distance, segment-length, radius, diameter and oriented-angle dimensions in applicable
  driving/reference modes; and
- tangent orientation, contact neighborhood, parameter-domain, span and winding edits, including
  incompatible/rejected attempts that retain the prior accepted scene and current branch.

Native editor/coordinator replay is authoritative for applicability, typed effects, disabled
reasons and branch transitions. Direct workbench tests own labels, control visibility, glyphs,
annotations and accessibility; the WASM adapter must emit the same action identities. Scenario
definitions may construct deterministic alpha operands that ordinary core tools do not yet author,
but accepted geometry and outcomes still come exclusively from public sketch/editor APIs. No
scenario contains an equation, expected-coordinate shortcut or browser-owned compatibility rule.

The reusable catalog root is `uat-scenarios` (**GeoSolve scenarios**). It retains the complete
eight-leaf `m53-host-semantics` subtree and contains this independent M55 subtree:

| Selector group | Stable scenario ID | Scenario title | Direct purpose |
| --- | --- | --- | --- |
| `m55-action-parity` (**M55 Contextual constraints**) | `alpha-parity-catalog` | Contextual relation & dimension catalog | Inspect the accepted public alpha corpus, semantic relation glyphs and all five dimension annotations without a legacy app. |
| `m55-action-parity` (**M55 Contextual constraints**) | `alpha-branch-recovery` | Contact branch & rejection recovery | Exercise typed A3 tangent-orientation state, a retained impossible fixed contact, accepted-state truth and bounded Undo recovery. |
| `m55-action-parity` (**M55 Contextual constraints**) | `circle-tangent-normal` | Circle tangent & radial normal | Compare true shared-contact tangency with circle-centre-on-line radial normal incidence. |

`alpha-parity-catalog` uses `AlphaScenarioKind::Corpus` through the public sketch scenario
constructor. It adds no workbench-owned fixture equation or expected-coordinate shortcut. The
ordinary workbench action surface remains usable outside scenario mode and is the executable owner
of the same 13 relation and five dimension identities.

`alpha-branch-recovery` begins from accepted A3 line-circle tangency plus two separate fixed
parallel lines. **Flip tangent orientation** applies one complete two-contact branch transaction:
the bounded and periodic contacts retain their persistent IDs, the periodic parameter advances by
half a turn and both tangent-orientation fields change together. That explicit candidate may be
retained rejected; it must never be presented as accepted geometry. **Submit impossible contact**
adds an explicitly bounded generic contact between the fixed parallel lines and retains the prior
accepted scene. **Undo rejected contact** performs bounded history recovery until current problem
metadata clears. Direct editor tests additionally prove accepted same-curve semantic-span
End-to-Start migration, bounded-to-supporting-line domain replacement, parameter scalar identity,
periodic winding edits and oriented-angle direction changes.

### M55-AP2 - Contextual constraint intent dispatch

The ordinary workbench and the reusable M55 scenario subtree use the same headless intent
resolver. Direct cases cover:

1. Coincident resolving to point/point coincidence, point-on-curve and curve/curve contact;
2. Equal resolving to line length, circular radius and explicit equal curvature;
3. Parallel resolving to line pairs, and Perpendicular / Normal resolving to either line pairs or
   radial circle/arc centre-on-line incidence;
4. Tangent resolving to generic all-family curve tangency with explicit contact orientation; and
5. Continuity resolving to ordered endpoint G0/G1/G2/parametric-C2 state with positive finite
   explicit rates.

The selector adds reusable demonstrations only after their direct fixture builders pass. Scenario
definitions contain no equation, coordinate-derived branch choice or browser-owned applicability
rule. The public domain-level `CurveDirection` relation remains available for deliberate explicit
contact consumers, but is not exposed as compact line/curve Parallel or Perpendicular because it
does not establish contact and is direction-vacuous on a full circle. The former Point-on-curve,
Equal-length, Equal-radius, Generic-contact and Generic-tangency workbench action identities are
removed rather than retained as aliases.

### M56-C1 - Prepared worker ordering and cancellation

The direct M56 corpus uses the accepted A2 document as one immutable worker input. The prepared
stamp contains the retained design identity, latest attempt, accepted state and accepted revision
high-water plus solve request/policy, effective activation, parameter and external-snapshot
revision/digest identities.

Three deterministic schedules qualify the host boundary:

1. move one prepared point edit to a native worker, finish it against scratch state, verify the
   owning session is bitwise/lifecycle unchanged, then publish its patch through exact-input
   compare-and-swap;
2. execute two different edits prepared from the same base, commit the first, then prove the
   out-of-order second patch returns `StalePreparedPatch` without changing the winner; and
3. cancel a prepared parameter-batch job before its first controlled boundary and prove it yields
   no patch, consumes no parameter/lifecycle revision and cannot enter the commit API.

A fourth input-stamp case adds a non-default unreferenced external point declaration/snapshot and
advances both parameter and external revisions through prepared jobs. It proves every stamp domain
changes explicitly and a same-design reattempt still invalidates older work through its latest
attempt identity. Native jobs/patches move as single-owner `Send` values. Immutable stamp,
operation and commit DTOs are `Send + Sync`; session-bearing values are intentionally not promised
`Sync` because core caches use safe single-owner interior mutability. The all-feature WASM build
uses the same prepare/execute/commit API synchronously and adds no browser scheduling semantics.

### M57-C1 - Dependency-local retained scale

The direct M57 corpus builds two disconnected constrained rectangles and performs five exact-input
updates against the retained document lifecycle:

1. a local point edit dirties its persistent dependent-source closure, retains runtime identity,
   reuses the other component and matches a fresh rebuild on accepted document and rank;
2. a host parameter changes one driving dimension and an immutable external point snapshot moves
   one referenced point; each replaces only its runtime source and reuses the unrelated component;
3. a newer empty activation payload preserves equation shape and reuses every component while
   still publishing a freshly validated accepted revision; and
4. a created point changes topology and therefore reports an explicit full rebuild rather than
   pretending to be incremental; and
5. rebinding one persistent contact across adjacent polyline spans keeps semantic IDs but changes
   residual incidence, so compatibility is rejected and the accepted edit takes the same explicit
   full-rebuild path.

Every optimized return reports `IncrementalUpdate`, fresh hard-row validation and valid numerical
rank. The fresh-build comparisons include accepted geometry and explicit branch-bearing document
state. A 16-component/64-point workload retains at least 15 clean components after one local edit,
stays inside canonical document storage limits and passes the bounded production rank assessment.
The rank assessment is intentionally `BoundedDenseSvd`: sparse hard steps do not claim sparse rank
certification, and supported connected components have at most 256 active rows and tangent
coordinates. A revision-local visual-profile cache returns identical bounded analysis for repeated
options and starts empty after the next accepted state. Deterministic component-work exhaustion
publishes no parameter, lifecycle, geometry or cache state.

### M58-C1 - Deterministic sketch operations and visible topology

The direct M58 corpus starts from immutable complete input stamps and exercises the closed
operations request surface without a browser or operation-owned equation:

1. split one line support at an exact parameter, break a subinterval and trim one side while
   retaining the immutable curve definition and publishing ordered visible intervals;
2. extend a selected line endpoint to a non-parallel accepted line and retain the source curve
   identity; an intersection on the wrong side returns typed incomplete evidence;
3. mirror and linearly pattern exact point-defined families into ordinary public geometry, while
   a circle mirror returns typed unsupported rather than a sampled approximation;
4. chamfer two line spans sharing one persistent endpoint using ordinary point-on-curve contacts,
   driving point-distance dimensions and contact-owned trim boundaries; deleting one owner freezes
   its boundary at the accepted parameter before removing owned contact state;
5. wrap the existing generic fillet transaction and expand rectangle, regular polygon and slot
   macros without a private residual or solver path; and
6. reject stale application, pre-cancelled/exhausted work, accepted geometry from an older design,
   non-finite values, excessive polygon/pattern counts and malformed visible intervals without
   changing retained lifecycle or accepted geometry.

Two preparations from the same stamped input and request publish the same identity disposition.
An exact shared split boundary closes the original rectangle profile by semantic parameter bits,
not coordinate proximity. Canonical v4 export/import rejects multi-interval and ordinary
constraint-contact topology, while the hidden draft-v5 bridge remains explicitly unsupported
pending a future schema-freeze decision. The
companion depends directly only on `geosolve-sketch` and `geosolve-geometry`; it has no direct
core, linkage, production-topology or UI dependency.

### M59-C1 - Complete production topology and fail-closed provenance

The direct M59 corpus captures immutable complete retained-input stamps only when the accepted
geometry belongs to that exact design and host input. It exercises the separate read-only
production-topology companion without a UI, B-rep owner or operation-owned equation:

1. one exact square publishes a complete counterclockwise wire and bounded region; concentric
   circles publish deterministic outer/hole nesting with certified signed area;
2. profile-only and profile-plus-construction queries publish their declared native scope, while
   external line inclusion publishes binding, source revision, digest and parameter-domain
   provenance and explicitly lists ignored external point entries;
3. open supports, overlaps, tangent contours, T-junctions and rejected self-intersections publish
   typed incomplete evidence and no consumable wire;
4. deterministic wire limits return `Truncated`, repeated identical snapshot/request queries are
   value-identical, and cancellation/work exhaustion remains a separate outer outcome;
5. a complete M58 split edge with two adjacent visible intervals closes the same square through
   exact semantic parameter provenance rather than coordinate welding; and
6. newer design/parameter/activation/external/request/policy input makes captured output stale,
   while cancelled/exhausted queries leave the live session and accepted geometry unchanged.

Visual-profile analysis is only bounded candidate evidence. M59 independently verifies declared
eligible-source coverage, source parameter enclosures, freshly evaluated edge endpoints, wire
closure, certified orientation/area and output counts. Only `Complete` constructs
`TopologyProductionProfile`, and host consumption must pass exact-input `validate_current`.
External line endpoints do not proximity-weld to native endpoints: M43 has no persistent
cross-owner endpoint relationship, so such mixed closure remains `Skipped` until a future host
identity contract exists. The companion depends directly only on `geosolve-sketch` and
`geosolve-geometry` and owns no residual, live solver/session, publication or B-rep state.

### M60-W1 - Advanced workbench and deterministic M61 scenarios

The sole directly tested workbench retains the complete M55 action surface and the ten M53/M55
scenario IDs that existed at the M60 freeze. M60 adds one sibling root group,
`m61-advanced-topology` (**M61 Advanced geometry & topology**), with four stable leaves:

| Stable scenario ID | Scenario title | Direct purpose |
| --- | --- | --- |
| `advanced-all-families` | Advanced all-family gallery | Present accepted analytic, conic, Bezier, B-spline and NURBS geometry with stable diagnostics from public domain APIs. |
| `nurbs-branch-topology` | NURBS branch & knot topology | Apply an explicit periodic next-span/winding transition and geometry-preserving knot insertion through typed public document edits. |
| `associative-companion-operations` | Associative & companion operations | Present an accepted generic fillet/trim, then publish split, exact mirror and bounded linear-pattern proposals through the public operations companion and ordinary retained-session boundary. |
| `production-topology-trust` | Production topology trust | Compare complete consumable output with open-support incompleteness, pre-cancelled query control and deterministic complete recovery. |

Selecting or resetting a leaf reconstructs all fixed scenario coordinators. The selected fixture is
the only one rendered, but deterministic typed evidence covers all four advanced families as well
as the preserved M53/M55 fixtures. Scenario state does not enter workspace persistence; exit
restores the unchanged ordinary coordinator.

The production-topology inspector captures only a current independently accepted complete input.
It exposes wire/region/hole counts and accepted revision only when a
`TopologyProductionProfile` exists. Skipped, truncated, cancelled, exhausted, unavailable and
query-error outcomes are explicit and never carry a consumable-profile marker. Adding one open
eligible line produces `UncoveredEligibleSource` and no profile; a pre-cancelled query changes no
input or accepted identity; recovery reconstructs the exact complete fixture.

The application workspace envelope is version 2. Each retained design and optional accepted
payload declares `canonical_v4` or `draft_v5`; version 1 migrates as canonical v4. Direct tests
round-trip canonical documents, M58 multi-interval draft-v5 state and lifecycle high-water
metadata, and reject malformed, unknown-version, unknown-field or unknown-encoding input.
Checkpoint encoding is reported by the headless coordinator rather than inferred by the browser.

These scenarios own no equation, curve evaluator, applicability rule, branch heuristic, solver
publication or B-rep state. The right-expanding selector, guide, transcript and topology card are
presentation only. Native/WASM direct tests are the objective qualification path.

### M61-R1 - Replacement interactive candidate

The first M61 candidate was withdrawn after human review found fixed-only UAT scenes, missing
representative mechanisms, clipped third-level navigation, missing advanced authoring and no
canvas camera. The replacement retains the four M60 leaves and nests them under **Advanced curves
& topology**. A sibling **Interactive mechanisms** group has **Compact mechanisms** and **Linkage
mechanisms** grandchildren:

| Stable scenario ID | Public alpha fixture | Initial mobility | Preselected driver |
| --- | --- | --- | --- |
| `drafting-compass` | `StressCompass` | equality/bounded `1/1` | `first_tip` |
| `bezier-c1-bridge` | `StressBridge` | equality/bounded `3/1` | `left_seam` |
| `twin-roller-bezier-cam` | `MotionCam` | equality/bounded `2/2` | `left_center` |
| `tangent-orbit` | `MotionOrbit` | equality/bounded `1/1` | `moving_center` |
| `elliptic-trammel` | `MotionTrammel` | equality/bounded `1/1` | `horizontal_slider` |
| `scotch-yoke` | `MotionScotchYoke` | equality/bounded `1/1` | `crank_pin` |
| `rotating-square` | `MotionRotatingSquare` | equality/bounded `1/1` | `corners[1]` |
| `scissor-jack` | `MotionScissor` | equality/bounded `1/1` | `slider` |
| `five-stage-scissor-tower` | `MotionScissorTower` | equality/bounded `1/1` | `right_levels[0]` |
| `peaucellier-linkage` | `MotionPeaucellier` | equality/bounded `1/1` | `input` |

This table records the M61 replacement candidate as reviewed at that milestone. M64 subsequently
removed preselected-driver, reset/exit and ephemeral read-only scenario behavior: current samples
are ordinary editable workspace documents. M65 also removes the twin-roller active/passive
metadata and second Temporary stability target. Current projected motion is sample-agnostic and
uses the accepted-nullspace locality contract documented in M65-S1 below.

Ordinary mode now exposes reusable headless construction tools for quadratic/cubic Beziers,
ellipse, directed elliptical arc, rational quadratic conic, trimmed parabola, chosen-branch
trimmed hyperbola and clamped/periodic NURBS. Conic values and NURBS form/degree/weights/gauge are
explicit editor state. NURBS weights are positive, the named gauge weight is exactly one, control
count exceeds degree and explicit weights match controls. Invalid terminal construction is atomic.
Advanced previews localize the proposal into a temporary public document and sample public visible
intervals/curve jets; neither editor nor web code owns a curve equation.

The web-only camera supplies the editor viewport. Wheel zoom preserves its cursor model point,
middle-drag changes only model center, `+`/`-` use the canvas center and Fit bounds all scene
points/curves including the tall scissor tower. Camera state is neither sketch nor scenario state.
Every desktop flyout keeps visible overflow, so the new third-level compact/linkage menus expand
to the right rather than clipping inside their parent.

### UAT-C3 - Advanced geometry and topology at M61

The four M60-W1 leaves plus the ten M61-R1 mechanism leaves are the replacement entry point. They
cover movable nonzero-DOF solver behavior, advanced authoring and camera inspection as well as
all-family accepted geometry,
periodic NURBS span/winding and knot topology, fillet/trim plus split/mirror/pattern operations,
complete production regions, intentionally open/incomplete topology, cancellation and fresh
recovery. The 60-90 minute replacement review judges local predictability, branch clarity,
coherent associated motion, topology trust and perceived desktop responsiveness.
`docs/M61_UAT.md` owns the scorecard; objective facts remain directly qualified and the
supervising human approved M61 for its recorded scope on 2026-07-29.

### M62-F001 - Accepted acute line-angle authoring

Two direct headless fixtures freeze the M62 UAT correction without adding a scenario-menu leaf.
The first retains line seed coordinates at 0.5 radians while an accepted vertical constraint
places the visible line at 90 degrees. Adding an angle dimension must measure the accepted
90-degree state, publish without moving either accepted endpoint and expose 90 acute degrees.

The second draws two 45-degree supporting lines with the second line's stored endpoints reversed.
Its persisted counter-clockwise target is therefore on the 225-degree directed branch, while
headless presentation and the canvas both report the unambiguous 45-degree acute intersection
angle. Editing the presented target to 60 degrees maps to 240 directed degrees on the same branch,
publishes a visible 60-degree acute angle and does not rewrite endpoint order, orientation or
persistence. Equivalent stored targets in all four directed quadrants present the same acute
angle. Inputs above 90 degrees reject before mutation. Retained-rejected creation or editing keeps
the prior accepted canvas and is labelled as rejected rather than accepted.

### M62-F002 - Single-owner authoring input

One direct workbench fixture reproduces the browser event sequence without adding a scenario-menu
leaf. Every physical canvas click generates a parameter-bearing pointer-down followed by a
bubbled generic click. Only pointer-down owns the canvas authoring operand; the click is ignored
for authoring on that surface. Tree items have no canvas pointer-down and retain their single click
owner.

The fixture enters Horizontal and sends both events for one line: exactly one application is
produced and terminal processing re-arms the tool. It then enters Normal/Perpendicular and sends
both events for each of two distinct lines: the first physical click produces one pending operand,
the second produces one complete application, and terminal processing returns to an empty pending
set. Terminal coordinator refusal follows the same re-arm rule so a failed complete candidate
cannot wedge the collector at full arity.

### M62-F003 - Relation-scoped authoring metadata

One direct coordinator fixture starts with two skew free lines. It builds Horizontal and
line-line Normal/Perpendicular applications through the public authoring state, applies them
through the retained coordinator, requires accepted publication and inspects the resulting
ordinary `Horizontal` and `Perpendicular` persistent definitions.

The regression owns the boundary that failed: picked curves are not automatically contact
operands. Contact domain, parameter, neighborhood, winding and tangent orientation are generated
only for point-on-curve, curve contact/tangency, equal curvature, endpoint continuity and radial
circle/arc Normal. Simple Horizontal, Vertical, Parallel, line-line Perpendicular, Equal Length
and Equal Radius definitions carry no contact choices and therefore cannot be rejected for hidden
branch input.

### M62-F004 - Closed constraint-authoring path audit

Two direct headless matrices freeze the complete M62 relation boundary without adding a
scenario-menu leaf. The request-level matrix enumerates every one of the sixteen resolved
constraint families and asserts exactly which contact, tangent-orientation and relation metadata
each family owns. It also picks the same semantic span at parameters `0.2` and `0.8` and requires
the two generated contacts to retain those values in occurrence order; identity-based recovery of
the first parameter for both operands fails the fixture.

The integration matrix starts from public `AuthoringState` activation for each family, checks the
resolved kind, lowers through `RetainedEditorCoordinator::apply_authoring`, requires accepted
publication and verifies that the ordinary persistent constraint exists. Endpoint continuity
uses an End pick followed by a Start pick, so swapping the parameter-compatible neighborhoods is
also detected. A third matrix drives all five dimension families through the same public authoring
adapter to accepted persistent dimensions; the dimension path measures accepted state and does
not manufacture contacts.

### M62-F005 - Pre-closure headless authoring matrix

The closed relation and dimension matrices now construct every application twice: once from a
compatible immutable preselection and once by entering repeated mode and supplying each operand
in sequence. The applications must be identical, intermediate prefixes must remain Collecting,
and a terminal attempt must clear pending operands while leaving the tool active.

Focused fixtures additionally require accepted point-on-curve persistence for line, circle,
quadratic Bezier and NURBS picks; exact Start/End parameters and neighborhoods for both continuity
orders; retained-rejected curve contact followed by Undo and a valid retry in the same active
Coincident tool; dimension-target Undo/Redo; and process-local option retention across tool
re-entry. The bounded line-endpoint recovery is the regression for the final defect found during
this pass: endpoint parameters may never be emitted with an Interior default neighborhood.

### UAT evidence and recheck policy

The M53-M63 guided checkpoint catalog is historical approval evidence. M64 removes its runtime
guide, action, transcript, evidence, reset/exit and alternate-workspace behavior. Current manual
review uses ordinary editable samples as described below. The earlier scorecards and stable IDs
remain documented only as records of the approved revisions; they are not current selector keys.
Findings capture the candidate revision, selected sample, workspace input and accepted/attempted
diagnostics from public APIs; a
human may attach an OS screenshot for a visual finding. Objective defects receive direct
owning-layer regressions. A targeted human recheck is preferred; a full checkpoint repeats
only after a material API, schema or primary-workflow change. Completed M40.7, M53, M61, M62 and M63
required explicit supervising-human sign-off; future milestones require the same explicit closure.

### M63-C1 - Geometry-anchored constraint annotations

`EditorScene` now projects every active persistent constraint and dimension into typed finite
screen-space presentation. Each annotation retains its constraint/dimension ID, semantic kind,
direct point/curve operands, visibility policy and hit geometry. Constraints resolve point,
curve-midpoint and evaluated-contact anchors through accepted public document data. Dimensions
publish linear, radial, label or angular geometry. The browser renders these DTOs and owns no
constraint-definition interpretation.

All angles and all driving dimensions are visible at rest. Non-angle reference dimensions and
constraint symbols appear only through direct operand hover/selection, annotation selection or a
targeted current problem. Selecting a symbol emphasizes its direct operands without adding them
to the editable selection. Shared glyph anchors fan out deterministically with compact leaders.
Select-mode pointer hits prefer visible annotations; authoring remains geometry-only.

### M63-U1 - Focused canvas-constraint UAT leaves

One new root group, `m63-canvas-constraints` (**M63 Canvas constraints**), owns three stable leaves:

| Stable scenario ID | Scenario title | Review purpose |
| --- | --- | --- |
| `canvas-angle-dimensions` | Canvas angle & dimension presentation | Always-visible angle arcs and values, driving dimensions, contextual reference dimensions and target editing. |
| `canvas-relation-glyphs` | Contextual constraint symbols | Direct-relation hover discovery, persistent symbol selection, operand emphasis, stable radial placement and authoring isolation. |
| `canvas-crowded-annotations` | Crowded relation fan-out | Dense rotating-square relations, deterministic offsets/leaders, zoom/pan and independent selection. |

The leaves reuse public deterministic fixtures and change neither canonical workspace persistence
nor solver behavior. `docs/M63_UAT.md` owns the approved human scorecard.

`M63-F001` adds an explicit radius-stability step to `canvas-relation-glyphs`: moving the tangent
line must not move the radius leader between mathematically equivalent circumference samples.
Headless presentation uses canonical public curve parameters rather than adaptive tessellation to
own that branch.

`M63-F002` strengthens `canvas-crowded-annotations`: deterministic fan-out is collision checked
against every final glyph center with a 22 px minimum separation, rather than merely assigning
nominal ring slots. The rotating-square headless regression checks every glyph pair and leader
exercise.

`M63-F003` initially made only visible leaders contextual hover corridors; human retest found that
insufficient for natural paths beginning elsewhere on related geometry. `M63-F004` retains the
last direct geometry-hover position and constructs bounded corridors from there to every directly
related annotation, choosing the nearest overlapping corridor deterministically and clearing on
unrelated blank canvas. The regression begins outside geometry and leader hit tolerances.

Human retest then found `M63-F004` insufficient because it still transferred the persistent
constraint into the same state slot as the geometry reveal owner. `M63-F005` separates the
geometry context owner, bounded transit and exact annotation occurrence. Corridors and inter-icon
links keep the complete directly related set visible without claiming icon hover; only a marker
within icon proximity publishes its deterministic marker index, and clicking any occurrence still
selects its one persistent constraint. The focused UAT steps now require passing one icon on the
way to another without hiding siblings or highlighting multiple occurrences.

`M63-F006` audits the complete relevant icon surface. The eleven authoring intents, five
dimension actions and nineteen accepted canvas constraint glyphs now come from one text-free
vector catalog. Shared concepts deliberately reuse the same shape across palette and canvas;
specialized persistent relations retain distinct geometry-representative symbols. The
`canvas-relation-glyphs` instructions explicitly ask the reviewer to compare that shared language
and distinguish contact, direction, normal and curvature variants.

`M63-F009` completes the adjacent non-scenario icon audit without changing any fixture. The
fifteen geometry authoring tools receive distinct text-free CAD vector symbols, sketch-tree rows
distinguish their object category, and targeted/global problem markers use vector alert geometry.
Ordinary labelled actions, Enter/Esc hints, sample disclosures and camera controls retain their
existing text because they are not placeholder concept icons.

### M64-S1 - Editable purpose-based sample library

The top **Samples** selector has exactly one group level:

| Purpose group | Editable samples |
| --- | --- |
| Mechanisms | Drafting compass · 1 DOF; Bezier continuity bridge · 1 DOF; Twin-roller cam · 2 DOF; Tangent orbit · 1 DOF; Elliptic trammel · 1 DOF; Scotch yoke · 1 DOF; Rotating constraint square · 1 DOF; Scissor jack · 1 DOF; Five-stage scissor tower · 1 DOF; Peaucellier inversor · 1 DOF; Four-bar coupler · 1 DOF; Pantograph linkage · 2 DOF; Three-link drawing arm · 3 DOF |
| Constraints & dimensions | Constraint and dimension sampler; Tangent and radial-normal construction; Contact branch specimen; Angle and dimension annotations; Contextual constraint annotations; Dense constraint junction |
| Curves & constructions | Construction and reference geometry; Curve family gallery; Periodic NURBS specimen |

Opening a leaf constructs a fresh public document/session/coordinator and replaces the current
ordinary workspace. It starts one-checkpoint history, fits the web-only camera and immediately
uses normal autosave. Reopening the same leaf reconstructs its pristine starting document.
Samples own no guide, scripted action, verification point, preselection, driver identity, read-only
flag or exit/reset lifecycle. Delete, Undo/Redo, authoring, branch and dimension editing,
selection, zoom/pan and projected drag are the same actions used for a blank or restored workspace.

Four-bar uses fixed grounds `(0,0)` and `(8,0)`, crank/coupler/rocker lengths `5`, `4` and
`sqrt(17)`, plus a coupler midpoint tracer, leaving one bidirectional freedom. Pantograph uses a
fixed origin, two independently rotating arms of lengths `sqrt(17)` and `sqrt(10)`, two parallel
translated sides and a diagonal midpoint, leaving two freedoms. Three-link drawing arm uses one
fixed origin and link lengths `3`, `sqrt(8)` and `sqrt(5)`, leaving three freedoms. Each has
scale-invariant persistent roles and is directly checked at `1e-6`, `1` and `1e6`.

### M65-S1 - Predictable bounded mechanism dragging

Projected drag remains sample-agnostic. At gesture start, the headless sketch layer derives an
opaque locality plan from the independently accepted hard nullspace. The selected point's rank
covers the motion it can control; deterministic point anchors cover only remaining passive
mobility. Anchor targets are the accepted visible positions at gesture start. The selected cursor
is the only Temporary target, planned anchors are the only PreviousState Preferences, and neither
sample keys nor presentation code choose a passive point.

Each non-stale pointer sample performs exactly one retained attempt from the complete last
independently accepted preview. A rejected or exhausted sample retains that preview bit-for-bit;
a later valid sample can recover in the same gesture. Stale and out-of-order request IDs do
nothing. Circle circumference picking resolves to that circle's own center and retains the
gesture-start pointer offset. At a true overlap, directly draggable point/semantic-curve geometry
wins over an annotation leader; offset annotation labels remain selectable.

The direct path corpus is table-driven:

| Existing editable sample | Required paths and assertions |
| --- | --- |
| Scotch yoke | Delete the horizontal guide, then exercise horizontal, vertical, diagonal and reversal paths through the two-DOF point without an unrelated valid-root jump. |
| Scissor jack and five-stage tower | Exercise opening/closing reversals; accepted previews stay locally continuous and all work remains inside the projected-sample envelope. |
| Pantograph | Drive input, guide, output and center independently, including natural off-manifold guide targets. A point whose active rank covers all hard mobility needs no anchor; otherwise deterministic anchors cover only passive mobility. Rank-one `2 x 2` cursor projection must be stationarity- and minimum-norm-certified under the authoritative rank cutoff. |
| Twin-roller cam | Drive both rollers separately from their centers and circumferences, including the left driving-radius overlap, then exercise horizontal, vertical, diagonal and reversal paths. The passive center moves by at most `1e-8`; a difficult rejected target retains the full last preview and a later valid target recovers in the same gesture. |
| Circle handle offset | Press away from a circumference's center and verify that the semantic center moves without snapping the cursor to it. |

Lifecycle coverage releases an accepted preview as one independently validated history edit,
cancels without mutation, exercises Undo/Redo, and delivers late/stale queued results after
release or a newer request. Ordinary constraint authoring and workspace save/reload remain
unchanged. One integrated regression authors through the headless adapter, round-trips the real
workspace envelope, restores the same persistent constraint and proves it remains editable.

Every sample uses the same synchronous operation limits: `16,384` each validation, dependency and
lowering items; `256` each nonlinear iterations, factorizations and rank kernels; `512` rejected
trials; `1,024` component linearizations; `256 × 256` dense kernels; `512` diagnostic candidates;
and `1,024` diagnostic trials. Exhaustion is a typed rejection, never partial publication.

Replacement `b6433d1` directly remediates `M65-F004` (twin-roller annotation hit priority) and
`M65-F005` (orientation-sensitive rank-one pantograph-guide projection). Native, WASM and release
qualification plus focused U2/U3 human approval pass; `docs/M65_UAT.md` records closure.

### M66-CF1 - Two adjacent corners in one FilletSet

Create a four-point open polyline with three native spans. Select both interior corners and Apply
once with one shared radius. Evaluation produces exactly two arcs; the middle source interval is
bounded independently at its Start and End, and all four source points remain ordinary draggable
sketch points. Reverse corner selection produces the same canonical intent and visible output.

Changing the shared radius changes only feature revision and computed output. The canonical sketch
document, accepted identity/coordinates, residual vectors, numerical rank and DOF remain exactly
unchanged. The ordinary workbench transaction contains no M28 association, trim view, radius
scalar, radius dimension or constraint.

### M66-CF2 - Sequential adjacent sets compose

On the same four-point/three-span source, Apply one Fillet to the first corner and a second
FilletSet to the other corner. Opposite endpoint claims on the shared middle span compose. At an
equal radius and branch choice, sequential visible geometry matches M66-CF1 while the two sets keep
separate identities and radii.

Deleting or suppressing either set leaves the other current. Deleting one generated arc from a
multi-corner set removes only that corner; deleting its final corner removes the set. Undo/Redo
restores the same set/corner IDs and allocator high-water.

### M66-CF3 - Atomic endpoint-claim conflict and recovery

Choose a radius that makes adjacent claims cross or consume the shared source interval, and also
exercise duplicate endpoint ownership from distinct sets. Every participating set publishes a
typed attributed failure and no output; an unrelated valid set remains current. Reducing the
radius or deleting/suppressing one conflicting set recovers output without changing surviving
intent IDs. No stale arc remains visible during failure.

### M66-CF4 - Source edits, missing sources and truthful failure

After authoring a valid multi-corner set, drag every native source point independently through
valid and invalid configurations. A valid sketch edit always commits. If feature construction
becomes singular, unsupported or outside its explicit branch/domain, the set's computed output is
withheld while the accepted sketch stays editable. Issues identify the feature, corner and source
where safe; only unattributable failures are global.

Delete one referenced source span. Feature intent remains as a repairable missing-source failure
with no ghost geometry. Undo restores the source and regenerates current output under the same
stable feature/corner IDs and fresh evaluation-local edge IDs.

### M66-CF5 - Shared-radius authoring and generated-arc interaction

Preselect several interior polyline points and confirm they remain grouped corner targets rather
than flattening into `2N` curve operands. Repeat using accumulated corner/curve-pair clicks and
reverse pick order. Preview begins from remembered radius or `0.1 * model_scale`. Numeric editing
and a preview arc/radius grip change the one shared radius. Apply/Enter commits without a final
canvas radius-confirmation click.

Selecting a generated arc resolves stable set/corner provenance. Dragging the arc or grip changes
only the set radius, never sketch coordinates. Computed arcs are unavailable as constraint
operands. The **Features** tree, canvas selection and Problems presentation resolve the same stable
intent identities. Known limitation `M66-KL001` qualifies the interaction feel of that drag, not
its transaction boundary or mathematical validation.

### M66-CF6 - Persistence, exact CAS and revision-local output

Round-trip application workspace v4 with multiple sets, labels, suppression, stable IDs, branch
intent and allocator high-water. Reload regenerates fresh output IDs while stable provenance and
visible geometry remain equivalent. Workspace v1-v3 migration creates an empty feature document
bound to the restored sketch and never reinterprets an existing M28 Fillet.

Undo/Redo, cancellation, deterministic work exhaustion, stale sketch identity, stale feature
revision/digest and stale evaluator policy are independently exercised. None publishes stale
output or reuses an allocated feature/corner ID. A feature-domain fixture emits zero, one and
multiple output fragments to prove the result container is not fixed to one arc.

### M66-CF7 - Compatibility and profile boundary

Existing M27/M28/M30 solver-owned Fillets and M58
`SketchOperationRequest::AssociativeFillet` remain readable, editable and directly tested. The
ordinary Fillet action creates a computed `FilletSet`; no automatic conversion occurs in either
direction.

With an active computed Fillet whose result is not represented in base sketch profiles, the
workbench withholds misleading base-only profile/fill presentation and reports typed “computed
geometry not yet included” status. M66 publishes no computed output to visual or production
topology consumers.

### M66-CF8 - Scoped family support and future variable topology

Affine/affine and affine/non-affine corners evaluate with explicit retained-side, neighborhood,
winding, normal-side, endpoint-order and sweep state. Two non-affine sources return a typed
unsupported feature issue without mutating the sketch or narrowing the underlying M28 API.

The snapshot/provenance container demonstrates variable output cardinality and exact source-
interval provenance for future topology-changing features. No Offset definition, implementation,
workbench action, placeholder, sample or UAT claim exists in M66.

### M66-CF9 - PF003 editable Fillet playground checkpoint

This M66-only leaf extends rather than rewrites the frozen M64 22-leaf record; the current catalog
therefore has 23 stable leaves. Open **Samples → Curves & constructions → 2D Fillet
playground**. The ordinary editable leaf contains an upper-left independent-line pair,
upper-right three-line high-valence junction,
line-circle and line-quadratic-Bezier pairs, an unlocked long-middle polyline for batch/sequential
Fillets and an unlocked short-middle polyline for claim-conflict recovery. For predictable manual
line-line UAT, click each line's interior away from their exact intersection; use the junction
itself only to verify typed ambiguity, then choose two branch interiors explicitly.

The leaf has no guide, special coordinator or protected geometry beyond its fixed reference
islands. SVG canvas gestures suppress native browser text-selection and element-drag defaults,
while the sibling Fillet options and other HTML remain selectable/editable. Native
screen/coordinator and focused presentation tests qualify this checkpoint; it is not browser E2E
evidence. `M66-PF003` is mechanically closed by direct regressions on `02649cc`; no separate human
retest is claimed by the scoped M66 close decision.

### M66-CF10 - PF004 preview-arc/native-support overlap

In active Fillet authoring, collect a valid two-support corner and press the generated preview arc
near either contact, where the arc's painted hit stroke and a native parent are both inside their
respective tolerances. The painted `FeatureCorner` owns a radius gesture only after the coordinator
matches the exact held candidate, accepted/computed scene provenance and an independent headless
hit on that corner's generated curve. The native parent is not collected as a new pending support,
the grouped candidate and preview remain unchanged, and move/release edit the shared radius.

The direct regression also supplies a foreign corner owner, a second pointer during the live
gesture and a Shift-modified press. Foreign/stale intent and the second press reject without
mutating authoring, preview, selection or durable identities; the original gesture still
moves/releases. Modifiers cannot toggle the explicit radius owner away, while ordinary selection
modifier behavior remains unchanged. `M66-PF004` is mechanically closed by direct regressions on
`ac31791`; the historical Tailscale candidate was HTTP-verified, but no separate human retest is
claimed by the scoped M66 close decision.

### M66-KL001 - Radius-drag and branch-choice interaction

Radius drag currently measures pointer distance from the held/old arc center while evaluated
center and contacts move, so tracking can drift or feel inverted. Post-placement contact/root,
retained-parent direction and alternate-arc choices lack intuitive controls, especially for
line-circle Fillets. Numeric radius editing, explicit persisted branch state, independent
validation, rollback and sketch-state invariance remain correct. The playground line-circle
specimen starts at radius `0.5`, near a branch fold.

At M66 close the potential follow-up was deliberately unassigned. Completed M68 now owns a
headless one-dimensional radius rail, frozen absolute branch intent, typed contact metadata and its
internal continuation seam, retention/continuation actions, bounded local-alternative previews and
a friendlier specimen while retaining the fold as a regression fixture. None of it was assigned to
M67; the M68 gate and focused UAT now pass. The supervising human accepted this limitation when
explicitly closing M66's
mechanically qualified computed-Fillet scope on 2026-08-08; that close does not claim a complete
post-PF004 replay of every scripted UAT step.

### M68-DM1 - Absolute same-branch continuation and radius rail

For orthogonal, acute and reversed line-line corners, every supported line-circle root/retained
direction and a line-quadratic-Bezier corner, begin from one accepted absolute
`NewComputedFilletCorner`. Continue radius forward and backward under translated, rotated and
scaled inputs while preserving normal sides, retained endpoints, contact neighbourhoods/windings,
output endpoint order, sweep and local root.

At each regular sample, derive centre sensitivity from the differentiated offset intersection and
independently reconstruct it from both parents. Compare it to a central finite difference taken
over the same absolute branch. Non-finite, singular, ill-conditioned or disagreeing rails reject;
they never become a zero/finite success. At the headless editor boundary, projecting pointer
displacement perpendicular to a frozen rail changes no radius, while radial displacement is
invariant to coarse/fine event sampling and viewport scale.

Status: mechanically implemented and directly qualified in `geosolve-sketch-features` and
`geosolve-constraint-editor`; clean release qualification and explicit human UAT are complete.

### M68-DM2 - Fold stop and bounded explicit alternatives

Use the retained radius-`0.5` line-circle configuration to approach its local fold from both
directions. Same-branch continuation publishes current samples up to the limit, retains the final
current sample beyond it and reports a typed fold/domain/regularity reason. It never silently
selects another root.

Enumerate alternatives only for the two persisted native parents and their local neighbourhoods.
Explicit source-contact, retained-direction and complementary/local-arc actions preview a complete
absolute replacement corner; a tied choice reports ambiguity. Hover/focus changes no durable
intent, and click/activation commits only the named preview. No test asks for global root
enumeration.

Status: mechanically implemented, directly feature/editor-qualified and accepted through the
approved M68 UAT; clean release qualification is complete.

### M68-DM3 - Current-only interaction transition model

Drive the closed headless Fillet interaction through idle, radius drag, explicit named-contact
transactions and branch preview. Exhaust pointer down/move/up/cancel orderings, radial/tangential
motion, coarse/fine sampling, several viewport scales, invalid-to-valid recovery, release while
invalid, modifiers, a second pointer, stale/foreign owners, work exhaustion and camera
cancellation.

The model records exact origin configuration and stamps plus only the last exact `Current` preview
token/sample. A commit is legal iff that evidence still matches. Rejected samples may not replace
the solid last-current preview; an invalid release or cancellation may not mutate durable intent,
the persistent feature allocator high-water or history. Revision-local computed-evaluation IDs
remain never-reused even for discarded previews. Authoring preview, published dragging and direct
numeric editing obey the same transition oracle.

Status: mechanically implemented and directly qualified. The bounded reference model enumerates
28 reachable states and all 240 applicable transitions, including invalid samples, delayed or
duplicate acknowledgements, foreign pointers, same-position retry and terminal-coordinate
validation. Clean release qualification and explicit human UAT are complete.

### M68-DM4 - Shared action resolution and crowded priority

At positions where a generated arc/grip and native support overlap, resolve both hover and click
through the same headless action resolver. While the Fillet is selected or being authored,
priority is explicit radius grip/generated arc, then native support. Named contacts remain typed
headless metadata, not endpoint canvas handles or compact-panel controls. Painted `FeatureCorner`
metadata is only a hint: exact owner, accepted/computed
provenance and model-space proximity must also match, preserving `M66-PF004`.

Canvas actions and the compact accessible panel expose identical stable IDs, labels,
applicability, disabled reasons, attribution and affected corners. A shared-radius action visibly
identifies every affected arc. Hovering a ghost alternative never commits it.

An isolated corner solution is only a candidate. Replace that corner in the complete cloned
feature document and advertise the action only if the owning feature remains `Current` after
source composition. In the three-segment/two-Fillet specimen, reversing the retained direction
of the shared middle segment would duplicate an endpoint claim; that control is absent rather
than presented as an arrow that cannot commit. Valid outer-segment actions remain present.

Status: mechanically implemented and directly editor/presentation-qualified; clean release
qualification and explicit human UAT are complete.

The `M68-F002` hotfix narrows the visible canvas affordance to one central radius handle per
selected corner. Direct editor and web tests prove Fillet endpoints have neither rendered contact
circles nor invisible contact-drag hit zones. Branch choices retain their lightweight icons and
arrows without circular handle-like backplates; the generated arc remains the visible radius
surface.

Painted arrow identity remains only a current-stamped hint. Where transparent action corridors
overlap, the adapter submits every painted action under the pointer and the headless resolver
selects the unique nearest applicable control from independently projected model-space geometry.
A visible validated arrow outranks an overlapping Fillet radius surface, while the central radius
grip retains priority where it visibly covers the arrow. Retained-direction arrows have no
adjacent duplicate glyph; only the exact headless preview adds the bright, thick glowing state.
Canvas SVG actions suppress their browser pointer-focus outline, while the separate accessible
panel buttons retain ordinary keyboard focus indication.

### M68-DM5 - Atomic history, persistence and sketch invariance

For one- and multi-corner FilletSets, atomically publish an accepted radius plus any replacement
absolute corner configuration in one feature revision/history step while preserving stable
feature/corner IDs. Exercise Undo/Redo, encode/decode/reload, cancellation and stale exact-CAS
work. Generated edge IDs remain revision-local and no persistence migration is introduced.

Before and after every accepted or rejected feature action, compare native sketch document/
accepted identity, coordinates, residual vectors, numerical rank and DOF bit-for-bit or by their
existing exact public contract. They remain unchanged. `M66-PF001` through `M66-PF004` and
M27/M28/M30/M58 compatibility remain mandatory.

Status: mechanically implemented and directly feature/coordinator/persistence-qualified; clean
release qualification and explicit human UAT are complete.

### M68-DM6 - Friendly and fold specimens with captured pointers

Keep the stable **Samples → Curves & constructions → 2D Fillet playground** leaf as one ordinary
editable save-like scene. Add a friendly line-circle island comfortably away from a fold for
normal contact/retention/alternative exploration and preserve the radius-`0.5` fold configuration
as a separately labelled stress island. Neither has guide text, protected state, a scripted
transcript or a sample-specific coordinator; the catalog remains purpose-owned rather than
milestone-owned.

The workbench captures/releases the initiating pointer for point, Fillet and pan gestures so a
release outside the SVG cannot strand state. A camera change cancels/restores live Fillet
manipulation before navigation, while pan/zoom stay available during collection/inspection.
Any automatically exposed solver/computed-feature problem detail is a bounded, non-intercepting
overlay inside the canvas panel. Entering or leaving invalidity cannot add a workbench grid row,
resize the viewport or change pointer-to-model mapping during the captured gesture.
Thin Rust/WASM presentation tests own event translation, pointer capture, overlay layout,
accessibility and browser-default suppression. No browser E2E is restored. Human interaction feel
is accepted through the approved `docs/M68_UAT.md` Tailscale scorecard.

Status: mechanically implemented and directly presentation-qualified; clean release qualification
is complete, and the distribution was published and byte-verified through Tailscale. Explicit human
UAT is complete.

### M68-DM7 - Affine source edits preserve grouped-Fillet manipulation

Create a four-point/three-span polyline and one shared-radius `FilletSet` over both adjacent
corners. Publish an initial radius change, then move the first and last native source points far
enough that at least one valid contact leaves the narrow neighbourhood of its persisted pre-edit
parameter. Reselect the feature, expose both radius rails and publish a second grouped-radius
change through the ordinary projected pointer transaction.

Both computed corners remain `Current`, both rails are finite and no false continuation/fold
status appears. The radius change creates one history step, preserves feature/corner IDs and
leaves the accepted post-source-edit sketch identity, coordinates/JSON, residuals, rank and DOF
unchanged. Affine/affine evaluation and continuation use the same complete certified cells;
line-curve cases retain bounded seed-local root protection and the true-fold fixtures remain
rail-less.

Status: mechanically implemented and directly qualified by the `M68-F001` feature/editor
regressions on `c82d420`; clean release qualification passes and the resolved finding is accepted
under the explicit M68 close decision.

### M68-DM8 - Closed-loop parents remain complete

Author a regular line-curve Fillet against a full circle or ellipse. The periodic parent still
owns exact contact, tangent, normal, winding, branch-neighbourhood and continuation state, and the
generated Fillet arc remains current. It does not emit a visual replacement fragment: the closed
native loop remains complete and has no meaningless retained-direction action.

Repeat the topology check with a directed circular arc and with a periodic support carrying an
explicitly open visible trim view. These open parents continue to emit source-fragment trim claims
and remain eligible for valid retained-direction actions. The distinction follows visible domain
topology, not a hard-coded circle/ellipse family list.

Status: mechanically implemented and directly feature/editor-qualified by `M68-F004` on
`a1ed6ff`; release Trunk and all seven Tailscale asset checks pass, and the resolved finding is
accepted under the explicit M68 close decision.

### Archived solver-owned M66 scenario record

The prior single-corner, M28-backed ordinary-UI scenarios and findings `M66-F002` through
`M66-F013` are preserved with commit `1034afc` at
`origin/archive/m66-associative-fillet-2026-08-07`. The still-earlier three-tool candidate,
including Offset findings `M66-F001`/`M66-F006`, remains at
`origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`). They remain compatibility and
diagnostic history, not active ADR 0031 qualification scenarios.

## Frozen near-singular fixtures

The regression corpus includes:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures test truthful singularity/rank reporting and finite state retention. They do not demand arbitrary global branch selection. M9 makes the machine-floor numerical rank contract and distinct near-singular warning band mandatory.

The detailed L3 fixture above demonstrates that geometric alignment does not itself justify an M9 warning when the selected driver makes the reported position/velocity matrices full-rank and well-conditioned. The detailed sketch fixture demonstrates actual dependent gradients and therefore does report numerical singularity.
