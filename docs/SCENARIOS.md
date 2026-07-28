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
- R5: rotating the A5 tangent line through its endpoint applies a transient stability target to the opposite cubic Bezier handle. The opposite handle and endpoint remain stable while the constrained handle and line satisfy contact, tangent orientation and driving length.
- R6: every supported constraint and dimension exposes a typed transactional editor delete
  action. Deletion removes owned hidden state, enters history only when accepted and restores
  the same persistent IDs on undo.
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
companions; M60 completes the advanced workbench and M61-M64 own the remaining human-UAT/release
scope. Every
new fixture must name its exact design, parameter, external-snapshot, activation and
accepted-state revisions. The workbench remains a desktop-only public-API consumer; no
mobile scenario is required.

Objective geometry, residual, derivative, rank, branch, persistence, migration, resource,
cancellation, presentation-adapter and topology assertions are directly automated at their
owning Rust/WASM layer. Old browser E2E is not a qualification path. Human review is
limited to completed M40.7 and M53, active M61 and planned M63 after direct
automated qualification.

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

### Future-HI1 - Remembered reference inference is headless

This scenario records a future interaction contract; it is not an M40 completion
requirement. Start a line draft, move within the headless snap tolerance of persistent
point `P`, then move away without placing the endpoint. The editor state remembers
`P` as a typed reference candidate. When a later pointer sample enters the horizontal
or vertical activation boundary relative to `P`, the editor—not the UI—publishes the
ranked prospective relation, guide and adjusted preview. Leaving that boundary removes
the assistance deterministically; explicit confirmation is still required before any
constraint changes the document.

The replay uses persistent identities and normalized 2D editor inputs and must produce
the same transitions natively and through WASM. A browser can render the guide, and a
3D CAD host can first map its camera ray onto the active sketch plane, but neither host
may remember the hovered point, calculate the inference tolerance, rank candidates or
adjust the preview. Replacing either UI must leave the replay result unchanged.

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

The headless qualification corpus and reusable workbench scenario catalog jointly cover every
preserved M13-M14 constraint, dimension and explicit branch action without restoring the old
application. The matrix includes:

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

The reusable catalog root is now `uat-scenarios` (**GeoSolve scenarios**). It retains the complete
eight-leaf `m53-host-semantics` subtree and adds this independent M55 subtree:

| Selector group | Stable scenario ID | Scenario title | Direct purpose |
| --- | --- | --- | --- |
| `m55-action-parity` (**M55 Action parity**) | `alpha-parity-catalog` | Alpha relation & dimension catalog | Inspect the accepted public alpha corpus, semantic relation glyphs and all five dimension annotations without a legacy app. |
| `m55-action-parity` (**M55 Action parity**) | `alpha-branch-recovery` | Contact branch & rejection recovery | Exercise typed A3 tangent-orientation state, a retained impossible fixed contact, accepted-state truth and bounded Undo recovery. |

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
constraint-contact topology, while the hidden draft-v5 bridge round-trips it pending M62. The
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

The sole directly tested workbench retains the complete M55 action surface and the exact ten
existing M53/M55 scenario IDs. M60 adds one sibling root group,
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

Canvas selection and projected point movement dispatch to the rendered scenario coordinator.
Accepted release changes only that ephemeral candidate; save remains suppressed. Reset rebuilds
the same public fixture and selected persistent driver. Exit reveals the ordinary coordinator
whose canonical workspace bytes never changed.

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
`docs/M61_UAT.md` owns the scorecard; objective facts remain directly qualified and M61 requires
explicit supervising-human approval.

### UAT-C4 - Integrated release candidate at M63

The frozen candidate starts from an empty workspace and proceeds through ordinary
and construction geometry, constraints, host parameters, an external reference, an
advanced curve, one associative operation, conflict repair, production profile,
save/reload/history recovery and short exploratory authoring. The 45-60 minute
review judges end-to-end trust and coherence rather than repeating an exhaustive
matrix.

### UAT evidence and recheck policy

Each checkpoint provides one manual entry point, deterministic resets and concise
instructions. The top **Scenarios** entry now contains the eight stable M53 leaves, two
direct-qualified M55 leaves, four direct-qualified M60 leaves and ten movable M61-remediation
leaves; the M53/M55
subtrees and M53 approval record are unchanged. UAT-C3 is ready after M60; the catalog machinery
may host UAT-C4 only after its preceding milestone is executed.
Findings capture the candidate revision, selected scenario,
workspace input, action transcript and accepted/attempted diagnostics from public APIs; a
human may attach an OS screenshot for a visual finding. Objective defects receive direct
owning-layer regressions. A targeted human recheck is preferred; a full checkpoint repeats
only after a material API, schema or primary-workflow change. Completed M40.7 and M53 required
explicit supervising-human sign-off; active M61 and planned M63 require the same.

## Frozen near-singular fixtures

The regression corpus includes:

- four-bar toggle/dead-centre configuration;
- slider-crank aligned near `0` or `180 degrees`;
- sketch point where two constraint gradients become dependent.

These fixtures test truthful singularity/rank reporting and finite state retention. They do not demand arbitrary global branch selection. M9 makes the machine-floor numerical rank contract and distinct near-singular warning band mandatory.

The detailed L3 fixture above demonstrates that geometric alignment does not itself justify an M9 warning when the selected driver makes the reported position/velocity matrices full-rank and well-conditioned. The detailed sketch fixture demonstrates actual dependent gradients and therefore does report numerical singularity.
