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
  ordinary hard residuals against a 180-second reference target and a 240-second
  shared-runner release ceiling. All semantic assertions precede the elapsed-time check.

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
are recorded at completed M40.7, M53 and M61-M77. M72's scoped UAT and exact final public-artifact
verification complete its direct automated qualification. Completed M73 qualified its F001-F003
construction-stage, contextual-authoring and candidate-trace consolidation plus F004 live
world-axis span precedence, passed the clean replacement release gate, published a byte-verified
immutable Tailscale UAT snapshot, received focused supervising-human approval and exact-verified
the final GitHub Pages artifact. M73 adds no new editable sample or browser scenario mode. M74 has
explicit scoped closure approval on its clean-qualified, byte-verified F001 replacement. Its
hands-on intrinsic-datum and desktop-polish scorecard is intentionally deferred into the next
bug-fixing/UAT follow-up milestone rather than claimed as completed human evidence. Exact final
M74 Pages publication passes. That follow-up became M75: its initial immutable nomination was
withdrawn after M75-F001, the F001 replacement passed the complete clean gate and immutable
nomination, and M75-F002 then withdrew it after browser paint order hid a valid computed-radius
owner. The F002 correction now passes the complete clean replacement gate, immutable Tailscale
nomination and exact served-byte checks. On 2026-08-16 the supervising caller accepted the exact
post-F002 candidate, focused F001/F002 hover recheck and U1-U12 for scoped closure. The detailed
UAT steps were not individually logged, so this is not a claimed step-by-step replay. Exact Pages
run `31939764951`, artifact `9261974799` and deployment `5929879555` now pass public-byte and
M72/M74/M75 browser verification, completing M75. M76 implementation and its final angle/Origin
feature refinements pass complete clean qualification and immutable byte-verified Tailscale
replacement publication. The caller accepts U1-U4 for scoped closure and explicitly waives a
separate post-refinement replay; that disposition does not invent individual observations. Exact
GitHub Pages publication, the unchanged retained M72 browser verification and M76-adapted retained
M74/M75 browser verification now pass, completing M76. M77 subsequently completes without changing
any completed M76 evidence. M78 is complete under ADR 0036 with an exact nine-family/
25-variant headless geometry authoring catalog. Initial candidate
`1b2ce0f9d843c036e3a7023674cbf219c9f593b7` passed complete clean qualification plus immutable
Tailscale nomination but is withdrawn from current UAT by M78-F011. Replacement source
`793e9de39d78bdabfded15d8c8e79f86df0f52bc` passes complete clean qualification plus immutable,
byte-verified Tailscale nomination. Human UAT and closeout approval pass. Approval descendant
`a6d504e` passes Pages run `32096209036`, artifact `9310104202`, deployment `5955688918` and exact
hosted-byte aggregate `bcf95289a347760a805da392d3064ef1b372b22505f3f150a4236b270b66c51f`
without replacing qualified product source `793e9de`.

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
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`, was served only on
Tailscale at `http://100.94.63.83:8080/` and byte-verified file-by-file plus `/`. It was current UAT
authority at that nomination checkpoint. The old server exited while its historical snapshot
remains. M73 remained open only for focused human UAT and explicit approval then.

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
annotation reuse the Symmetry glyph and paired-point midpoint as the leader origin while placing
the movable mark at its deterministic offset and including the selected datum for related
highlighting. Suppress/reactivate/delete/dependency/Undo/Redo and draft-v5
checkpoint reload follow the ordinary constraint lifecycle; canonical export returns
`UnsupportedM74State`. Existing line-backed symmetry stays green. Focused sketch 9/9, editor
native/WASM 5/5 and nine append-only golden rows own this scenario; the earlier M74 candidate is
not UAT authority after this product change.

### M74-W1 - Reference presentation is polished but equation-free

At `1440x900` and approximately `1024x720`, inspect the dedicated Reference tree group, which
retains Origin/X/Y labels, plus the protected inspector and normal/hover/selected/related styling.
The canvas renders the X/Y axes and their labels; their intersection communicates Origin without a
duplicate marker, label or focus target. Headless Origin picking, authoring and protection remain
unchanged. Native geometry must paint and pick above overlapping datums. References and Grid toggle
independently. The adaptive SVG grid
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
deferred into the subsequently completed M75 bug-fixing/UAT follow-up milestone.

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
publication authority. U1-U8 transferred into M75 and are accepted under its scoped close decision.

### M75-H1 - Select hover predicts the primary pointer-down owner

Status: implemented and clean-qualified natively and under WASM, accepted for scoped closure on the
post-F002 candidate, and exact-verified through the final public artifact.

Construct finite accepted scenes whose projected hit envelopes overlap at every adjacent priority
edge. For the same Select tool, scene, camera, problem set, policy and pointer coordinate, resolve
pointer-move and pointer-down through one headless candidate construction. The primary owner must be
the first applicable entry in this exact order:

1. a current applicable Fillet radius surface or grip;
2. draggable geometry: stored points and published semantic centers;
3. a visible constraint or dimension annotation occurrence;
4. other native or computed geometry;
5. a visible intrinsic datum;
6. no primary target.

Exercise Fillet-over-point, point/center-over-annotation, annotation-over-curve, native/computed
geometry-over-datum and datum-over-empty cases in both candidate insertion orders and at several
viewport scales. Hover returns the existing semantic owner that an unmodified pointer-down would
use. Shift/Ctrl/Command may subsequently change selection membership but cannot introduce a
different primary hit order. Independently verify finite screen/model conversions and prove no
hover sample mutates accepted geometry, selection, history, Fillet state or scene authority.

### M75-H2 - Problem annotations, deterministic occurrences and targetless context

Status: implemented and clean-qualified natively and under WASM, accepted for scoped closure on the
post-F002 candidate, and exact-verified through the final public artifact.

Create one constraint and one dimension occurrence that are normally hidden but forced visible by
the current problem set. Each visible occurrence participates in the same annotation hit path used
by pointer-down; clearing or replacing the owning problem/accepted scene removes its eligibility.
For crowded visible annotations, compare finite screen distance, then stable semantic item identity,
then occurrence identity. Repeat exact ties, reversed construction order, native/WASM replay and
scene reconstruction; the selected occurrence and related operands remain deterministic without a
browser-side tiebreak.

Also sample every geometry/annotation contextual corridor outside all primary hit envelopes. The
headless scene may reveal related annotations or operands, but its primary hover target is `None`
and pointer-down cannot select a corridor-only item. Moving from that corridor onto a visible
occurrence changes the primary target without losing the valid related context. Existing annotation
placement, visibility, fan-out and hit tolerances remain unchanged.

### M75-H3 - Hover authority is revoked with its interaction context

Status: implemented and clean-qualified, accepted for scoped closure on the post-F002 candidate,
and exact-verified through the final public artifact.

Acquire each owner class, then change the active tool or selection, pan/zoom/Fit/Origin camera,
replace the accepted scene through edit/Undo/Redo/reload, hide the relevant visibility class, or
transfer pointer ownership to a tool popout/dialog/overlay. The stale hover target and related
paint clear before the changed state can render. Returning to Select or the canvas does not
resurrect it; a new mapped pointer sample is required. Existing pointer-leave/cancel behavior
remains consistent. While ordinary or Fillet authoring owns uncaptured movement, no unrelated
Select owner is published; its exact compatible next operand is published instead. An already
captured radius gesture continues normally.

At the thin web boundary, submit only normalized pointer/context inputs and render only the current
headless hover/related DTO. DOM/SVG event targets, CSS `:hover`, painted hit strokes and cached
browser candidates cannot choose or preserve a second canvas owner. A painted computed corner is
only an intent hint and must pass current-preview/provenance/proximity validation. Focused
native/WASM tests own resolution semantics; adapter tests own event translation and stale-paint
clearing. Additive public surface is limited to problem-aware Select and domain-aware authoring
pointer movement over existing DTOs; existing lifecycle paths revoke host camera, scene and
input-owner context. Solver, persistence, role ordering, hit tolerances and golden bytes do not
change.

### M75-F001 - Active authoring hover matches its unchanged click

Status: confirmed against the initial frozen candidate; corrected with native/WASM 11/11 parity
and a complete clean replacement gate. The intermediate replacement is withdrawn after M75-F002;
the corrected behavior is accepted and publicly verified on the final candidate.

Activate an ordinary relation/dimension tool and sample an applicable point or curve. Compare the
read-only hover item to the operand consumed by the unchanged click resolver. Repeat with an
inapplicable nearer point overlapping an applicable line: both paths must skip the point and choose
the line. Empty or wholly inapplicable samples clear the hover target. Exercise both collecting and
immediately applying stages.

Repeat for grouped Fillet authoring with an unambiguous native corner point, one native curve,
isolated-point-over-curve fallback and a current computed preview radius. A painted computed corner
must match the complete retained candidate, feature/corner binding, accepted/design/computed scene
stamps, geometry policy and exact headless radius hit. Hover then reports that `FeatureCorner`, and
an unchanged pointer-down begins the same radius gesture. A stale/foreign/spoofed painted corner
fails closed without falling through to native geometry.

Every hover-only sequence preserves authoring state, preview candidate and snapshot, selection,
active gesture, history cursor/length, replay transcript, design/accepted documents and identities,
feature identity and scene geometry. The browser RAF queue keeps the latest painted hint paired
with the latest coalesced pointer coordinate; captured gestures remain editor-owned.

Exact F001 replacement source `57f407ada2eb8a16f8162d1db4126d5c5024f1b4`, tree
`7bff59c5d4d36d1acb687a93d78707b32e323d65`, passes the complete gate with native/WASM M75 11/11,
demo-web 116/116, unchanged 270-row golden evidence and a 143.27-second sparse crossover. Its
read-only snapshot `/tmp/geosolve-m75-f001-uat.2Ju7gq`, aggregate
`9ecf1dde82ca777ae8de6dc380606512008b3bf088808e995fd0c4b2b8896967`, is byte-verified at the
Tailscale endpoint. M75-F002 supersedes it before any human evidence.

### M75-F002 - Computed-radius owner survives browser paint overlap

Status: confirmed against the F001 replacement, corrected, clean-qualified, accepted for scoped
closure on the post-F002 immutable candidate and exact-verified on GitHub Pages. The focused
F001/F002 recheck and final publication pass.

In the ordinary `fillet-workshop`, collect point `6600000000000000000000000000004f` and curve
`66000000000000000000000000000038`, then sample the computed-radius grip where a native point
paints above the correct `FeatureCorner`. The headless scene resolves the correct
`SceneFilletHit::Radius`, but the former top-target-only adapter supplied native curve
`66000000000000000000000000000052`; an unchanged press destroyed the valid preview and captured no
radius gesture.

The same adapter review must sample the visible radius rail and spoke. Both are pointer-active
headless radius surfaces and therefore expose the same existing `FeatureCorner` identity as the
grip; an identity-free rail/spoke target is the same F002 defect, not a new semantic owner.

For uncaptured Fillet authoring only, move and down now share one adapter helper that enumerates the
complete `elementsFromPoint` stack and reconciles the exact headless radius owner to its painted
`FeatureCorner`. With no matching radius owner, it preserves the top painted item only as an intent
hint and cannot promote a foreign computed item or create semantic browser precedence. The retained
coordinator still authenticates candidate, preview, accepted/design/computed provenance, geometry
policy and exact radius proximity before hover or down can consume the hint; captured movement
remains editor-owned. The shared radius-affordance group supplies the same identity to grip, rail
and spoke, while presentation coverage freezes all three pointer-active surfaces.

Focused evidence passes demo-web 117/117, native/WASM M75 11/11, warnings-denied Clippy, demo-web
WASM, formatting, diff and unchanged 270-row golden checks. Chromium script
`/tmp/m75_f001_browser_check.mjs`, SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`, passes all 6/6 cases on a
provisional corrected local build, including the overlapped grip and visible spoke/rail
hover/capture/release paths.

Exact post-F002 source `553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`, passes the complete 480.94-second release gate with
demo-web 117/117, native/WASM M75 11/11, unchanged 270/270 golden, the 141.82-second sparse
crossover and Trunk. Its no-rebuild read-only snapshot `/tmp/geosolve-m75-f002-uat.hlSQYT`,
aggregate `eae64913c29d760f6eb64d7681212facca0c6d8869dee9631aeb9d77b059a139`, is byte-verified and
was served by PID `37152` at `http://100.94.63.83:8080/`; old PID `4026985` was already retired.
PID `37152` was subsequently retired before M76 took the shared endpoint. HTTP evidence is
retained at `/tmp/geosolve-m75-f002-http-verify.1nRxtz`. Tailscale M72/M74 checks pass at both sizes,
and M75 script hash `1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e`
passes 6/6 on the frozen bytes. This was current mechanical UAT authority at nomination. Automated
evidence alone disposed no human item. The supervising caller subsequently accepted this exact
candidate, the focused F001/F002 recheck and U1-U12 for scoped closure on 2026-08-16 without
claiming an individually logged execution of every detailed step. Documentation-only approval
descendant `f80235978fbcdccd58c45a08bccf3969a20110c9` passes Pages run `31939764951`, artifact
`9261974799` and deployment `5929879555`. ZIP/tar SHA-256 values are
`8c031953dec4975c9b701a5ba30f060a95d5e0772286396f3c03ac74fb665fc0` and
`8ac419fbea39c306e6ee529309f2d3965c93d4ff0459fd2e21179714e9b89c1d`; the seven-file aggregate is
`4c2da7d7860ac0bcadc64722007b5accb01aa999aa79f3046ba9d2868e86ef3b`. Every public response and
the public M72/M74/M75 browser matrices verify, completing M75.

### M75-U1 - Deferred M74 review and hover accessibility matrix

Status: accepted for scoped closure under the supervising caller's 2026-08-16 approval. U1-U12
were not individually logged, so this status records the explicit closing disposition rather than
a separate step-by-step execution transcript.

Run all eight deferred sections of `docs/M74_UAT.md` through the M75 scorecard: permanent/selectable
datums, ordinary datum relations including axis symmetry, datum-inference priority, visual grid and
camera behavior, coordinate HUD/cursors, editing-owned Undo/Redo, inert SVG letterbox bands and
compact-desktop polish. Repeat the new H1-H3 hover paths at `1440x900` and approximately
`1024x720`, at coarse and fine zoom and on both sides of existing point, annotation, curve, Fillet
and datum tolerance fringes. Human review judges prediction stability and visual truthfulness;
direct tests own exact boundary equality.

Tab through existing tree, inspector, accessible Fillet controls, tool popouts and dialogs. Focus
must remain visible without synthesizing pointer hover, accessible names remain meaningful, canvas
hover never steals focus, and overlay ownership clears canvas hover. Selected/related/problem and
primary-hover states retain non-colour visual distinctions. The prepared matrix remains the
reference for any future focused regression. For M75 closure, M74-U1/M75-U1 through M75-U12 are
accepted under the scoped approval above; that disposition comes from the supervising caller
rather than automation and does not invent individual observations.

### M76-A1 - Seven-family dimension annotation sampler

Build one accepted scene containing point distance, affine line/polyline-span length, radius,
diameter, oriented angle, supporting-line offset and exact translated-segment offset occurrences.
Verify compact four-significant-digit values and truthful family-specific baselines, witnesses,
leaders, arrows, arcs and label bounds. Exercise driving and reference presentation, extreme values,
negative-zero cleanup, coarse/fine zoom and deterministic recomposition. Rendering and hit tests
must consume the same finite primitives; an arc diameter must not invent an opposite arc point.
For shared-endpoint finite line rays whose interior wedge matches the accepted acute/right angle,
the vertex, rays, arc, label and hit geometry occupy that wedge and the opposite arc point misses.
An obtuse finite join retains the acute supporting-line presentation so sector and value remain
consistent.
Reference values must come only from exact matching accepted-document/revision hydration. A
detached, revision-mismatched or accepted-document-mismatched scene reports `value unavailable`
and never presents a dormant target scalar as a measured value.

### M76-A2 - Contextual twenty-category constraint sampler

Cover every `SceneConstraintGlyph` category in one reviewed native/WASM catalog. Fixed,
coincident, contact, tangent, continuity and Fillet marks remain near their actual operands with
leaders; H/V marks describe actual alignment; paired relationships use consistent ticks/slashes;
symmetry, direction and normal rotate with local frames. Marks are contextual by default and the
Display “show all” option reveals them all. Only a genuine perpendicular corner owns a fixed
right-angle square.

### M76-A3 - Presentation-only move, cancel and reset

In Select mode press a dimension label or movable glyph, cross the 3 px threshold and move it.
Other dimension primitives select without moving. Verify geometry-appropriate placement state,
then cancel independently through Escape, pointer-capture loss, camera change and tool change; each
restores the exact original placement. Commit a move and exercise selected reset and reset-all.
Every operation leaves solve count, design/attempt/accepted revisions, branch state, reproduction
geometry and sketch Undo/Redo length unchanged. Delete and ordinary same-document Undo/Redo retain
surviving cached placements; new document/sample clears them.

### M76-A4 - Workspace-v6 cache recovery

Round-trip a v6 workspace with a self-versioned optional annotation-layout cache and reproduce the
same resolved placement. Restore v1-v5 as empty layout. Independently corrupt the cache version,
item identity, placement form and finite values while preserving valid sketch payloads; restoration
must accept the sketch, discard only unusable layout and regenerate deterministic auto placement.
Canonical sketch v1-v4 bytes, unsupported draft-v5 policy and `GEOSOLVE_REPRO_V1` authority remain
unchanged.

M76 closure uses exact clean-qualified source `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`,
tree `248cba4509a992aeff7a02dd6d57a1a2481380a4`, and no-rebuild snapshot
`/tmp/geosolve-m76-final-uat.65Y8J1`, aggregate
`967f0c1943c16b9c4a9975aeb973ad0cfe2c6e3dbfab45f414d0dac1bb9088f3`.
U1-U4 are accepted for scoped closure under the caller's 2026-08-17 approval; the individual
steps were not separately logged and a post-refinement replay was intentionally not required. The
angle-side correction is an M76 feature refinement, not an `M76-Fxxx` defect. The earlier
`37eade50` and `9b4e7f7` nominations remain superseded historical evidence.

Final public scenario evidence comes from successful Pages run `31961652265`, qualify job
`95200423007`, deploy job `95204687455`, artifact `9267811418` and deployment `5933831093`.
The artifact ZIP/GitHub digest is
`dba7e2f5e1b7a51390ec1d840e7869d69968114bcf13250e641448a02d0cb60b`, its inner-tar digest is
`be18173d61fef8ead3d00cf2dd560f893a7731eff7fa3bdfc0b81aadab6298e5`, and its exact seven-file
manifest aggregate is `41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96`.
Root plus all seven public paths byte-match that hosted artifact. The unchanged M72 check passes at
both desktop sizes. M76-adapted copies of the retained M74 and M75 checks replace only their
obsolete Origin-canvas expectations with the approved two-axis intersection contract; hashes
`4aff982c6a9e10702d7b0179c17682c6904bb6c28362ebefe967705a984c3355` and
`161e96d541dbcc189dbbc23c47da672e3080b7c7646e45c11ef458a5e521a067` pass M74 at both sizes and
M75 6/6. Pages is final public-byte authority; the Tailscale snapshot is separately frozen
candidate evidence and no cross-build byte identity is claimed. M76 is complete.

### M77-A1 - Selected curve control inventory and owner parity

Build one accepted scene with circle, circular/elliptical arc, ellipse, rational quadratic,
parabola, hyperbola, Bezier, B-spline and NURBS examples. For each selection, publish exactly the
family controls in `docs/M77_GOALS.md`, finite cage/guide geometry, stable curve-owned identities
and shared paint/hit primitives. Deselection, tool/input-owner change, camera change and accepted-
scene replacement revoke them. Headless hover and pointer-down resolve the same role and owner;
stored points keep point ownership, visible selected-curve handles outrank underlying curve or
annotation paint, and active Fillet output arcs expose only their Fillet-owned affordances. At a
radius, minor/conjugate-axis or projective guide origin shared with a stored point alias, the point
owns its acquisition region and the guide remains directly hittable beyond that region.
At an elliptical arc's major pole, the stored axis point owns the physical pole and a coincident
derived trim receives a zoom-independent tangential presentation offset. When trims occupy either
or both minor poles, the size grip chooses the clearer signed rail or shifts outward while all
semantic controls and the size rail remain independently hittable.

### M77-A2 - Inverse trim endpoints preserve discrete state

Drag Start and End handles on circular arcs, elliptical arcs, parabola segments and both explicit
hyperbola branches. Lower each finite cursor target through the existing inverse trim projection
and durable scalar edit. Verify exact support-curve incidence, stable Start/End identity, directed
trim order, angular unwrapping near the current seed, unchanged arc sweep and unchanged hyperbola
branch. Invalid crossing, center, domain and stale-owner samples fail closed while retaining the
last valid finite preview; they never exchange endpoints or change discrete state implicitly.
For parabola and hyperbola trims, exercise Start and End crossings from both initially ascending
and initially descending parameter order and require exact state/history/publication retention.

### M77-A3 - Rational middle and stored control cages

Select positive- and negative-nonzero-weight rational quadratics, render their Euclidean
`P1 = Qh / w` middle control and endpoint cage, then move only that handle. The prepared edit
atomically writes `Qh = w·P1` while preserving the exact middle-weight scalar and both persistent
endpoint owners. Verify that non-unit construction clicks have the same `P1` meaning. For an exact
zero-weight curve, render and edit an explicitly projective `Qh` vector rather than dividing by
zero or publishing a synthetic point; entering/leaving projective mode preserves `Qh` explicitly.
When a host binding supplies an accepted effective weight different from the stored fallback,
spatial movement uses that accepted weight for `Qh` and retains the fallback bit-for-bit through
Undo/Redo, replay and checkpoint restoration. A nonzero Euclidean control that cannot survive a
finite precision-preserving `P1 -> Qh -> P1` round trip rejects atomically.
Bezier, B-spline and NURBS cages expose their existing stored control points through ordinary point
dragging. Rational/NURBS weights remain explicit numeric inspector values; no spatial weight rail,
synthetic point, new constraint operand or persisted handle is created.

### M77-A4 - Family size rails and domains

Exercise circle/circular-arc radius, ellipse/elliptical-arc minor-axis ratio and hyperbola
semi-conjugate handles along deterministic family rails. Preserve the initial grab offset and wait
for the 3 px movement threshold. Accepted candidates update only the owned existing scalar subject
to its exact domain: radius and semi-conjugate are positive, and minor ratio is positive and at
most one. Crossing a boundary retains the last valid preview without non-finite geometry, ellipse-
axis exchange, curve-family change or a false accepted-state replacement.

### M77-A5 - Exact prepared preview, history and persistence

For endpoint, rational-middle and size gestures, prepare every preview from one exact accepted-
session clone. Accept only independently validated finite candidates and publish the exact final
prepared patch through compare-and-swap. Escape, capture loss, camera/tool change, rejection and
stale scene work restore the origin and add no history; one successful gesture adds exactly one
Undo/Redo step regardless of pointer-sample count. Save/reload and reproduction copy/restore retain
only existing scalar or weighted-middle data, then recompute selected handles from accepted
geometry. Native/WASM semantics, thin demo mapping and the two-size/zoom UAT matrix must pass.

Candidate render scenes retain their own truthful design, accepted revision and computed input.
The private pointer-down origin binds the exact accepted preview request and model position without
granting drafting authority. A prior sealed generation may not sample or release a newer unseen
candidate; rejection clears transient work and preserves the durable document, transcript and
history.

### M77-A6 - Spatial circular and elliptical arc authoring parity

Author a circular arc through Centre, Start and End clicks. Author an elliptical arc through
Centre, Major axis, Start and End clicks. After the elliptical axis click, publish a finite
headless-evaluated support ellipse; inverse-project both later spatial clicks radially in normalized
ellipse space and retain explicit sweep. The final projected endpoints lie on the support curve,
numeric Start/End construction fields are absent, and the browser renders only supplied preview
points and role markers without reconstructing an ellipse equation.

M77 is approved for closeout. Implementation and direct coverage for every A1-A6 contract pass;
resolved findings `M77-F008` through `M77-F016` add no golden row. Source `f53934f` contains the
replacement corrections. Exact product source `cc99b11071dc62732e02b630ba7a1381d754b04c`, tree
`3315a2bdd0137f59657ea2500962ef971a23ea15`, passes the complete clean gate; its no-rebuild snapshot
`/tmp/geosolve-m77-uat.ARrQFw`, aggregate
`abfa7ef6b75f127fa6d93ff6ad6960c7f5df7d4c799a578c785e1192c2b7ee94`, is immutably frozen and
byte-verified on Tailscale. Exact source `51a3b95`, tree `8d154a1`, snapshot
`/tmp/geosolve-m77-uat.1mDjQv` and aggregate
`af7c2fbca1a6481c8c055142c9a64578b570fbcb297f687f09cc8ffc85bd1b8b` are superseded historical
evidence only. On 2026-08-17 the supervising caller explicitly approved the current replacement and
requested closure; U1-U6 pass under that scoped disposition. Approval descendant `66a89b7` passes
Pages run `32012819635`, artifact `9283439225` and deployment `5942438795`. Root plus all seven
hosted paths byte-match exact ordered-manifest aggregate
`872719a0f4323f978bf31a4e567646b61a8bd607a2dbc384e47b676054979f15`. M77 is complete.

### M78-A1 - Exact family/variant catalog and semantic stages

Enumerate `GeometryToolFamily::variants()` for Point, Lines, Rectangles, Circles, Arcs, Ellipses,
Béziers, Conics and Splines. Require exactly 1/3/4/3/3/4/2/3/2 variants, 25 unique stable keys,
deterministic family/variant order, one valid default per family and a defined coarse `EditorTool`
projection. Exact activation and compatibility activation agree on the default recipe without
requiring a 25-case legacy enum.

Drive every exact variant through its semantic stages. Status names the variant and stage, reports
progress/finishability and publishes only applicable finite typed measurements/branch actions.
No terminal stage becomes committable from an ordinal or coordinate count alone. Native and WASM
transition transcripts are identical.

### M78-A2 - Line/polyline/midpoint and rectangle recipe intent

Author one Segment, an Enter-finished open Polyline, a first-vertex-click closed Polyline and a
Midpoint Line. Closure reuses the first persistent point without a duplicate allocation or
zero-length span. Midpoint Line stores its centre, reflects its opposite endpoint and commits one
ordinary Midpoint relation. Backspace/Undo first removes unfinished stages; first Escape cancels
the shape but retains the tool, and a second draft-free Escape activates Select.

Author all four rectangle variants normally and with regularization. Every rectangle has four
explicit shared-corner lines, no lock/dimension/target scalar and the minimal ordinary aligned or
oriented shape relations. Centre variants add one visible Construction diagonal and one centre
Midpoint relation. Shift adds persistent adjacent-edge EqualLength. Shift plus suppressed inference
retains the intrinsic square while committing no ambient source. One accepted rectangle recipe is
one retained history step and survives Undo/Redo and reload with exact roles/relations.

### M78-A3 - Derived circles, arcs and existing-point incidence

For 2-Point Diameter and 3-Point Circle, sample free coordinates and existing persistent points.
Derive one finite centre/radius without allocating a point for a free rim sample. Add PointOnCurve
for each snapped existing point against the prospective created circle in the same plan. Repeated
or scale-aware near-collinear samples reject with document, accepted scene, allocator, history and
terminal draft unchanged; correcting the last stage succeeds without restarting.

Repeat for Center Arc and ordered Start/End/Through 3-Point Arc. Require exact Start/End identity,
Through incidence, explicit sweep and no synthetic trim point. Flip Center Arc between
complementary sweeps before commit and preserve the chosen branch through Undo/Redo and reload.

### M78-A4 - Native endpoint Tangent Arc

Start from finite nonzero jets at both endpoints of representative native open affine, conic,
Bézier and spline spans. Select one endpoint and target `E`, derive the unique outgoing tangent arc
from the exact endpoint tangent/normal and chord, then commit generic tangency with explicit source
contact, endpoint neighbourhood, orientation and created sweep. Independently validate finite
radius, endpoint incidence, tangent alignment and hard residuals.

Reject an interior contact, periodic/computed-only source, zero-speed endpoint, zero chord,
zero-normal chord projection (the infinite-radius tangent-line limit), non-finite radius and
vanishing sweep. Each case retains the complete previous accepted state and correction-ready
draft, allocates nothing durably and never reports convergence for invalid geometry.

### M78-A5 - Ellipse frames, spatial trims and advanced-family compatibility

Author Center–Axes and Axis-Endpoints full ellipses with the same accepted principal frame and
typed minor-ratio options. Author both corresponding elliptical-arc variants, inverse-project
spatial Start/End through the headless support ellipse and retain endpoint identity plus explicit
sweep. Complementary-sweep flip changes only branch state. Degenerate axis/frame or invalid trim
projection rejects transactionally; the adapter receives render-ready preview/status and performs
no ellipse equation or projection.

Author Quadratic/Cubic Bézier, Rational Quadratic, Parabola and both Hyperbola branches through
their exact grouped variants. Preserve the pre-M78 persistent geometry, scalar domains, ordinary/
projective rational middle meaning and explicit branch state. Grouping cannot change later M77
control cages, property edits or accepted document bytes for an equivalent recipe.

### M78-A6 - Variable-length open and periodic NURBS

Drive Open and Periodic Control NURBS with valid degree/topology options. Step back one unfinished
control, then finish separately through Enter and double-click. Open output contains exactly the
confirmed controls; periodic output records explicit periodic topology without duplicating the
first control or using proximity closure. Finishability comes from typed headless status.

Too few controls, malformed degree/knot/weight options, non-finite input and resource exhaustion
retain the draft and accepted state for correction. An invalid inactive spline overlay cannot block
another exact geometry variant.

### M78-A7 - Provenance, precedence and atomic recovery

For representative single- and multi-curve recipes, inspect the authenticated construction plan.
Every relation is exactly `RecipeIntrinsic`, `RecipeRegularization` or `AutoInference`; intrinsic
sources apply first, regularization second and compatible ambient sources in stage order. A
redundant/conflicting horizontal, vertical, midpoint or equality inference yields to the recipe and
does not persist as hidden duplicate intent.

Allocate, solve and independently validate one cloned retained transaction, then publish once by
exact compare-and-swap as one history entry. Rejected, ambiguous, exhausted, stale or cancelled
work retains document, accepted scene, history, allocator high-water, last valid preview and
terminal draft. Correction, cancel or Undo clears its local status and cannot leave a stale global
problem or blank scene.

### M78-A8 - Thin family overlays and role ownership

At both supported desktop sizes, open all nine family overlays and exercise the 25 exact variants.
One bottom-left overlay persists through blur, canvas click, pan and zoom, remembers session-local
variant/options and closes to Select only by explicit close, Escape-to-Select or tool/family
switch. Accessible labels, prompts, live measurements and keyboard focus match returned headless
state; Tab focus does not synthesize pointer hover.

Main created geometry follows the active Profile/Construction role and centre-rectangle helpers
remain Construction. Save/reload and reproduction retain only ordinary document geometry,
relations, contacts, roles and branch state; overlay memory/drafts are not canonical persistence.
Thin demo tests prove event/modifier/action mapping and render parity without browser recipe
geometry, inference precedence or branch choice.

