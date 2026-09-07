<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M96: finite-width manifold channels and silicone grooves

The user reported that the PC water-manifold sample contains channel centre lines
and an O-ring centre line instead of actual finite-width boundaries. They requested
two reusable custom patches accepting polylines, specifically using two offsets,
fillets at the corners and arcs at the ends. Implementation is authorized; M95
remains the accepted product baseline.

The patch helper composes existing native supporting-line offset dimensions,
computed Fillets, Center Arc and endpoint-contact declarations. It adds no solver equations or new
persistent computed-feature kind. Water and silicone patches expose width and
centreline bend radius. Open water routes leave their reservoir inlet open and
cap their outlet; the closed seal has two continuous walls. The existing reservoir,
three original outlets and meaningful reservoir/outlet edit witnesses remain.
The user’s 2026-09-08 amendment adds a separate two-bend stair passage between
the middle and lower routes, with two rounded ends and two additional port bores.
It uses a point-to-point water-patch variant and its own seventh group.

The amended dimensions are water width 12 mm, centreline bend radius 8 mm, and
silicone groove width 2.4 mm. Water wall radii are R2/R14 and caps are R6.
The seal retains the existing enclosing perimeter and R5 centreline bends.
Original outlet ends move to x96 at y40, y18 and y−40, with reservoir inlets
at y22, y0 and y−22. The independent stair runs from [0,−20] through [30,−20]
and [30,−2] to [82,−2], translated with the reservoir-width datum.
This is a planar layout, with no depth, flow, pressure or seal-compression claim.
Self-intersecting or geometrically infeasible offset/fillet inputs must fail
transactionally. No solid Boolean union is in scope.

The helper accepts one keyed native polyline of straight spans, open or closed.
Coordinates and lengths must be finite; width must be positive and centreline
bend radius must exceed half the width. Every interior/closed vertex must turn:
collinear vertices, zero-length spans and reversing or numerically unresolved
corners are rejected rather than silently removed. Each wall Fillet must fit its
adjacent spans, and the final composed boundary must pass the separate intersection
and connectivity checks. Curve segments and automatic path cleanup are outside
this helper's contract.
Lengths use the ordinary `model`/`mm`, `cm`, `m` and `inch` conversion; angular
units reject. Equivalent unit spellings produce identical native geometry.

For an open polyline, `caps: "both"` closes both ends, `"end"` leaves the start
mouth open, and `"none"` leaves both mouths open. A closed polyline has no caps
regardless of this option. The six fixed outputs are the two wall features and
four wall endpoint points; for a closed loop the point outputs refer to its
first/last keyed vertices. The boundary validator permits at most 512 composed
straight/arc edges, including Fillets and caps; the existing managed-source and
expanded-node resource bounds also apply. Inputs exceeding those bounds reject.
Clearance between separate channels, the seal and other geometry is checked by
the sample's independent test, rather than a generic Boolean union.

M96-F001 is the independently reproduced native keyed-polyline offset failure:
the same span has member, indexed and byKey aliases, which the managed projector
mistook for unrelated output ownership. The fix authenticates the known trio and
uses its stable byKey path; arbitrary duplicate paths remain errors.

M96-F002 reproduces a Fillet polishing failure on the 220 by 100 mm seal with
R3.8 inner corners. At model scale 1, the two evaluated offset centres differ
by 1.42e-14 because of coordinate rounding, exceeding the 1e-14 root-polishing
target. The owner correction accepts a representable error floor only after root
polishing stalls, while preserving independent final geometry validation and
singularity checks. [M96-F002](M96_F002.md) records the exact regression and checks.

M96-F003 reproduces a channel cap intersecting an earlier wall despite every
individual feature being valid. A validation-only composition request checks
the final finite line/arc boundary after Fillet replacement during cold loading,
incremental publication and restoration. It rejects intersections, overlaps,
touching branches, missing joins and incorrect component counts. The operation
is bounded to 512 composed edges and adds no geometry or equation.

M96-F004 covers generated-wall navigation: one side owns both native straight
segments and computed corner arcs. Selecting the side must include both without
selecting the input polyline or the opposite wall.

[M96-F005](M96_F005.md) fixes channel boundary validation after driving dimension
edits: native fragments and computed arcs must both use current accepted solved
geometry, rather than mixing retained design seeds with solved Fillets.

[M96-F006](M96_F006.md) fixes the large fully constrained priority-solver boundary
exposed by the added stair, without weakening hard residual or rank validation.

Acceptance requires finite independently validated native geometry, independent
boundary closure/area/width and separation checks, retained source association,
edit/history/restore checks, visual review and proportional integrated qualification.
Implementation and provisional integrated qualification pass in `20260908T012505-e9d6c999`.
Final M96 human acceptance and clean-source qualification remain pending.
The earlier 6 mm preview was positively reviewed, then amended; that feedback
is not final acceptance of the new 12 mm layout. Integrated run
`20260907T235737-86172414` was deliberately interrupted during browser qualification
for this scope change, preserving 241 completed stages and the unchanged golden.
The [implementation and focused evidence](M96_IMPLEMENTATION.md) records delivered
files, mathematical behavior, executed commands and input limitations.
