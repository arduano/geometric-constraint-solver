<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 — certified computed all-family Curve Offset

Status: **implemented; M82-F005 replacement qualification pending**. The first frozen candidate is
withdrawn after correcting stale native-only Offset help; the focused red/green regression passes
and a replacement clean gate and freeze precede human UAT. This milestone broadens the ordinary
Offset tool from M80's exact native line/circle/circular-arc
association to deterministic computed offsets for every regular built-in planar curve family. It
does not change or replace M80 `ProfileOffset`. ADR 0038 owns the architecture and certification
boundary. M82 is not accepted or closed.

## Product outcome

The existing **Modify → Offset** interaction accepts one bounded face or one ordered continuous
open chain. Operand routing is semantic and invisible to the user:

- an operand wholly eligible for M80 continues to create the existing native, bidirectionally
  editable `ProfileOffset` with exact same-family target geometry; and
- a mixed or general-curve operand creates one associative computed `CurveOffset` feature whose
  generated output is revision-local and read-only.

The computed path supports all regular built-in planar families: Line, Circle, CircularArc,
Ellipse, EllipticalArc, RationalQuadratic, Parabola, Hyperbola, Quadratic/Cubic Bezier, B-spline
and NURBS. Native line/circle/circular-arc portions remain exact; families whose exact parallel is
not a native sketch primitive are represented by deterministic certified cubic patches.

The operation is deliberately fail-closed. If the requested mathematical parallel or its fitted
representation cannot be certified as regular, simple and topology-preserving within bounded
work, the complete feature is unavailable. M82 does not silently reduce distance, trim, split,
remove loops, heal intersections or publish a partial contour.

## Mathematical contract

For a regular oriented source curve `C(u)`, chosen signed distance `delta` and continuous unit
normal `N(u)`, the intended parallel is

```text
Q(u)  = C(u) + delta N(u)
Q'(u) = (1 - delta kappa(u)) C'(u)
```

where `kappa` is signed curvature in the selected traversal. Evaluation must certify finite source
jets, nonzero speed and every rational/spline denominator required by the admitted interval. It
must prove `1 - delta*kappa > 1e-8` throughout every interval; reaching that margin is a cusp or
offset singularity, not a valid approximation opportunity.

Line, Circle and CircularArc pieces publish exact analytic output. Every other regular interval is
approximated by deterministic adaptive endpoint-Hermite cubic patches. Each patch has exact
endpoints and aligned regular endpoint tangents, with independently certified bounds:

- position error at most
  `max(1e-8 * model_scale, 256 * epsilon * coordinate_scale)`; and
- tangent error at most `2e-7` radians.

Certification covers both the mathematical parallel and the final fitted patch chain. A fitting
success is not sufficient if either representation can cusp, self-intersect, touch another
contour or change topology.

## Operand and topology contract

One `CurveOffset` owns either:

- one bounded face, including its material-left outer contour and every hole; or
- one explicit ordered open chain with normal-translated terminals.

The feature persists source span identities, traversal, side/direction, exact adjacency
provenance, tangent/miter junction cells and source order. Native persistent shared points and
ordinary endpoint contacts authenticate adjacency; spline-family intrinsic neighbouring spans may
also authenticate adjacency through their exact shared support/control topology. Coordinate
proximity never creates a chain.

The resulting contour count, open/closed state, loop winding, edge order, hole count and strict
nesting must match the operand. Non-adjacent self-contact, contour contact, hole collapse, split,
merge, inversion or any other topology change rejects the whole output. Junction construction is
bounded and branch-explicit; M82 does not search another miter/tangent cell after a source edit.

## Computed-feature ownership

`ComputedFeatureDefinition::CurveOffset` persists intent in the ADR 0031 sidecar. Its evaluated
geometry is associative, one-way and revision-local:

- it is not solver state, a sketch variable, an ordinary persistent generated entity or a valid
  constraint/operation operand;
- source edits, feature distance/direction edits, suppression, deletion, Undo/Redo and reload
  reevaluate from the exact current accepted sketch;
- generated edge IDs are never serialized and are invalid after any input, feature or evaluator
  revision change; and
- selecting any generated Offset edge resolves the stable owning feature. Existing generated
  Fillet arcs retain their stable corner-level selection semantics.