### M78-F001 - Representable Tangent Arc survives extreme scale

Construct the same finite outgoing Tangent Arc at scales `1e-6`, `1` and `1e6`, then exercise an
extreme finite chord whose squared length would overflow or underflow before division. The recipe
uses the scale-safe equivalent offset, returns finite centre/radius/sweep and independently validates
the ordinary generic-tangency result. Zero normal projection and genuinely nonrepresentable output
still reject locally.

### M78-F002 - Recipe provenance owns conflict while compatible ambient intent survives

Declare one rectangle plan out of source order with ambient Vertical on a span, intrinsic Horizontal
on that same span and regularizing EqualLength on an adjacent pair. Lowering publishes intrinsic then
regularization, carries exact curve roles and omits the conflicting ambient source. In a separate
oriented rectangle, retain a compatible ambient Horizontal baseline alongside intrinsic
Perpendicular/Parallel intent. Both plans solve and publish exactly once.

### M78-F003 - Positive acknowledgement requires exact retained publication

Produce a tokenized Tangent Arc plan and acknowledge it as accepted without applying it through the
coordinator. The token is consumed as a local rejection, accepted document/history remain exact and
the terminal draft stays correction-ready. Repeat through ordinary and controlled coordinator
publication: only the exact matching expected input/plan gains publication evidence, and the later
positive acknowledgement clears the preview/draft once.

### M78-F004 - Controlled proposal work stops before allocation

Submit a 10,000-point Polyline plan through controlled publication. With zero
`DocumentValidationItems`, stop at `DocumentValidation`; with one validation item and zero
`DocumentLoweringItems`, stop at `DocumentLowering`. In both cases document, accepted prepared input,
history/cursor, transcript, sketch identity high-water and computed-evaluation high-water are bit-
exact. Unlimited control can continue through the normal atomic path.

### M78-F005 - Stale operands and Tangent Arc jets reauthenticate or recover locally

Leave a Tangent Arc plan awaiting acknowledgement, then move its source endpoint through a separate
accepted edit and reject the now-stale plan. Against the next authenticated scene, require the
preserved prefix to refresh source position, endpoint jet, centre, sweep, contact parameters/
neighbourhoods and tangent orientation before correction. Repeat after deleting the source curve:
publish no preview/commit, show only a draft-local rejection and allow step-back/Escape to recover
without a global problem or allocator/history change.

### M78-F006 - Extreme-finite midpoint, segment, conic and derived recipes remain representable

Use like-signed endpoints around `8e307` and `1.2e308` for a Midpoint relation, diameter circle,
Midpoint Line, axis-endpoint ellipse/elliptical arc and centre-based rectangle recipes. Use an
extreme radius with a tiny Center Arc direction sample. Each mathematically representable result
contains only finite points/scalars, solves and passes independent hard-residual validation. Check
the unchanged midpoint Jacobian by finite differences at `1e-6`, `1` and `1e6`; genuinely
nonrepresentable reflections/projections remain rejected.

### M78-F007 - Circumcircles use normalized frames and local incidence

Author 3-Point Circle and 3-Point Arc from opposite-extreme and diagonal-extreme finite samples whose
raw chord `hypot` can overflow. Require a finite locally valid centre/radius and accepted solve.
Translate a small triangle to a large world coordinate where centre rounding would miss a sample;
validate the rounded circle by local point-to-centre distances rather than normalized absolute world
coordinates. A false-incidence result emits no plan and stays `InvalidTerminalGeometry`.

### M78-F008 - Tangent Arc validates both requested endpoints

From a native open endpoint translated near `1e16`, request a moderate finite Tangent Arc target.
After plan lowering, evaluate the created arc at its terminal parameter and require the requested
target within focused tolerance while source incidence and tangency remain valid. A rounded centre/
radius that cannot represent both source and target incidence emits no construction plan and remains
a local draft issue.

### M78-F009 - Draft status omits nonrepresentable derived measurements

Create a Center-Radius Circle with finite radius `1e308`. The draft status includes exactly the
finite radius, omits the overflowing `2 * radius` diameter and contains no NaN/Inf measurement. The
underlying plan remains finite and independently accepted. Apply the same finite-only assertion to
all extreme circle/arc length, angle, ratio and width/height status values.

### M78-F010 - Ambient point-on-curve contact labels stay byte-compatible

Lower an auto-inferred `PointOnCurve` occurrence before and after typed provenance routing. Its
contact label remains exactly `auto point-on-curve contact N`, including one-based occurrence
numbering. Recipe-owned relations/contacts may expose their provenance-specific audit labels, but
legacy ambient contact bytes and identity ordering do not change.

### M78-F011 - Endpoint tangency does not implicitly lock either arc centre

At source `7018e87`, author a counterclockwise source arc centred at `(0,0)`, radius `2`, spanning
`0` to `pi/2`, then a clockwise Tangent Arc centred at `(0,3)`, radius `1`. Join source End
parameter `1` to created Start parameter `0` with exactly one Aligned generic tangency and no lock
or dimension. Drag each centre diagonally through the ordinary projected-move coordinator.

Both drags must publish a history-neutral accepted preview and attain the active target within
`1e-8`. The coupled centre remains finite, an unrelated point remains bit-exact, the independent
hard residual is at most `1e-9`, endpoint parameters remain bit-exact, contact neighbourhoods and
orientation remain End/Start/Aligned, and source/created sweeps remain counterclockwise/clockwise.
Locality evidence reports one anchor and two point-observable passive DOF; scalar-only nullspace
freedom cannot require an impossible point anchor. Dense-nullspace and projected-CGLS secondary
working sets keep every fixed coordinate bound equality-active even when its projected normal is
dependent, preventing a repeated zero-length bound event and backend-specific `NumericalFailure`
or `Stalled` termination.

M78 is complete. A1-A8 implementation and M78-F001 through M78-F010 pass through product
commit `4845df7`; F011 additionally passes focused core, sketch-locality and actual authored-
coordinator regressions through product fix `e43aa85`. The unchanged 270-case golden
survey/check/clean authority still matches. Initial source
`1b2ce0f9d843c036e3a7023674cbf219c9f593b7`, tree
`321ca280a5f581ee9755d615733617c98c0e21d7`, passes the complete clean gate and historical
nomination but is withdrawn by F011.

Replacement source `793e9de39d78bdabfded15d8c8e79f86df0f52bc`, tree
`9f74ec9b63955bfffdf2338fd1ab95ac8092856a`, passes the complete clean release gate from 11:07:08
through 11:19:06 AEST with 1,734 passing locked all-feature workspace tests, three intentional
ignores, unchanged 270/270 golden authority, warnings-denied Clippy/Rustdoc, native/WASM parity,
performance/licence/package checks, the 149.39-second sparse crossover and Trunk 0.21.14. Its exact
seven-file output is frozen without rebuilding at `/tmp/geosolve-m78-f011-uat.MOsOFy`, ordered-
manifest aggregate
`a51e76c2567d7e6c0352503cb3abeed23bddb7ecbd04e5c3d7acd1dd1d45fd97`, and byte-verified at
`http://100.94.63.83:8080/` for root plus every file. On 2026-08-18 the supervising caller accepts
U1-U8, reports the focused F011 replacement behaving correctly and requests closure.
Documentation-only approval descendant `a6d504e1d15ddcdd7e4cb02190b0ef83de814be0`, tree
`ca50d013fe4a6ac040336344056a1d142c5629fa`, passes Pages run `32096209036`, artifact
`9310104202` and deployment `5955688918`. Root plus all seven hosted paths byte-match ordered-
manifest aggregate `bcf95289a347760a805da392d3064ef1b372b22505f3f150a4236b270b66c51f`.
Exact gate-qualified product authority remains `793e9de`; M78 is complete.

### M79-F001 - One stationary cohort cycles without latch churn

Resolve a point-stage frame with at least two equal semantic anchors and a line-stage frame with
ranked positional/directional alternatives. Capture the complete candidate IDs, order, relations,
coordinates and guides. Feed the exact frame back with candidate A, candidate B and candidate A
again, then cycle the entire ranked list through two wraps.

Every advertised ID remains selectable and publishes only its own guides. Candidate order and
payload stay exact; explicit choice does not change automatic anchor, datum, direction,
concentric, point-tracking or remembered-reference state. A genuinely foreign frame or ID remains
`StalePreferredCandidate`, confirms no stage and exposes no cycleable replacement cohort.

### M79-F002 - Stationary browser choice cannot poison another context

Choose a drafting candidate with Tab, then independently change pointer coordinate/identity,
Ctrl/Cmd suppression, Shift regularization, blur/leave, tool/stage, Escape/Backspace, Undo/Redo,
camera, scene/import context and canvas ownership. The pointer queue retires the choice before the
next sample and sends no candidate when geometry drafting does not own that sample. An unchanged
pointer-down may forward one choice once; success or failure leaves no retry authority. A hover-
only stale result clears and resolves the same stationary sample once without preference, while a
stale pointer-down remains noncommittable.

### M79-F003 - Latest queued movement owns Tab

Queue movement from candidate frame A to a distinct frame B and invoke Tab before the animation
frame callback. The adapter drains and resolves B first, then asks B's headless resolution for the
next candidate. No ID or guide from A is applied to B, the cancelled scheduled callback is inert,
and accepted geometry/history remain unchanged until an explicit current pointer-down.

### M79-F004 - Strong midpoint snap survives a redundant direction

Create an axis-aligned Center Rectangle centred on the immutable Origin. Start a Midpoint Line from
that stored centre and hover the exact midpoint of the rectangle's right edge. The ranked cohort
contains `Midpoint + Horizontal`, `Midpoint`, `PointOnCurve@0.5 + Horizontal`,
`PointOnCurve@0.5` and `Horizontal` in deterministic semantic order.

Commit the default mixed candidate through its authenticated token. The first trial must prove the
auto Horizontal source fully redundant while the auto Midpoint source remains useful. Retry once
from the original retained state without exactly that direction; independently validate finite
accepted geometry and hard residual at most `1e-9`; then publish one history/transcript step. The
retained document contains the recipe Midpoint and associative endpoint Midpoint, no auto
Horizontal, and the original token receives positive acknowledgement only after effective-plan
publication. Undo/Redo/replay reproduce that effective plan. Generic direct plans, partial
redundancy, redundant positional intent and direction-only candidates retain fail-closed
`RedundantInferredConstruction` behavior.

M79 product source `6874aa1` passes focused native/WASM, demo, workspace and clean release
qualification plus unchanged 270-row golden authority. Its immutable Tailscale candidate is byte-
verified and supervising-human UAT accepts U1-U5 without a new finding. Documentation-only
approval descendant `2560ca5`, tree `bad5662`, passes Pages run `32116835502`, artifact
`9317131695` and deployment `5959116526`. Root plus all seven hosted paths byte-match ordered-
manifest aggregate `5692d4a994d9d14b2bd867dd8740af0f83c497fa88888cc189b7b1fcc0a994ca`.
Exact gate-qualified product authority remains `6874aa1`; M79 is complete. These focused scenarios
do not expand the stable golden because the defects concern stationary interaction/coordinator
lifecycle rather than a missing durable authoring-family dimension.

### M80-O1 - Grouped closed linear face offset

Create an axis-aligned rectangle and a non-axis convex polygon from complete native Profile line/
polyline spans with authenticated shared-point junctions. Apply one grouped Profile Offset in both
directions. The outer loop remains material-left and has the same edge count/order; every target
support is parallel, same-traversal and at the one positive shared distance. One source mapping
owns every ordered residual block, one annotation/dimension owns the operation and one Undo removes
the complete publication. A source polyline span creates one standalone native
`CurveDefinition::Line` target rather than rebuilding a polyline container.

Move eligible source and target points without crossing a topology barrier. Both sides remain
finite and offset-associated, independent normalized hard residual is at most `1e-9`, and rank/DOF
match the grouped equations rather than a hidden lock. Suppress/delete only the dimension and prove
the target curves plus ordinary shared-point connectivity remain and become freely editable.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O2 - Circular and mixed line/arc face offset

Offset a one-circle face Outward and Inward. Centers remain equal and
`r_target - r_source = D*w*d` for explicit face direction `D` and traversal winding `w`; a radius
collapse rejects without replacing the accepted circle. Build a simple closed loop containing line
and circular-arc edges with one persisted miter and one persisted tangent join. Target edge family,
traversal, sweep, connectivity provenance and join branch remain exact through regular source
edits. No polyline approximation or arrangement fragment is emitted.

Each line block reuses the supporting-line Jacobian; each circular block's equal-center/signed-
radius rows and every tangent anchor pass central finite differences at scales `1e-6`, `1` and
`1e6`. Structured audits retain deterministic high-level source/edge/join attribution.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O3 - One face carries all holes with material semantics

Create a native bounded face with one outer counter-clockwise loop and at least two clockwise holes,
including one circular hole. One Apply owns all loops. Outward expands the outer loop while shrinking
both holes; Inward shrinks the outer loop while expanding both holes. Ordered loop/edge pairing,
orientation, strict nesting and hole identities survive Undo/Redo and draft-v5/workspace/repro
round trips.

Increase distance to the first outer/hole contact, hole/hole contact, hole collapse or nesting
barrier. Independent topology validation rejects the whole candidate atomically: it may not trim,
drop a hole, split a loop or publish a partially valid subset.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O4 - Exact one-edge open chains

Collect one native line in its explicit traversal and apply Left and Right. The two supporting-line
rows plus both terminal tangential anchors are equivalent to exact endpoint normal translation; a
fixed source leaves no target endpoint slide or length freedom. Collect one directed circular arc
and apply both sides. Equal center, signed radius and both terminal anchors fix the exact Start/End
branch and reject the antipodal endpoint root.

Both source and target arc endpoint angles remain active for bidirectional editing. Explicit source
Start/End `Preference` rows retain only the otherwise-free shared-angle gauges; one or both hard
target endpoint drivers take precedence and propagate to the source without any weighted hard-row
substitute.

Flip and signed negative authoring input change only the durable direction while retaining a
positive scalar. Canonicalization, insertion order changes, Undo/Redo and reload never reverse the
stored source or target traversal.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O5 - Ordered multi-edge open chains and junction provenance

Manually collect a connected non-branching line/arc chain with miter and tangent joins. Every next
edge must connect to the current terminal through the persisted shared point or active endpoint-
contact source; coordinate coincidence is insufficient. Store the same provenance on ordinary
target junctions, one explicit left/right miter turn or Tangent branch at internal joins and normal-
translation policy at both terminals.

Non-branching is evaluated over the selected spans. A deliberate selected path may traverse a
junction with additional unselected incident Profile edges; those edges remain outside the
operand and unchanged. A selected set with more than one continuation at a selected endpoint,
with no continuous order, or with a closed traversal remains invalid.

Target connectivity, terminal translation, same-family pairing and collection order survive edits
inside the cell. A disconnected edge, branch, reversed tangent, cusp, miter-to-tangent transition,
turn reversal or deleted junction owner rejects locally and leaves the last complete accepted scene
and all history/IDs unchanged.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O6 - Unsupported provenance and topology barriers are atomic

Attempt Construction, external, computed-Fillet, arrangement-partial, ellipse/elliptical-arc,
conic, Bezier, B-spline and NURBS operands. Reject before target allocation and emit no approximate
or sampled substitute. On supported profiles, exercise edge/loop collapse, self-intersection,
non-adjacent chain contact, contour contact, split/merge, hole loss and invalid circular radius.
The topology certificate covers only the selected source/target operand paths and their contours;
unrelated sketch arrangement geometry is not compared and cannot veto an otherwise valid offset.

For each rejection compare document JSON, accepted identity/coordinates, residuals, scene,
history/transcript, source diagnostics and persistent high-water with the exact prior accepted
state. Recovery at a later valid distance publishes normally without refresh, stale global error or
partial target geometry.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O7 - Persistence and compatibility remain explicit

Freeze the v2-v4 dimension wire language at its historical seven variants and prove prior canonical
and empty draft-v5 fixture bytes unchanged. A document containing Profile Offset returns typed
`UnsupportedM80State` from canonical-v4 export. Its private draft-v5
`profile_offset_dimensions` side section round-trips the exact positive scalar, driving mode,
operand, loop/chain order, traversals, source-target pairs, junction provenance, branches, terminal
policies and suppression state. Workspace v6 and `GEOSOLVE_REPRO_V1` reconstruct through their
ordinary strict atomic path.

Undo/Redo, deletion/suppression, stale exact-CAS, cancellation and forced allocation/resource
failure preserve identity non-reuse and one-step transaction semantics.

Status: implemented; focused, broad and clean-nomination mechanical evidence passes.

### M80-O8 - Headless authoring, preview and Tailscale UAT

The shared scene resolver returns whole-face or eligible ordered-edge ownership consistently for
hover and pointer-down. A separate Offset authoring state owns Distance, Flip, collection, exact
scene/topology stamp and Apply/Cancel. Preview target geometry is provisional and non-selectable;
scene/history/import/tool changes revoke it, and the browser does not calculate supports, miters,
branches or topology validity.

The provisional target edges and grouped distance presentation are nevertheless an explicit
authoring-only distance surface. A press on that exact held preview captures one pointer; movement
starts at the shared 3 px threshold and samples the positive distance absolutely along a frozen
headless source/target rail. A rejected or topology-invalid sample keeps the prior complete
preview, a later valid sample recovers, release changes no history, and Escape, capture loss,
camera/tool change or stale authority restores/cancels the pointer-down candidate. Apply remains
unavailable while the pointer is captured and alone publishes the final exact held patch after
release. Preview, release and restore effects carry one monotonic gesture identity; delayed effects
from an earlier drag over the same proposal reject without touching the current captured gesture or
its exact pointer-down rollback state.

Modify → Offset uses the persistent bottom-left panel, remembers only the last valid process-local
distance with `0.1 * model_scale` fallback and returns to Select on explicit close/Cancel. A
negative Distance entered before operand selection supplies transient direction intent. Typed
unavailable hover/click feedback survives for unsupported and dynamically invalid candidates;
ordered collection renders traversal arrows and Start/End terminals; pointer, tree and keyboard
activation share one semantic Offset pick. One movable Profile Offset annotation uses the
disposable cache and recomputes safely after cache loss.
Workspace-v6 retains a compatible placement, while reproduction copy omits it and reproduction
load ignores any legacy cache row so placement is recomputed.
Focused native/WASM/presentation tests and the complete clean gate precede a no-rebuild read-only
snapshot kept byte-verified on `http://100.94.63.83:8080/` through `docs/M80_UAT.md`. Human UAT and
exact Pages publication now pass; the listener is retired and M80 is closed.

Historical pre-amendment source `b83dad2`, tree `440d66e`, snapshot
`/tmp/geosolve-m80-uat.hggNdd` and ordered-manifest aggregate
`d8d740fb852e793925ce4e54e8777a225b68ea5cfa39b2f36060bd3566938e37` pass the complete gate,
temporary-port verification and final byte verification at `http://100.94.63.83:8080/`. The former
`949c3db` snapshot is withdrawn and no longer served. The native-Fillet scope amendment withdraws
`b83dad2` from acceptance; its recorded server has exited and the snapshot is no longer served.
Status: F016 replacement source `29d8e41`, tree `44ecb95`, snapshot
`/tmp/geosolve-m80-uat.CPuVgx` and ordered-manifest aggregate
`75ee83edc5a5985272e00c005dae95c9091851a7c928c2b55e9a7b096f328997` pass the clean replacement
gate, immutable freeze and exact temporary/final HTTP verification. Retired PID `1031421` served
those bytes at `http://100.94.63.83:8080/` through accepted human UAT. Approval descendant
`ece6c3c`, Pages run `32262792440` and artifact `9369119336` pass exact hosted-byte verification. The
former `05b8b3b` nomination is withdrawn and no longer served.

### M80-O9 - Explicit native line-line Fillet publishes ordinary Offset-ready topology

Start from exactly two distinct standalone, untrimmed native Profile Lines sharing one persistent
endpoint and one independently valid single-corner Fillet preview. **Apply computed** remains the
unchanged/default ADR 0031 computed-feature publication. **Apply native profile** authenticates the
exact accepted input, both parent identities, shared endpoint, radius, normal sides, retained
endpoints, parent/arc endpoint order, sweep and tangent orientations before allocating anything.
The sharp point has exactly those two direct curve owners and no other point-based dependent;
neither source has an existing Profile Offset or persisted computed-feature claim.

The coordinator prepares and retains the exact complete solved native patch beside the held
preview; the terminal action authenticates and consumes that patch without reconstruction. The
native proposal physically shortens the two parent lines, inserts one ordinary persistent
`CircularArc`, adds exactly two endpoint `LineCurveTangency` definitions and one driving Radius
dimension, then trials the complete document. Independently require finite regular geometry, exact
incidence/tangency/radius and branch predicates, and normalized hard residual at most `1e-9`.
Publish all changes in one retained transaction and one history step. Undo restores the exact
original shared corner while persistent-ID high-water remains monotonic; Redo restores the same
native identities and explicit branch state.

Feed the resulting native line-arc-line path, in both collection traversals and as a rounded face
corner, through the existing Profile Offset proposal and validator unchanged. An ordinary computed
Fillet arc and its discarded fragments remain rejected. Apply native profile creates no
`FilletSet` and leaves the computed-feature sidecar unchanged.
Polyline-owned, line-circle/other-curve, batched, dependent/high-valence, stale and already-
published computed-Fillet conversion attempts reject before allocation and preserve exact
document/scene/history/transcript/high-water state.

Status: implemented and closed; focused and broad amended qualification plus the F016 clean
replacement gate and frozen nomination pass at exact source `29d8e41`, tree `44ecb95`. Human UAT
and exact Pages publication pass.
The superseded `b83dad2` candidate contains no Apply native profile action and cannot satisfy this
scenario.

### M80-F001 - Computed Fillet fragments cannot impersonate native operands

A computed-Fillet discarded occurrence may carry a semantic native source-span ID for selection
and diagnostics, but Offset target resolution must also authenticate
`SceneCurveOrigin::Native`. The focused editor regression proves a computed occurrence receives no
hover/click authority and creates no operand or retained state.

Status: fixed and mechanically passing.

### M80-F002 - Bidirectional arc endpoint edits retain a deterministic gauge

Activate both source and target arc endpoint angles for Profile Offset. One and both hard target
endpoint drivers added after the association must propagate to the free source without a shared-
angle gauge or insertion-order dependence. Structured source Start/End `Preference` rows retain
only the free gauges; hard residual validation and priority semantics remain unchanged.

Status: fixed and mechanically passing.

### M80-F003 - Only exact endpoint contacts own Offset junctions

A supporting-line or interior contact placed at endpoint coordinates must not authenticate
adjacency. Both topology discovery and persistent validation require exact bounded `[0, 1]`
domain, winding `0`, matching Start/End neighborhood and the bit-exact endpoint scalar for every
contact-owned junction. Invalid ownership rejects before publication without mutation.

Status: fixed and mechanically passing.

### M80-F004 - Accepted point edits remain current for fresh Offset authoring

Successfully move an ordinary point, rebuild the exact accepted scene and immediately activate
Offset without refresh or another solve. The fresh topology index must authenticate the current
accepted publication even though one-shot point-edit guidance has been consumed, while retaining
the current prepared input as the exact proposal/preview CAS stamp. Face selection, preview and
Apply then complete through the normal coordinator lifecycle as one next history step.

Status: fixed and mechanically passing.

### M80-F005 - Persistent Offset supports must remain Profile geometry

Reject a Construction source or target before creating a Profile Offset. After creating a valid
association, attempting to change either side to Construction must preserve the complete document
and draft bytes. A draft-v5 payload that marks either retained support Construction must fail
strict restoration. One central document invariant owns all three entry paths; no adapter-only
filter may make invalid persistent state acceptable.

Status: fixed and mechanically passing.

### M80-F006 - Ambient junction degree does not make the selected chain branch

At a T-junction, explicitly select two connected incident spans that form one ordered traversal
and leave the third span unselected. The topology index continues to report the truthful global
degree, but collection and operation planning consider only selected continuations. The two-span
operand previews and applies with one persisted junction while the third curve remains ordinary,
unchanged and outside the association. Selecting all three incident arms reports a selected-set
branch; disconnected and closed selections keep their existing typed failures and no state mutates.

Status: fixed at the editor and operation-planning owners; focused and collateral mechanical
qualification and pre-amendment mechanical nomination pass. That candidate is superseded by the
native-Fillet amendment; F016 replacement clean qualification and immutable nomination now pass at
`29d8e41`; human UAT and exact Pages publication pass.

### M80-F007 - Provisional shared distance has a direct authoring gesture

After one face or chain produces a current provisional target, hover and press a target edge or
its grouped distance presentation. The pre-F007 path treated every provisional item as wholly
non-interactive, so pointer-down fell through to base operand collection and no gesture could
begin. The corrected headless owner authenticates the exact held preview and source/target pair,
captures one pointer and changes the positive distance only after the shared threshold. Absolute
sampling, current rerenders, invalid/valid recovery, cancellation, stale scenes and foreign
pointers preserve one last-valid candidate. Release is history-neutral; the later Apply publishes
that exact preview in one step. A full-circle grouped annotation samples the same displayed radial
side as its source-to-target dimension line, rather than the antipodal circle parameter. The
browser supplies only normalized events, capture and paint. A superseded rendered candidate that
retains the same provisional target IDs clears both operand and editor hover rather than
advertising a grab which the identical authenticated press rejects; the current rerender becomes
hoverable immediately.

Status: fixed at the headless interaction and thin browser owners; focused and collateral
mechanical qualification and pre-amendment nomination pass. That candidate is superseded by the
native-Fillet amendment; F016 replacement clean qualification and immutable nomination now pass at
`29d8e41`; human UAT and exact Pages publication pass.

### M80-F008 - A self-adjacent span is closed, not a one-edge open chain

Create a regular bounded circular arc whose exact Start and End contacts are joined by an ordinary
G0 endpoint-continuity constraint. Even though the curve family is non-periodic, its authenticated
topology is already closed. Offset chain collection must report `WouldCloseChain`, direct operation
planning must report `ProfileOffsetClosedChain`, and neither path may allocate a target or mutate
retained state. A genuinely open single arc remains eligible.

Status: fixed at the editor and operation-planning owners; focused mechanical qualification passes.
Pre-amendment mechanical nomination passes, but that candidate is superseded by the native-Fillet
amendment; F016 replacement clean qualification and immutable nomination now pass at `29d8e41`;
human UAT and exact Pages publication pass.

### M80-F009 - Delayed Offset effects cannot consume a newer distance drag

Start one provisional distance drag, retain its preview and terminal effects, finish it, then start
a second drag over the unchanged proposal. The two gestures intentionally share the same prepared
input, proposed commit and sampled distance, so those fields alone cannot authenticate authority.
Every preview, finish and restore effect carries a monotonic gesture epoch. Replaying the first
drag's preview, finish or restore during the second must return `OffsetPreviewMismatch` while
preserving the second state, visible payload, pointer capture and epoch. Cancelling the second drag
must emit its distinct epoch and restore its exact pointer-down state and payload.

Status: fixed at the retained editor/coordinator owner; focused mechanical qualification passes.
Pre-amendment mechanical nomination passes, but that candidate is superseded by the native-Fillet
amendment; F016 replacement clean qualification and immutable nomination now pass at `29d8e41`;
human UAT and exact Pages publication pass.

### M80-F010 - Native Fillet publication consumes the exact accepted preview

Create a line-line Fillet preview whose retained first-line seed points opposite the independently
accepted solved line while preserving the same explicit line branch. Native availability must
stage a complete independently accepted ordinary line-arc-line patch from that accepted preview;
Apply must consume that exact held patch without reconstruction. The durable result preserves all
pre-existing retained point bits, gives only new contacts branch-valid retained seeds, keeps both
tangent orientations explicit, and independently validates normalized hard residual at most
`1e-9`. Removing, superseding or making the held cache unavailable must reject without changing
document, accepted state, history, transcript or allocator high-water.

Status: fixed at the retained sketch session and coordinator preview-authority owners. Focused
accepted-versus-retained, cache/CAS, native topology and unchanged Offset chain/face regressions
pass; F016 replacement clean nomination passes at `29d8e41`; human UAT and exact Pages publication
pass.

### M80-F011 - Reverse manual line pick keeps canonical parents and preview branches aligned

Author the same eligible native line-line corner by manually picking the two line parents in
forward and reverse order. Both previews retain the same canonical parent order, but each computed
arc contact and tangent orientation must remain paired with its corresponding canonical parent.
The reverse path swaps those two contact/orientation entries when it canonicalizes the parents; it
does not reorder the arc or reconstruct a branch from coordinates. The owner regression
`reverse_line_pick_order_keeps_canonical_parents_and_preview_contacts_aligned` freezes this
evaluation contract. The grouped editor regression
`grouped_adjacent_authoring_is_canonical_and_keeps_corner_branches_on_radius_edit` requires native
requests from both pick orders and retains the corner branches through a Radius edit. No residual
equation, Jacobian, priority, persistence format or golden-row expansion changed.

Status: fixed at the computed-Fillet evaluation owner; focused owner and editor regressions pass.
F016 replacement clean qualification and frozen nomination pass at `29d8e41`; human UAT and exact
Pages publication pass.

### M80-F012 - Restored radius origin remains natively publishable without revision reuse

Prepare one eligible native line-line Fillet, capture its radius-gesture origin, preview a changed
radius and then cancel back to the exact pointer-down candidate. The temporary sample advances the
computed-evaluation allocator, but rollback must retain the original single-owner native sketch
patch. It renews only that patch's computed-scene parity and checkpoint from the current monotonic
allocator. Native availability and Apply must therefore agree on the restored preview, the
discarded evaluation revision must not be reused, and Apply must publish the visible origin as one
atomic history step.

`native_fillet_preparation_tracks_radius_refresh_and_exact_origin_restore` freezes the patch
identity, allocator and terminal-Apply contract. The companion exact-preview regression requires
the held rendered centre, contacts, radius, Start/End angles and sweep to match the staged accepted
native arc bit-for-bit. A parity-refresh failure remains a typed cached native-unavailable state
without invalidating the still-current computed preview or mutating durable sketch state.

Status: fixed at the retained coordinator preview-authority owner; focused regression passes.
F016 replacement clean qualification and frozen nomination pass at `29d8e41`; human UAT and exact
Pages publication pass.

### M80-F013 - Rejected preview replacement preserves the live allocator and native action

Prepare one eligible native line-line Fillet preview and confirm Apply native profile is
available. Without clearing it, attempt to replace it with a grouped preview whose shared-span
trims cross and must reject. The rejected replacement must not advance the live computed-
evaluation allocator. The exact prior preview, native prepared patch and metadata remain held;
availability and Apply must still agree, and Apply publishes that visible prior corner atomically.

`rejected_feature_preview_replacement_keeps_last_valid_native_apply_authoritative` freezes the
retained coordinator boundary. Computed evaluation uses a candidate allocator and publishes its
high-water only after a Current replacement succeeds, so failure cannot silently stale retained
authority.

