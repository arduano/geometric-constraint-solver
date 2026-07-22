# ADR 0024: All-family visual profile analysis

Status: accepted

## Context

ADR 0021 intentionally limits visual profile analysis to accepted line and
polyline spans. The sketch model now contains circles/arcs, conics, Beziers,
B-splines and NURBS, and users reasonably expect those visible curves to
participate in filled visual boundaries. Silently tessellating them into chords can
miss intersections, invent topology, corrupt area and still claim `Complete`.

The existing common curve-jet API evaluates local geometry but does not prove a
global root count. A truthful extension therefore needs family-specific bounded
pieces and explicit failure when finite work cannot isolate every relevant root.

## Decision

### Scope and product boundary

`SketchDocument::analyze_visual_profiles` accepts every built-in planar
`CurveDefinition` and every accepted M28 visible interval. The output remains
read-only, visual-only, equation-free and absent from persistence, history,
selection and autosave. No region or B-rep entity is introduced.

The public result states its geometry scope, completeness, issues and consumed
budgets. `Complete` means every eligible support interval in every published
component was analyzed and all topology, area-sign and containment decisions were
resolved. An empty complete result can no longer mean "line-only scope" when other
visible families are present.

### Bounded family pieces

The implementation uses a crate-private closed enum rather than a public curve
trait:

- linear pieces for lines and polyline segments;
- circular pieces for circles and circular arcs;
- analytic-conic pieces for ellipses, elliptical arcs, parabolas and hyperbolas;
- polynomial Bezier pieces for quadratic/cubic Beziers and extracted B-spline spans;
- homogeneous rational Bezier pieces for rational quadratics and extracted NURBS spans.

Periodic seams, semantic spline spans, winding and M28 trim intervals remain
explicit. Piece subdivision never mutates the document or allocates persistent IDs.
All finite interval bounds use outward-rounded arithmetic. Rational pieces must
establish a finite denominator interval excluding zero before participating.

### Intersections and self-intersections

Linear/circular combinations use scaled analytic kernels. Other combinations use
recursive parameter rectangles with family-specific convex, conic or homogeneous
bounds followed by interval-Newton/Krawczyk certification of a unique transverse
root. Self-pairs analyze disjoint parameter rectangles while excluding the identity
diagonal and adjacent shared boundaries.

A root is published only with deterministic parameter enclosures on both source
intervals. Roots are merged by overlapping certified parameter boxes, never by
world-coordinate proximity, and duplicate boxes are merged only after a fresh
uniqueness certificate over their combined neighborhood. Transverse roots on
artificial recursive boundaries may be retried in a larger source-domain box; true
visible or semantic boundaries remain one-sided and fail closed unless separately
certified. Tangency, positive-length overlap, unresolved root
multiplicity, denominator/pole uncertainty, zero speed or exhausted subdivision
budget marks the affected component incomplete. No Newton seed or render sample can
turn such a case into `Complete`.

Full periodic curves receive deterministic ephemeral seam/antipodal split vertices
when required to avoid self-loop half-edges. These anchors retain unwrapped source
parameters and have no persistent identity.

### Topology and explicit joins

Half-edges follow exact source intervals. Their local rotation order uses the actual
outgoing derivative: `C'(start)` forward and `-C'(end)` in reverse. An unresolved
tangent order is a typed ambiguity.

Shared design-point identity and active independently validated coincidence/contact
topology may join endpoints. An active endpoint-to-curve interior contact splits the
contacted source at its persisted parameter after fresh accepted-residual validation;
no unrelated nearby endpoint is considered. M28 fillet-owned trim boundaries join the corresponding
ordinary output-arc endpoint through explicit owner/contact/endpoint-order state
after fresh position validation. Coordinate equality alone never joins unrelated
endpoints. Explosion removes association ownership; a remaining fixed trim and arc
endpoint need another explicit relation to stay topologically welded.

### Area and containment

Line and circular Green-area terms are analytic. Polynomial Bezier and extracted
B-spline terms are integrated exactly from their polynomial coefficients. Analytic
conics use closed forms where available. Rational pieces use adaptive outward-
rounded interval enclosure of `0.5 * (x*y' - y*x')` with an explicit integration
budget. Contributions are accumulated about a contour-local origin.

Signed orientation and visual area publish only when the accumulated enclosure
excludes zero and meets the configured display uncertainty. Otherwise the component
is incomplete.

Containment isolates all roots of a deterministic ray against line, conic,
polynomial and rational pieces using the same bounded machinery and half-open
endpoint rules. Boundary, tangent or unresolved ray events are ambiguous. Exact
family extrema or bounded derivative roots provide contour boxes; sampled boxes are
not authoritative.

### Budgets and browser behavior

Candidate-pair, subdivision/root, fragment, integration, containment and face work
are overflow-checked and reported. Global preflight exhaustion returns `Skipped`
without partial faces. Component-local ambiguity may retain clean components under
overall `Truncated`. A face-count limit may return one deterministic prefix marked
`Truncated`; no partial result is `Complete`.

The browser evaluates each returned edge's exact `CurveSpan` and directed parameter
interval through public document APIs. Adaptive screen-space sampling has a separate
render budget and cannot alter native topology or area. Sampling failure omits the
affected rendered face and exposes a warning. Overlays remain pointer-transparent
and identity-free.

## Consequences

- Arc and general curve boundaries become available without making tessellation an
  undocumented topology oracle.
- Difficult tangencies, overlaps and high-degree/rational cases may report
  incomplete instead of producing a plausible but false fill.
- The private implementation is materially larger than M26 and landed in ordered
  circular, polynomial/conic and spline/rational slices; M31 completed only after
  the all-family matrix passed.
- Persistence remains sketch JSON v4 unless an unrelated domain change requires a
  new schema; profile output itself adds no persisted state.
