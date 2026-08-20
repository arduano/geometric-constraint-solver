<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0038: Certified computed all-family Curve Offset

Status: accepted and implemented for active M82; M82-F005 replacement qualification and human
acceptance pending

## Context

ADR 0037/M80 provides a deliberately narrow native `ProfileOffset`: exact one-to-one Line,
Circle and CircularArc targets are solver-associated through one grouped dimension while topology
remains unchanged. That is the right ownership model when both sides should remain ordinary,
bidirectionally editable sketch geometry.

The same representation cannot cover the ordinary CAD expectation that Ellipse, conic, Bezier,
B-spline and NURBS faces or chains can be offset. Their parallels usually are not members of the
same primitive family, and inserting an adaptive patch set into the solver would make generated
cardinality, fitting tolerances and topology accidental persistent variables. Refusing all such
curves leaves a major product gap; approximating them without a regularity and topology proof
would allow cusps, self-intersections or stale partial geometry to masquerade as success.

ADR 0031 already establishes the appropriate one-way ownership model: persistent feature intent
evaluates against one exact accepted sketch into revision-local generated topology. Its original
Offset discussion anticipated variable topology, but M82 takes the smaller certified cut: broaden
curve families while still rejecting any topology change. Trimming/splitting/loop removal remains
a future decision.

## Decision

### Preserve native Profile Offset and add semantic routing

M80 `ProfileOffset` is unchanged. The ordinary Offset tool chooses its route from the authenticated
operand:

- a wholly M80-eligible face or chain uses native `ProfileOffset`; and
- any eligible mixed/general-curve face or chain uses
  `ComputedFeatureDefinition::CurveOffset`.

Routing is headless and deterministic. The browser neither chooses the representation nor owns a
curve-family allow-list. No hidden fallback converts a rejected native proposal into computed
output after topology or solve failure; the route is decided from operand semantics before Apply.

### Certified mathematical parallel

For regular oriented `C(u)` and signed distance `delta`, evaluate

```text
Q(u)  = C(u) + delta N(u)
Q'(u) = (1 - delta kappa(u)) C'(u).
```

The kernel consumes public accepted sketch curve jets. It certifies finite denominators, nonzero
speed, continuous orientation and `1 - delta*kappa > 1e-8` over every admitted interval. Failure
to prove those facts is a typed complete rejection.

Line, Circle and CircularArc output remains exact analytic geometry. Ellipse/EllipticalArc,
RationalQuadratic, Parabola, Hyperbola, Quadratic/Cubic Bezier, B-spline and NURBS intervals use a
deterministic adaptive sequence of cubic endpoint-Hermite patches. Patches reproduce exact
endpoints and aligned regular endpoint tangents. The independent fit certificate requires
position error no greater than
`max(1e-8 * model_scale, 256 * epsilon * coordinate_scale)` and tangent error no greater than
`2e-7` radians.

Subdivision and certificate work are bounded and deterministically ordered. Exhaustion returns no
partial patch chain. The evaluator independently checks the mathematical parallel and fitted
output; fitting cannot hide a singularity or self-contact in the source parallel.

### Topology-preserving computed output

One feature owns one bounded face, with outer contour and every hole, or one explicitly ordered
open chain. Intent persists source spans, traversal, side/direction, exact adjacency provenance,
tangent/miter junction cells and normal-translated open terminals. Native shared points and exact
endpoint-contact provenance remain authoritative. Intrinsic adjacency between spans of one spline
support is also admissible without inventing a stored point. Coordinates alone never establish
connectivity.

Evaluation must retain open/closed state, contour count, edge order, winding, hole count and strict
nesting. It rejects if the mathematical or fitted output cusps, self-intersects, touches another
contour, splits, merges, collapses/inverts a hole or crosses a persisted junction cell. M82 does
not trim, split, remove loops, heal, reduce distance or otherwise repair topology.

### Feature composition and dependency direction

M82 approves the narrow acyclic dependency

```text
geosolve-sketch-features -> geosolve-sketch-topology -> geosolve-sketch
```