Status: fixed and mechanically passing through the clean F016 replacement `29d8e41` nomination;
human UAT and exact Pages publication pass.

### M80-F014 - Native dependency refusal is concise and identity-free

At an otherwise valid two-line Fillet corner, add either another point-based dependent or a third
incident line at the sharp point. Computed preview remains available, but Apply native profile is
disabled with exactly `shared corner must be owned only by the two selected source lines`. The
reason must contain no persistent ID, `InvalidField` wrapper or generic document-error wording,
and the refusal allocates or mutates nothing.

The sketch-domain invalid/ineligible matrix freezes the dependency and high-valence cases. The
coordinator regression `native_fillet_high_valence_disabled_reason_omits_document_error_boilerplate`
and the demo presentation regression require the same exact sentence across the adapter.

Status: fixed and mechanically passing through the clean F016 replacement `29d8e41` nomination;
human UAT and exact Pages publication pass.

### M80-F015 - Native preparation itself obeys cooperative work control

Start native preparation with zero document-validation work. It must return typed WorkExhausted
before completing the preparation trial, leaving the source document bit-for-bit unchanged and
publishing no expected identities or patch. The unlimited convenience path remains behaviorally
equivalent for ordinary callers, while the coordinator maps incomplete bounded preparation to
`NativeFilletWorkStopped` and retains all visible/durable authority.

`controlled_native_fillet_preparation_exhaustion_is_state_neutral` exercises exhaustion during
preparation and during the subsequent prepared job. Final trial validation shares the same
controller; no complete uncontrolled validation may run before bounded work begins.

Status: fixed and mechanically passing through the clean F016 replacement `29d8e41` nomination;
human UAT and exact Pages publication pass.

### M80-F016 - Ordinary line-arc tangency activates circular-arc endpoint angles

Two supplied native-Fillet reproductions are frozen by identities
`GEOSOLVE_REPRO_V1:12083:cf25674611a32202` and
`GEOSOLVE_REPRO_V1:11441:60d3d06bea383818`. For the first, drag the native arc centre diagonally:
both parent line directions and both circular-arc Start/End angles must change while the complete
accepted scene remains finite and visible. For the second, fix the arc centre, leave an eligible
remote parent endpoint free and drag that endpoint off-axis: it must reach the requested 2D target,
change the line direction and change the corresponding arc endpoint angle rather than permitting
only line-length motion. An explicit Horizontal/Vertical line constraint still owns orientation.

The compiler must include both persistent circular-arc Start/End scalars whenever
`LineCurveTangency` references that arc. Incidence exposes two angle variables and seven variables
for the line-arc tangency row set, with central finite-difference agreement. Both interaction
regressions require finite independently validated normalized hard residual at most `1e-9`. The
native Fillet remains ordinary shortened lines, one circular arc, two tangencies and Radius; no new
residual, Fillet-specific relation, inferred branch or persistence format is allowed.

Status: fixed and mechanically passing through replacement clean source `29d8e41`, tree `44ecb95`,
and its exact Tailscale nomination; human UAT and exact Pages publication pass. The former
`05b8b3b` nomination is withdrawn from current UAT.

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
covers the motion it can control; deterministic point anchors cover only remaining
point-observable passive mobility. A nullspace direction that changes only scalar curve/contact
state and no persistent point neither admits nor requires an anchor. Anchor targets are the
accepted visible positions at gesture start. The selected cursor
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
| Pantograph | Drive input, guide, output and center independently, including natural off-manifold guide targets. A point whose active rank covers all point-observable hard mobility needs no anchor; otherwise deterministic anchors cover only point-observable passive mobility. Rank-one `2 x 2` cursor projection must be stationarity- and minimum-norm-certified under the authoritative rank cutoff. |
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

## M81 architecture-preservation scenarios

M81 adds no geometry/authoring scenario or golden row. It qualifies the existing corpus through
the same public boundaries while private implementation ownership changes, and adds one focused
coordinator transactional regression:

- core dense/sparse, bounds, priority, continuation, cache, rank, conflict/redundancy, diagnostics,
  audit and cancellation scenarios preserve status, source/trace ordering and independent returned-
  row validation;
- sketch canonical persistence, curve-control/conic queries and all Profile Offset equation,
  audit, topology, native-Fillet and source/target-edit scenarios preserve exact behavior;
- feature Fillet composition scenarios preserve endpoint-claim conflicts, opposite-end source
  trims, discarded Construction fragments, branch continuation and revision-local output identity;
- editor history/restore, computed authoring/drag/action, native Fillet, Profile Offset, Undo/Redo,
  workspace/reproduction and accepted-scene-authority scenarios preserve their existing output;
  and
- focused `m81_f001_rejected_computed_feature_mutation_is_allocator_neutral` supplies the only new
  exact case. It forces bounded durable feature mutation rejection through the public retained
  coordinator and directly preserves feature identity/payload, retained design and accepted
  identities/JSON, history length/cursor, transcript, computed input/absent-snapshot/problem state
  and computed-output allocator high-water. It does not construct or compare an `EditorScene` or
  the complete retained session/checkpoint state.

The stable authoring/scene golden remains exactly 271 rows. M81-F001 is an isolated owner
regression rather than a new systemic family/lifecycle dimension, so no golden input or authority
byte changes. Exact source `e4eca327fc69c92f95b1722142289302ba4f67bc` passes the clean release
gate and its no-rebuild seven-file snapshot is byte-verified at the retained Tailscale endpoint;
the supervising caller accepts the qualified behavior-preservation scorecard and requests closure
without opening a new finding. Approval descendant `b582b82` passes Pages run `32328472125`,
artifact `9392295853` and exact hosted-byte verification; the Tailscale listener is retired and M81
is closed.

## M82 withdrawal and preservation scenarios

M82 is closed by explicit deferral and adds no accepted geometry scenario. Its closeout requires:

- rollback commit `fa54f30` has exactly the M81 closeout tree
  `17b2eeab0eda39e19df81d3cf3e505ceac274825`;
- M80 native Line/Circle/CircularArc Profile Offset, native-published Fillet topology and every M81
  architecture-preservation scenario still pass the complete release gate;
- the authoring/scene golden is exactly the restored 271-row fixture, with no computed Curve Offset
  family, proxy or lifecycle rows on `main`;
- no computed Offset feature definition, v2 persistence, inverse proxy, UI route or M82 fixture is
  reachable from the supported product; and
- local and remote branch `archive/m82-certified-computed-offset-2026-08-21` points to `d1e2613`
  and retains the complete unaccepted design for future study.

The rejected frozen candidate and its seven M82 findings are historical archive evidence only.
Former UAT PID `3024723` was retired and port `8080` was free at M82 closeout. No M82 feature UAT or
Pages publication is claimed; existing accepted M81 Pages bytes remain public authority. See
`docs/M82_DEFERRED.md`.

## M83 projectional design-intent fixtures

M83 adds no residual equation and does not reinterpret any earlier geometric oracle. The corpus
qualifies semantic ownership, deterministic lowering and accepted-scene authority around the
existing solver. G1-G6 implementation and focused/proportional coverage are complete through
M83-F001 through M83-F010. Initial nomination `232b83a` plus sources `a621cdd`, `fafea4e`,
`1e70f3f` and `b0de5af` are superseded historical evidence. F010 source
`ee18dbda89b6973ac54baea3ac0e0dbbd126ca59`, tree
`889f730e033ce5c728fddbac345263d8c26b8b93`, passes clean replacement qualification and immutable
Tailscale nomination. The supervising user's 2026-08-25 milestone-level approval accepts the
U1-U10/F001-F010 disposition without claiming a separate row-by-row replay. Approval descendant
`2006c86`, Pages run `32817232564`, exact hosted-byte verification and service retirement pass;
Pages is final public-byte authority and M83 is closed.

### M83-G1 — schema, alias and order independence

Enumerate all 25 M78 geometry recipes, every current persistent relation/dimension, every M58
operation, computed `FilletSet`, parameter/binding/output and external-reference declaration.
For each node, compare its Rust-generated logical ports, writable leaves, children and typed
native reservations with the reviewed catalog. Existing-point operands must alias the exact stable
point port and allocate no point reservation. Constraint/source and dimension/source reservations
are contiguous owner-first pairs. Curve handles which are not persistent `DesignPointId` values
remain logical-only.

Every canonical input slot/reference appears in Structured Source and Inspector as the exact
stable typed port. Rebinding switches both projections from the old port to the new port without
an editable Inspector input control.

Enumerate the concrete descriptors for all 109 declaration families, every input-choice branch,
maximum Polyline/NURBS/Fillet/LinearPattern children and sparse/mixed operation outputs. Each
canonical input, definition field, stable output and writable leaf has one bounded semantic path.
Inputs, definitions and instance leaves admit neither duplicate/prefix coordinates nor a location
that is an object in one path and an array in another. Fixed roles project as named members;
repeated operands, children, contacts and results project as real numeric arrays, including stable
`null` holes for sparse indices. Padded slot/field/selector strings remain internal authority and
do not appear as Structured Source keys, Inspector labels/breadcrumbs or projected references.

Create the same dependency graph from several patch-array, alias-name, cell, declaration and
source-display orders. Canonical dependency scheduling, semantic identity, native reservations,
materialized sketch bytes, accepted geometry, branch state, source order, rank and DOF agree.
Only organization identity changes under cell/name/order edits.

### M83-G2 — stable identity, continuation and tombstones

Create a shared-point polyline, a constraint, a dimension, a computed Fillet and a Profile Offset.
Exercise alias, identity continuation and exact dependent-closure deletion. Stable ports survive
instance edits and organization moves. Retired native identities produce typed never-reused
tombstones; divergent work after Undo allocates above prior high-water. Undo/Redo restores semantic
owners and accepted geometry without lowering any node/port/child/reservation/native cursor.

Malformed ports, wrong kinds, cycles, identity forks, use-after-retire, stale CAS, incomplete
cascades, duplicate unordered targets and resource exhaustion reject without changing graph,
instance, organization, external inputs, accepted evidence, allocator or history.

### M83-G3 — accepted, retained-failed and cold authority

Materialize representative affine, circular, conic, Bezier, spline/NURBS, native-Fillet and
Profile-Offset graphs under ordinary, parameter-bound and external-reference inputs. Every success
has finite geometry, `HardValidity::Valid`, independent normalized hard residual at most `1e-9`,
the exact persisted branch/domain state and a complete logical/native ownership map. A canonical
cold reconstruction agrees with the warm accepted result on document semantics, accepted
measurements, rank/DOF, diagnostics and ownership.

Submit structurally valid but incompatible explicit intent with retain-failure policy. The graph
and typed failed-node diagnostic advance, while canvas authority remains the exact previous
accepted materialization and cannot be rebound as current inference publication authority.
Cancellation, work exhaustion and stale output retain both intent and accepted authority exactly.
Cold reload of retained-invalid migrated/bootstrap v8 intent reconstructs that authenticated prior
accepted canvas while preserving the current failed graph, history and exact Undo; corrupt or
mismatched accepted evidence rejects.

### M83-G4 — free-leaf drag and exact release

For a free point, constrained line endpoint, circular/conic control, Fillet radius and Profile
Offset distance, prepare one route from exact accepted semantic/native ownership. Ordinary point
movement may write only the genuinely free instance leaves returned by that route; a fixed target,
driving dimension and explicit branch remain bit-exact. Dedicated Fillet/Offset gestures edit only
their existing property owner.

Queue several pointer samples around one animation frame, accept one preview, then include a newer
invalid target and release. Pointer frames perform no intent/session JSON encoding, workspace save,
durable panel rebuild or history mutation. Attempted and accepted identities remain distinct;
pointer-up commits the newest visible authenticated accepted preview once, exact-coordinate release
reuses it, and delayed/foreign/duplicate capture terminals are inert. Suppressed-output bindings
and retained-invalid current intent cannot start a route. One Undo restores the exact pre-gesture
graph/instance/accepted state; cancellation without an accepted sample publishes nothing.

Create a two-point aligned rectangle, then create a Segment whose endpoints alias opposite
rectangle corners. Drag the shared corner through six successive accepted targets. Each release
must commit the visible preview, retain finite geometry and independently validated hard residuals,
add exactly one history entry, preserve the Segment's explicit branch bit-for-bit and return to the
origin after six Undo operations. Preview/cold comparison may canonicalize only recomputable
Polyline and four-rectangle-recipe line branches, and only when preview/cold vectors have positive
dot product. A flip, non-finite vector, unrelated draft-v5 difference, Segment branch or Midpoint
Line branch must reject rather than be normalized. Cold Midpoint Line materialization must preserve
its explicit stored branch even when its coordinates imply another finite direction.

### M83-G5 — source, panel and unified history

Create equivalent typed patches through canvas, Inspector, structured-source token edit and
DOM-free RPC. The target semantic identities and outcomes agree. Rename/reorder cells and source
rows, including drag/drop stress after dependent geometry exists; materialized document and
accepted evidence stay exact. A structural source edit follows typed schema validation, while
arbitrary syntax/execution is unavailable.

Drop declarations and cells on upper/lower target halves for adjacent, non-adjacent and end moves;
these resolve to explicit before/after slots and exact organization-only Undo. Retain a numeric
source token, reorder so that ID would name a different owner, and submit the old token with its old
session identity: exact CAS rejects before lookup with source, history and accepted evidence
unchanged. Source and Inspector expose identical stable input bindings after rebind, and Inspector
bindings remain read-only.

Project Segment start/end, sparse Polyline vertices, NURBS controls/weights, aggregate spans and a
two-corner Fillet. Structured Source uses nested `inputs`, `definition` and `instance` objects and
arrays in schema/CAD order. Inspector groups the same paths as named fieldsets and one-based human
array-item labels while retaining zero-based typed path indices. A malformed, empty, index-rooted,
oversized, duplicate, prefix-colliding or object/array-conflicting decoded path rejects before it
can become presentation or mutation authority. Inspector edits also authenticate the exact
identity stamped on their own projection.

The `Outline | Structured source | History` projection selects the same stable declaration.
Inspector fields are schema-derived, retained invalid intent stays inspectable and History is
read-only. A mixed sequence across all projections produces exactly one bounded composite
Undo/Redo stream with no mirrored coordinator entry.

Canvas/tree selection resolves through accepted logical/native ownership to that same visible
declaration. Ambiguous, unowned, protected-datum, multi-owner or mixed-owner selections clear the
declaration target; Outline/source selection clears competing native selection. Toolbar and
Delete/Backspace share one editor deletion route after authoring precedence. A private one-consumer
Profile Offset aggregate is grouped beneath its visible operation, cannot be selected or reordered
as an invisible source row, and is removed with that operation even when explicit intent is
retained-invalid over older accepted ownership. Recognized source tokens remain typed edit targets.

Populate geometry and History, enter an authoring tool, then invoke New. The action is enabled,
creates the same canonical empty projectional authority as startup, clears accepted/design geometry,
intent Undo/Redo and transient authoring/feature/offset/pointer/outline/problem state, selects
Select, resets the camera and autosaves workspace v8. Reload must remain projectional and empty;
the retired flat event path is not an allowed fallback.

### M83-G6 — workspace, WASM and TypeScript parity

Workspace v8 round-trips current/accepted graph and instance identities, organization, external
inputs, reservation/tombstone high-water and bounded history byte-identically. Abandoned version 7
rejects. Every frozen v1-v6 workspace restores through its historical strict decoder and becomes
typed per-object bootstrap declarations whose ports bind the exact existing point/scalar/curve/
contact/constraint/dimension/source identities; no aggregate flat peer authority, recipe grouping
or ownership is invented. Explicit supported ejection preserves accepted sketch semantics.

For a migrated/bootstrap v8 session whose current intent is retained-invalid, reload derives
accepted ownership from the authenticated accepted graph/bootstrap prefix and retains current
failure/history/Undo. Exact session identity is carried consistently through native, WASM/RPC and
branded TypeScript source-token requests.

Native and WASM/RPC transition transcripts match for valid, retained-invalid, stale, malformed and
resource-exhausted patches. Branded TypeScript builders reject cross-session or wrong-port kinds at
compile time where representable and at runtime otherwise. Neither generated source nor the
package contains a curve equation, residual evaluator, expression graph or solver.

The optional M76 annotation-layout cache is not an intent-session component. It may survive an
ordinary compatible workspace round trip solely as presentation state, is omitted/ignored by
reproduction authority, never affects materialization or Undo/Redo and is discarded so automatic
placement can be recomputed when missing, stale or malformed.

### Post-F007 architecture hardening

This automation-owned architecture qualification slice predates and is separate from the later
M83-F008/F009 product findings. It changes no solver equation, branch or product interaction
contract.

Round-trip canonical graph/session wire v2 and compare SHA-256 identities. Rewrite authentic v2
fixtures into canonical legacy v1, including accepted and retained-failed current/Undo/Redo states,
then require exact validated one-way migration back to v2. Tamper with nested graph, instance,
reservation, host input, accepted evidence and cached identity fields while recomputing public
outer digests; each import rejects. Repeated ordinary identity queries perform no retained-history
rehash.

Construct genuine Undo and Redo stacks, then independently reauthenticate checkpoint-body
permutations, crossed or duplicate descriptor revisions and descriptors attached to another body.
Import rejects causal-edge, chronology or descriptor/body inconsistency. Genuine full traversal,
bounded-history eviction, divergent editing and canonical reload remain executable and preserve
allocator/reservation tombstones.

Enumerate all 109 declarations and maximum child/operation-output variants. Allocation-free port
counts equal actual allocation; hostile invalid shapes reject before expansion. Project every
central descriptor through Inspector, graph Snapshot and TypeScript. Large bootstrap payloads must
appear only as kind, codec, byte length and SHA-256; stable ports, children and operation spans must
remain complete.

Exercise structured and JSON RPC at boundaries below, exactly at and above the 16 MiB mutation-
receipt and 64 MiB response limits. Oversized success becomes one bounded `response_too_large`
failure and leaves session/native authority unchanged. Rust and TypeScript accept all graph shapes
inside the Rust catalog, reject unknown/method-confused/cross-session shapes and preserve integer
fidelity. Native and WASM transcripts remain exact.

For computed Fillet/Profile Offset Apply and radius/distance drop, prepare one accepted transaction,
publish it exactly once, and prove no second patch plan occurs. Intervening accepted mutation makes
the prepared object stale and atomically preserves newer intent/native authority.

Feed workspace decode a 64 MiB-plus-one hostile payload and require rejection before JSON parsing.
Within the bound, wide non-string annotation cache trees are consumed without allocation into a
generic JSON `Value` and discarded. Annotation-layout strings above 4 MiB are discarded only after
authenticated-v8 digest/canonical validation, and before cloning for flat legacy workspaces.
Canonical v8, legacy-v8 migration, retained-invalid accepted authority and all v1-v6 bootstrap
paths continue to pass. The complete demo library must pass on the ordinary test stack; no
`RUST_MIN_STACK` override may hide a multi-fixture test overflow.

The superseded post-F007 no-rebuild release output is frozen at
`/tmp/geosolve-m83-f007-uat.52r7H7`, aggregate
`bc04955f52ac14f3eba96637b23210772ab339e1a2f3ac60be59558bcb4c5973`. The existing 3/3 frozen
browser suite and focused F006/F007 2/2 suite pass on temporary and retained endpoints. Both served-
byte ledgers have SHA-256
`9573901313adf09b23e93e27857639fa7bf96eb969121a6f22277cadb275b9ec`. Historical retained service
PID `4006665` is retired; its immutable snapshot remains historical evidence.

Source `1e70f3f4dc6778881ce180b2922235a6cc103cf7`, tree
`77251dbe393cd57b9d036e9611f5a8aaa192f5ee`, and its post-hardening snapshot remain superseded
historical evidence after F008/F009. The superseded F008/F009 source
`b0de5af55a8c9fe3550137cda91dae63c87666b1`, tree
`ff0b29dee074bc67a136c23feb5ee56c99deeba1`, passes the fresh clean gate. Its exact no-rebuild
snapshot `/tmp/geosolve-m83-f008-f009-uat.zLfB22EK` has aggregate
`f2092e54b1b014618dcdded21e3bc0907a280fc15aa93b0c18913cf87d9b30d6`; the existing 3/3, focused
F006/F007 2/2 and focused F008/F009 2/2 suites pass locally, and both focused suites pass on
temporary and retained Tailscale listeners. Both temporary/final eight-path byte ledgers have
SHA-256 `b5bef9cc6274258c217f5edf44c7a6ed06b7299c524f5f0ed3ea4aa64d8866b4`. Those historical bytes
were retired with PID `3376452` only after F010 replacement verification; the immutable snapshot
remains preserved. Accepted F010 authority is the exact seven-file no-rebuild snapshot
`/tmp/geosolve-m83-f010-uat.Qmrz2R36`, aggregate
`e01d642438ae8337e9abe1ddeadb7b176375ae40f8b411785edae717e30b5d54`, formerly served at
`http://100.94.63.83:8080/` by retired PID `276377`. Its 9/9 browser set passes locally, on a
temporary Tailscale listener and on the retained listener; both temporary/final HTTP ledgers have
SHA-256
`9914a99483df9dee2059a1e0eabf173c8055af6a9173c2a7e5288e31ffeef10b`.
Approval descendant `2006c86b936c3522cc48fbf26cf78664d5e31e90`, tree `c4a59d2`, passes Pages
run `32817232564`. Downloaded artifact SHA-256 is
`06bce15ddea6d21048a25e3630a368ebe0ba883be98ee296869f77c47b86218b`, its seven-file aggregate is
`75234fd6ff4349e4b75c858b90e90630002a7dd9b9171e47dfe28bc253cf23fc`, and hosted-results SHA-256
is `bb7423477868aafc7752b766ea2f6fb5461e1d31846dd14f9ebafad7ede42ace`. PID `276377` is retired,
the endpoint refuses connections and the snapshot remains. `docs/M83_UAT.md` owns the accepted
scorecard and exact closeout evidence.

### M83-F001 — deterministic accepted drag identity and exact-once terminal capture

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
A rejected newer sample may not erase the newest visible accepted preview, an exact release must
reuse that accepted identity, and pointer-up/cancel/capture-loss may retire one capture only once.
Suppressed outputs and retained-invalid current intent expose no direct-manipulation route.
Coordinator, editor, browser-adapter and frozen Playwright regressions own the contract.

### M83-F002 — Outline and cell before/after insertion semantics

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Upper/lower drop halves resolve to before/after slots after excluding the moving identity.
Adjacent/end moves are visible; self, stale and cross-cell targets reject; accepted geometry and
semantic identity are unchanged; Undo restores exact organization.

### M83-F003 — retained-invalid migrated/bootstrap reload authority

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Workspace v8 cold restore retains the newer failed intent and its history while reconstructing the
exact prior accepted canvas from authenticated accepted bootstrap authority. Undo repairs it;
corrupt or mismatched evidence rejects rather than blanking or inventing geometry.

### M83-F004 — exact-CAS Structured Source token edits

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Source token requests carry exact `IntentSessionIdentity` through browser, Rust RPC and TypeScript.
A token kept across reorder rejects before numeric lookup and cannot mutate the declaration that
newly occupies its old ID; source, history and accepted evidence remain exact.

### M83-F005 — stable input binding projection

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Structured Source and Inspector deterministically show every canonical input slot and exact stable
typed port. Rebind replaces the old reference in both projections. Inspector exposes no input,
button, `contenteditable` or typed edit marker for these references.

### M83-F006 — shared recipe drags commit deterministic preview/cold parity

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Reproduce by drawing a rectangle and a diagonal Segment which aliases two rectangle corners, then
repeatedly dragging a shared corner. Before repair, each retained preview solved correctly but
pointer-up could return `PreviewColdMismatch` and snap back: native continuation retained an edge's
prior derived branch vector while cold rectangle materialization recomputed an equivalent vector
with floating-point noise.

The coordinator first preserves exact document equality, then permits a field-local canonical
comparison only for Polyline and the four rectangle recipes. Every owned line branch must remain
finite and in the same positive cell; only that recomputable metadata is substituted before exact
draft-v5 equality. Segment and Midpoint Line are deliberately outside this set, unrelated state
cannot be hidden, and an actual branch flip rejects. Midpoint Line cold lowering separately honors
its explicit `branch_direction`. Unit regressions cover the closed recipe inventory, same-cell
canonicalization, flips and unrelated differences; the owning projectional regression covers six
accepted drops, exact history/Undo, residual validation and explicit diagonal branch preservation.
No equation, residual, priority or tolerance changes.

### M83-F007 — New creates a fresh projectional workspace

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
The projectional availability map had disabled New and supplied no event route, making it difficult
to reset a populated sketch. Startup and New now share one canonical empty projectional authority.
New clears durable authored geometry/declarations/history and transient interaction/authoring
state, returns to Select, resets the camera and autosaves workspace v8. Native adapter tests prove
the empty accepted and design documents, empty Undo/Redo, projectional v8 round trip, enabled
action and durable event route. The frozen browser scenario additionally proves that both a
populated scene and its History clear and remain empty after reload.

### M83-F008 — complete projectional reproduction transport

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval. On a fresh
projectional workspace, create a rectangle and a shared diagonal so both accepted geometry and
unified intent history are populated. Copy repro must be enabled and emit one bounded
`GEOSOLVE_REPRO_V1` payload whose decoded workspace is v8, whose current/accepted intent authority
and Undo history are complete, and whose disposable annotation cache is absent. Clipboard denial
or an insecure origin must leave the entire payload selected for manual copy.

Close the dialog, invoke New and load the saved payload. Decode, workspace validation and
projectional reconstruction must all complete before live authority changes; the exact scene,
stable IDs, current history and deterministic reproduction payload return. Change the payload
checksum and attempt another load. It must reject visibly with the dialog still open and leave the
complete live authority unchanged. Overlay close/Escape restores focus ownership, and a late
clipboard completion from an earlier request cannot overwrite newer text or state.

The Rust persistence regression owns exact bounded v8 authority/history round trip. Thin adapter
coverage owns enabled controls and complete routes; the frozen browser scenario owns Copy-New-Load
round trip and atomic corrupt-payload rejection. No new solver or persistence language is added.

### M83-F009 — suppressed Fillet parents and frame-local scene failure

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval. Create a
rectangle, apply a computed Fillet to one corner, then suppress that declaration. The suppressed
feature and corner identities remain editable and its evaluation becomes typed Suppressed, but it
publishes no computed edge or radius affordance. Every finite native parent curve is visible; cold
restore agrees; Undo restores the same active feature/corner and Redo restores the same suppressed
state. Independent accepted-state validation must pass throughout.

Scene composition must distinguish a legitimate history position with no accepted authority from
an actual composition error. The first produces `data-scene-state="empty"` without inventing an
error. The second produces `data-scene-state="unavailable"` and a visible frame-local `Canvas scene
unavailable: ...` message instead of disappearing through `.ok()`. A valid next frame returns to
`ready` and restores the durable workbench notice, so no stale global error persists.

The projectional Fillet owner regression covers suppression, restore and Undo/Redo. The demo owner
test covers empty/unavailable/ready status semantics, and the frozen browser scenario covers the
reported rectangle-Fillet suppression path without blank geometry or stale canvas failure.

### M83-F010 — semantic fields and genuine arrays

Status: repaired, clean-qualified and accepted under the 2026-08-25 milestone-level approval.
Reproduce by opening Structured Source or Inspector for a repeated declaration such as a computed
Fillet or NURBS. Before repair the presentation exposed canonical storage coordinates such as
`point:0000`, `child:0001` and
`corner_0000_first_parameter` as if they were user-authored field names.

`IntentProjectionPath` is now the one bounded semantic presentation coordinate. The central Rust
descriptor maps it bijectively back to canonical `InputSlot`, `IntentFieldKey`, stable output and
`LeafRef`; those exact canonical values continue to own persistence, validation and patches.
Structured Source and Inspector build nested ordered trees from the descriptor, references use a
declaration symbol plus semantic output path, and TypeScript parses the same recursive object/
array shapes. Closed enum definitions render as selects with schema defaults; Inspector uses its
own exact session identity and rejects a stale projection.

The owning exhaustive oracle covers 109 declarations, input-choice variants and maximum/sparse
shapes. Adapter fixtures cover sparse Polyline inputs and writable vertices, NURBS controls,
aggregate spans, two-corner Fillet fields/parents and array `null` holes. Rust and TypeScript
decoders independently reject malformed paths and tree ambiguity. No graph identity, persistence
wire, patch vocabulary, materialization, solver equation, Jacobian, priority, tolerance or branch
rule changes.

Clean product source `ee18dbda89b6973ac54baea3ac0e0dbbd126ca59`, tree
`889f730e033ce5c728fddbac345263d8c26b8b93`, passes the complete release gate. The 5,683-line,
385,323-byte log has SHA-256
`71495557ca5638000cfec265d9b97c0b0e71c72d3cbdfbbd09dbea8518484f9a`. Its exact no-rebuild
seven-file snapshot `/tmp/geosolve-m83-f010-uat.Qmrz2R36` is frozen at directory/file modes
`0555`/`0444` with aggregate
`e01d642438ae8337e9abe1ddeadb7b176375ae40f8b411785edae717e30b5d54`. The same 9/9 browser
checks pass locally, on temporary Tailscale and on retained PID `276377`; both served-byte ledgers
have SHA-256 `9914a99483df9dee2059a1e0eabf173c8055af6a9173c2a7e5288e31ffeef10b`.
This mechanical qualification plus the milestone-level acceptance and exact Pages closeout above
complete the F010 disposition without claiming a separate row-by-row replay.

## M84 optional code/GUI authoring fixtures

