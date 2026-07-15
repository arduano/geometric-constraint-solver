# ADR 0003: Parametric curve contact uses latent parameters

Status: accepted

## Decision

Advanced sketch constraints evaluate curves through an internal parametric adapter providing position, first derivative, parameter-domain information and degeneracy state. Interior point-on-curve and tangency constraints introduce latent scalar contact parameters into the solver problem.

The solver core remains unaware of Bézier, B-spline, NURBS or conic entity types. `geosolve-sketch` owns curve entities, span/branch identity, bounds and compilation to generic residual blocks.

## Residual pattern

For a curve `C(t)`:

```text
point on curve:       P - C(t) = 0
line tangent:         cross(unit(line_dir), unit(C'(t))) = 0
curve/curve contact:  C1(t1) - C2(t2) = 0
curve/curve tangent:  cross(unit(C1'(t1)), unit(C2'(t2))) = 0
```

M15 curve jets provide derivatives through third order; M20 uses them for curvature, G2 and separately named parametric C2 constraints.

## Consequences

- Curve tangency does not require a solver-core rewrite.
- Contact parameters are hidden implementation variables, not ordinary dimensions.
- Bounded segments/arcs/spans use the M10 bounds/active-set contract and explicit parameter-domain validation.
- Multiple contacts require serialized span/contact-neighbourhood and orientation state plus warm-start continuation.
- Zero derivative, cusp, discontinuous-knot and escaped-domain cases produce explicit invalid/ambiguous diagnostics.
- The M1-M7 baseline proves the seam with line/circle/arc behavior and quadratic/cubic Bezier fixtures. M13 migrates it to the closed sketch graph, and M15-M20 extend internal generic adapters across built-in curve families before any public trait is considered.
