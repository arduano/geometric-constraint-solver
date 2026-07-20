# ADR 0020: Associative linear constructions and sketch JSON v2

Status: accepted

## Context

M25 adds line offsets and point-defined mirrors without weakening the rule that
branch choices are persistent domain state. A single ambiguous "offset" mode
would hide whether segment length and axial position remain free. A privileged
mirror residual would duplicate the existing point-symmetry equation and make
topology refinement difficult to audit. The new persisted dimension definitions
also cannot be added to the frozen version-1 wire enum from ADR 0019.

The existing oriented-angle dimension already has directed segments, explicit
clockwise/counterclockwise state and target-local unwrapping. M25 needs public
workflow coverage for that equation, not a second angle formulation.

## Decision

### Offset equations and branches

`SupportingLineOffset` and `ExactTranslatedSegmentOffset` are separate runtime
and persistent dimension variants. Both store a positive finite target,
`LineSide::{Left,Right}` and
`LineOffsetOrientation::{Same,Reversed}`. Orientation maps the target's logical
start/end before evaluating equations; it is not inferred from current point
coordinates.

For source unit tangent `u`, left normal `n = (-u_y, u_x)`, oriented target unit
tangent `v`, logical target start `q0`, source start `p0`, side sign `s` and
positive target `d`, supporting-line offset contributes

```text
cross(u, v) = 0
dot(q0 - p0, n) - s d = 0
```

The first row is dimensionless and the second uses `model_scale`. Independent
candidate validation requires `dot(u, v) > 0` after endpoint orientation and
requires the selected side to remain positive. The target segment therefore
retains one axial-slide and one length DOF.

Exact translated-segment offset contributes the four Cartesian rows

```text
q0 - p0 - s d n = 0
q1 - p1 - s d n = 0
```

all scaled by `model_scale`. This preserves endpoint correspondence and length
and leaves no target-segment DOF when the source is fixed. Both evaluators have
structured row audit and local-AD Jacobians; acceptance independently recomputes
their normalized rows and branch predicates.

### Mirrors as ordinary constructions

`SketchDocument::add_mirrored_curve` supports lines, polylines, quadratic and
cubic Beziers, and non-rational B-splines. It creates reflected design points, a
same-family curve and one ordinary `SymmetricAboutLine` constraint for every
corresponding point pair. Stored line/polyline branch directions are reflected
explicitly. No mirror-specific runtime equation or hidden dependency graph is
introduced.

`insert_mirrored_bspline_knot` is the only associative topology-changing path
for a mirrored B-spline pair. It verifies identical basis topology and an active
symmetry constraint for every existing control pair, inserts the same parameter
into both curves and adds the new control-pair constraint in one document
transaction. Ordinary one-sided knot insertion remains available, but makes no
claim that a separate mirrored curve stays associated.

### Persistence and angle reuse

Canonical sketch JSON is version 2. `SketchDocumentV1` owns a private frozen
five-variant dimension DTO; strict v1 parsing converts it deterministically to
the current in-memory model and canonical re-export emits v2. A v1 payload that
claims an offset variant rejects. `SketchDocumentV2` serializes the current
definitions, including explicit offset side and orientation.

The existing `OrientedAngle` runtime residual, persistent definition, generic
document command and playground Angle tool remain authoritative. The M25 native
example and branch-cut corpus exercise that public path without adding another
equation.

## Consequences

- Supporting and exact offsets have visibly different row counts and truthful
  mobility rather than undocumented mode-dependent behavior.
- Side, endpoint correspondence and angle direction survive persistence and do
  not depend on solver initialization.
- Mirrored geometry remains inspectable, suppressible and auditable as ordinary
  points, curves and constraints.
- Coordinated B-spline refinement is more explicit than automatic discovery;
  callers must identify the associated pair and axis.
- NURBS, conics, circles and arcs are not mirror-construction inputs in M25.
- Canonical v1 bytes are no longer emitted by current documents, but strict v1
  import remains deterministic and the frozen v1 accepted language does not
  expand.