M84 adds no residual equation and does not reinterpret the 271-row milestone-neutral golden. Its
separate reviewed ledger owns managed-source parsing, data-artifact expansion, typed references,
keyed reconciliation, unified history and code-project persistence. ADR 0041 and
`docs/M84_GOALS.md` are authoritative. The F001-F004 fixtures below are historical implemented
coverage. The former complete clean candidate and clean-qualified F003/F004 local/Tailscale
nominations remain withdrawn historical evidence. M84-F005 also withdraws the direct-authoring
`41e65a4` nomination. Collaborative overlay and semantic interaction authority plus F006 audit
hardening are implemented and focused-qualified. F007 then withdraws combined source `ff2e142` and
its frozen candidate after reproducing false terminal conflicts on multi-frame producer drags.
F007 source `cc2f05e`, tree `6b8fc41`, is historical mechanical evidence. The eight-demo creative-
catalog amendment and M84-F008/F009 corrections were clean-qualified and frozen at source
`c74651c`, tree `a904584`, snapshot `/tmp/geosolve-m84-f009-uat.q8cKIN3v`, but M84-F010 withdraws
that nomination after a Compass Rose center release durably selected another valid solution.
Coupled semantic-terminal durability is implemented, clean-qualified and frozen without rebuild at
source `cf463838`, tree `992e587`, snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz`. M84-F011
withdraws that nomination. Exact F011 source `e28721a`, tree `0152097`, snapshot
`/tmp/geosolve-m84-f011-uat.ps736NLh` passes replacement clean qualification, immutable freeze,
exact temporary/retained byte verification and focused frozen manifold/PNG/authority 1/1 on both
endpoints. M84-F012 withdraws F011 from current nomination while annotation paint/pick visibility
and WYSIWYG provisional-clean export become the clean-qualified immutable replacement. Exact F012
source `84dd768`, tree `429ed56`, snapshot `/tmp/geosolve-m84-f012-uat.nMOymIIM` passes exact
temporary/retained byte verification and focused browser checks 1/1 on both endpoints. F011 remains
historical rollback evidence. U1-U16 pass under milestone-level approval without claiming a
separate row-by-row replay. Approval descendant `e6e960d`, Pages run `33068058169`, artifact
`9644770095`, exact hosted-byte verification and service retirement pass. M84 is closed and Pages
is final M84 public-byte authority.

### M84-G1 — optional dependency boundary

Build/test core, sketch, linkage, intent and constraint-editor without `geosolve-sketch-code` and
prove no reverse dependency or code/module/parser type appears. Separately compose the optional
crate and `@geosolve/sketch-code`. Both paths must use the same public intent/materializer audit
authority; plain M83 workspace-v8 transcripts remain unchanged.

### M84-G2 — managed source and artifact authority

Round-trip the complete accepted managed-v1 CST subset with comments/formatting byte-identical,
then perform exact authenticated declaration, organization, lens and override rewrites. Reject
unsupported syntax, forward/duplicate symbols, stale spans and files over 4 MiB without canonical
mutation. Compile custom helpers only in the caller-owned Node step; validate artifact source/
interface/ABI digests, existing-family templates and 16 MiB bound. Rust/WASM/browser/load must
consume only the canonical artifact and expose no TypeScript execution path.

### M84-G3 — typed semantic results

Type-check named rectangle corners/edges/profile, a Fillet record keyed exactly by
`lowerLeft | upperRight`, and a derived Fillet collection keyed by Polyline corners. Compile-fail
raw IDs, misspelled paths, cross-project refs and point/curve/corner mismatch. Generated
TypeScript result descriptors and the central Rust declaration catalog must agree exactly.
Direct Polyline `vertices` and `segments` must lower as keyed root collections beside the existing
member paths and `filletableCorners`: each authored vertex key maps to a Point port and each
directed span's starting key maps to a CurveSpan port. Return those roots directly and consume
`vertices` through one recorded artifact `each`; neither path may copy coordinates, expose raw IDs
or derive identity from ordinal position.

### M84-G4 — adaptive keyed reconciliation

Start **Rounded polyline · dynamic corners** with six keyed vertices, five spans and four Fillets.
Insert `crest` and require seven/six/five; reorder, remove, reinsert outside Undo and Undo/Redo.
Unchanged invocation/template/member/output paths retain every logical/native identity. Allocation
advances only for new keys, removal tombstones, retired-key reuse gets a new generation and an
outside dependent blocks deletion without cascade or retarget. Open/closed mode, radius lens,
point override/reset and impossible-radius retained failure remain deterministic.

### M84-G5 — one transaction and retained interaction

Mix managed Apply, canvas/Inspector edit, organization, lens, override/reset and adaptive
cardinality changes. Each accepted action produces one code-session history row containing one
nested delegated editor checkpoint; Undo/Redo restores source/artifacts/program/expansion/
overrides/accepted scene together. Pointer-frame instrumentation must show zero parsing, expansion,
project serialization or durable-panel rebuild. Exact release commits the newest authenticated
accepted preview once after cold parity; existing M83 frame/terminal ceilings remain green.

### M84-G6 — genuine demos and offline persistence

The original F009/F010 catalog qualified these eight code-project sessions rather than equivalent
flat imports:

1. adaptive rounded Polyline with keyed corner Fillets;
2. typed aligned panel with mapped named Fillets and compile-fail cases;
3. GUI rectangle → `crossBrace(frame)` → ordinary GUI dimension/constraint on
   `brace.diagonals.rising`;
4. reusable AI-authored mounting plate with rounded profile and keyed `nw/ne/se/sw` holes.
5. adaptive Lantern Garland with one keyed bulb per Polyline vertex and one keyed Fillet per
   interior corner;
6. typed Suspension Bridge with native deck/tower producers and generated cables/stays;
7. Compass Rose with native shared-centre spokes, ordinary axis relations and generated
   ring/markers;
8. artifact-free Neon Manifold with connected native spans and one branch-explicit two-corner
   direct FilletSet.

The mounting helper remains byte-identical after GUI edits. Save/reload/repro restores every file,
artifact, lock, expansion provenance, override, nested accepted intent and unified history
offline. Missing/tampered artifacts, retained-invalid geometry, corrupt payloads and the 64 MiB
project boundary reject atomically while preserving the previous accepted scene.

### M84-G6 extension — direct code-authored starter

On an exact canonical fresh workspace, open Code and require exactly one **Start from code**
action plus all nine genuine project cards. Fresh classification requires only the canonical
document foundation, exact current/accepted semantic-identity parity and independently validated
empty native/computed authority. Starting must install a distinct artifact-free `Authored` project
through public `CodeProject::managed_only(ProjectKey, source)`, not promote or fabricate an
ordinary GUI scene. Every card must open fitted finite visible accepted geometry. Invalid project brands, malformed managed source and custom patch imports
without pinned artifacts reject before installation; parsing alone grants no solver authority.

The complete editable starter declares a rectangle and a dependent diagonal using lexical
`frame.corners.lowerLeft`/`upperRight` references. Valid Apply and complete source replacement
must cold-materialize finite geometry through ordinary native validation and alias the diagonal to
the rectangle's exact native point IDs. A valid managed edit, retained-invalid collapsed
rectangle, exact Undo/Redo, save/reload and repro must preserve authored origin, source and prior
accepted canvas atomically. Sample identity and managed-source focus change only after successful
installation. At its original direct-authoring checkpoint this extension added neither an
M84-F005 finding nor another bundled project/ledger row; F011 later adds the ninth project while
the separate code-project ledger remains distinct from the milestone-neutral golden.

### M84-F005 — collaborative overlay and semantic interaction authority

Start a managed code project with a direct literal point, a point referenced by another managed
declaration, a rectangle and at least one GUI-owned declaration. Drag each permitted code-owned
point and release; then Reset its draft. For a referenced point, verify that only the consumer is
detached when that consumer is uniquely selected; with no semantic preference or the producer
selected, verify that the consumer remains attached and follows the producer. After detachment,
repeat the consumer drag and Undo/Redo it. The projected consumer Segment may change intent/native
identity, but its code owner must remain stable and retained code-owned plus ordinary GUI dependents
must rebind. Multiple matching lenses for the selected declaration reject without mutation. Drag
each rectangle-corner role and verify its canonical two-seed coupling. Save/reload, Undo/Redo and
reproduce the session. Inject an unknown address, stale owner generation, non-finite or wrong-type
draft and two unequal same-tier updates to one address; each must reject without publication. Equal
duplicate terminal updates to one address must collapse.

The persistent overlay is bounded and keyed by project plus generation-authenticated semantic
owner/output/writable field. Its placement-draft entries are finite Cartesian point seeds only;
scalar values remain managed-source lens edits, and generated-child suppression is the separate
reversible overlay entry family. Point-seed precedence is typed overlay draft > legacy generated
override > managed source seed, and Reset removes the complete semantic edit bundle so the
applicable lower tier applies again. It must not introduce a residual, constraint or undocumented
solver priority. Pointer frames still do no parse/expand/serialization/panel rebuild; terminal
publication uses the newest authenticated accepted preview, cold parity and one outer history
entry.

After accepting one overlay draft, make a parseable structural edit which removes its owner and
also fails later native publication. The attempted overlay must be the deterministic owner-pruned
projection, while the exact accepted overlay/canvas remain unchanged. Persistence restores both,
and Undo restores the prior source, overlay and editor together. Serialized code sessions and
composed workbench payloads identify explicitly as `geosolve-sketch-code-session-v2` and
`geosolve-code-workbench-v2`; prototype-v1 M84 payloads reject, while plain M83 workspace-v8 is
unchanged.

Select a projected code-owned declaration, an ordinary GUI-owned declaration, then one absent or
malformed code-owned provenance row. The workbench must resolve the managed declaration through
accepted expansion provenance, never by decoding the opaque hashed `code.*` alias. Deleting an
independent managed declaration rewrites source and accepted scene together; deleting its producer
rewrites the exact code-owned dependent closure while retaining/rebinding surviving code-owned and
ordinary GUI dependents. Deleting a generated child produces reversible suppression without
deleting its invocation. Reuse a target after any accepted revision, including a same-named
declaration: its exact code-session identity plus accepted alias/semantic address must reject as
stale. A dirty source draft, retained code failure, stale provenance or GUI-owned selection must
refuse the semantic path without changing source or the accepted scene; GUI ownership remains on
the ordinary deletion route. This F005 fixture is implemented and focused-qualified, but does not
accept any UAT row or nominate replacement bytes.

### M84-F006 — adversarial persistence and semantic-authority hardening

Import code-session persistence with session IDs at the accepted ceiling and above it; accept only
the bounded value and never poison the process allocator. Attempt to persist a managed editor draft
above 4 MiB. Exercise typed Reset and Restore tokens against their exact session/semantic owner,
then cross them with another revision or owner. Every hostile input rejects without changing
source, overlay, editor checkpoint, accepted scene, history or allocator high-water.

Remove direct and generated overlay owners in a parseable source edit that later fails native
publication. Current overlay pruning must match the exact attempted owner set while accepted
overlay/canvas remain unchanged. Detach a generated Segment reference and generated circle centre,
retain their semantic owner generation while replacing native identity, and rebind direct/generated
and GUI dependents. Cancel before terminal publication, then repeat two accepted drags and walk exact
Undo/Redo. Transient cancellation restores exact attachment; each accepted drag remains local and
repeatable.

Submit duplicate same-tier point seeds in both orders. Bit-identical IEEE values collapse under
deterministic provenance; unequal values, including `+0.0` versus `-0.0`, reject atomically. F006
changes no residual equation, solver priority, constraint, tolerance or branch state. Its focused
owners live beside F005's semantic-overlay, native-composition, persistence and workbench suites;
the historical F007/F009 clean qualifications and now-withdrawn F010 replacement nomination are
recorded in the release gate section below.

### M84-F007 — pointer-down lens owns semantic terminal publication

In an authored project, drag a rectangle corner through at least two accepted preview frames, first
with no semantic declaration selected and then with the producing rectangle selected. The F005/F006
source `ff2e142` solved and displayed valid coupled geometry, but terminal after-the-fact
classification collected every solver-moved code-owned point as a semantic placement. Incidental
coupled-corner roundoff therefore produced multiple unequal same-tier writes to one address and the
bit-exact conflict rule rejected the legitimate release.

At pointer-down, resolve exactly one semantic point lens from accepted expansion provenance. With
no selection, choose the unique producer; a selected producer keeps the referenced consumer
attached; a uniquely selected referenced consumer still detaches only itself. Ambiguous producers
or selected lenses reject without changing transient or durable authority. Ordinary GUI-owned
points resolve no code lens and remain on the delegated editor route.

Retain the authenticated lens through every native preview frame. That lens alone authorizes the
terminal publication; solver-derived movement cannot become independently authorized sibling
intent. Exact Undo/Redo, attachment/detachment semantics and independent native validation remain
mandatory. M84-F010 below supersedes only the single-seed durability interpretation by requiring
the complete authenticated solver-coupled movement closure to be persisted atomically.

The pending route stores exact `CodeSessionIdentity`, pointer and authenticated lens. Only that
pointer's dedicated terminal publisher may consume it; generic saves reject without consumption,
and foreign/reentrant preparation or foreign terminals reject while preserving the original route.
Any non-pointer durable code action invalidates the route. Before any Outline, Inspector, source,
code, history or Apply mutation, retire internal capture and restore transient detachment before
deriving the candidate, even when the DOM viewport is unavailable. An unexpected generic save must
preserve the live editor, token, capture, notice, history and persisted bytes rather than restoring
accepted authority beneath the token. No-motion release/cancel is history-neutral; exact stored-
session mismatch and Apply/Undo invalidate a stale terminal without reverting newer accepted
authority.

The earlier corrected provisional release-WASM/browser coverage passed 14/14. Post-audit demo-web
passes 270/270, including real no-motion, exact stored-session mismatch, mutation-order and
generic-save preservation regressions; the sketch-code suites and focused warnings-denied
Clippy/WASM checks pass. The frozen `ff2e142` candidate is withdrawn. Exact replacement source
`cc2f05e`, tree `6b8fc41`, then passes the complete clean gate, immutable freeze, retained
Tailscale `:8080` publication and refreshed 14/14 browser matrix on both endpoints. UAT remains
pending.

### M84-F008 — fitted installed scenes and visible sample Fillets

Open the off-origin Mounting Plate through the ordinary code-project installer. The composed
accepted scene must be fitted inside the canvas margins rather than left outside a canonical
Origin reset; empty or unavailable scene authority alone falls back to Origin. Open Rounded
Polyline and require exactly four finite computed Fillet paths of radius `4`, each visibly clear of
the five-pixel point markers and inside the fitted viewport. Catalog/card tests must enumerate all
eight distinct demos. This fixture changes camera/sample presentation only and adds no equation,
constraint, priority, tolerance or branch inference.

Cold-materialize all eight demos and independently require finite points/scalars, current computed
features and normalized Hard residual at most `1e-9`. Drag a Bridge tower peak, Compass spoke,
Lantern vertex and Neon shared endpoint; their generated cables/markers/bulbs/Fillets must remain
attached and current. The separate eight-row M84 ledger is reviewed at SHA-256
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`; the milestone-neutral
271-row golden remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

### M84-F009 — multi-output shorthand preserves semantic output kind

Expand Mounting Plate and inspect `plate.profile`. Before repair, a multi-output template published
the first canonical-map output as its direct shorthand, so `profile` could resolve to the unrelated
alphabetically first `ne` Point. Compile and expand an additional patch returning a renamed nested
`{ nested: { shape: rounded.profile } }` result and a mapped multi-output selection. The artifact
must record `result_output: "profile"`; expansion must publish that exact Profile at its public path
and nested collection root while removing arbitrary one-child prefix fallback. A collection member
without compiler-recorded result provenance fails closed. Full qualified paths remain available.

For all eight demos, enumerate every declared semantic output and independently require its
`FeatureRef` expected kind to equal both the reviewed catalog kind and the kind derived from the
expanded target. In particular, Mounting Plate `plate.profile` is Profile and each keyed hole output
retains its declared Curve kind. This is optional-layer routing only; native geometry, solver
equations, constraints, priority, tolerance and branch state remain unchanged.

### M84-F010 — authenticated terminal persists its complete coupled closure

Open **Compass rose**, drag the shared center through a valid accepted preview and release. The
accepted native terminal, outer code publication, durable render, +50/+250/+500/+1000 ms samples
and reload must retain the exact same center. Every spoke start remains attached to that native
point; the semantic overlay contains the complete coupled Cartesian draft closure in one outer
history action. Repeat at least six center drags without a delayed alternative solution, non-finite
geometry, stale feature, request error or browser error.

At pointer-down authenticate one exact producer or selected-consumer lens. At release compare the
accepted terminal to the exact authenticated origin (the post-detachment checkpoint for a detached
consumer), classify only provenance-owned semantic point leaves and stage their complete
solver-coupled closure atomically. The authenticated lens must be present in that closure; an
unrelated sibling edit cannot hitchhike on its token. Cold rematerialization must match terminal
design and current accepted documents, computed features, ownership and allocator state. Require
finite geometry, all active features Current and independently validated normalized Hard residual
at most `1e-9`.

Ordinary point aliases remain bit-exact. For a rectangle's four corner lenses over two real seeds,
make the authenticated corner and diagonal opposite exact anchors. Only the two redundant adjacent
aliases may normalize when finite values are same-sign within 8 ULP or both values lie within
`32 * f64::EPSILON` of zero. Different signed zero, non-finite values and material disagreement
reject atomically. Run all four corner roles sequentially and preserve exact Reset/Undo/Redo. This
is code-workbench terminal transaction/parity behavior; no equation, constraint, solver priority,
tolerance or branch state changes.

Exact source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passes the complete clean gate and is frozen without
rebuild at `/tmp/geosolve-m84-f010-uat.7R5eXQoz`, modes `0555`/`0444`, aggregate
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`. Focused Compass 1/1 and
the carried 14/14 browser matrix pass against exact temporary and retained bytes. The focused case
performs six drags, samples each release at +50/+250/+500/+1000 ms, retains all four spoke
attachments, finite accepted authority and exact reload. This mechanical nomination accepts no UAT
row; U1-U14 remained pending at that historical checkpoint.

### M84-F011 — fully constrained PC water-manifold dogfood and PNG export

Open **PC water manifold · fully constrained dogfood** through the ordinary code-project catalog.
Cold materialization must accept a finite 240 × 120 mm plate containing a 60 × 84 mm reservoir bay,
three open channel centreline Polylines, three closed O-ring groove loops and eight circular screw
bores. Each bore has a solved radius of 2.5 mm and one driving Diameter target of 5 mm. The complete
sketch is located relationally by 21 construction spans from exactly one `FixedPoint` and zero
`FixedCoordinate` constraints. Independently require normalized Hard residual `<= 1e-9`, numerical
right nullity zero, equality DOF zero, bidirectional bounded DOF zero and every active feature
Current.

The managed source owns all coordinates, native roles, incidences and driving dimensions. Three
calls to the separately compiled `waterChannel` custom patch consume each route's keyed
`filletableCorners`; its artifact `each` rule emits one existing native Fillet for every current
corner. Require exactly six stable one-output Fillet hosts across those invocations. Inserting or
removing a keyed route vertex must change corner cardinality structurally without requiring the
managed caller to list corner IDs. The simple closed O-ring loops remain ordinary fully constrained
Polylines so this dogfood fixture does not confuse adaptive routing with a new offset or sealing
algorithm.

Compile and pin the custom patch through the caller-owned TypeScript toolchain, then require exact
byte parity for its package fixture and Rust-bundled source/artifact copies. The managed vocabulary
used by the sample—Circle, Coincident, FixedPoint, curve-length/diameter dimensions, keyed
Polyline `.byKey` roots and profile/construction line roles—must lower only to existing intent and
native document semantics. This fixture adds no solver residual, Jacobian, branch heuristic or
runtime JavaScript execution. The separate reviewed code-project ledger grows from eight to nine
rows; historical eight-demo F008/F009 evidence remains intact.

Click **Export PNG** in both an ordinary flat sketch and this code project. The browser must
rasterize the already-composed authoritative SVG into a downloaded `geosolve-sketch.png` whose
signature is PNG and whose IHDR is exactly 2000 × 1400. The image keeps accepted native, computed,
datum, construction and annotation paint, but omits hit corridors, controls, provisional geometry
and error overlays. Export must not mutate the accepted document, code session, history,
persistence or selected project, and it must remain available in a build that does not use the
optional code authoring layer.

### M84-F012 — annotation visibility uses one paint/pick policy

In both an ordinary flat sketch and a code project, open display options and require
**Annotations** to begin checked. Move a constraint or dimension annotation over underlying
geometry and over the Origin, select it or mark it as a problem, then uncheck the option. The same
default-true transient `EditorScene` policy must omit its SVG paint/DOM hit corridors and make
direct plus contextual/corridor annotation hit tests return none. Any stale annotation hover is
cleared; the exact underlying geometry/datum owns the next pointer move/down. Toggle back on and
require the same derived layout to return without changing selection or recomputing durable intent.

With annotations hidden, exercise an existing Fillet radius/continuation affordance and require it
to remain visible and pickable. Toggle visibility repeatedly and compare accepted document/scene,
selection, history length, Intent IR, code session, persistence and repro bytes; all remain exact.
The visibility choice is default-on session-local presentation only and must never acquire design,
layout, solver, branch, history or serialization authority.

Export PNG once with annotations visible and once hidden in each workbench. Each 2000 × 1400
image must match the currently composed annotation paint while retaining accepted datum,
construction, native and computed geometry. Start an authoring draft/inference/provisional state
before another export; export-only styling must remove draft geometry, inference guides/candidates,
hit targets, controls and error/provisional overlays without changing the live display, selection,
layout, history or accepted authority. This scenario is implemented, clean-qualified, frozen and
byte/browser-verified at exact source `84dd768`, tree `429ed56`; it is not human-accepted.

### M84-F001 — generated-point terminal checkpoint parity

Drag one generated rounded-Polyline point through the ordinary retained preview and release a
valid newest sample. Terminal publication must install exactly one fully parity-checked warm
checkpoint, retain explicit final Segment branches and computed ownership, and never reconstruct
stale schema-derived branch metadata that rejects the release. A rejected final pointer sample
retains the newest accepted preview; Undo/Redo restores the override and complete native scene.

### M84-F002 — authenticated aggregate reverse edit

Drag Corner 0 or Corner 2 of a managed rectangle. The reverse edit must target the authenticated
semantic value span for `lowerLeft` or `upperRight`, preserve every unowned/custom byte and reject
a stale source digest. Sequential X/Y rewrites must reparse and authenticate their fresh spans,
publish once, update generated dependents and round-trip through exact outer Undo/Redo.

### M84-F003 — ordinary GUI dependencies become lexical managed references

Draw an aligned rectangle, then draw a Segment whose endpoints alias two rectangle corners.
Ordinary Intent IR may continue to describe each binding with a serializable declaration/output/
kind DTO, but the optional authoring-code projection must instead emit a dependency-ordered
rectangle variable and `$.geometry.line` call whose endpoints are lexical members such as
`frame.corners.lowerLeft` and `frame.corners.upperRight`. Parsed endpoint values must be
`ManagedValue::Reference`; raw strings/DTOs, wrong kinds, cross-project values and misspelled
members reject. Cold rematerialization must preserve exact shared native point ownership, finite
geometry, current feature state and normalized Hard residual `<= 1e-9`. Promotion from the
ordinary scene creates one genuine code project/session, persists/reproduces coherently and keeps
the rectangle-to-line dependency live under later managed edits. The exact canonical fresh-
workspace document foundation may be omitted as infrastructure, but every other bootstrap object
must make promotion fail atomically rather than disappearing. Current code-expansion ownership is
also required before a Segment branch may be normalized; an ordinary GUI Segment retains exact
explicit branch authority. Focused owner coverage rejects raw strings, DTO-shaped objects,
foreign-project and forged reserved-project references, wrong kinds and misspelled members.

### M84-F004 — ordinary computed Fillets keep Code discoverable

In an ordinary sketch, draw a Horizontal Segment, continue it with a Vertical Segment and place one
computed Fillet between them. Ordinary drafting creates the two existing inferred axis constraints.
Complete GUI bootstrap formerly returned an unsupported-recipe error first for
`ComputedFeature::FilletSet` and, after that direct slice, for those Horizontal/Vertical
declarations; the old workbench also treated every conversion error by omitting Code, making
unsupported projection look like absence of the code layer.

The supported complete projection must emit the second Segment endpoint as a lexical declaration
member, `$.constraint.horizontal`/`$.constraint.vertical` declarations whose `curve` values are
the lexical owning spans, and one direct `$.computed.filletSet` declaration. Constraint
suppression is exact and lowering uses the existing Intent Horizontal/Vertical kinds. Each Fillet
corner has exactly two
ordered lexical `NativeCurveSpanRef` parents and explicit parameter, winding, neighborhood, normal
side, retained endpoint and periodic anchor; endpoint order, sweep and suppression are also
explicit. The central declaration-result catalog brands only direct line spans, rectangle edges
and Polyline segments. A computed host Fillet arc is not a native span: it must fail TypeScript
assignment and must still reject during Rust lowering if the typed boundary is bypassed. Radius
accepts only a positive finite model-unit number or branded `mm(...)`; forged unit records, other
length-unit calls, angular, nonpositive and nonfinite values reject. No raw native ID or
`{ declaration, output, kind }` transport DTO may appear. Direct lowering reconstructs the existing
Intent `ComputedFeature::FilletSet` without rerunning Fillet authoring heuristics or adding a solver
path, and its output remains opaque `FilletSetFeature` authority rather than fabricated child-arc
ports.

Promotion and cold reload must authenticate the accepted feature, corner and parent spans, retain
finite Current computed geometry and independently validate normalized Hard residual `<= 1e-9`.
The exact checked-in managed-v1 two-line/two-axis-constraint/one-Fillet fixture is compiled by
TypeScript, parsed by Rust and cold-materialized through that authority; separate hand-maintained
fixtures may not mask schema drift.
Projection remains all-or-nothing for any other unsupported declaration, but Code must stay
visible with an escaped read-only conversion diagnostic, Intent IR fallback and no Promote action.
The same rule applies when retained-failed intent no longer matches the prior accepted semantic
identity: projection rejects that authority mismatch before serialization and cannot offer Promote.
This focused ordinary-bootstrap family does not alter the four-demo M84 code-project golden
ledger. Rust bootstrap/ownership, direct-lowering, descriptor-parity, workbench/persistence,
TypeScript and UI tests plus the clean replacement qualification pass. The exact frozen nomination
is recorded below; no human UAT row is claimed yet.

### M84 release gate

The separate code-project ledger, native/WASM/RPC/TypeScript parity, type failures, finite geometry,
explicit branches and normalized Hard residual `<= 1e-9` pass alongside the byte-identical
milestone-neutral golden. Focused format, warnings-denied Clippy/Rustdoc, locked all-feature,
actual-WASM, TypeScript and package-closure checks pass. Withdrawn historical source `79078ec`,
tree `05aefb0`, passed Trunk and the complete clean gate (log SHA-256
`0f50e6bcdf019c71d70497acc301dcdfd194db1142b248bcd469d0f3ed9efda0`). The no-rebuild
snapshot `/tmp/geosolve-m84-uat.aHw5ePSW`, aggregate
`99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`, passes exact local and
retained-Tailscale HTTP verification plus focused browser 4/4 on each endpoint. Those bytes are
defect evidence only; old PID `2426265` was retired after the replacement passed temporary
verification.

Replacement source `b9e67bad7f4935b1e0591ea4f149fae478b32675`, tree
`7062806695e1e134c339cfa47903145d321f6350`, passes the complete clean gate from 00:24:02 through
00:46:20.940662 AEST on 2026-08-26, exit 0. Its 6,192-line, 414,397-byte log
`/tmp/geosolve-m84-f003b-release-gate.log` has SHA-256
`eb05d3c1e460f5cb7be410dc44d0af0c4b4eaf4fd775423b676c77a84a433f90`. The unchanged 271-row
golden has SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and the separate
M84 ledger remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.
The exact no-rebuild snapshot `/tmp/geosolve-m84-f003-uat.mO67NI`, directory/files `0555`/`0444`,
has seven regular non-symlink files and aggregate
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`; source, copied and frozen
manifests match. Temporary and retained eight-path HTTP ledgers match at
`438d747641522dd567e5790c56663f9bcb596147d48b843e1fe3367830822d0c`, existing browser 4/4 and
F003 1/1 pass on both, and retained service PID `3736900` served the snapshot at
`http://100.94.63.83:8080/`. M84-F004 withdrew those F003 bytes from current UAT and PID `3736900`
is retired. At that historical checkpoint M84 remained active with U1-U12 pending and no F004
replacement nomination was claimed.

Historical F004 replacement source `c2cf160d3a7d5065e582f2ba982881380d2b871c`, tree
`94a178699f9b2e8bd2a6497c9b0334d43cad2b20`, passes the clean Nix release gate from
12:37:30.923 through 12:55:14.714 AEST on 2026-08-26, exit 0 in 1,064 seconds. Its 6,118-line,
412,411-byte log `/tmp/geosolve-m84-f004b-release-gate.log` has SHA-256
`f4c005912392c11cab6600706876c37dab855f0a59c0b7c06565c242b014d6fd`. The unchanged 271-row
golden remains `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate M84
ledger remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

The exact no-rebuild seven-file output is frozen at
`/tmp/geosolve-m84-f004-hv-uat.FF5RFBZe` with directory/file modes `0555`/`0444`, ordered-manifest
aggregate `f34c46ee5876c4bdb458863cc90c6c6b25281cc8e44f89c8d00eba0f16ca5bbc`, and complete evidence
at `/tmp/geosolve-m84-f004-hv-freeze-evidence.FiCfIvir`. Temporary and retained eight-path HTTP
ledgers are byte-identical at SHA-256
`54efcd30699a8632b868d753af88b1f17433c284201688834f7a0f35e2598153`: every path returns 200,
zero redirects, exact MIME/length/body, no `Location`/`Content-Encoding`, and `/` equals
`index.html`. Sequential browser suites pass baseline 4/4, F003 1/1 and F004 2/2 on both endpoints.
Historical service PID `3316682` served those immutable bytes at
`http://100.94.63.83:8080/`; it and the temporary/obsolete pre-axis F004 services are retired. The
later direct-authoring amendment withdraws those bytes from current nomination solely because they
predate amended scope; the qualified replacement below owned the retained endpoint at that
historical checkpoint.

Historical post-F004 direct-authoring source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, passes public artifact-free construction,
fresh-surface catalog/fail-closed classification, exact native rectangle-corner aliasing after
Apply, retained-invalid persistence, complete source replacement, exact Undo/Redo and conflicting-
origin rejection. The complete clean release gate passes with log SHA-256
`34bf408f6a565dec5705745f916eb628397a549b4a7002d865269e8d167e179f`; the four-demo M84 ledger and
271-row golden remain unchanged. The exact no-rebuild snapshot
`/tmp/geosolve-m84-authored-uat.ZYQQyBQQ`, aggregate
`6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`, passes byte-identical
temporary/final eight-path HTTP verification and direct 3/3, baseline 4/4, F003 1/1 and F004 2/2
browser suites on each endpoint. The PID `4081080` service record and complete evidence under
`/tmp/geosolve-m84-authored-freeze-evidence.ngNf7jxZ` are historical only. M84-F005 withdraws this
nomination because it predates collaborative draft overlay and semantic drag/deletion authority.
F007 also withdraws the later frozen `ff2e142` candidate. M84 remains active and unaccepted with
refreshed U1-U14 pending. Historical F007 product source
`cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
`6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, passes the clean gate; its no-rebuild snapshot
`/tmp/geosolve-m84-f007-uat.KgW8fpLf` has aggregate
`8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`. Temporary and retained
eight-path ledgers match at
`efa609c6bac127753336c3634730b81bed04699a25c6394ab039c7f06b0b2b64`; baseline 4/4,
direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser suites pass on both endpoints.
PID `62376` served the exact frozen bytes at `http://100.94.63.83:8080/`; complete evidence is at
`/tmp/geosolve-m84-f007-freeze-evidence.rP5rQcTG`. The F009 replacement supersedes it and PID
`62376` is retired.

Withdrawn F009 source `c74651cc82506e31926042df65a1eeec08a6af9d`, tree
`a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passes the complete clean gate. Its 6,218-line,
421,590-byte log has SHA-256
`c9b743c8f95d6df7706b04e2d820ac67426f1b11ec447d2bffdc08cd1fe0f6f1`; the unchanged 271-row
golden and expanded eight-demo M84 ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`. The exact no-rebuild
snapshot `/tmp/geosolve-m84-f009-uat.q8cKIN3v`, directory/files `0555`/`0444`, has ordered-
manifest aggregate `23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`; complete evidence is
at `/tmp/geosolve-m84-f009-freeze-evidence.3FoVTQ6m`.

