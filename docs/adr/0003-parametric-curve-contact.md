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

Curvature constraints may later request second derivatives.

## Consequences

- Curve tangency does not require a solver-core rewrite.
- Contact parameters are hidden implementation variables, not ordinary dimensions.
- Bounded segments/arcs/spans require explicit parameter-domain handling.
- Multiple contacts require serialized span/contact-neighbourhood and orientation state plus warm-start continuation.
- Zero derivative, cusp, discontinuous-knot and escaped-domain cases produce explicit invalid/ambiguous diagnostics.
- Line/circle/arc use the seam first; a quadratic/cubic Bézier fixture must validate it before a public trait is stabilized.
