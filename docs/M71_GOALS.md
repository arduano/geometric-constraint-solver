<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 candidate goals — Retained primitives for richer drafting inference

Status: deferred candidate backlog only. M71 is ordered after the active M70B bugfix placeholder,
but is not scoped or authorized for implementation.

M70 deliberately proved auto-constraint interaction using constraint definitions already admitted
by the ordinary retained editor workflow. This document records likely follow-up primitives and
branch contracts so they are not smuggled into M70 as fixed coordinates, zero dimensions, hidden
construction geometry or coordinate-only guesses. It is not a replacement for a future M71 plan,
ADR, acceptance matrix or supervising-user scope decision.

## Priority A — complete high-value retained relationships

### Arbitrary point-pair horizontal and vertical

The M37 semantic catalog already has point-pair H/V equations, but the ordinary retained
document/editor lifecycle does not expose them as a stable authoring/inference target. A future
cut should either admit that existing semantic source through persistence, history, prepared-input,
diagnostic and workspace ownership or define one deliberate supported replacement. It must not
encode the relation as two fixed coordinates or a zero distance.

Architecture risk: **medium**. The mathematics exists; the risk is duplicate source languages and
incomplete retained lifecycle/schema ownership.

### Certified intersections, collinearity and line extensions

Intersection inference needs certified native parameter/domain evidence for both supports and an
explicit policy for bounded span versus supporting-line extension. A placed persistent point may
then use an atomic pair of existing curve contacts or a deliberately retained intersection source.
Collinearity should reuse the existing semantic relation where its lifecycle can be completed.
Parallel/near-parallel, tangential, multiple, coincident/overlapping and out-of-domain roots must
remain typed ambiguous or unavailable.

Architecture risk: **medium-high**. The main risk is turning a visual sampled crossing into false
topology or silently changing bounded-domain meaning.

## Priority B — circular semantic anchors

### Concentric and quadrant inference

Concentric inference should promote the existing semantic relation into the ordinary retained
lifecycle rather than duplicating it. Circle/arc quadrant anchors require exact accepted centre,
radius, orientation, span and winding evidence; bounded arcs expose only quadrants in their active
domain. A quadrant click may combine an exact semantic anchor with an applicable retained relation,
but cannot persist an anonymous coordinate snap.

Architecture risk: **medium** for concentric lifecycle integration and **medium-high** for
quadrants because periodic/bounded branch and semantic-anchor identity must be explicit.

## Priority C — branch-sensitive nonlinear relations

### Tangent and normal inference

Generic tangent and sided-normal equations already exist, but automatic intent requires explicit
contact span, parameter, winding, neighbourhood, tangent orientation, normal side and containment/
root policy. Hover alone must not choose among tied nonlinear contacts. Candidate construction and
the relation must validate atomically against the same accepted native sources.

Architecture risk: **high**. This is primarily a branch-selection and local-root continuity
problem, not a missing tangent equation.

### Equality and symmetry inference

Equal length/radius/curvature and point/entity symmetry already exist in parts of the domain and
contextual authoring surface. Automatic inference needs a strong semantic trigger and explicit
operand correspondence; mere geometric similarity or mirrored-looking coordinates are not enough.
Hosts must be able to disable each family independently because accidental equality/symmetry can
overconstrain a sketch while appearing visually harmless.

Architecture risk: **high**. Ambiguous intent and correspondence dominate the mathematics.

## Priority D — host-owned references

### Workplane axes, grid and increments

Axes and grids are host/workplane input, not hidden sketch geometry. A future interface should
consume immutable revisioned semantic axes/grid definitions, publish whether a snap is tracking-
only or constraint-backed, and specify whether a placement creates a supported datum relation,
ordinary constraint or no persistent source. Grid and angular increments must remain independently
configurable from geometric relation inference.

Architecture risk: **high**. Host identity/revision ownership, units, workplane mapping and
persistence must be decided without adding callbacks during solving or fixing coordinates
implicitly.

## Cross-cutting requirements for any future scope

- Reuse existing residuals and semantic-source definitions when their complete retained lifecycle
  can be made coherent; do not create equation-shaped aliases for UI convenience.
- Every genuinely new residual requires structured audit metadata, central finite-difference
  Jacobian coverage, transformations/scales and independent acceptance validation.
- Every multi-root relation stores branch, span, winding, side, neighbourhood and correspondence
  explicitly; initial coordinates are never the persistent branch authority.
- Candidate ranking, wake memory, hysteresis, suppression, ambiguity and atomic commit stay in
  `geosolve-constraint-editor`; hosts render typed DTOs only.
- Incomplete certification, resource exhaustion, exact ties and stale identities fail closed and
  never fall through to a different displayed intent.
- Any schema or workspace migration needs its own compatibility decision. M70 and this backlog do
  not imply canonical sketch v5.

Before M71 becomes active, M70B must close and the supervising user must select a bounded subset,
accept any required ADR/schema work and define M71's own direct gate and end-of-milestone UAT.