Temporary and retained eight-path ledgers are byte-identical at SHA-256
`add827e88d17735cfb6cb0bbecec885f5680db0bd11b67bb591673d566b90676`; baseline 4/4,
direct-authored 3/3, F003 1/1, F004 2/2 and F005-F007 4/4 browser suites pass on both endpoints.
Temporary PID `3943194` and F007 PID `62376` are retired. M84-F010 withdraws these bytes;
historical retained PID `3965271` is retired and the immutable F009 snapshot remains preserved.

Historical F010 source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passes the clean release gate from
15:58:23.055857854 through 16:17:18.303733569 AEST on 2026-08-27, exit 0. The 6,209-line,
420,425-byte log `/tmp/geosolve-m84-f010-gate.9NvAi3z5/release-gate.log` has SHA-256
`bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and final Trunk success.
The unchanged golden and eight-demo M84 ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

The exact seven-file output is frozen without rebuild at
`/tmp/geosolve-m84-f010-uat.7R5eXQoz`, directory/files `0555`/`0444`, ordered-manifest aggregate
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`, with evidence at
`/tmp/geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary `:18091` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`; all eight paths have exact
bytes and MIME. Focused Compass 1/1 and the carried 14/14 browser matrix pass on both. The focused
spec and config have SHA-256 `4b97f570d5122a353b4ee104b26ea46427ca1c3b79aa5a8d35fee7875302dab0`
and `c0900c1132352ed9471321a2cf5727baf2c004d8eebf1a1bcd8a43146289df9c`.

Historical F010 PID `650971` and temporary F010 PIDs `238809`/`621532` are retired. M84-F011
withdraws this nomination; its snapshot remains rollback evidence.

Historical F011 rollback source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, passes the complete clean gate. Its immutable
no-rebuild snapshot `/tmp/geosolve-m84-f011-uat.ps736NLh`, aggregate
`056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`, passes byte-identical
temporary/retained HTTP verification at ledger SHA-256
`9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`. Focused frozen
manifold/PNG/authority checks pass 1/1 on both endpoints and preserve lifecycle, history length,
project title and viewport markup authority. Retained `geosolve-m84-uat.service`, PID `1485656`,
served only this immutable snapshot at `http://100.94.63.83:8080/`. M84-F012 withdraws this
nomination; PID `1485656` is retired and the snapshot remains historical rollback evidence.

Current F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the complete clean gate from
20:36:27.840305756 through 21:01:30.826307821 AEST, exit 0. Its 6,274-line, 424,393-byte log
`/tmp/geosolve-m84-f012-gate.PmjeGNUa/release-gate.log` has SHA-256
`04e35c73fe92ca3e089b87bd13b5221c60835b72c9eeba5ed38916b51150004a` and final Trunk success.
Its immutable seven-file, zero-symlink no-rebuild snapshot `/tmp/geosolve-m84-f012-uat.nMOymIIM`,
modes `0555`/`0444`, aggregate `166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`,
has complete evidence at `/tmp/geosolve-m84-f012-freeze-evidence.qua6ci1b`. Temporary/retained
eight-path ledgers are byte-identical at SHA-256
`66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`.

Focused annotation paint/pick, underlying-target, authority-neutrality, exact-restoration and
WYSIWYG PNG checks pass 1/1 on both endpoints. The supervising user accepts U1-U16 at milestone
level without claiming a separate row-by-row replay. Approval descendant
`e6e960d7ac297eb099ba80c19148393e46427606`, tree
`173c65d5c39c9cb371869fccf22d5b41ddec6ceb`, passes Pages run `33068058169`, build/deploy jobs
`98503000701`/`98504823968`, deployment `6121981526` and artifact `9644770095`. Its downloaded
15,032,320-byte tar has SHA-256
`da43ee8d85f81461d579cafa24b9e884451c97a58909955ae188ef9281f7412e`; seven-file aggregate is
`1f2228abcb163e09ff50db2f19d79b13d9c74e638731d6cd26d5ade324b27835`, and exact hosted ledger
SHA-256 is `8930c71b3d7bb9b454653ebcb24c58b594f4750a433f745333cf56c01e4c33fb`. Former retained
`geosolve-m84-uat.service` PID `2241323` is retired, its endpoint refuses connections and the
immutable F012 snapshot remains preserved. M84 is closed and Pages is final public-byte authority.

## M85 retained presentation performance fixtures

M85 adds no residual equation and does not reinterpret or expand the milestone-neutral golden
authoring matrix. It owns focused `geosolve-demo-web` presentation regressions and a narrow
candidate-only Chromium timing trace. `docs/M85_GOALS.md` is authoritative.

### M85-P1 — Confirmed full-render camera defect

Against exact immutable M84-F012, open `pc-water-manifold` with annotations visible. Thirty
middle-button pan samples take `14,933 ms`, replace viewport children 33 times and produce a
`583.3 ms` p95 frame gap. Eighteen wheel events take `9,976 ms`, replace children 19 times and
produce a `583.2 ms` p95 gap. The annotations-hidden diagnostic still takes `16,937 ms` with a
`733.3 ms` p95 pan gap. This is M85-F001: camera callbacks rebuild the complete scene/markup/DOM.

Native release measurements provide the control: retained preview `4.340 ms` p95, relation-heavy
control `17.094 ms`, Fillet radius `2.313 ms` and Offset distance `5.743 ms`. These do not excuse
browser timing failures but route the primary defect to presentation rather than solver equations.

### M85-P2 — Camera queue and admitted work

For projectional/code and flat compatibility adapters independently, submit multiple pan samples
before one RAF and require one presentation of the latest sample. Submit ordered wheel deltas and
require their exact sequential anchored fold. Stale RAF and idle callback generations do nothing.
The final camera matches the exact sequential oracle and is visible no later than two frames.

Snapshot the actual work ledger around every camera-only frame. Exactly one lightweight camera
presentation is allowed; exact reprojection, solver/preview, materialization, computed evaluation,
native-history publication, code parse/expansion/publication, workspace encoding/write, durable
panel, full scene, full SVG and viewport replacement counts remain zero. Accepted/current identity,
document, Intent/code-session identity, history, selection, annotation layout and canonical
persistence bytes remain bit-identical. A separately authenticated exact reconciliation may rebase
retained paint once after a completed burst, but is never admitted as a camera RAF and may cross no
semantic or durable owner.

Direct owners include
`retained_camera_queue_coalesces_latest_frame_and_authenticates_idle_boundary`,
`retained_camera_exact_reconciliation_revokes_an_unpainted_frame`,
`production_camera_admission_records_one_completed_paint_and_stale_callbacks_record_zero`,
`toolbar_camera_commands_have_route_parity_and_admit_no_durable_work` and
`terminal_pan_sample_owns_the_exact_final_camera_for_both_routes` in `geosolve-demo-web`.

### M85-P3 — Retained/cold camera parity

Use fixed translation, zoom-in, zoom-out, off-centre anchor and combined pan/zoom states. Compare
retained presentation with a cold exact scene semantically: every finite model position maps to the
same screen position; grid and protected axes use the same camera; visible geometry/annotations and
semantic DOM IDs agree; strokes, points, labels and hit envelopes remain screen-usable. Nodes
remain stable during a burst; a separately authenticated exact reconciliation may replace them once
afterward while preserving semantic IDs. The first hover/click/drag after navigation targets the
painted item with current-camera coordinates and no visual rebase jump.

Public headless owners include
`retained_scene_reprojection_matches_cold_geometry_and_preserves_annotation_layout`,
`retained_scene_reprojection_updates_computed_fillet_affordances_and_actions`,
`retained_scene_reprojection_is_transactional_for_malformed_public_geometry` and
`retained_scene_reprojection_rejects_every_mutable_derived_surface_without_resealing`. Browser-
adapter owners separately authenticate retained hover against scene, view and display policy.

### M85-P4 — Real-browser budgets

Measure the ordinary authored rectangle plus diagonal and the exact visible-annotation manifold
for at least three warmed bursts and 120 samples per action class. Camera callback p95 is at most
`1 ms`; RAF CPU p95 at most `8 ms`; ordinary next-paint p95 at most `16.7 ms` and 55 fps; manifold
at most `33.3 ms` and 30 fps. No task exceeds `50 ms` during a two-second burst. The hidden-
annotation manifold is diagnostic only and cannot replace the visible result.

The real timing trace runs against local/frozen Tailscale candidate bytes at nomination, not broad
PR CI. Deterministic queue/work/parity regressions remain ordinary native tests.

### M85-P5 — Hover, drag and exact terminal

After camera repair, measure ordinary, Compass Rose and Rounded Polyline hover/drag. Browser
preview p95 is at most `33.3 ms`; existing ordinary native retained preview remains at most
`16 ms`. Mutating release creates one history/save/durable-render boundary and becomes visible
within `250 ms` ordinary or `500 ms` code-coupled; cancel/no-motion creates none. Geometry is
unchanged at +50/+250/+500/+1000 ms and after reload.

`InteractionWorkReceipt` reports attempted native preview/release, cold Intent materialization,
computed evaluation and native-history publication, including work crossed before rejection.
Optional `CodeWorkReceipt` independently reports managed parse, deterministic expansion and
accepted outer-code publication. The browser composes those receipts with transient scene/SVG,
persistence and durable-panel work rather than inferring work from an event name. Pointer previews
may solve/materialize/evaluate only when their receipt says they did; they never publish history or
code, parse/expand code, save or rebuild durable panels. A code-owned release may absorb one native
accepted transaction into one outer code publication but leaves one user-visible Undo step. Exact
terminal publication retains independent solver validation.

Focused owners include `audited_pointer_receipts_report_only_crossed_projectional_work`, the
interaction/code receipt unit suites, browser ledger predicates and the existing Compass Rose,
rectangle-diagonal and generated-endpoint terminal regressions.

### M85-P6 — Default-stack history-neutral code terminal

Use exact `pc-water-manifold` code authority and stage one literal point overlay without changing
its six native host requests. The unchanged-host path expands once, performs no managed parse or
outer publication at the composition layer, preserves all six native host identities, independently
validates finite accepted authority and returns a delegated editor with empty Undo/Redo whose base
identity matches its Intent session.

`m85_large_unchanged_host_overlay_is_default_stack_and_history_neutral` runs on the ordinary test
thread. Before repair, the same path created a normal nested Intent transaction and cleared its
large Undo snapshot; dropping that snapshot overflowed the ordinary 2 MiB stack and terminated with
`SIGABRT`. The history-free accepted-authority fork and delegated patch path repair M85-F002. A
release-wide enlarged test stack is not equivalent evidence. The exact ordinary-stack command
passes in `52.59 s` at source `e9b4407`.

M85-F003 extends that stack contract to ordinary structural editing. Select the independent
managed line from the authored rectangle-plus-diagonal project and delete it: the rectangle stays
finite/current and exact Undo restores the prior source/checkpoint. Then select the rectangle and
delete it: exact transitive dependency closure produces an authenticated empty project. Before
repair, inline `MaterializedCodeProject` return slots accumulated across structural, audited and
warm-native adapters; the partial case reached the constraint compiler at the 2 MiB guard page and
the process aborted. The final repair preserves every pre-M85 public incremental-code signature:
the unaudited public path calls the private receipt-aware worker directly instead of constructing a
large intermediate audited return. Six large optional projectional Fillet/Profile Offset preview
and gesture states are privately boxed, reducing `ProjectionalEditorSession` from 35,488 to 15,296
bytes and `MaterializedCodeProject` from 36,320 to 16,128 bytes without a public replacement API or
an increased thread stack. Experimental public boxed returns at `d2b7d38` are superseded.

`managed_canvas_deletion_publishes_validated_scene_and_exact_undo` and
`managed_canvas_deletion_uses_exact_transitive_dependency_closure` pass together with explicit
`RUST_MIN_STACK=2097152` in approximately `2.04 s`; the complete default-stack demo-web library
passes 300/300 in `98.61 s`. No golden-authoring row is added because this is code-composition
ownership, not new geometry or solver semantics.

### M85-P7 — Qualification authority

Format, warnings-denied Clippy, workspace tests, release performance owners, WASM/Trunk, unchanged
clean golden and the complete clean release gate pass. Freeze without rebuild, exact-verify local
and retained Tailscale bytes, then complete hands-on M85-U1-U9/U11-U12 and final-source native
flat-adapter M85-U10 evidence. Persisted v1-v6 workspaces normalize into projectional authority and
the flat retained-coordinator adapter has no normal browser bootstrap; qualification therefore uses
its direct compatibility/parity owners rather than an artificial UAT fixture. Only explicit
supervising-user approval authorizes Pages publication, exact hosted-byte verification, service
retirement and closure. Those post-approval gates now pass.

Committed implementation checkpoint `fd2c560c5c61338a96f145ecb87106af49e93749`, tree
`97591f3d8c268e36db2e3e728c52163dca87d055`, passes format/diff, affected-crate warnings-denied
all-target Clippy, `geosolve-constraint-editor` at 756/756 passed with 3 ignored, the complete
`geosolve-sketch-code` crate, `geosolve-demo-web --lib` at 300/300, the ordinary-stack F002
sentinel in `55.45 s` and the explicit 2 MiB F003 pair in approximately `2.04 s`. This is final-code
native evidence; no solver equation, persistence format, branch, constraint or code-authoring
semantic changed.

An earlier five-test browser profile passes 5/5 in `2.2m`, with 1,200/1,200 camera-only admissions,
zero forbidden admissions/navigation long tasks, worst completed-presentation RAF p95
approximately `0.8 ms` and sustained navigation `59.6–61.3 fps`. It predates both the experimental
F003 public API change and the final repair, and its exact source was not pinned, so it is
provisional pre-F003 ancestor evidence only and is preserved as historical directional evidence;
none of the final qualification below is inferred from it.

Exact qualified source `5c265e211e20dabc8a27f6402d530f5d645ff15c`, tree
`b55d012443f4dbf7551e30041da2912e666de9db`, passes the clean release gate from
`06:40:49` through `07:13:16` AEST with exit `0`. The 6,993-line, 456,580-byte log is
`/tmp/geosolve-m85-gate.8vK5wDu4/release-gate.log`, SHA-256
`0b09720dfd4491575ab10bd3baba2f8f6e7fae9e8e90954ff0026a64de4458eb`. Without rebuilding, its
exact seven-regular-file, zero-symlink output is frozen at `/tmp/geosolve-m85-uat.QX8fU3Q6` with
directory/file modes `0555`/`0444` and ordered-manifest aggregate
`dc729ce5fa28929dba2aa086e49246aac7d5173a0583d3b0b64748ffbbb7fda5`. Local and Tailscale
eight-path HTTP ledgers are byte-identical at SHA-256
`305eccfc8fa60786aabfae59edd612e695ce3c15b7224abbf3be0d0852ae0d27`.

The fixed final harness runs against those frozen local bytes and passes 5/5. Its complete log
SHA-256 is `d1174515c320e1f3a0e006bbaad47cc47ba0aeda52fe9bf05c956d91750951c7`
and machine summary SHA-256 is
`db508a28f46774fbc74f9bdebfc20c513c46934bfd29a7b0775d96962caa63e6`. Across 30 navigation
bursts, all 1,200/1,200 camera admissions are camera-only, with zero forbidden admissions or
navigation long tasks. Worst p95 input callback is `0.2 ms`, camera RAF CPU `0.6 ms`, completed
presentation `0.8 ms` and frame gap `16.8 ms`; minimum sustained navigation is `59.8249 fps`.
Worst drag preview/terminal pairs are ordinary `5.4 ms`/`87.23 ms`, Compass Rose
`8.7 ms`/`175.50 ms` and Rounded Polyline `6.6 ms`/`184.93 ms`; all retain preview/terminal parity,
one exact save per release and no delayed movement.

Final-source M85-U10 evidence at `/tmp/geosolve-m85-u10-final.D1auvz5d` passes all 19 direct
compatibility/parity cases without changing source, tree or frozen distribution. Its command and
result ledgers have SHA-256
`5da8bf46936228d22034f4195e9e571f6f0227510db257f943ef27686dee6545` and
`5efaa7d874d1c07189ab8ec7398863945abd1d15bb50933e55f47bac888f7f79`; the evidence manifest file
has SHA-256 `ec2b205710bf6d79c09e696fb6023b01ebcac1f922bcae04c2e8486c80702189`.
The same immutable candidate was served at `http://127.0.0.1:18100/` and
`http://100.94.63.83:8080/`. On 2026-08-28 the supervising user approved M85 and requested closeout;
that milestone-level decision accepts M85-U1 through M85-U12 without claiming a separately logged
row-by-row replay. Approval head `e8dfec3467424a9051533658df34c5b406bf3743` then passes Pages
run `33128387637`, assemble/deploy jobs `98711889276`/`98713212444`, deployment `6133200015` with
status `17437536547`, and artifact `9669411681`. Its 4,931,164-byte ZIP has SHA-256
`22c8d1f0ec4fc660ed30574e822fa6cc435b40f4d6a3a731f441ad51f6b1e03f`; its inner 15,144,960-byte
tar has SHA-256 `2ebb890c033edadfd46b7ac4a0c166fe40be65b32d790ebf39df6dc8d3fd4be4`.
The extracted artifact has exactly seven regular files, zero symlinks/non-regular entries and
ordered-manifest aggregate `8b569bcb7a003d6f3613acdbf66d22b6847fa02bad645805221f161d268101ce`.
Public `/` and all seven paths return HTTP 200 with no redirects, exact MIME, `Content-Length` and
artifact bytes, no `Location`/`Content-Encoding`, and root equal to `index.html`; hosted results
`/tmp/geosolve-m85-pages-verify.39FJNL/results.tsv` have SHA-256
`206a99797aba73ae9df5acf4b1d690d7fc98498c21b008525895c07e9599e867`. The Pages artifact is a
fresh repo-prefixed build and is not byte-identical to the frozen UAT snapshot; that snapshot
remains historical evidence. Product source `5c265e2` is an ancestor of the approval head with only
the eight closeout documents changed. Both M85 user services are inactive/dead with `MainPID=0`
and both endpoints refuse connections. Pages is final M85 public-byte authority and M85 is closed.

## M86 focused bug-fix fixtures

M86-F001 adds no residual equation and does not reinterpret or expand the milestone-neutral golden.
Its focused optional code-project regressions and thin browser-Inspector adapter proof pass;
complete clean qualification, unchanged-golden confirmation and exact no-rebuild local/Tailscale
nomination also pass. The supervising user's scoped “Looks good” assessment accepts F001 without
claiming a separately logged row-by-row replay. Expanded M86-F002 adds only focused headless
picking rows, and M86-F003 adds a focused retained code-terminal regression; neither broadens the
golden. The pre-expansion F002 nomination is withdrawn. The exact saved combined patch passes the
complete provisional dirty-worktree gate and its trace-enabled descendant passes bounded trace,
demo-web 316/316, WASM, release-build, golden and exact dual-endpoint byte checks. The supervising
user's 2026-08-29 close decision accepts M86-U1-U8 without claiming a separately logged row-by-row
replay. Accepted source `88d1b5e` / tree `09018e5` is clean-qualified and its exact no-rebuild
output passes isolated HTTP verification. Approval head `ccf791f` passes exact Pages publication;
both M86 services are retired and M86 is complete.
`docs/M86_GOALS.md` is authoritative.

### M86-C1 — Exact managed curve-length target rewrite

Open the checked-in `pc-water-manifold` code project and resolve exact alias
`code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`
through accepted expansion provenance. It must name managed declaration `topScrewRail3Length`, a
direct `dimension.curveLength` target whose source contains `target: mm(16)`. Do not infer any
meaning from the opaque alias hash.

Apply the Inspector-equivalent target `8` through authenticated code-project authority. Exactly
one source token changes to `target: mm(8)`; ordinary parse/expansion/Intent materialization/native
solve and independent validation run before installation. Accepted target is `8`, all accepted
geometry is finite, normalized Hard residual is at most `1e-9`, immediate rematerialization
reselects the stable semantic alias and exactly one outer code history entry is added. Undo restores
exact source, target and checkpoint `16`; Redo restores `8` and proves the alias remains available.
The Undo/Redo proof does not assert durable UI selection.

### M86-C2 — Family and rejection boundaries

The focused accepted-diameter fixture changes a direct managed `dimension.diameter` target from
`mm(5)` to `mm(8)`, installs independently validated current authority, immediately reselects its
alias and exactly Undo-restores source/checkpoint/native value `5`. A wrong port/field/index,
unsupported or generated declaration, stale/foreign Inspector identity, dirty/retained-failed
authority and non-finite input reject before source or delegated Intent mutation. An ordinary GUI-
owned dimension continues through the existing generic Inspector path.

A representable but invalid target preserves the complete prior accepted geometry/checkpoint while
retaining failed managed source and a local corrective diagnostic under existing code-session
semantics. The manifold regression freezes diameter `5 -> 0`, byte-exact accepted-checkpoint
retention, one outer retained-failure row and exact Undo to source/native value `5`. While that
failure remains active a further Inspector edit rejects until source correction or Undo. No partial
accepted-editor/selection publication is allowed.

### M86-C3 — Thin Inspector adapter parity

Construct the exact projectional Inspector control for M86-C1 and dispatch it through the browser
adapter. It must reach authenticated managed-source rewriting, not generic code-owned checkpoint
classification, install the complete returned `WorkbenchDocumentAuthority` revision metadata,
avoid a duplicate save-time outer row, and preserve byte-exact accepted authority on a retained-
invalid edit. Owner assertions remain in C1/C2; this adapter test duplicates no solver equation or
accepted-state oracle. Existing GUI-owned reference-dimension and generic Inspector Undo/Redo
regressions remain collateral authority.

### M86-C4 — Historical F001 candidate authority

Source `90504245e19858f986d5f506f6e42d237e9665b5`, tree
`65e092540dab82618d1129229b566a2e791aa40c`, passes the complete clean Nix release gate and
unchanged 271-case golden. Its exact seven-file Trunk output is frozen without rebuild at
`/tmp/geosolve-m86-uat.vdEFAxsF`, ordered-manifest aggregate
`1f872c6b51317ff810b48ab8965e1e0a0f6cb45feb01f5cbfe654eafbedd5882`. The former local and
retained Tailscale processes served only that snapshot; all eight paths were exact and their
byte-identical ledgers have SHA-256
`b5ad691e14198791fa801281724fc3186161b2d2aae7358affea402e9a5f0acd`. This is historical F001
evidence, not public authority; Pages remained M85 until M86 publication and exact hosted-byte
verification later passed.

### M86-C5 — Fillet source-corner Select priority

Create two joined native line/polyline spans and a computed Fillet whose visible radius surface
also contains their persistent shared endpoint. Independently prove
`EditorScene::hit_test(position, default_tolerance)` returns that Point and
`EditorScene::resolve_fillet_hit(position, default_tolerance)` returns the Fillet Radius owner.
Through one unchanged `ConstraintEditor`, pointer move must publish Point hover/context and pointer
down must select the same Point and start `ActivePointerGestureKind::Point`.

Repeat with two distinct stored endpoints joined by an active explicit Coincident relation. The
lowest deterministic hit identity wins as one semantic corner. Repeat without Coincident while
leaving the two solved coordinates identical: the Fillet retains radius ownership, proving that
coordinate proximity is not topology. Finally Apply the computed Fillet and repeat against the
Current persistent scene.

Then reproduce the exact UAT scope expansion with one open two-span right-angle Polyline whose legs
are length `2`. Requested radius `2` is the evaluator's tangent-at-endpoint fold boundary; apply
radius `1.99` as the robust accepted equivalent and assert the resulting visible radius surface
contains both remote endpoints. Each endpoint must independently win hover/down, selection and an
ordinary Point gesture. A compact radius grip placed over a point remains the more specific Fillet
control. If distinct endpoints from the two parents both lie inside the point tolerance, the nearer
one wins; an exact disconnected distance tie stays with the Fillet rather than inventing topology.
Repeat with two accepted Fillet affordances whose broad surfaces overlap the first Fillet's remote
endpoint, arranging the other Fillet as the nearer broad hit while keeping both compact grips away.
The parent endpoint still wins hover/down and begins a Point gesture. If disconnected endpoints are
made an exact point-distance tie, the globally best broad Fillet retains ownership.

All overlapping Fillet owners share one request-local lazy Coincident `OnceCell`. Endpoint
arbitration may initialize it at most once; broad arc/spoke/rail hover outside every point halo must
leave it uninitialized. The focused
`broad_fillet_hover_defers_coincidence_work_until_an_endpoint_halo_is_hit` regression proves this
with a real broad arc sample, rather than a synthetic early-return path.

The existing unrelated draggable point at a computed Fillet contact must still resolve to
FeatureCorner/FilletRadius for every modifier; removing it still leaves the passive native parent
below the computed radius surface. Direct radius grip/rail gestures, native authoring at a computed
contact and painted-radius reconciliation remain unchanged. Native and WASM results must match
exactly. No new golden row, residual/Jacobian test or browser-specific hit rule is warranted.

Historical source `dbe94daf152515169b78a310cf2286f9ea04c80b`, tree
`77f86c0a198af12e10537dc4d6d7d90066ba48e8`, passes the complete clean gate from 13:46:55 through
14:20:45 AEST. Its exact seven-file output is frozen without rebuild at
`/tmp/geosolve-m86-f002-uat.CPfe9QD8`, aggregate
`e3f9581a05a8cbf5731b33625fa63f2b35e62f4ebcfdacb6d75a4486f80fc850`. Temporary-local, final
local and Tailscale eight-path verification are identical at SHA-256
`e5513ab3e36262f2ccedf175006f1283d5504180c8d0be46e1e90dded999a3df`. The remote-endpoint escape
withdraws that pre-expansion nomination; preserve its snapshot and evidence without presenting its
post-reboot listeners as current candidate identity.

### M86-C6 — Typed Panel terminal-derived roundoff

Open the genuine `typed-panel` code project and sequentially drag its upper-left rectangle corner
from `[0,40]` to `[3,38]`, then `[5,37]`, then `[-2,36]`. For each gesture, use at least two native
preview frames and require pointer release to retain the latest exact preview. Dedicated terminal
publication must add exactly one outer revision, keep the canonical rectangle overlay at two seed
drafts, and publish the exact released coordinate. Persist/reload after every release and require
the same exact coordinate, finite accepted geometry, all keyed Fillets Current and independently
validated normalized Hard residual at most `1e-9`.

This is retained code-terminal authority, not a snapping/inference or core-solver scenario.
M84-F010 already permits tightly bounded normalization for two redundant rectangle aliases. Derived
computed scalars may use that same finite 8-ULP/near-zero cell only when both design and accepted
document parity normalize the same non-empty point set, and only for edges/construction fragments
sourced from curves incident to those points. Unrelated computed geometry remains bit-exact. Public
edge kind/topology, provenance, sweep, tangent orientation, contact source/winding, ownership,
feature allocator and persistent sketch allocator remain exact. Negative rows change an unrelated
edge, exceed 8 ULP, provide unequal normalization sets, flip sweep/tangent/winding or alter
provenance; every row must reject. Evaluation-local stamps and private continuation cells are not
claimed as fields compared by this adapter.

### M86-C7 — Historical provisional combined UAT authority

The served F002/F003 build identity is the saved pre-gate 160,117-byte, 2,976-line binary patch
over HEAD `4730e156e17cf3df88b9681a22961d41b686c2ff`, tree
`23a76c3b7141f10064d899113b97135932d23033`, with SHA-256
`feafcc2a717a9c1bf9ff7a708b705903b2e18c6ef67327f533d66819a8784a57` and saved status SHA-256
`945ef3534016a5735c42c6fedaf72e66be2acc41ce9dc764db6e5896c3636b5a`. Saved pre/post-gate patch
and status files are byte-identical. Subsequent documentation-only worktree edits are outside that
served-build patch and do not alter the frozen seven-file candidate.

The complete provisional dirty-worktree gate exits `0`; its 6,582-line, 441,920-byte log has
SHA-256 `93b645c2a2f1850f589b406943f3618da4fc833a42ff6066884602ec3e31ddb6`. The exact no-rebuild
snapshot `/tmp/geosolve-m86-f002-f003-uat.yGY3Nvly` has ordered-manifest aggregate
`8f5a4ffcd96819b986ba81a9467d0c83a64365b2d21338cd134e164fa4444ce4`. Temporary, local and
Tailscale eight-path ledgers are byte-identical at SHA-256
`dca3e6eeba66e12c873ba4b5ba9b6cadd489060f0e4c7d5ce1070ed3344ec96f`. Temporary PID/invocation
`965128`/`a06c89f580744568b0d39677ee776da1` passed on `127.0.0.1:18102` and is stopped. Local
PID/invocation `969297`/`c1681e5beb234ce487dbf9b639cbd9dd` and Tailscale PID/invocation
`973390`/`c77a5b3abbe94752b864af9bda53c355` subsequently served only that snapshot before the
trace-enabled services below replaced them. This is provisional dirty-worktree UAT evidence,
never clean-source nomination or public authority.

### M86-C8 — Bounded managed gesture trace

On managed code-project pointer-down, reset one memory-only trace and record exact raw/coalesced/
animation-frame input, native terminal position, authenticated semantic route, staged publication,
first computed mismatch with bits and ULP bounds, parity result, rollback/restore, persistence and
presentation work. The trace is capped at 128 KiB and preserves its opening row plus newest
terminal/rejection/rollback evidence under overflow. It never enters managed source, retained
documents, history, local storage or reproduction payloads.

`Copy trace` is enabled only for managed code projects. It reuses the reproduction overlay in
read-only mode, hides Load, selects text before insecure-context clipboard access and restores
focus to the trace command. An empty trace remains empty until a real pointer gesture; flat/non-code
workspaces keep the command disabled.

Focused bound/sanitization/overflow tests, forced 9-ULP mismatch and full traced rejection tests,
the real three-release Typed Panel causal stage regression, full demo-web 316/316, warnings-denied
Clippy, formatting, locked WASM check, release Trunk build, browser smoke and unchanged golden
`--check` pass. The exact seven-file snapshot `/tmp/geosolve-m86-trace-uat.U1C0QPSf` has aggregate
`f5f429f70e42e3b39a8f22696c19ff81f358cfb10c43f7910baf386c9d82fd44`; local/Tailscale `/` and
every file match it. Evidence `/tmp/geosolve-m86-trace-http-verify.fXS06h` has results SHA-256
`b2de59e63fc30a2dcbef108e671b1038103983bb95fea53d7410bb5799d080f3`. The supervising user's
final close decision accepts this descendant as UAT authority. The separate clean committed-source
nomination below passes.

### M86-C9 — Final clean nomination authority

Accepted source `88d1b5e06a7ce8ffe38931f792492f6f837a1d74`, tree
`09018e5aeb7e824396ae2ee2c70a3e30912414fa`, passes the complete clean Nix gate from 13:16:02 to
13:35:50 AEST with pipeline `0 0`, identical empty pre/post status, unchanged 271-row golden,
native/WASM F002 18/18, demo-web 316/316, TypeScript, licences, Rustdoc, package, benchmark,
release-performance and final Trunk checks. The 6,573-line, 438,432-byte log at
`/tmp/geosolve-m86-clean-gate.w0UKa8fu/release-gate.log` has SHA-256
`e3adef1b33f1b840d9bc44ea7e30a5c76248766187d705eb1bdeb682cfc3bad0`.