so computed evaluation consumes the same certified native face/adjacency evidence as operations.
No reverse dependency is added. Curve equations continue to come from public sketch evaluation,
not topology or feature-local copies.

`CurveOffset` adds generated geometry and does not claim source replacement intervals. Existing
computed-Fillet source replacement therefore retains precedence in scene composition. Version-one
computed-on-computed chaining remains forbidden: active computed Fillet output cannot be an Offset
operand. A Fillet explicitly published as ordinary native Profile geometry under M80 is ordinary
eligible source geometry.

Generated Offset edges resolve selection to their stable feature. Fillet arcs keep stable
feature/corner selection. Generated edge IDs, patch boundaries and certificates are revision-local
and may not become sketch constraints, operation operands or persistent topological names.

### Coordinator, scene and persistence

The retained coordinator owns exact accepted-sketch/feature/evaluator input authentication,
bounded evaluation, complete scene publication, history and replay. A source edit may commit even
when Curve Offset becomes Failed; output is withheld and attributed without replacing accepted
native geometry. A successful feature edit publishes intent plus its Current snapshot atomically;
failure preserves feature intent, output allocator, accepted scene, history and transcript.

Feature-tree, Problems, scene, picking and related-highlighting paths generalize the existing
Fillet-shaped computed seam. The Offset authoring panel reuses M80 distance/direction/drag
lifecycle and renders only headless-provided preview geometry and typed failures.

The private computed-feature format advances to version 2 only when Curve Offset intent is
present. Empty and Fillet-only documents emit version 1 byte-for-byte. Workspace v6,
`GEOSOLVE_REPRO_V1`, canonical sketch v1-v4 and private draft-v5 versions remain unchanged.
Generated geometry and certificates are never serialized.

## Consequences

- Every regular built-in planar family can participate in ordinary face/chain Offset without
  becoming new solver state.
- M80 retains its exact native, constraint-friendly behavior and remains the preferred route for
  wholly eligible operands.
- General offsets are associative and deterministic but one-way; generated geometry is not
  directly editable or constrainable.
- Singular, self-contacting, topology-changing or uncertified output fails closed instead of
  publishing a visually plausible approximation.
- The feature crate gains a topology dependency and broader scene/persistence paths, but curve
  equations and topology authority are not duplicated.
- A later topology-changing Offset remains free to introduce trimming and variable contour
  cardinality under a separate milestone/decision.

## Rejected alternatives

- **Broaden `ProfileOffset` with spline target variables:** rejected because fitted patch
  cardinality and tolerance would become solver/persistence state.
- **Always use computed Offset, including native operands:** rejected because it would discard
  M80's valuable bidirectional ordinary geometry and association.
- **Sample a polyline at fixed density:** rejected because it is neither curvature-adaptive nor a
  certificate of regularity, error or topology.
- **Accept the mathematical parallel but skip fitted-output checks:** rejected because the
  approximation itself can introduce contacts or lose the required error bound.
- **Trim loops or reduce distance until valid:** rejected because that silently changes user
  intent and output topology.
- **Consume computed Fillet output:** rejected because version-one computed-on-computed
  dependency/naming semantics remain intentionally unavailable.
- **Persist generated cubics:** rejected because stale derived geometry must not become authority.

## Verification

M82 must directly cover analytic exactness; every general family; mixed and spline-intrinsic open
chains; closed faces/holes; reversal and both sides; source/distance edits; regularity/cusp margins;
mathematical and fitted self-contact; contour touch/topology barriers; deterministic work
exhaustion; Current/Failed publication; suppression/delete; Undo/Redo/reload; native routing;
computed-Fillet exclusion; native-Fillet eligibility; strict feature v1/v2 compatibility; and
native/WASM/editor/demo parity. `docs/M82_IMPLEMENTATION.md` records evidence only after commands
actually pass. The first frozen candidate is withdrawn after M82-F005 corrected stale native-only
Offset help; `docs/M82_UAT.md` remains pending until a replacement passes the mechanical gate.
