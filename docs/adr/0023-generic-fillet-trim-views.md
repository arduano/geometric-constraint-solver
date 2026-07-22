# ADR 0023: Generic fillets and persistent trim views

Status: accepted

## Context

M27 associates one ordinary circular arc with two bounded line contacts but leaves
both parents visibly untrimmed. M28 must generalize the same equations across the
existing immutable curve-jet families, expose truthful visible parent intervals,
and permit executable constraints on an associated output arc without omitting
derivatives through its derived endpoints.

Destructively rewriting support curves or spline controls would lose stable span
identity and make explosion, history and migration family-specific. Inferring a
retained parent side, a periodic root or a full-circle seam from coordinates would
also make branch state accidental. Sketch JSON v3 contains no retained-side state,
so its line fillets cannot be migrated to visibly trimmed parents by inference.

## Decision

### Immutable support and visible intervals

`CurveDefinition` and `CurveSpan` continue to describe immutable support geometry.
M28 adds at most one persistent `DocumentCurveTrimView` per `CurveSpan`. Absence of
a view means the complete native span is visible. A view contains a directed start
and end boundary; each boundary is either:

- a fixed finite parameter plus explicit winding; or
- a contact boundary owned by one fillet association.

The view is keyed by its stable support `CurveSpan`, not by another object identity.
It is not a solver entity and contributes no residual row. A fillet parent stores
whether its contact owns the visible start or visible end. For bounded supports the
opposite boundary is the native zero or one endpoint. A full circle or ellipse has
no natural retained seam, so construction additionally requires an explicit fixed
periodic anchor and winding. M28 rejects a second trim view or conflicting owner on
the same support instead of guessing split topology.

Support contact neighborhoods and visible endpoint topology remain distinct. A
fillet contact is strict-interior or explicitly local on its support even though it
becomes a visible endpoint. Periodic local neighborhoods use unwrapped parameters;
their principal value and winding remain persistent branch state. Executable
ordinary contacts must lie inside the accepted visible interval. Rendering, hit
testing, selection and visual profile analysis consume public visible-interval
queries and never reconstruct trim choices.

### Generic fillet association

New version-4 fillets use one `CurveCurveFillet` association over an ordinary
circular output arc and two explicit parent contacts. Each parent persists:

- exact `CurveSpan` and contact parameter/winding;
- local root neighborhood;
- left/right normal side; and
- the visible endpoint owned by the contact.

Endpoint order and clockwise/counterclockwise output sweep remain explicit. The
association uses the same common curve-jet equation for every regular line,
circle/arc, ellipse/conic, Bezier, B-spline and NURBS span. For parent jet
`C_i(t_i)`, increasing unit tangent `T_i`, left normal `N_i`, side sign `s_i`,
center `O` and radius `r`, the four Cartesian rows remain:

```text
O - C_1(t_1) - s_1 r N_1 = 0
O - C_2(t_2) - s_2 r N_2 = 0
```

No family-pair residual variants or browser equations are added. Each parent
offset must also be regular. With signed curvature `kappa_i`, independent
validation rejects a non-finite or numerically unresolved factor
`1 - s_i*r*kappa_i`, as well as zero-speed jets, cusps, rational poles, escaped
local roots and parallel offset intersections.

### Differentiable output arc

An associated output arc promotes its retained start and end angles to ordinary
solver scalar coordinates. The fillet contributes one dimensionless cross-product
row per ordered endpoint, aligning the corresponding radial unit direction with
the solved parent contact vector. Positive dot-product, radius, endpoint order and
explicit sweep checks select the intended branch independently.

Common arc incidence uses those two angle coordinates and a fixed integer-turn
offset selected from the explicit sweep. Consequently point, contact, tangency,
curvature and continuity consumers of the output arc receive complete global
Jacobian incidence through the two endpoint rows. Accepted solves still rederive
and independently validate endpoint angles before publication; retained persistent
angles are warm state, not an alternative topology source.

### Ownership, suppression and explosion

The association owns both contacts and both contact-derived trim boundaries. The
output arc, center, arc scalars and radius dimension remain ordinary objects under
the M27 ownership rules. Direct or cascading output deletion remains `ObjectInUse`,
including while the association is suppressed.

Suppression disables equations but preserves output ownership and freezes the last
accepted arc and visible parent intervals. Deleting the association explicitly
explodes it: each owned trim boundary becomes a fixed boundary at the last accepted
parameter, owned contacts are removed, and the ordinary arc plus fixed visible
parent views remain. Undo and redo restore the same IDs, intervals and branch state.

Spline refinement while a support span has a trim view is rejected in M28 unless an
operation implements an atomic semantic-span remap. Clearing a fixed trim view is
an explicit operation and rejects while either boundary remains association-owned.

### Persistence and migration

Canonical sketch JSON advances to version 4. Version 3 receives a frozen constraint
wire DTO before the in-memory fillet syntax changes. Versions 1 through 3 continue
to parse only their frozen languages and migrate deterministically.

Version-3 `LineLineFillet` definitions migrate as explicitly untrimmed legacy
associations because no retained parent endpoint was persisted. New version-4
`CurveCurveFillet` definitions persist trim endpoint ownership, periodic anchors,
root neighborhoods and all existing side/order/sweep state. Relabeling version-4
trim or generic-fillet syntax as an older version rejects.

## Consequences

- Support geometry, stable spline spans and existing curve-jet equations remain
  reusable and family-independent.
- Visible topology is persistent and equation-free, while accepted endpoint motion
  remains atomic with the two solved fillet contacts.
- Downstream constraints on associated arcs become truthful through explicit angle
  coordinates and endpoint rows rather than hidden post-solve differentiation.
- M28 supports one visible interval per support span; arbitrary multi-fragment trim
  topology remains outside this milestone.
- Existing version-3 fillets preserve their untrimmed visual behavior after
  migration and require an explicit new trim operation to gain parent views.