Without rebuilding, the gate output was frozen at `/tmp/geosolve-m86-clean-uat.d7DF9hcM`. It contains
exactly seven top-level regular files, zero symlinks/nested entries, directory/files `0555`/`0444`
and ordered-manifest aggregate
`d9d88bfb8ac4acd3f8d45f2cbbc965694297d76b61192be12c4fe65b9deb557e`. Isolated temporary HTTP
PID/invocation `3502269`/`c3a6eeec322d454ab97b246fbd67e8e1` at `127.0.0.1:18104` returned
exact bytes for `/` and every file; results SHA-256 was
`cda649921ad08469b50642f23afda334c3fff821848a8365718e0713ca9f909b`, with evidence at
`/tmp/geosolve-m86-clean-freeze-evidence.MDl33z2L`. The temporary verifier is retired and refuses
connections. Pages and retained-service retirement then pass in M86-C10.

### M86-C10 — Exact public authority and service retirement

Approval head `ccf791f131ba8de07a0d32df6938719cf4bcab12`, tree
`be39d05c8a5dc2f76db91f6be3270f0b95dd8c5e`, passes Pages run `33232073614`, assemble/deploy jobs
`99046509230`/`99047255428`, deployment/status `6152066188`/`17489139435` and artifact
`9708871725`. Its 4,975,804-byte ZIP SHA-256 is
`182080c59b54815a11fa799de4068ed2d92cd4b3e716baae81f0c91e25d56ae7`; the sole 15,267,840-byte
tar SHA-256 is `6459745421cfa3a80038400d80d50c25cef7b46935fb37638b7ca3e7d39a80d8`. Validate exactly seven
regular files, zero symlinks/non-regular entries and manifest aggregate
`ecf6a5550c54fe8fecc1f635500c3a2b208638beacb38ee98da379e8dcd7a7d2` before trusting extraction.

At `https://arduano.github.io/geometric-constraint-solver/`, `/` plus all seven artifact paths
must return HTTP 200, zero redirects, exact MIME, `Content-Length` and artifact bytes, no
`Location`/`Content-Encoding`, and root equal to artifact `index.html`. Results
`/tmp/geosolve-m86-pages-verify.NfnpNi/results.tsv` have SHA-256
`9a6c0df627cfde7a9a4deef3b38946b219addf67dd4b9084be279570fc01623f`. The Pages artifact is a
fresh repository-prefixed build and final public authority. Both M86 user services are
inactive/dead with `MainPID=0`, no exact listener remains, and both endpoints refuse with curl exit
`7`/HTTP `000`; evidence is `/tmp/geosolve-m86-service-retirement.tLQ4hcSG`. M86 is closed.

## M87 managed-control and browser-free authoring fixtures

Status: **COMPLETE and accepted on 2026-08-31**. These fixtures retain M87's focused owner and
crossover evidence for managed controls, browser-free authoring, the twelve-project catalog, shared
rendering and full-detail retained-camera recovery; the entire adaptive-detail/LOD prototype remains
deleted. The supervising user's close decision accepts M87-U9/U10 at milestone level without a
separate row-by-row replay.
Exact source `32c7289`, tree `38f7175`, passes the complete clean release gate. No immutable
candidate, public deployment or service-retirement result is claimed; the mutable Tailscale
listener remains development infrastructure. M87 adds no milestone-neutral golden row. M88
followed and is now complete.

### M87-C1 — One shared Typed Panel radius, complete fan-out

Load the checked-in **Typed Panel · keyed Fillets** project, expand its accepted artifact and derive
the managed-control manifest. Resolve `cornerFillets.radius: mm(4)` without consulting an
`EditLens`. Exactly one editable source control must cover the exact `mm(4)` span and disclose both
generated Fillet-radius consumers, including each invocation/template/member/output identity and
current owner generation. Neither generated Fillet receives a second source control.

Apply an exact-CAS replacement `mm(2)`. Exactly one source token changes; both accepted generated
Fillets have radius `2`; their stable semantic aliases and logical/native ownership remain valid;
all geometry is finite, every active computed feature is Current and the independently recomputed
normalized Hard residual is at most `1e-9`. The live outer owner adds exactly one history row. Undo,
Redo, cold reload and manifest reinspection restore the exact source/value/fan-out sequence, while
the old token becomes stale after the first edit.

Negative rows alter source bytes, project identity, expected IEEE bits, generated generation,
unit, schema or one authenticated consumer. Each rejects atomically without rewriting source or
accepted authority. `+0.0` and `-0.0` are distinct expected values.

### M87-C2 — Direct literal matrix and deliberate read-only leaves

Construct bounded managed-only projects whose direct declarations expose every current
declaration-backed scalar class: real unit and unitless numbers, integer values, booleans, closed
choices, constraint/dimension suppression or driving state, explicit branch/orientation state and
computed-feature parameters. Every independently editable leaf has one authoritative schema with
finite/domain bounds and round-trips through one exact typed source rewrite.

Separately validate the closed schema/replacement helper for bounded Natural values and escaped
Text formatting. No current real declaration or managed-only fixture owns either Natural or Text,
so this helper-level coverage does not claim a manifest control fixture and must not manufacture a
synthetic owner.

In the same sources inspect point/solver-instance coordinates, references, structural identities,
object/array containers, member keys/order, explicit `null` and a value behind an unproven or
incompatible inverse. Each parser-owned read-only leaf publishes the applicable typed reason and,
for a reference, the actual owner navigation path; it never receives an editable token. An absent
member owns no parser span and therefore produces no fabricated manifest or read-only row. An
explicit `null` owns its parser span and produces one read-only `Null` row without an editable
token. Every bundled definition leaf must be either schema-backed or deliberately classified.
Deriving the manifest changes no project, expansion or session serialization byte or digest.

Reject an empty batch, duplicate control, overlapping span, wrong representation, non-finite value,
out-of-domain integer/unit value, incompatible shared schemas and independent control/consumer
resource-limit cases before any partial rewrite. An unordered valid multi-edit batch rewrites and
reparses only its complete candidate.

### M87-C3 — One code-control RPC and one outer history

Install the genuine code workbench and send strict bounded `inspect_managed_controls`,
`edit_managed_controls`, `undo` and `redo` requests through the dedicated code-control RPC. Inspect
returns the exact current `CodeSessionIdentity`, manifest and outer history availability. Each
mutation authenticates the expected identity; an accepted Typed Panel radius edit returns the
receipt for its one new outer identity, and Undo/Redo move only that outer cursor while installing
the matching delegated editor checkpoint.

Unknown fields/methods, an oversized request or response, malformed/non-finite data, unsafe integer
identity, foreign/stale expected identity and stale control token return the fixed typed failure
envelope and do not move authority. A representable source candidate which fails native acceptance
returns a retained-failure receipt and diagnostic while leaving the prior accepted editor scene in
place. Rust JSON and the `@geosolve/sketch-code` `CodeControlClient` must accept and reject the same
wire values.

While code owns the workbench, every mutating Intent RPC method rejects with the dedicated code-
authority failure before touching nested history. Read-only Intent inspection and every mutation in
a plain non-code projectional workspace retain their existing behavior.

### M87-C4 — Inspector and grouped Fillet grip share the source owner

Select either generated Typed Panel Fillet and edit its Inspector radius from `4` to `2`. Resolve
the selected alias and exact Inspector output/definition path through accepted expansion provenance
to the C1 control; never decode an opaque alias or guess from a property name. Show that the control
has two consumers, change one source token, update both Fillets, immediately reselect the stable
alias and add one outer history row with exact Undo/Redo/source/native parity. Editing an ordinary
GUI-owned Fillet property continues through the existing Inspector route unchanged.

Repeat through the selected Fillet radius grip. At pointer-down authenticate the generated child,
control token and complete two-feature group. Multiple pointer frames prepare one grouped preview
per frame and display the same proposed radius on both Fillets without parsing/expanding source or
moving history. Release returns one delegated proposal and no nested Intent transaction, then the
outer owner applies the same control edit once. Stale/mixed/duplicate/over-bound groups, cancellation,
camera interruption and retained publication failure restore the prior accepted scene. M86 compact-
grip/endpoint/broad-surface picking priority and ordinary non-code Fillet gestures remain unchanged.

### M87-C5 — Invocation-local custom-patch controls

For Mounting Plate and Adaptive Lanterns, inspect every shared or transitive custom-artifact input
and compare its disclosed consumers to the exact accepted generated provenance. Insert, remove and
reorder keyed members; unchanged keys retain identity, removed keys tombstone and reused keys gain a
new generation. A token from the prior generation cannot edit the new owner.

For PC Water Manifold inspect all three `waterChannel` invocations. A route-specific input edits
only its exact invocation even when every module use has the same artifact/template paths. A value
intentionally shared above those calls discloses and updates all applicable consumers atomically.
No route falls back to the first invocation, ordinal position, native ID or copied coordinate.
Custom patch TypeScript and its pinned data-only artifact remain byte-identical; Rust, WASM and the
headless path never execute TypeScript.

### M87-C6 — Shared renderer bytes and deterministic native raster

Decode the frozen M87 browser scene fixture and compose it with the extracted
`geosolve-sketch-render` API. The browser adapter and shared composer produce exactly the frozen SVG
markup bytes, including icons, annotations, computed geometry and interaction-state classes. The
standalone static composer uses the same target-neutral camera math while deliberately omitting
hover, selection, drafts, inference, action affordances and error overlays.

Fit empty, point-only, degenerate-axis, off-origin and extreme finite bounds. Empty/invalid bounds
use the canonical finite fallback; valid bounds remain inside the logical `1000 x 700` canvas with
64 px margins and a scale clamped to 2--2000 px/model-unit. Static curve tessellation uses 0.25 px
chord tolerance.

M87-F002 additionally submits finite pan inputs whose screen delta overflows and invalid grid work
with negative, zero, NaN, infinite-extent or non-advancing tiny spacing. Camera rejection is
transactional; bounded grid construction returns no path after at most 4,096 total line attempts.
Normal composer bytes remain frozen. These are focused renderer-owner regressions, not golden rows.

Rasterize the self-contained standalone SVG through the native pure-Rust path. The result is exactly
`2000 x 1400`, contains non-background geometry and bundled-font text pixels, and uses no system
font, file/network resource or browser canvas. Oversized dimensions, external image/font resources
and mismatched standalone dimensions reject. SVG bytes are deterministic authority; PNG pixels are
visual evidence, not a solver or cross-platform byte oracle.

### M87-C7 — Stateless browser-free inspect/edit/render loop

Exercise `geosolve-headless` through each admitted input form: artifact-free managed source plus an
explicit project key, canonical digest-pinned `CodeProject` JSON and every bundled demo key.
`inspect` returns the deterministic report and exact transient control manifest. Copy C1's complete
token into an `edit` batch, publish a fresh generation and inspect its emitted `project.json` to
obtain the next token. `render` over that project publishes the same accepted scene.

Every success-like report proves finite current accepted geometry, independently validated Hard
residual at most `1e-9` or a validated empty hard set, and Current active computed features. Cold-
materialize all twelve bundled demos. Repeating inspect/render with identical input produces
identical report, controls and SVG bytes and the fixed semantic PNG properties from C6.

Run the actual CLI subprocess for `demos`, `inspect`, `edit` and `render` with browser/server/DOM/
network/Node facilities absent. A successful output directory is new and contains exactly the
canonical project/source, report/control JSON and SVG/PNG products. Reusing a destination, supplying
a stale token, invalid project/source, failed acceptance or forcing publication failure never
overwrites an existing generation and leaves no partial new directory.

### M87-C8 — DoF drafts, persistence and plain-workspace non-regression

In Typed Panel drag the upper-left point through the existing semantic overlay and repeat M86's
three exact terminals. The managed manifest exposes the coordinate leaves as solver-instance
read-only values; pointer frames and release still use the existing draft/overlay transaction and
never rewrite solved coordinates into `sketch.ts`. Undo/Redo/reload/reproduction preserve the same
overlay precedence, stable aliases, finite accepted geometry, Current Fillets and exact terminal
authority.

Derive controls, perform a shared scalar edit and save/reload. The transient manifest/token bytes do
not enter `CodeProject`, `ExpandedCodeProject`, `SketchCodeSession`, workspace or reproduction wire
authority. Plain Intent workspaces, direct solver hosts and GUI-owned point/Fillet/dimension edits
remain usable without `geosolve-sketch-code`, `geosolve-sketch-render` or `geosolve-headless`.

### M87-C9 — Qualification and UAT authority

The pre-dogfood provisional dirty-worktree gate, routing-board gate and original manufacturing gate
passed for their exact historical pre-F003 slices. The then-current dirty-worktree release gate
passed at exit `0` on 2026-08-30 with:

```bash
TMPDIR=/home/arduano/.cache/geosolve-m87-tmp GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run 'TMPDIR=/home/arduano/.cache/geosolve-m87-tmp ./scripts/release-gate.sh'
```

The current post-F003 reviewed twelve-row code-project ledger has SHA-256
`f6ecd037cef8befc59f9a057fef499a14f0851f8ec5a0d3a1468a69e66a9d1bc`; the historical gate does not
qualify its revised CNC/Gridfinity rows.
Formatting, warnings-denied workspace Clippy, all-feature workspace tests, Rustdoc, locked WASM,
both TypeScript packages, licence checks, release Trunk assembly and unchanged golden checks passed
inside that explicitly dirty mechanical gate. The gate is not a clean source/tree, immutable no-
rebuild freeze, nomination or publication result.

The supervising user's earlier 2026-08-30 scoped disposition remains historical acceptance of
U1-U8 without a separate row-by-row replay after complete LOD removal and the graphics audit. The
later CNC/Gridfinity amendment adds automated evidence. Exact source
`32c72892772ee09f8b904153484b02fd9923dc25` and tree
`38f7175f93c87d11422f5de00e78208f8cf315bb` passed the clean release gate. The supervising user's
2026-08-31 milestone-level close decision accepts U9/U10 without claiming a separate row-by-row
visual replay. The bundles under `/tmp/geosolve-m87-manufacturing-uat.K7YRaV/` and
`/tmp/geosolve-m87-post-f003.FPHP3b/` remain mutable historical evidence, not an immutable freeze or
published candidate. No clean nomination, publication or service retirement is inferred from the
close decision. `docs/M87_UAT.md` records that accepted boundary.

### M87-C10 — Retained frame recovery with complete scene paint

Build a dense accepted scene and exercise pan, wheel and toolbar camera changes. Every retained
camera frame keeps the complete accepted SVG DOM and paint together with semantic/hit authority,
solver state, persistence, exports and headless products. There is no adaptive-detail policy,
scene-complexity threshold, reduced-paint DOM state or display control.

Between camera admission and animation-frame presentation, remove the retained
`.wb-accepted-scene` group. The failed paint records no camera presentation and retains its desired
camera. Both projectional and flat adapters attempt exact transient reprojection, recreate current
accepted presentation when the viewport remains available. A later camera input must still admit
exactly one `camera-only` frame; no solver, materialization,
persistence or durable-panel work may cross that callback.

Focused queue and static-presentation regressions cover this without adding a golden authoring row.
Browser UAT repeats maximize/restore/resizing on the user's dense multi-model reproduction because
presentation feel and platform scheduling remain human/browser evidence, not a mathematical oracle.

### M87-C11 — Robotic cable-harness routing-board dogfood

Load **Robotic cable-harness routing board** as the tenth checked-in code project. Cold expansion
must accept one finite 360 x 220 mm fixture board, four mounting bores, two banks of eight connector
centres and eight keyed open route Polylines with ten vertices each. Exactly 72 native route spans,
80 generated clip circles and 64 generated host Fillets must be present. Board corners, bore and
connector centres and both endpoints of every route are fixed; interior vertices, including
`serviceRoute.vertices.byKey.serviceLoop`, remain ordinary solver-instance points. Every active
feature is Current and independent Hard-residual validation passes.

The separately compiled `harnessRoute` artifact maps each invocation's keyed vertex collection to
clip circles and its open-chain `filletableCorners` collection to existing host Fillets. Derive one
editable `sharedClipRadius = mm(2.4)` control with 80 circle consumers and one editable
`sharedBendRadius = mm(5)` control with 64 Fillet consumers. Replace only `serviceHarness` inputs
with direct local literals. The resulting manifests partition exactly 70/10 clip and 56/8 Fillet
consumers; shared and local exact-CAS edits affect only their own partition, and an earlier shared
token becomes stale. No invocation may route through the first harness, a copied coordinate or a
native ordinal. All three TypeScript patch copies and both artifact copies stay byte-identical.

Insert keyed `inspectionClip` between `strainReliefB` and `sink` on `serviceRoute`. Exactly one new
point, one segment, one clip and one Fillet identity appear. Remove it, then reinsert it. Every
unrelated generated and native identity remains unchanged, the four retired owners advance
generation on reuse, and the previously issued control token cannot cross that generation. Cold
materialization of the localized and structural candidates remains independently valid.

Drag `serviceRoute.serviceLoop` through its generated-address point lens, accept two terminals,
Undo twice, Redo twice and reload persistence. The overlay moves accepted route/clip/Fillet
authority without rewriting `sketch.ts`; the direct Polyline provenance is `GeneratedOverride`.
Exactly audited incremental work is parse/expand/publish `0/1/0`, all 64 host identities remain
stable and Current, and warm materialization contributes no nested history. The browser locator is
the exact generated address: invocation `serviceRoute`, template `polyline/vertex`, member key
`serviceLoop`, output `point`; it never decodes an opaque alias or uses coordinate proximity.

Finally render the unchanged bundled project twice through `geosolve-headless`. Pretty-encoded
report and control bytes, logical scene markup, standalone SVG and pinned-build PNG bytes must be
equal. The report inventory is 104 points, 176 curves, 41 constraints, two dimensions, 64 features
and 136 computed edges. Append the exact routing-board row to the separate ten-project ledger after
PC Water Manifold without altering the manifold row. This scenario adds no equation, route-specific
renderer, milestone-neutral authoring-golden row or TypeScript runtime.

### M87-F003 — Manufacturing samples use relational authority

Reproduce the pre-repair samples through cold materialization. The Gridfinity contour owns 26
literal-seeded points but zero native constraints, so its apparent standard profile is not fully
constrained. CNC locates seven otherwise constrained components using seven unrelated
`FixedPoint` locks. Classify this as a sample-authority/rank-DOF defect, not convergence or an
equation failure. C12 and C13 freeze the repaired minimal-datum designs, independent residuals,
zero numerical/structural nullity and zero equality/bidirectional bounded DOF. Nominal coordinates,
topology, Fillets and 2D/2.5D manufacturing semantics remain unchanged.

### M87-C12 — CNC joinery fit-coupon dogfood

Load **CNC joinery fit coupon · keyed corner reliefs** as the eleventh checked-in code project.
Cold expansion must accept one finite 120 x 140 mm female blank, three 70 mm-wide mortises and
three 95 x 18 mm tabs. The keyed loose, nominal and press mortises have respective heights of 18.4,
18.0 and 17.6 mm. Blank and tab geometry remains ordinary constrained sketch authority; no coupon-
specific solver, editor, renderer or runtime is admitted.

M87-F003 requires exactly one absolute `FixedPoint`, zero `FixedCoordinate` rows and seven
relational construction datums. Each datum is a horizontal or vertical construction span with an
explicit length dimension, locating the shared mortise X station, three mortise Y stations, the
nominal mortise-to-tab station width and the two remaining tab Y stations. The accepted inventory
is 62 declarations, 69 generated members, 14 outputs, 29 points, 47 curves, 36 constraints, 33
dimensions, 10 host outputs, 10 Current features and 23 computed edges. Require numerical and
structural left/right nullity zero plus equality and bidirectional bounded DOF zero.

One shared `3.175 mm` cutter-radius input must own exactly twelve keyed corner-relief circles, four
for each fit station. The circles are explicit conservative dogbone-style overcuts authored into
the profile; they are not inferred toolpaths or automatic cutter compensation. One shared `6 mm`
handling-radius input must own exactly ten existing host Fillets: four on the female blank and two
on each of the three tabs. Both controls disclose their complete bounded fan-out and every active
Fillet remains Current.

Replace only the press-station relief input with a direct local radius. The circle manifests must
partition exactly 8 shared/4 local. Replace only the press-tab handling input with a direct local
radius; the Fillet manifests must partition exactly 8 shared/2 local. Exact-CAS edits to any of
these controls affect only the authenticated partition, and a token from before localization is
stale. Editing one fit-station definition changes only that station and leaves the
other two station definitions, geometry and semantic identities unchanged.

Remove one keyed relief and reinsert the same key. Removal changes only that circle owner;
reinsertion restores only that logical relief, advances only its reused generation and leaves every
other relief, host Fillet and native profile identity unchanged. A token issued for the retired
generation cannot edit the reinserted owner. The localized, edited and structural candidates must
all cold-materialize with finite accepted geometry, independently validated Hard residual at most
`1e-9` (or a validated empty hard set) and Current active computed features.

Render the unchanged bundled project twice through `geosolve-headless`. Pretty-encoded report and
control bytes, logical scene markup, standalone SVG and pinned-build PNG bytes must be identical.
Append its exact reviewed row after the historical routing-board row without changing any of the
first ten ledger rows. The resulting view is a 2D/2.5D fit-design drawing only: it claims no CAM,
toolpath, cutter compensation, machinability or production-manufacturing validation.

### M87-C13 — Gridfinity 1 x 1 x 3U central cross-section dogfood

Load **Gridfinity 1×1×3U section · keyed standard profile** as the twelfth checked-in code project.
Cold expansion must accept exactly one symmetric closed 26-point material contour representing a
vertical central section. Its reference geometry preserves a 41.5 mm outer width, 35.6 mm base-
bottom width, staged base totalling 4.75 mm and rising to a 7 mm cavity floor, 21 mm 3U body,
0.95 mm walls and nominal 4.4 mm lip rise. The single contour must visibly retain the complete
base, cavity floor, both walls and both top-lip profiles; it is not a collection of inferred solids
or boolean results.

M87-F003 requires zero `FixedPoint` rows and exactly one Y `FixedCoordinate` at the base. Thirteen
`symmetricAboutDatumAxis(... axis: "y")` relations govern every left/right contour pair. Five
diagonal standard stages are each governed by horizontal and vertical construction projections,
for ten construction spans and ten component dimensions; the remaining right-side standards use
explicit length plus horizontal/vertical authority. The accepted inventory is 62 declarations, 66
generated members, three outputs, 31 points, 36 curves, 31 constraints, 18 dimensions, four host
outputs, four Current features and 11 computed edges. Require numerical and structural left/right
nullity zero plus equality and bidirectional bounded DOF zero.

One `2.8 mm` floor-radius control owns exactly the two floor Fillets and one independent `0.6 mm`
lip-radius control owns exactly the two lip Fillets. Each manifest therefore has a complete 2/2
fan-out. Exact-CAS editing either source changes only its own pair, preserves the other radius and
pair, and retains stable contour and unrelated Fillet identities. All four active Fillets remain
Current, geometry stays finite and independent Hard-residual validation is at most `1e-9` (or a
validated empty hard set) after cold materialization.

Render the unchanged bundled project twice through `geosolve-headless`. Pretty-encoded report and
control bytes, logical scene markup, standalone SVG and pinned-build PNG bytes must be identical.
Append its exact reviewed row after the CNC row, yielding twelve rows while leaving the first ten
unchanged. This is strictly a 2D/2.5D design/profile sketch: it claims no solid, boolean, print-fit,
CAM, toolpath, machinability or manufacturing-validation authority.

The original C12/C13 focused owner qualification, all-twelve cold materialization and complete
dirty-worktree gate are retained pre-F003 mechanical evidence only. For the revised sources, the
separately reviewed twelve-row ledger, focused owner suite, native composition and all-demo
headless/deterministic products pass. Exact source `32c7289`, tree `38f7175`, then passes the
complete clean release gate. The user's milestone-level close decision accepts U9/U10 without
inventing a separately replayed visual session. No immutable freeze, publication or service
retirement is inferred; the mutable Tailscale listener remains development infrastructure.

Final post-F003 qualification: the focused manufacturing owner suite passes 3/3, native composition
passes 13/13, the exact reviewed-ledger check passes 1/1 and all-demo headless/deterministic
products pass 10/10. Fresh mutable CNC/Gridfinity review bundles remain preserved.

## M88 workflow-led workbench fixtures

Status: **complete and accepted on 2026-09-01**. The immutable M88-F004 React replacement remains
live as the accepted UAT identity. The supervising user's milestone-level close decision accepts
M88-C1 through M88-C5 and M88-U1 through M88-U10 without claiming a separately logged row replay.
The frozen Rust-DOM snapshot formerly served on port `8080` remains untouched rollback evidence
after reboot and is not proof of the current React presentation. These scenarios do not add solver
equations or replace the owning-layer regressions required for the carried Gridfinity stack/
performance defect.

Historical initial React checkpoint: Rust still owns solver, history, managed code, interaction, picking and
accepted SVG authority; React owns the live DOM, CodeMirror, focus/layout, browser file/download
work and guarded browser-local persistence. Middle-button pan bypasses semantic gesture authority,
and the bounded Recent section stores canonical sample identities only. The initial full checkpoint
passed bridge `11/11`, renderer `25/25`, frontend `17/17`, actual-WASM `4/4`, real-WASM Playwright
`6/6` and the complete mechanical gate. Its no-rebuild eight-file candidate at
`/tmp/geosolve-m88-react-uat.TAMXyz`, aggregate
`91c3a2349f1466a64720cb1cfba8a4f7d18aed0be03e9eec256ccf8c4eb0f9de`, WASM SHA-256
`9032a07bc6ac465816b3b3f3bb44c3881f815f290e39eece88890589f2cdfe5f`, passes exact local/frozen/
current-dist manifest equality and historical all-file plus root/index served-byte verification at
`http://100.94.63.83:18088/`. M88-F001 preserves but supersedes those bytes for UAT. Historical
M88-F001 replacement `/tmp/geosolve-m88-react-uat.nGkL4i`, aggregate
`0bd35f3dba50c592c6eea34ed18afed1b6f908803d75feb9ca3bbb620a1572c4`, WASM SHA-256
`beb76d47889055f9d8344ac4cdc8a01fc06a1dbd797979f636927031d3383996`, passes exact dist/frozen/
historical served verification at `http://100.94.63.83:18089/`. M88-F002's post-reboot identity is
recorded below and is now the frozen M88-F003 reproduction target, not a continuing-UAT candidate.
The final close decision accepts M88-C1–C5 at milestone scope without inventing separate replay
observations.

### M88-C1 — Presentation-only Design/Split/Code layout

Open one ordinary sketch and one managed-code project at `1440 x 900`, then switch through Design,
Split and Code and drag each splitter through multiple frames. Split mode must provide at least
`520 x 500` CSS px of editable source; at `1024 x 720`, Code mode must keep at least `720 x 500` CSS
px of usable editor. No mode or pane change may solve, expand code, publish accepted scene/history
or save canonical workspace state. Canvas camera, selection and applicable right-side tab remain
stable. Parameters and Problems retain one logical state while rehosting between layout modes.

### M88-C2 — Durable source editor continuity

Open `sketch.ts`, place a multi-line selection in an uncommitted edit, scroll away from the cursor,
then select geometry, resize panes, switch right-side tabs and round-trip Split/Code. The same file,
exact draft bytes, cursor, selection and scroll must remain. Editable text is at least 12 CSS px.
Apply performs exactly one ordinary outer code transaction; Revert restores the accepted source.
Durable scene rendering must not replace the live editor element.

### M88-C3 — Search, tools and exact source ownership

Freeze the complete pre-redesign command/variant manifest, then reach every entry by pointer and
keyboard with one-for-one inventory parity. From the start/open surface, find and open each of the
25 ordinary and 12 code samples using only keyboard and text search; no hover-only flyout is
required. Semantically exercise one common and one advanced variant from each Sketch, Constraint,
Dimension and Modify group. Selecting a generated Fillet and invoking **Open in code** must select
the exact authenticated owner file/range. Ambiguous or encoded values show a typed reason and never
route to the first invocation.

### M88-C4 — Invalid source and browser/headless handoff

Introduce one source-positioned managed-code error. The prior independently accepted canvas stays
visible and usable, exactly one persistent current-attempt Problem appears, and no accepted history
is published. Correct or Revert the source and require ordinary independent acceptance before the
Problem clears. While either a valid dirty draft or invalid draft exists, canonical export must
return a typed refusal; raw draft-source download is separate. After Apply/Revert, export canonical
`project.json` plus `sketch.ts`; inspect and render it with `geosolve-headless`, requiring the same
controls, Current features, finite accepted scene and Hard residual at most `1e-9`.

### M88-C5 — Gridfinity stability prerequisite

Run the Gridfinity open and `baseBottomWidth: 35.6 -> 20` edit in the release UAT configuration and
under the explicitly supported browser/WASM stack contract. Separately retain a one-MiB native
proxy regression until shared-path stack use is reduced enough to pass; neither result proves the
other. On the recorded machine, five fresh release cold-opens have median/max at most 2.0/2.5 s and
five edits have median/max at most 1.25/1.75 s. Instrument cold open to prove a checkpoint is not
restored/validated twice and no no-op post-open encode/save occurs. Any full-row-rank redundancy
shortcut is tested first at `geosolve-core` and must preserve finite state plus independent
residual, domain, branch, rank and DOF validation.

### M88-F001 — Semantic Explorer and Inspector node labels

The deterministic default document previously rendered `IntentGraphNodeKind` through Rust
`Debug`, leaking `IntentBootstrapMetadata` twice across Explorer and Inspector together with
payload-like `Bootstrap { ... }` text. The bridge must reuse the compact semantic node-family label
already owned by the workbench. Explorer rows display only the node title and omit the redundant
type/detail suffix; Inspector may display exactly one compact semantic kind. Row and group icons
remain non-shrinking under long titles. No visible default-document text contains
`IntentBootstrapMetadata` or `Bootstrap {`.

The exact bridge regression failed on the leak before the repair and now passes. Real release-WASM
Playwright asserts the negative leak strings plus Explorer row/icon and Inspector structure.
Post-fix qualification passes bridge `12/12`, full demo-web `384/384`, frontend `17/17`, real-WASM
Playwright `6/6`, focused all-feature library Clippy, `npm run check`, distribution validation,
format and diff checks. The candidate WASM is built with Cargo `--release`, wasm-bindgen and
`wasm-opt -Oz`; no debug candidate is nominated.