M82 introduces one narrowly reviewed acyclic dependency
`geosolve-sketch-features -> geosolve-sketch-topology` so the evaluator consumes certified native
topology rather than recreating adjacency or face logic. The sketch/topology crates never depend
on computed features.

Computed-on-computed chaining remains unavailable. In particular, active computed-Fillet output
cannot be consumed by computed Offset. A Fillet published explicitly as ordinary native Profile
geometry under M80 is ordinary eligible input. Where independent Fillet and Offset features share
native sources, existing Fillet source-replacement precedence remains unchanged; Curve Offset does
not claim or rewrite the native source intervals.

## Authoring and presentation

The same Offset panel and canvas interaction serve both routes. The headless owner discovers all
curve families, faces, holes, intrinsic spline-span adjacency and continuous selected paths; it
returns the chosen route, typed unavailable reason, traversal, direction, distance and complete
provisional output. The browser renders and dispatches those results and owns no curve evaluator,
normal, fitter, intersection or topology decision.

Computed preview geometry remains provisional. Distance dragging retains the M80 absolute rail,
three-pixel threshold, pointer capture, last-valid complete preview and stale-gesture authority.
Apply publishes exactly the displayed authenticated feature intent in one history step. Any
evaluation, certification, topology, work-budget or exact-CAS failure retains the prior complete
scene, feature state, history, transcript and allocator authority.

Problems and the feature tree distinguish source invalidity, offset singularity, fitting budget,
self-contact and topology-change failures without exposing unstable generated identities. Scene,
picking, related highlighting and reproduction use generated provenance supplied by the computed
feature evaluator.

## Persistence compatibility

The computed-feature sidecar gains a strict private version 2 for `CurveOffset`. Empty and
Fillet-only feature documents continue to emit version 1 byte-for-byte. Version 1 restores exactly
as before; version 2 rejects unknown fields, malformed operands, invalid finite values, impossible
topology/provenance and allocator inconsistencies atomically.

Workspace remains version 6, reproduction remains `GEOSOLVE_REPRO_V1`, and sketch canonical v1-v4
plus private draft-v5 remain unchanged. Generated patches, intersection certificates, caches and
revision-local output IDs are never serialized.

## Acceptance gate

- Direct kernel tests cover exact Line/Circle/CircularArc preservation and Ellipse,
  EllipticalArc, RationalQuadratic, Parabola, Hyperbola, Bezier, B-spline and NURBS patches across
  translation, rotation, reversal and scales `1e-6`, `1` and `1e6`.
- Independent tests verify source regularity, `1 - delta*kappa` margin, endpoint Hermite data,
  position/tangent error bounds, deterministic subdivision and bounded-work atomicity.
- Open chains, mixed analytic/general chains, closed faces and faces with holes preserve explicit
  order, side, adjacency, junction cells, terminals, winding and strict nesting.
- Cusp/tight-curvature, mathematical self-contact, fitted-output self-contact, contour touch,
  split/merge/hole loss and uncertified intersection cases reject the complete feature without
  partial output or durable mutation.
- Source edit, distance/direction edit, suppression/delete, Undo/Redo, reload, stale-CAS and
  work-exhaustion paths preserve current-only authority and allocator non-reuse.
- Existing native-only Offset continues through M80 byte- and behavior-compatible routing;
  computed Fillet output is excluded while M80 native-published Fillets remain eligible.
- Feature v1 bytes remain exact for empty/Fillet-only state; strict v2, workspace-v6 and repro-v1
  round trips pass without serializing output geometry.
- Focused native/WASM/editor/demo tests, reviewed golden coverage, formatting, warnings-denied
  Clippy/Rustdoc, locked all-feature workspace tests and the complete clean release gate pass
  before a no-rebuild Tailscale UAT nomination.

## Explicit non-goals

M82 does not implement trimming, loop removal, self-intersection repair, split/merge output,
distance reduction, best-effort partial contours, computed-on-computed chaining, Bake/Explode,
stable generated-edge naming across revisions, canonical sketch v5, arbitrary third-party curves,
surface/B-rep offset, 3D offset or mobile UI. M80 native `ProfileOffset` remains the only
constraint-friendly bidirectional offset association.