Historical pre-React evidence: the one-MiB native proxy, full-row-rank core owner and actual-WASM
open/edit regressions passed. Five optimized Chrome 151 processes measured cold-open median/max
`1561.9/1795.7` ms and edit median/max `897.1/1314.3` ms. Final open/edit maximum normalized Hard
residuals were `2.8866e-15` and `4.8486e-12`; accepted numeric state was finite and no browser trap
or runtime error occurred. Read-only snapshot `/tmp/geosolve-m88-uat.nmhcRj`, aggregate
`6c4af7e96e30687654d691b577954c22b051fb3ad37e9472c51e16c9c002a274`, remains preserved after its
transient `http://100.94.63.83:8080/` listener disappeared on reboot. The React replacement's
actual-WASM contract now
passes mechanically and its frozen browser/build identity is recorded above; candidate-specific
timing exercise and human execution of M88-C1–C5 through `docs/M88_UAT.md` were still pending at
that historical F001 checkpoint.

### M88-F002 — Semantic CAD toolbar and canvas-local view controls

The migrated React rail had collapsed the prior CAD glyph vocabulary and semantic families into
generic icons, including a scissors-like bucket that mixed unrelated authoring and display actions.
Reproduce at the Rust/React boundary by inspecting the static command catalog and opening every
rail category in real release WASM. The hot snapshot must not carry the static catalog.

Require exactly five rail entries: Select, Sketch, Constraint, Dimension and Modify. Sketch owns
all 25 geometry variants grouped by geometry family. Constraint owns 13 tools grouped as Placement,
Orientation, Equality & symmetry and Curve join. Dimension owns five tools grouped as Linear,
Circular and Angular. Modify owns only Fillet and Offset. Every row and rail category renders the
bounded Rust-owned CAD icon key. Profile/Construction appears in contextual tool settings. Grid,
Fit sketch and Center on origin appear as exactly three canvas-local controls and no display action
appears in Modify.

Activate Segment, then toggle Grid and invoke Fit/Origin plus the geometry-role route. The active
tool must remain Segment throughout. Category replacement, outside pointer dismissal, destination
click delivery and Escape focus restoration remain deterministic. At `1024 x 720`, scroll the full
Sketch catalog to its end and require both elliptical-arc labels to fit without horizontal
overflow; at `1440 x 900`, require the full catalog to fit and every long label to remain readable.
The canvas view toolbar stays inside the canvas with 12 px top/right insets at both sizes.

Owning bridge regressions pass 14/14 and prove the static catalog is complete/bounded/absent from
hot snapshots plus canvas-action active-tool neutrality. Complete demo-web passes 386/386,
frontend passes 18/18 and final real release-WASM Playwright passes 7/7 with no runtime, console or
network errors. The exact post-reboot snapshot `/tmp/geosolve-m88-react-uat.kdSCUU`, aggregate
`d328fdded4ae963230eef2c64c5fb22459dec7a55467e0a7c051bed739b2cd47`, release-WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`, is served by
`geosolve-m88-react-uat-current.service` at `http://100.94.63.83:18088/`. Root/index/all eight
paths byte-match with HTTP 200 and a frozen-endpoint browser smoke passes. Human M88-C1–C5 and UAT
were still pending at that historical F002 checkpoint. M88-F003 below subsequently withdraws this
snapshot from continuing UAT.

### M88-F003 — Ordinary click-click geometry authoring survives capture release

Reproduce in frozen release-WASM snapshot `/tmp/geosolve-m88-react-uat.kdSCUU`, WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`, based on source
`71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`. Activate Segment and click two distinct canvas
positions without dragging; repeat with Center–Radius Circle. Before repair, the browser paints a
stage but neither ordinary click-click sequence commits. Direct calls through the Rust bridge do
commit both geometries, routing the failure to the React pointer-capture lifecycle rather than the
presentation-independent authoring state machine.

The browser's ordinary terminal ordering is `pointerup` followed by `lostpointercapture`. Require
the viewport to remember the exact captured pointer and retire that ownership synchronously on
`pointerup`, before awaiting adapter work. The expected later capture-loss event is stale and must
do nothing. A genuine `pointercancel` or unexpected loss while ownership is still active must
cancel exactly once. Bridge cancellation must dispatch every returned editor effect, including
`ClearConstructionPreview`, so canceled staged geometry leaves no misleading draft paint. Live
provisional Fillet and Profile Offset drags must cancel through their projectional restoration
routes; their cleanup acknowledgements must never become false durable Problems.

Freeze the contract at four layers: a React unit regression that distinguishes normal release from
genuine loss, direct Rust bridge Segment plus Center–Radius Circle click-click parity, a direct
bridge cancellation-clears-preview regression and ordinary-click authoring through the real
release-WASM Playwright stack. Add live Fillet-radius and provisional Profile Offset capture-loss
rows requiring capture retirement, inactive drag routes, unchanged revision/export authority,
accepted status and zero Problems. Post-repair qualification passes bridge `19/19`, full demo-web
`391/391`, frontend `19/19` and Playwright `8/8`; `npm run check`, focused all-feature demo-web
Clippy, formatting, the locked WASM check and distribution validation pass.

The exact eight-file replacement is `/tmp/geosolve-m88-react-uat.QkVU1k`, aggregate
`e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167`, optimized release-WASM
SHA-256 `5f49f49a880dd8529982bfbcf68a3f1a92c79ee96f256ed56931254ca884b22a`. It has zero symlinks,
read-only modes and exact final-dist/frozen/local-HTTP/Tailscale-HTTP parity. User service PID
`518679` serves it at `http://100.94.63.83:18088/`; every path is HTTP 200 and the frozen-endpoint
Chrome scenario commits Segment and Center–Radius Circle with zero runtime errors. M88-C1–C5 and
all human UAT rows were still pending at that historical F003 checkpoint.

### M88-F004 — Truthful canvas chrome and adjacent affordances

Reproduce against frozen F003 release-WASM snapshot `/tmp/geosolve-m88-react-uat.QkVU1k`, aggregate
`e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167`. In Select, observe an
apparently actionable Profile/Construction control with no contextual effect. Activate Profile
Offset in an unavailable context and observe the error row move the canvas vertically. Activate
Polyline before placing a point and observe an enabled Finish action that performs no work.

Require Rust presentation authority to publish `canFinish`, `canUndo` and `canRedo`. For Polyline,
assert Finish false immediately after activation, false after one point, true after two points and
false after completion. A premature direct Finish must preserve exact revision, project bytes,
history and retained draft. Fillet/Profile Offset may finish only when their retained candidate
matches the painted preview. Enter completes only from canvas focus and only under the same gate.
The role control is hidden when irrelevant and explicitly labels either new-curve authoring role or
selected-curve role. Error feedback is a fixed bounded overlay; compare canvas bounds before and
after an error and require equality.

Exercise adjacent false affordances: ordinary and outer-code Undo/Redo false/true transitions and
dirty-draft blocking; complete preselection relation application returning to Select; camera and
first-Escape gesture cancellation retaining the active Offset collector; no primary canvas tool
navigation in Code-only layout; functioning Ctrl-O/Ctrl-S; blocked project replacement and exact
reproduction while source is dirty; and a right-click that creates no pointer-owned draft while the
browser context route remains unprevented.

The exact regressions pass bridge `23/23`, full demo-web `395/395`, frontend `26/26` and real
release-WASM Playwright `10/10`. Focused warnings-denied Clippy, locked WASM check, release build,
distribution validation, formatting and diff hygiene pass. Immutable eight-file snapshot
`/tmp/geosolve-m88-react-uat.KGhA7s`, aggregate
`700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`, WASM SHA-256
`22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`, is exact across build,
frozen, staged and live HTTP. PID `1021511` serves it at `http://100.94.63.83:18088/`; the live
Chrome smoke proves secondary-click neutrality and Polyline Finish readiness with zero runtime
error. The user's 2026-09-01 close decision accepts this F004 identity, M88-C1–C5 and M88-U1–U10
at milestone scope without a separately logged replay. The obsolete Rust-DOM compatibility host is
retired from source; the accepted Tailscale service stays live and no public deployment or service
retirement is inferred.

## M89 executed, reversible managed-sketch fixtures

Status: **Superseded by M90's closed clean-break contract.** M89-F004/F005 implementation, final
provisional dirty-tree mechanical qualification and immutable F005 nomination completed, but the
Compass Rose retest, targeted F004/F005 preflight and M89-U1 through M89-U8 were not run and are not
retrospectively passed or waived. F005 and every earlier M89 snapshot are historical rather than
current product candidates. This is not clean-source qualification or standalone M89 human
acceptance; M90 owns replacement product authority.

### M89-C1 — closed-subset normalized round-trip

Parse a v2 file containing Unicode comments/identifiers, scalar expressions, direct declarations,
a patch invocation, groups, declaration and generated-member suppression, and no final outputs
return. Print, reparse and compare the complete IR. Comments remain attached, order and suppression
are exact, and the normalized second print is byte-identical to the first. Independently attempt
loops, helpers, conditionals, mutation, async, dynamic imports and ambient access; each rejects
before callback execution at its exact source position.

### M89-C2 — execution-derived graph and edit provenance

Execute an instrumented managed callback whose one radius feeds two generated Fillets. Require a
complete typed result tree, both generated members, source-site IDs and the exact two-consumer
provenance edge set with `edit_lenses: []`. Rewrite the radius through that provenance and require
only the two consumers to change. Prefix the source with multi-byte text and prove Rust byte ranges
select the exact CodeMirror UTF-16 token. Stack frames are never accepted as an edit address.

### M89-C3 — atomic source-backed canvas authoring

For every enabled geometry recipe, every standalone-replayable persistent constraint, and
representative dimension and operation recipes, start from an accepted code project and complete
one ordinary gesture. The preview performs zero source compilation. Release appends exactly one
user-facing declaration closure under `Canvas additions`, executes and cold-materializes it,
publishes one independently accepted scene plus one outer history row and leaves no GUI-only
duplicate. Most closures contain one declaration; Profile Offset keeps its explicit aggregate
helper and operation root in one authenticated closure. Cancellation, stale tickets, unsupported
projection and forced native failure preserve exact accepted source/IR/artifact/scene/history.
Undo/Redo restores all authorities together; the declaration high-water name is not reused.

Across the 25 geometry variants, require direct `$.geometry.line(...)` for Segment, direct
`$.geometry.polyline(...)` for exact native Polyline and compact lossless
`$.geometry.recipe(...)` for the remaining 23. No geometry gesture may emit a transport-level
`$.intent.recipe(...)` declaration. Rust independently authenticates the compact form against the
central recipe descriptors; TypeScript contains no geometry equation or independent result schema.

Across the 35 persistent constraint variants, require direct `$.constraint.horizontal(...)` and
`$.constraint.vertical(...)` for the two axis relations and compact lossless
`$.constraint.recipe(...)` for the other 31 standalone-replayable variants. Cold replay must
preserve the exact semantic signature, finite accepted state and independently validated Hard
residual at most `1e-9`. `ExternalPointCoincident` and `ExternalLineCollinear` must fail closed
because managed source lacks immutable host snapshot/binding authority. Keep them compile-visible
and schema-authenticated, but require separately supplied host snapshots; F004/F005 implement no
standalone host-snapshot execution path, so neither is a passed standalone replay row.

For computed Fillet, require one direct `$.computed.filletSet(...)` declaration with lexical
native-span parents, exact parameter/winding/neighborhood/normal-side/retained-endpoint/periodic-
anchor/endpoint-order/sweep state, radius, display name and explicit source-owned suppression.
Cold replay must retain computed ownership and branch/contact metadata. The existing Profile Offset
closure is unchanged; F004/F005 introduce no new Offset scenario or claim.

### M89-C4 — ordered declaration and suppression surface

Display top-level source declarations in lexical order with patch-generated members nested beneath
their owner. An exact private Profile Offset aggregate nests beneath the operation root: it remains
selectable, editable and source-navigable, but independent reorder/suppress/delete refuses. Moving
or deleting the root operates on both explicit declarations as one source block; an aggregate with
another semantic consumer remains an independent root. Move two independent declarations and
require the source/IR order to change after reload. Attempt to move a producer after its consumer
and require a typed source-neutral refusal. Suppress and restore one direct declaration and one
generated member; explicit source/IR state, accepted scene, persistence, Undo/Redo and headless
inspection must agree.

### M89-C5 — legacy upgrade and host parity

Open every checked-in v1 sample without changing its bytes. Make one structured edit in an active
copy, require one complete v2 normalized source with no outputs return or edit lens, then reopen the
bundled sample and recover its original bytes. A persisted v1 project with GUI-only hybrid
additions rejects entirely. Compile the same v2 source in browser and pinned Deno and compare all
digests; then render its compiled project twice through native headless without Deno, requiring
deterministic output, finite geometry and independently normalized hard residual at most `1e-9`.
M90 is the planned clean break that normalizes bundled legacy samples and removes the v1 path; this
fixture proves M89 preserves those bytes and compatibility instead of performing that future work.

### M89-F001 — Compass Rose Polyline active-copy upgrade

Reproduce against the withdrawn 2026-09-01 release-WASM candidate by opening Compass Rose, choosing
Sketch → Polyline, clicking three points in a right angle, and pressing Finish. The deterministic
browser fixture uses normalized canvas positions `(0.35, 0.40)`, `(0.60, 0.40)`, and
`(0.60, 0.65)`. Intended behavior is one managed-v2 active-copy transaction containing the
Polyline geometry and inferred horizontal/vertical constraint declarations under `Canvas
additions`, an accepted scene with two added curve spans and one outer history row. Observed
behavior was unchanged managed-v1 `sketch.ts`; native candidate expansion retained
``semantic reference `core.markers` cannot be resolved`` and published no candidate authority.

Classify this as `DEFECT` at `geosolve-sketch-code` patch-invocation expansion. Compass Core has
two declared collection outputs, `markers` and `ring`. The v2 path lowered their nested members but
did not publish each authenticated named output path when the invocation root itself was an opaque
feature, so existing `core.markers` references failed before the added Polyline could publish.

The repair publishes and kind-checks every declared patch output, while the code-session layer
permits accepted-v1/current-v2 disagreement only for a retained failed upgrade and rejects clean or
inverse mismatches. The frontend resolver exposes the retained Rust diagnostic instead of masking
it as recursive compiler work. Regressions are:

- `m89_f001_compass_v2_multi_collection_outputs_materialize_cold`;
- `m89_f001_compass_rose_managed_v1_canvas_segment_upgrade_publishes_atomically`;
- `m89_f001_retained_v1_to_v2_failure_preserves_accepted_version_authority`;
- `clean_current_v2_with_accepted_v1_version_mismatch_is_rejected`;
- `retained_current_v1_with_accepted_v2_version_mismatch_is_rejected`;
- frontend `surfaces Rust's retained native diagnostic instead of masking it as recursion`;
- optimized Playwright `Compass Rose Polyline Finish upgrades and publishes inferred constraints
  into sketch.ts`.

Historical F001 checkpoint qualification passes the complete `geosolve-sketch-code` crate,
demo-web `357/357`,
frontend `54/54`, warnings-denied Clippy, locked WASM check, unchanged clean golden
survey/check/require-clean, optimized release-WASM Playwright `13/13` and the complete provisional
dirty-tree release gate. The exact gate-produced nine-file output is frozen without rebuilding at
`/tmp/geosolve-m89-f001-replacement-uat.IwvrBh8x`, ordered-manifest aggregate
`7d40c31c0eba5aae0d1b6e7febf16a0b5fa44a541c82a9ddbbb0125cd8707424`, release-WASM SHA-256
`a2b2fd4ec4852e3d16b10ab0fa41eb8e4d64b52db430447bdbf28bb8d82f3241`. Strict byte verification
and the exact no-build Compass regression pass first at temporary `:18189` and then at
`http://100.94.63.83:18089/`; unit `geosolve-m89-react-uat-current.service`, PID `1589113`,
invocation `d05d85152a4547f7b32e316b6a3d4b9a`, historically served only the frozen snapshot until F002.
The withdrawn snapshot remains preserved as defect evidence and M88 remains unchanged. Human UAT
is not inferred.

### M89-F002 — compact semantic source for canvas-authored Polyline

Reproduce on the withdrawn F001 replacement by opening Compass Rose, choosing Sketch → Polyline,
clicking a right angle and pressing Finish. The exact captured source with one inferred axis was
`4,238` bytes/`172` lines (SHA-256
`fa76f529e5bce663e54d22409498ed1cb382dc5e682ae13017a9bddede5f4925`); the two-axis form was
`4,943` bytes/`199` lines (SHA-256
`43cd56d5c617e98be4ded3022dcc062b7497f444b1c454465143d8e75616f133`). Both correctly published
source-backed accepted geometry, but encoded an ordinary Polyline and inferred constraints as
transport-level `$.intent.recipe(...)` declarations.

Classify this as `DEFECT` at `geosolve-sketch-code` reverse projection. Only Segment had an
ergonomic projection, so Polyline and Horizontal/Vertical fell through the complete generic recipe
serializer. The repair may use a compact builder only after proving the exact accepted node shape:

```ts
const geometry1 = $.geometry.polyline("geometry1", {
  vertices: [
    { key: "v0", position: [0, 0] },
    { key: "v1", position: [20, 0] },
    { key: "v2", position: [20, 10] },
  ],
  closed: false,
  role: "profile",
  representation: "singleCurve",
});
const constraint2 = $.constraint.horizontal("constraint2", {
  curve: geometry1.segments.byKey.v0,
});
```

Deterministic `vN` keys own vertices and directed spans. An input-bound vertex retains its lexical
point reference. `representation: "singleCurve"` is an explicit opt-in that lowers to one native
Polyline recipe; omitting it preserves the historical composite direct-Polyline lowering. At the
F002 checkpoint an unproved shape retained the generic recipe fallback; F003 replaces that fallback
with compact `$.geometry.recipe(...)`. Open result collections contain all vertices,
only `n - 1` spans and only interior filletable corners; closed results retain their cyclic sets.

Regressions require:

- `canvas_polyline_and_inferred_axes_project_to_compact_semantic_declarations`;
- `compact_polyline_preserves_an_input_bound_vertex_as_a_lexical_point_reference`;
- `polyline_without_the_exact_single_curve_source_contract_uses_the_compact_geometry_fallback`;
- `opt_in_single_curve_polyline_round_trips_one_native_node_and_exact_span_references` and its
  legacy composite/invalid-form collateral;
- `prepared_mutation::tests::open_polyline_insertion_result_uses_exact_keyed_subsets`;
- managed-v2 runtime execution of the compact three-declaration form without
  `$.intent.recipe`, at most `30` lines and `900` bytes;
- the optimized Compass browser case with one Polyline, two direct axis constraints, no generic
  recipe, at most `75` lines/`2,500` bytes for the complete source, exact declaration navigation,
  two added accepted spans, revision `r2`, one Undo route and no runtime error.

Focused Rust, TypeScript and frontend suites pass. During the real browser regression, prepared
mutation validation first exposed open-Polyline result overpublication and terminal parity exposed
the staged-source/pre-source-GUI alias split. One shared exact result-leaf reconstruction now owns
cold and prepared validation, and the authenticated prepared declaration-label projection bridges
only that canvas publication seam; ordinary code drags still require the exact expansion alias.
The final optimized release-WASM no-build regression passes `1/1` against both frozen staging
`:18189` and live `http://100.94.63.83:18089/`. The isolated source is `24` lines/`722` bytes; the
complete normalized Compass source is `64` lines/`1,841` bytes (`65`/`1,842` with the CodeMirror
trailing blank). The post-audit optimized release-WASM run passes `13/13`, and the frozen normal
product passes `12/12` without rebuilding. Snapshot `/tmp/geosolve-m89-f002-uat.bSM6Nn59`, aggregate
`0b9a4ed357a452edd79953679e5bbb68c0cb5b1261a8ba5478a1dbaa597ab433`, release-WASM SHA-256
`77e5739cb81565d5a0bc400507e1a35f2f63ba22c1d821442b16baedcb24b72a` and evidence
`/tmp/geosolve-m89-f002-freeze-evidence.Xs977S3o` own the superseded F002 candidate. The pre-audit
`/tmp/geosolve-m89-f002-uat.Taocj5bI` bytes remain preserved but withdrawn. The F002 service has
been replaced after exact F003 staging proof; these bytes are not F003 acceptance authority.

### M89-F003 — compact semantic source for all geometry variants

Reproduce from any F002 canvas-authoring path other than Segment or exact-native Polyline; Cubic
Bezier is the smallest conspicuous example. The geometry publishes correctly, but reverse
projection serializes the native draft as a transport-level `$.intent.recipe(...)`, exposing
selectors, initial-instance leaves and operation-output structure that are not useful authored
code. Classify this as a systemic `DEFECT` in `geosolve-sketch-code` reverse projection and managed
execution. It changes no solver equation, geometry definition or accepted scene.

The repaired 25-variant matrix is exact:

- Segment: direct `$.geometry.line(...)`;
- exact native Polyline: direct `$.geometry.polyline(...)`;
- compact lossless `$.geometry.recipe(...)`: Sketch Point, Midpoint Line, Two-Point Aligned
  Rectangle, Three-Point Corner Rectangle, Center Rectangle, Three-Point Center Rectangle,
  Center–Radius Circle, Two-Point Diameter Circle, Three-Point Circle, Center Arc, Three-Point Arc,
  Tangent Arc, Center-Axes Ellipse, Axis-Endpoints Ellipse, Center-Axes Elliptical Arc,
  Axis-Endpoints Elliptical Arc, Quadratic Bezier, Cubic Bezier, Rational Quadratic Conic, Parabola,
  Hyperbola, Open-Control NURBS and Periodic-Control NURBS.

For every compact-recipe row, start from the real presentation-independent canvas construction
plan, accept the native candidate, reverse-project it, compile the emitted source and cold-
materialize through Rust. Require exactly one geometry node of the original recipe kind, byte-
identical ordered semantic result path/kind descriptors, finite accepted points/scalars and
independently validated normalized Hard residual at most `1e-9`. Require canonical slash paths,
exact writable coverage and fail-closed rejection of missing, duplicate, unknown, non-canonical or
wrong-kind inputs/fields/values/results. TypeScript may parse, execute and expose dynamic result
references, but Rust must re-derive and authenticate all structure from central Intent descriptors;
no TypeScript geometry equation is permitted.

Tangent Arc additionally retains its source-span dependency, contact parameters, neighborhood,
orientation/branch state and cold replay even when writable contact values came from materializer
defaults. Segment separately preserves Construction role. Polyline retains its keyed native spans
after reopen, and logical multi-output patch roots with no native node do not shadow authenticated
concrete outputs.

Focused regressions and ceilings are:

- `compact_geometry_recipe_roundtrip.rs`: `3/3`, comprising 22 independent compact variants,
  separate Tangent Arc replay and direct Segment role; existing direct Polyline tests complete the
  25-variant catalog;
- `m89_editor_insertion`: `13/13`, and the complete all-feature sketch-code suite: `109` unit tests
  plus every integration/doc test;
- every generated single compact declaration at most `1,024` bytes; Cubic Bezier `387`, Tangent Arc
  `921`, Periodic NURBS `872`, and the full normalized three-shape TypeScript sketch `3,087` bytes;
- TypeScript runtime `27/27`, mutation `18/18`, pinned-Deno `2/2` plus type/managed checks, frontend
  `54/54`, focused warnings-denied sketch-code Clippy and `git diff --check`;
- the exact Compass Rose integration regression passes after the logical-root repair.

Final qualification passes demo-web `357/357`, formatting, affected warnings-denied Clippy, the
locked WASM check, optimized nine-file distribution validation and release-WASM Playwright
`14/14`. The unchanged normal product passes `13/13` before freezing and on frozen staging and live
endpoints. Frozen path `/tmp/geosolve-m89-f003-uat.uF6Yjsc3`, aggregate
`c37d832a6dc076982e3fda8f2ffbcc8b2f26ed99ad6ffd0da0b7ba4f96d705ed`, optimized release-WASM
SHA-256 `99eeaa5668276282201cb1971b042d3d0904a13b7a75d40e2357ce08a34c748a` and evidence
`/tmp/geosolve-m89-f003-freeze-evidence.5TcRPlem` own the historical F003 candidate. Its
staging/live ten-route HTTP
ledgers are identical at SHA-256
`c5efab20409773cc7f43cecdd9c6fca5b961402611d073f6bf0e8f40cca0ce1e`; unit
`geosolve-m89-react-uat-current.service`, PID `3637680`, invocation
`73bdb82cf2a64436ab6f65fe76b08ccf`, served it at the historical checkpoint and has since been
replaced by F005 at `http://100.94.63.83:18089/`. F003 remains rollback evidence; human scorecard
acceptance was not inferred.

### M89-F004 — compact semantic source for persistent constraints

Reproduce from a canvas-authored non-axis relation such as Parallel after F003. The relation is
accepted and published atomically, but reverse projection uses transport-level
`$.intent.recipe(...)`. Classify this as a systemic `geosolve-sketch-code` reverse-projection and
managed-execution defect, not a solver-equation or constraint-semantics defect.

Run the complete persistent `ConstraintKind` catalog as one focused matrix. For Horizontal and
Vertical, require the existing direct builders. For the other 31 standalone-replayable variants,
require one descriptor-authenticated `$.constraint.recipe(...)` declaration, exact canonical
input/field/value/result coverage, cold Rust materialization, the original semantic signature,
finite accepted state and independently validated normalized Hard residual at most `1e-9`. Reject
unknown, missing, repeated, malformed, non-canonical or descriptor-inconsistent content before
publication.

The matrix must inspect all 35 persistent variants but report exactly 33 standalone replay passes.
`ExternalPointCoincident` and `ExternalLineCollinear` require immutable host-external snapshot/
binding authority that `sketch.ts` does not encode. They remain compile-visible and schema-
authenticated, but require separately supplied host snapshots. F004/F005 implement no standalone
host-snapshot execution path: require fail-closed projection, do not serialize them dishonestly and
do not count them as passed cold-replay rows.
Focused regression
`every_standalone_constraint_reverse_projects_and_host_external_kinds_remain_input_gated` passes
`1/1`. Final provisional dirty-tree candidate-wide qualification and immutable F005 nomination
also pass as recorded below.

### M89-F005 — direct computed-Fillet source ownership and lifecycle

Author a computed Fillet between two accepted native spans. Require one direct
`$.computed.filletSet(...)` declaration under `Canvas additions`, not
`$.intent.recipe(...)`. Its source must include lexical native-span parents, exact parameter,
winding, neighborhood, normal-side, retained-endpoint, periodic-anchor, endpoint-order and sweep
state, radius, display name and explicit source-owned suppression. Cold replay must reconstruct the
same computed owner, children and branch/contact state with finite accepted geometry and
independently validated normalized Hard residual at most `1e-9`.

Exercise a same-gesture Polyline/Fillet so the parents use deterministic keyed segment paths.
Exercise a non-default periodic branch and require exact replay. Independently reject a computed
host arc as a native parent plus non-lexical, wrong-kind and malformed state without partial
publication.

Select the Fillet from the accepted canvas/Explorer and require `Modifiable in source` plus the
whole authenticated declaration span. Edit its radius through the source-owned control and require
selection to survive cold rematerialization. Suppress, restore, delete and Undo; require exact
source, accepted scene, one outer history route and Fillet ownership to follow each operation.
Persist/reload and repeat the source-navigation and ownership assertions. Direct Fillet lowering
passes `7/7`, and `m89_editor_insertion` passes `19/19`; focused bridge ownership/lifecycle tests
and the release-WASM Fillet case `1/1` pass.

F005 adds no Offset work. Keep the M89-C3/M89-U3 Profile Offset closure scenario unchanged and
introduce no broader Offset acceptance claim.

Final F004/F005 mechanical scenario qualification uses exact gate
`nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh'`. The ambient attempt
successfully built through workspace and golden checks, then its first WASM parity leg returned
`HARNESS_ERROR` only because `wasm-bindgen-test-runner` was absent after reboot. The pinned shell
with runner `0.2.121` passed the complete gate; this was a harness/environment failure, not a
product defect. Final counts are sketch-code unit `114`, compact geometry `4/4`, constraint matrix
`1/1`, direct Fillet `7/7`, `m89_editor_insertion` `19/19`, demo-web `358/358`, TypeScript runtime
`28/28`, mutation `19/19`, pinned-Deno parity `2/2` and frontend `55/55`. Historical F003 counts
above remain unchanged.

Historical immutable M89 F005 scenario authority is snapshot `/tmp/geosolve-m89-f005-uat.hzNuDxF0`,
manifest `/tmp/geosolve-m89-f005-uat.hzNuDxF0.sha256`, evidence directory
`/tmp/geosolve-m89-f005-freeze-evidence.VAoQDl8n` and aggregate
`fb488ad2bf29e8897cf9811c002b748693e5d211bae4bb54c83ed060db5db668`. Its nine-file distribution
contains three JavaScript files, one CSS and one `16,333,537`-byte WASM, and has two directories
and no symlinks/other entries;
the WASM is `assets/geosolve_demo_web_bg-52ybei8k.wasm` at SHA-256
`3e6f515ff1e5de0f668c13e86c02d280c0dc085bbd89b314bf9ece6c82aae575`. Staging/live strict ten-
route ledgers are byte-identical at SHA-256
`4de184eb1eb237f98997b70702567a2b110b40d96df5d0d9653f024ac5f23e8d`; the focused Fillet case
passes `1/1` and normal frozen product `15/15` on each endpoint. The normal product intentionally
excludes the compiler-parity-only harness test.

Unit `geosolve-m89-react-uat-current.service`, PID `1007459`, invocation
`a3fea6dddb59438795e52c6a8fab136f`, started `Wed 2026-09-02 20:21:15 AEST` with the F005 snapshot
as `WorkingDirectory` and was recorded serving it at `http://100.94.63.83:18089/`. Staging PID `995137`,
invocation `efe4de2264774f68bf5852284fa69887`, is retired. This is provisional dirty-tree M89
evidence, not clean-source qualification or row-by-row human acceptance. The Compass retest,
targeted preflight and M89-U1 through M89-U8 were not run and are not retrospectively passed or
waived. M90's closed clean break supersedes this compatibility-stage nomination.

## M90 typed executed sketch clean-break fixtures

Status: **Closed by explicit scoped supervising-user approval on 2026-09-04. M90-F005/F006 repairs,
collateral qualification, the complete dirty-tree release gate, optimized release-WASM build and
immutable Tailscale nomination pass. M90-U1 through M90-U10 transfer/defer, without passing or
waiver, into M91's composite UAT. The exact closing candidate remains Tailscale-only at
`http://100.94.63.83:18090/`; no GitHub Pages deployment or public push was authorized or made.**
These scenarios define the qualified contract and do not claim that a human row passed.
`docs/M90_GOALS.md`, `docs/M90_IMPLEMENTATION.md` and `docs/M90_UAT.md` own the current product and
interaction contract. The M89 section above remains a historical compatibility-stage record.

The historical pre-F001 pinned dirty-tree release gate exited `0`; its log has SHA-256
`2dd3663430c730eea84303954598f3a0696868aa4d433f3e32836a42024b250b`. The unchanged 271-row
milestone-neutral golden passes `--survey`, `--check` and `--require-clean`; that is not clean-source
qualification. Historical snapshot `/tmp/geosolve-m90-uat.vuI7sBKt`, manifest
`/tmp/geosolve-m90-uat.vuI7sBKt.sha256` and aggregate
`b5bae1aca28787f026a11100c94e425d1c5e057ce3539170f4399b7cd8b05dc2` preserve the withdrawn
immutable release-WASM nomination. Its `20,007,307`-byte
`assets/geosolve_demo_web_bg-BAUG7n7P.wasm` has SHA-256
`51fc04d4129dd73791afb20b4403efe1f4fb95af6d607037b2a95956865fe7f5`. Local and Tailscale HTTP
ledgers matched at SHA-256 `17477e87e897b5ac080547df41b528bc16c252d4c67a4634ad21a384f4ccd29b`.
Historical unit `geosolve-m90-uat-18090.service`, PID `2747435`, invocation
`767e65048ece4573834acf8f580f87bc`, served that working-directory snapshot at
`http://100.94.63.83:18090/`. M90-F001 withdraws the nomination from continuing UAT while retaining
all identities and hashes above as exact reproduction evidence. The unit is inactive/dead with no
PID; the endpoint is now reused only by the replacement identity below. At that historical
checkpoint no M90-UAT row had been accepted and M90 was not closed.

### M90-C1 — one directive and one reproducible V3 authority

Compile a representative managed sketch containing comments, named values, groups, declaration
dependencies, explicit suppression and Unicode before source spans. Require the exact
`"use geosolve sketch"` directive, `geosolve-managed-sketch-ir-v3` and
`geosolve-executed-sketch-artifact-v3`. Parse, canonically print and parse again; require identical
lexical meaning, declaration/group order, names, expressions, suppression and source navigation.
Execute the printed source and require the same authenticated declaration/results and value-
consumer graph. Reject a v1/v2 directive or envelope, a generic recipe, tuple-key input table,
edit lens, result manifest and operation-output transport payload before native publication.

### M90-C2 — named typed catalog and result routing

Compile and cold-materialize the catalog fixtures for all 25 geometry variants, all 33 standalone
constraints, all eight dimensions, every ordinary operation, both aggregates and computed
`FilletSet`. Each declaration uses its family-specific named method and typed named-argument
object. Bezier rows expose `start`, `control`, `end` or `start`, `firstControl`, `secondControl`,
`end`; binary relations expose semantic operands rather than indexed transport slots. Require the
complete authenticated result shape and lexical dependency path for every row. Keep
`ExternalPointCoincident` and `ExternalLineCollinear` input-gated and fail closed without their
immutable host inputs. A custom patch may use its private compiled transport, but that transport
must not become an admitted managed V3 declaration.

### M90-C3 — execution-derived value provenance without edit lenses

Use one lexical radius value in several direct declarations and in a keyed computed-Fillet
consumer. Require instrumented execution to record every consumer at stable source sites and Rust
to authenticate that graph against the lexical IR. Prepare one exact value mutation, compile its
candidate receipt and resolve it through Rust. Every intended consumer changes, unrelated values
remain byte/semantically unchanged, and one outer history row publishes. Remove, duplicate or
retarget one runtime consumer/source site and require source/IR/artifact authentication to reject
without consulting a stack trace or explicit `editLens` metadata.

### M90-C4 — canvas declarations always return to source

From a managed sample, author representative geometry from every geometry family, inferred and
explicit constraints, one dimension, an ordinary operation and a computed Fillet. Each accepted
gesture inserts a compact named declaration in `sketch.ts` with a monotonic generated identifier;
deletion and Undo never permit identifier reuse. Reload from the published source/IR/artifact and
require the same standalone-authoritative geometry and relationships with no nested GUI-only
addition. Exercise Fillet radius edit, non-default branch/contact state, keyed members,
suppress/restore, delete and Undo. Profile Offset may use only its authenticated aggregate-helper
plus operation-root closure.

### M90-C5 — one prepared transaction for every structural edit

For value edit, insertion, reorder, suppression and deletion, bind a Rust-prepared ticket to the
exact accepted project/source digest and expected semantic delta. Have the compiler host apply and
execute the candidate, then require Rust to validate the receipt before cold materialization,
native solving, branch/domain checks and independent Hard-residual validation. A valid edit
publishes source, IR, artifact, scene and exactly one outer history entry together. Exercise stale
ticket, wrong candidate bytes, extra semantic change, dependency-invalid reorder, unsupported
syntax, NaN/Inf and native validation failure; every case retains the complete previous accepted
authority and publishes no partial history.

### M90-C6 — twelve normalized samples and cold native replay

Load all twelve checked-in sample source/envelope pairs and require V3 authority immediately, with
no legacy prompt, compatibility parser or active-copy upgrade. Cold-materialize every project in
Rust and require finite points/scalars, Current active computed features and independently
validated normalized Hard residual at most `1e-9`. Reopen each project and require deterministic
declaration identities, explicit branch/orientation/span/winding state and usable source-backed
selection. Any mismatch between checked-in source, lexical IR, executed artifact or project digest
rejects rather than reconstructing missing authority from GUI state.

### M90-C7 — browser-free compiled use and authenticated Deno editing

Run native `inspect`, solve and SVG/PNG rendering twice from a compiled sample with no browser,
DOM, web server or JavaScript runtime; require deterministic accepted reports and static output.
For a raw-source value change, run Rust `prepare-edit`, the pinned Deno mutation sidecar and Rust
`resolve-edit`. Require the receipt to authenticate the one permitted change and the resulting
project/report/SVG/PNG to agree with browser semantics. Refuse an existing output directory, a
different Deno/compiler identity, stale source or altered receipt without overwriting any prior
file or accepted project.

### M90-C8 — release candidate and human interaction boundary

Only a fresh byte-verified release-WASM candidate recorded in `PLAN.md` may enter M90 UAT. Execute
the ten rows in `docs/M90_UAT.md`, recording each as pass, fail or explicit waiver, or retain them
as unexecuted and transfer/defer them only under explicit scoped supervising-user approval. Automated
catalog, native, TypeScript, frontend, WASM, golden or release-gate evidence may nominate that
candidate but accepts no human row. M90 closes only after the supervising human explicitly
disposes the scorecard and approves the milestone.

### M90-F001 — code-owned point drag must not compile managed source

Exact user reproduction: open **Compass Rose**, drag a point and observe that the entire canvas is
then blocked by `pointer input is unavailable while a managed-source mutation is compiling`.
Independent reproduction classifies this as a code-workbench/Rust-bridge ownership defect rather
than a solver, rank/DOF or equation defect.

The faulty terminal route prepared `ManagedSketchMutation::SetValues` and invoked the browser
compiler. The native preview had accepted one finite continuation, while cold rematerialization
from the rewritten single point seed chose another valid configuration of the underconstrained
sketch. Exact native/compiler parity correctly rejected that disagreement. The rejection retained
the ticket in `pending_managed_mutation`, and the bridge's global pending guard then rejected every
later pointer gesture with the misleading "compiling" message.

The repair is frozen at the code-workbench bridge with ordinary pointer-down/move/up Compass Rose
cases and selected-reference detachment. Successful terminals publish a persistent
`CodeInteractionOverlay` synchronously, retain the accepted native continuation, produce finite
accepted geometry with independently validated Hard residual and add exactly one outer/native
history action. They require exact unchanged `sketch.ts`, lexical IR, executed artifact and compiler
identity, no browser compiler request, no `pending_managed_mutation` and acceptance of an immediate
following pointer gesture. A failed or mismatched delegated terminal clears its exact semantic
route and restores accepted editor, selection, project, source, code-session, persistence and
history authority before the next gesture. Separately retain managed compiler routing for explicit
source/scalar edits, declaration insertion/reorder/suppression/deletion and computed-Fillet radius
edits. No solver equation, rank/DOF rule, tolerance or explicit branch state changes.

The four focused demo-web bridge regressions and sketch-code
`managed_v3_instance_overlay_is_exact_cas_history_and_persistence_authority` pass `1/1` each. Full
demo-web `--lib` passes `282/282`; sketch-code all-feature and affected multi-crate suites pass;
frontend Vitest `56/56`, TypeScript runtime `31/31` plus types/managed/build, release-WASM exact
two-drag Compass `1/1`, locked WASM parity/`actual_wasm`/check, format, diff and warnings-denied
workspace Clippy pass. The golden survey is clean; after a transient combined exit `101`, both
affected scene rows pass exact rerun and complete unchanged `--check`/`--require-clean` pass. The
monolithic dirty release gate ended by harness termination at exit `143` and is not a pass. Final
normal release-gate completion and all human UAT remain pending; no milestone closure is asserted.

The historical pre-F002 provisional dirty-tree replacement is frozen at
`/tmp/geosolve-m90-uat.xk0AGnz0`,
external manifest `/tmp/geosolve-m90-uat.xk0AGnz0.sha256`, aggregate
`030e9f4aa98690b8cd35cdbb51a29220674f1bfcfba310192afc467f5afc38a4`, with nine mode-`0444`
files, two mode-`0555` directories and zero symlinks. The `19,958,913`-byte
`assets/geosolve_demo_web_bg-B6mOdH7K.wasm` has SHA-256
`9607cfd48f1ec23b2c29e120704277bbb70247bdbed6a67945c5cc64b7af8761`. Evidence is
`/tmp/geosolve-m90-f001-replacement-freeze-evidence.UpMqFyrm`. All ten staging/live routes are
byte-identical with correct MIME, no redirects/compression and ledger SHA-256
`56a5aff7b23000e1b009f2eb479b9545fcfb17dbe5d1a4f9721f7c3951a761ae`; optimized release-WASM
two-consecutive-drag Compass passes `1/1` against each endpoint. Tailscale-only
`geosolve-m90-f001-replacement-uat-18090.service` historically served that snapshot with PID
`3332035`, invocation `258e6d4da4b14661bd6d8e44856c64a5`, on `100.94.63.83` at
`http://100.94.63.83:18090/`; M90-F002 subsequently retired it.

This is immutable provisional dirty-tree replacement evidence only. The terminated exit-`143`
monolithic gate is not promoted to a pass. At that historical checkpoint the replacement gate
remained incomplete and all ten human rows remained Not run.

### M90-F003 — canvas insertion closes generated unit imports

Start from the exact authored-empty coded sketch whose SDK import contains only `sketch`. Select
Center-Radius Circle, click once for its center and once for its radius, and preserve the exact
generated declaration including finite center coordinates, monotonic symbol/label, positive
`radius: mm(...)` and `role: "profile"`. Before repair, the mutation printer left the import
unchanged and compilation failed with
`managed sketch mutation is invalid: unsupported managed sketch value expression`. Classify this
as a managed-source compiler/prepared-receipt `DEFECT`, not a geometry-solver defect.

Require TypeScript mutation compilation to change only the owning SDK import to
`import { sketch, mm } from "@geosolve/sketch-code";`, cold-recompile the complete candidate and
record radius consumer provenance. A second or batched insertion must not duplicate or reorder
`mm`; an existing helper stays in place; a unit-free Segment changes no import; and an angle-bearing
insertion appends `rad` once. Generated values admit only finite `mm`/`rad` units and recurse through
nested arrays/objects. Preserve every unrelated import declaration/binding exactly.

Require Rust to derive the identical helper closure before computing candidate semantic authority.
The exact circle receipt passes; an extra helper, missing helper, changed helper order, removed
existing binding, changed custom import or additional unrelated import rejects atomically. Every
non-insertion mutation must retain imports exactly. Also cold-lower the emitted direct circle with
its required `center`/`radius` and optional `label`/`role`; map those presentation fields to native
display name/geometry role, and continue to reject any other field. This is the second blocker
revealed once the helper import compiled.

At the retained bridge, repeat the two-click gesture and resolve the real compiler receipt. Before
the receipt, source, revision and history remain unchanged. After it, require exactly one source and
outer-history publication, exactly one imported `mm`, one finite circle with positive finite radius,
Current active features and independently validated normalized Hard residual at most `1e-9`.
Require no pending mutation, an immediately usable complete pointer gesture and Undo back to the
byte-exact starter. Repeat this contract through optimized release-WASM Chromium before nominating
new bytes.

M90-F003 withdraws the post-F002 snapshot `/tmp/geosolve-m90-uat.TN2NP9eF` from continuing UAT;
its exact hashes and service identity remain historical evidence in `PLAN.md` and
`docs/M90_UAT.md`. TypeScript package checks pass with `34` runtime tests; Rust prepared-mutation
passes `34/34`, all-feature sketch-code and the exact retained bridge pass; frontend Vitest passes
`73/73`; format, warnings-denied Clippy, diff hygiene, golden `--check`/`--require-clean`, frontend
build checks and distribution validation pass. Optimized release-WASM F003 and carried F001/F002
browser rows pass `1/1` each on staging and live.

The post-F003 provisional snapshot is `/tmp/geosolve-m90-uat.yIPVNICT`, with external manifest
`/tmp/geosolve-m90-uat.yIPVNICT.sha256`, aggregate
`d31e311c4e0e69974690d299819df6a33b7ae13b7eff5d037406e602c033f33a`, and freeze evidence
`/tmp/geosolve-m90-f003-freeze-evidence.Y4jLQxD3`. Its `19,967,322`-byte
`assets/geosolve_demo_web_bg-DuuevZyx.wasm` has SHA-256
`c3160d7f8f6fc49db6294588cedd38ba5b520a80743d3977039957074fa8ca31`; staging/live ten-route
ledgers byte-match at SHA-256
`47a0229dbf46ea0549f4e424a6ce7ccd452810eb24161db61c4cb33436107726`. Historical Tailscale-only
`geosolve-m90-f003-replacement-uat-18090.service`, PID `305121`, invocation
`f456b1c8cea044638bae1119709b94d9`, served only that snapshot at
`http://100.94.63.83:18090/`. M90-F004 withdraws it from continuing UAT; the service is inactive/
dead and the immutable snapshot remains historical pre-F004 evidence. No human row was accepted.

### M90-F004 — simultaneous Point-on-Curve declarations retain native ownership

Start from managed source containing two accepted Center-Radius Circles. Draw one Segment whose
endpoints snap to the two circumferences. One managed insertion must publish the Segment, two
Point-on-Curve relations and inferred Horizontal together. Before repair, the two simultaneously
ready Point-on-Curve declarations received monotonic source names in GUI insertion order but native
objects were allocated in durable Intent-symbol order during terminal and cold materialization.
Their source-declaration/native-contact ownership crossed, strict parity rejected with
`declaration relabel witness is not owned by its allocated source declaration`, and the rejected
ticket left later pointer input falsely blocked as compiling.

Canvas insertion now assigns already reserved names within each namespace/name family in the same
durable native-symbol order used by cold replay. The exact bridge regression requires one atomic
four-declaration publication, explicit periodic Point-on-Curve parameter/domain/winding/
neighbourhood/orientation state, finite accepted geometry, Current features, independently
validated normalized Hard residual at most `1e-9`, immediate next-pointer availability and exact
Undo. A thin frontend regression consumes only the unchanged authenticated terminal relabel
rejection; stale or unauthenticated receipts remain retryable. No solver equation, rank/DOF rule,
tolerance or explicit branch state changes.

The focused owner and frontend recovery tests, full demo-web `284/284`, sketch-code all-feature,
TypeScript runtime `34`, frontend Vitest `74/74`, format/diff/warnings-denied Clippy, optimized
release-WASM build and browser bundle pass. The 271-row golden remains byte-identical because this
focused ownership/lifecycle defect exposes no missing systemic matrix dimension.

Historical provisional snapshot `/tmp/geosolve-m90-uat.O4xZJBxg`, manifest
`/tmp/geosolve-m90-uat.O4xZJBxg.sha256`, aggregate
`fd0a4635edcc6bd24d36eeca831a57bbb62cdf1d67589c6245af9e7b88bf52be`, and freeze evidence
`/tmp/geosolve-m90-f004-freeze-evidence.F0HdfUMF` preserve the immutable post-F004 candidate. Its
`19,987,485`-byte `assets/geosolve_demo_web_bg-CoONmEL1.wasm` has SHA-256
`e61ff5e9183883c1872293ad5d4c38c06175bc12575668f3f770282387bcf457`; staging/live HTTP ledger
SHA-256 is `47589602797db38fb23a70da0d1cc31c7b032b7ab0907f3688d2a39886ebe921`.
Tailscale-only `geosolve-m90-f004-replacement-uat-18090.service` historically served it with PID
`792138`, invocation `ff367dd5f5bb4f24a8661dd83a26168b`, at
`http://100.94.63.83:18090/`. The supervising user's reboot stopped that unit, and M90-F005
withdraws the bytes from continuing UAT. At that post-F004 checkpoint no current candidate remained
live. The normal monolithic release gate, M90-U1 through M90-U10 and explicit closure remained
pending; M90 stayed open.

### M90-F005 — accepted contact-workspace terminal must not snap back

Restore the exact user-supplied native workspace from
`/home/arduano/Downloads/project (1).json`, `956,305` bytes at SHA-256
`c5f748d31c90f8fd575ab2acaddfb8b7d20bbc9f31189dc0716995ad05a46ee7`. Its checked-in bounded
capsule is `crates/geosolve-demo-web/tests/fixtures/m90_f005_native_drag_repro.txt`, `86,736` bytes
at SHA-256 `1b1dba9d039ee8756030174731ab3a03e7f77a8554853a96c29e4b10f8c49cf7`; require ordinary capsule
decode to recover the supplied workspace byte-for-byte. Drag each connected point whose persistent
ID ends `066d`, `0670`, `0673`, `0674`, `067e`, `0685` or `0686` by a finite visible distance.
Point `...067f` is an unconstrained control and is not substituted for any of those seven cases.

Before repair, the first blocker treated a periodic contact's native `ScalarUnit::Angle` storage as
an authored angle even when the owning leaf was `Parameter`. Require one closed mapping in which
generic `Value` inherits its valid native unit, `Angle` is angle, and `Weight`/`Parameter` retain
dimensionless authored semantics; a periodic angle-backed `Parameter` is specifically
dimensionless. Use that mapping for ownership validation, bootstrap application, point-drag reverse
projection and curve-control reverse projection. Reject every invalid field/storage pair.

After that unit repair, preserve the second red case: the contact-rich document is disconnected and
underconstrained. A fresh sequential solve of the correct terminal patch can select another valid
representative for unrelated free degrees of freedom and produce `PreviewColdMismatch`. Require
point release to project the exact already accepted terminal preview onto the candidate's cold
native design and independently certify it. The continuation carries numerical accepted state
only; it must not add an Intent operation, source declaration, temporary drag, history entry or
branch choice. Apply the same certification boundary to a curve-control terminal.

For Undo/Redo, reconstruct the target history position using its stored authenticated accepted
materialization as continuation. Intent history may retain a newer allocator cursor, so reconcile
only the candidate-owned persistent-identity high-water before exact native projection. Require all
objects, topology, sources, branch/contact state and numerical materialization otherwise to match.
Undo must restore the pre-drag accepted document exactly and Redo the terminal accepted document
exactly. Do not require complete workspace bytes after Undo to equal their pre-drag bytes: the
composite history intentionally advances CAS revision and preserves a Redo entry.

Exact bridge owner
`m90_f005_supplied_native_workspace_constrained_drags_publish_and_undo` runs the seven points as
independent continue-through-failure cases and passes `1/1`. Each case requires a moved finite
preview, release publication equal to that terminal preview, no managed compilation, one revision/
history action, finite state, Current features, independently validated normalized Hard residual at
most `1e-9`, retention of contact ID/curve/parameter/domain/winding/neighbourhood/orientation,
exact accepted-document Undo, exact terminal Redo and persistence/reload retention. Focused
projectional applications pass `12/12`, including scalar-unit, Point-on-Curve, mixed line/circle
tangency and disconnected-contact continuation coverage.

No equation, rank/DOF rule, tolerance or explicit branch state changes. Constraint-editor
all-features passes with unit layer `439/439` plus every integration suite, demo-web passes
`286/286`, frontend Vitest passes `74/74`, warnings-denied workspace and targeted Clippy pass, and
format/diff hygiene plus the unchanged reviewed 271-row golden pass. The optimized release-WASM
build and nine-file distribution validation pass, and the supplied-workspace browser row is one of
the `4/4` rows passing on both staging and live. Automated evidence accepts no M90-UAT row.

### M90-F006 — imported project retains live viewport/pointer alignment

Import the supplied workspace while the browser owns a non-default letterboxed canvas. Before
repair, atomic `project.import` replaced the complete bridge and therefore reset live host size and
pixel ratio to `1000 x 700` and `1`. The DOM box itself did not change, so `ResizeObserver` emitted
no new sample. SVG presentation continued using the real browser dimensions while Rust pointer
normalization used the defaults; a visible point could miss semantic hover by about 60 CSS pixels.

Successful import must retain the authenticated live `host_size` and `pixel_ratio` after complete
project restoration and before atomic bridge replacement. Failed restoration must remain entirely
non-mutating. Exact regression
`successful_project_import_retains_live_viewport_and_pointer_alignment` owns a non-default
letterboxed viewport and semantic point hover. This is presentation-host state only and changes no
document, solver, branch, tolerance, history or persistence authority.

The first corrected four-row browser attempt compared SVG coordinates from the drag-time camera
with coordinates after a deliberately fresh restore fit. Exact project and accepted authority were
unchanged, so this is `HARNESS_ERROR`, not a product finding. Fitting both presentation sides makes
the comparison deterministic; the corrected persistence row passes `5/5` repeated and retains its
persistence and Undo checks. The separate Rust F005 owner regression proves both Undo and Redo.

Current immutable optimized snapshot `/tmp/geosolve-m90-uat.EtWyWQlt`, external manifest
`/tmp/geosolve-m90-uat.EtWyWQlt.sha256` and freeze evidence
`/tmp/geosolve-m90-f006-freeze-evidence.HqyaA7Qp` have ordered aggregate
`b3fd72b9ec98d318d7bfa7bf8c09d0fcbd3856ea0723d81e01e64945e301debe`, nine mode-`0444` regular
files, two mode-`0555` directories and no symlinks. Its `19,990,463`-byte
`assets/geosolve_demo_web_bg-Dc5MH04n.wasm` has SHA-256
`bad16242c2ec0fa0c6c0ba6882428372c1bf0b7dd1a70235467b5febdfc80712`. Staging/live HTTP ledgers
match at SHA-256 `41d11e1c56bad8dcc57edf229f0bfec20d8f5e602b3c385e5e68d54dd42c816a`; optimized release-WASM
browser rows pass `4/4` against both endpoints. Tailscale-only unit
`geosolve-m90-f006-replacement-uat-18090.service`, PID `462021`, invocation
`4a17e69e926446eba21439ac4dd6f4e6`, exact-serves only that snapshot at
`http://100.94.63.83:18090/`.

### M90-C9 — full gate and scoped closeout

The final full dirty-tree gate command
`env GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'`
ran from `23:20:45` through `23:47:22 AEST` on 2026-09-03, exited `0` after `1,596,726 ms`, and
wrote the `573,421`-byte log
`/tmp/geosolve-m90-f006-full-gate.hKSTh4/release-gate.log` with SHA-256
`bda7f5f92f15a5f0a0cf26ed93cb514943d9a9d1ad49bf0ba0e148c9239b205d`. The reviewed 271-row
golden remained unchanged, the release-only 256-moving-body performance row passed in `137.82 s`,
frontend Vitest passed `74/74`, and the optimized distribution validated nine files. This is
complete dirty-tree qualification, not clean-source qualification.

The supervising user explicitly approved scoped closure on 2026-09-04. M90-U1 through M90-U10
remain unexecuted and transfer/defer—not pass or waive—into M91's single composite UAT. Automated
evidence accepts no human row. The immutable F006 snapshot and service above remain the exact
Tailscale-only closing publication; no GitHub Pages deployment or public push was authorized or made.
M90 is closed.

## M91 cohesive code-driven-authoring fixtures

Status: **Implementation complete; nomination evidence pending.** No candidate is nominated and all
14 composite human UAT rows remain exactly Not run. These fixtures mechanically qualify the five
integrated workstreams; they do not substitute for `docs/M91_UAT.md`.

### M91-C1 — authored contact range reconciles from accepted continuation

Start with a source-owned Point-on-Curve contact accepted at parameter `0.8`, then change only its
optional admissible range to inclusive `[0, 0.5]`. Intrinsic topology remains owned by the referenced
curve. The authenticated retained/accepted pair may seed the candidate numerically, but candidate
source remains sole authority for objects, topology, range, winding, locality, orientation, host
inputs and publication.

The feasible case must publish exactly at `0.5`, report the active upper bound, preserve unrelated
geometry and remain draggable within the new interval. Removing the authored range restores the
intrinsic topology. Native and managed fixtures must agree. An impossible range/contact combination
retains exact source, accepted document/scene and history authority; its bounded diagnostic names the
contact, requested range and empty feasible intersection. Undo/Redo and later valid recovery must not
erase the rejected attempt or fabricate success.

### M91-C2 — advisory TypeScript language intelligence

Create a syntax error, unknown builder/property and wrong argument type in `sketch.ts`. The dedicated
TypeScript 5.9.2 Worker must return precise diagnostics plus contextual completion, hover and
signature help from the generated pinned SDK declarations. Fixing the source clears the results.

Language analysis never applies a mutation or changes accepted source/IR/artifact/native scene/
history. Requests are generation-stamped and bounded by file count, UTF-8 bytes and result count.
Stale replies, worker synchronization faults, project replacement and disposal settle pending UI
state harmlessly. Declaration-drift qualification proves the Worker and compiler see one API.

### M91-C3 — all 37 samples are source authoritative

Enumerate the one visible catalog and require exactly 37 unique code projects: the 25 former native
samples exported deterministically plus the 12 curated projects. Each has readable `sketch.ts` and a
matching compiler envelope; no native/code UI split or automatic upgrade remains. Native
constructors are test-only semantic references.

For every entry require cold finite materialization, Current feature authority, independently
validated normalized Hard residual at most `1e-9`, expected raw/effective DOF, one representative
edit with exact Undo, parser/printer/runtime inverse mutation and release-WASM frame/source parity.
Scotch Yoke, Scissor Jack and Five-stage Scissor Tower each execute one deterministic intended
managed drag and retain its accepted terminal. Computed Fillets remain computed definitions rather
than flattened ordinary curves.

Current Segment, Midpoint Line and Polyline source preserves explicit branch direction. Only the
authenticated historical contact-schema migration may normalize the legacy representation needed by
the exact M90 supplied workspace. Spline construction is explicit across open/periodic control
B-spline and open/periodic control NURBS; Parametric C2 preserves exact `firstRate`/`secondRate`.

### M91-C4 — Explorer visibility composes without design mutation

Toggle individual geometry and generated members, then their owning groups. A group bulk toggle
must retain descendant choices; mixed and inherited states remain truthful. Isolate one group and
restore the prior visibility vector. Toggle the pinned Construction filter, reload and repeat after a
managed reprojection.

Effective hidden entities do not paint, pick or expose controls/annotations. The operations must not
change source, IR/artifact, compiler identity, native solve state, suppression or history. Stable
semantic visibility keys, not row position, bind persisted choices after restore/reprojection.

### M91-C5 — native/managed semantic golden parity

For each applicable authoring, lifecycle, computed-Fillet and accepted-scene golden case, export the
native semantic case to typed managed source, compile it through the pinned batch, cold-materialize
through the ordinary managed path and compare canonical semantic snapshots. Require geometry and
constraint meaning, dimensions, operations, explicit branch/contact state, rank/DOF, independent
residual validation, history/lifecycle and accepted-scene authority. Do not compare backend IDs,
serialization bytes or arbitrary underconstrained coordinates as semantic identity.

The reviewed exclusion ledger contains exactly four reachable fail-closed cases:

- `constraint.external-line-collinear.*` — no immutable external-line host snapshot/binding;
- `constraint.external-point-coincident.*` — no immutable external-point host snapshot/binding;
- `dimension.profile-offset.*` — a flattened document cannot recover the aggregate-helper/root
  source closure;
- `spline.noncanonical-knot-topology.*` — the typed recipe cannot reproduce knot-inserted native
  control structure.

Reject missing, duplicate or stale exclusions. Each exclusion counts as no parity result. Fillet
cannot be excluded and must retain computed geometry/provenance and scene authority on both paths.

### M91-F001 — raw compiler envelopes exceed release bundle ceilings

The first integrated optimized build produced a `27,296,927`-byte WASM against a strict `< 20 MiB`
limit and `36,085,444` distribution bytes against a strict `< 30 MiB` limit. The owner is
`geosolve-sketch-code::demos`: all 37 source-authoritative compiler envelopes, about `10.96 MB` raw
JSON, were embedded with `include_str!`. This is a release-bundle `DEFECT`, not a solver, equation,
sample-semantics or server defect.

The pending focused repair must deterministically zlib-compress only
`assets/{samples,demos}/*.compiled.json` at build time using pure-Rust `miniz_oxide`. Runtime lazily
inflates each exact envelope once. Before accepting it, enforce the existing `MANAGED_WIRE_LIMIT` on
compressed and declared decompressed length, exact output length, complete input consumption, valid
zlib checksum/terminal status and UTF-8. Preserve public `compiled_source: &'static str`, exact
authenticated JSON bytes, catalog order and deterministic semantics.

Focused owner coverage must compare all 37 reconstructions byte-for-byte with test-only raw assets
and reject oversized input/output declarations, short/long lengths, truncation, corruption, trailing
bytes and invalid UTF-8. The repair, final source and measured output remain pending in this document:
`@M91_F001_COMMIT@`, `@M91_WASM_BYTES@`, `@M91_DIST_BYTES@`, `@M91_WASM_SHA@`.

### M91-C6 — qualification and immutable nomination boundary

From one clean commit run formatting, warnings-denied all-target/all-feature Clippy, locked
all-feature workspace tests, TypeScript/package/frontend/declaration checks, the complete 271-row
golden survey/check/clean sequence and full release gate. Freeze the already-built distribution
without rebuilding, make regular files mode `0444` and directories `0555`, reject symlinks, write an
external sorted SHA-256 manifest, verify local/live bytes plus MIME/redirect/compression behavior and
serve only through a distinct transient M91 Tailscale service. Do not modify/restart M90 or publish
GitHub Pages.

Evidence placeholders are `@M91_FINAL_COMMIT@`, `@M91_FINAL_TREE@`, `@M91_RELEASE_LOG@`,
`@M91_RELEASE_LOG_SHA@`, `@M91_SNAPSHOT@`, `@M91_MANIFEST@`, `@M91_SNAPSHOT_SHA@`,
`@M91_WASM_ARTIFACT@`, `@M91_WASM_BYTES@`, `@M91_WASM_SHA@`, `@M91_HTTP_LEDGER_SHA@`,
`@M91_SERVICE@`, `@M91_SERVICE_PID@`, `@M91_SERVICE_INVOCATION@` and `@M91_UAT_URL@`. Until they
are all replaced with verified identities, status remains “Implementation complete; nomination
evidence pending”. Automated evidence does not execute, pass or waive a human UAT row.

## Frozen near-singular fixtures

The regression corpus includes:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures test truthful singularity/rank reporting and finite state retention. They do not demand arbitrary global branch selection. M9 makes the machine-floor numerical rank contract and distinct near-singular warning band mandatory.

The detailed L3 fixture above demonstrates that geometric alignment does not itself justify an M9 warning when the selected driver makes the reported position/velocity matrices full-rank and well-conditioned. The detailed sketch fixture demonstrates actual dependent gradients and therefore does report numerical singularity.
