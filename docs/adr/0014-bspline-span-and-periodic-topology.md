# ADR 0014: B-spline spans, periodic topology and refinement

Status: accepted

## Context

M21 adds non-rational B-splines to the closed sketch graph established by ADR
0008 and the latent-parameter curve constraints established by ADR 0003. Knot
indices are not stable under refinement, periodic curves have no intrinsic seam,
and a solve step cannot change local support without changing residual incidence.
The roadmap therefore requires explicit stable spans, one-sided knot evaluation
and active-span-local derivatives before B-splines can participate in constraints.

## Decision

### Numerical representation

`geosolve-geometry` owns immutable numerical B-spline bases and planar curves.
Degree is at least one and a curve has at least `degree + 1` controls. All knots
and controls are finite, knots are nondecreasing, every represented parameter
interval has positive finite length, and an interior knot multiplicity may not
exceed the degree.

A clamped curve stores its complete knot vector. For `N` controls and degree
`p`, it has `N + p + 1` knots, exactly `p + 1` equal knots at each end, and
native domain `[U_p, U_N]`.

A periodic curve stores `N` unique cyclic controls and `N + 1` one-period knot
breaks `V_0 .. V_N`. The canonical origin is `V_0 = 0`, the period is
`V_N > 0`, and `V_{N-1} < V_N`. Evaluation extends the representation by

```text
U_{p+j} = V_j
U_{i+N} = U_i + V_N
P_{i+N} = P_i
```

so callers never duplicate seam controls. A finite unwrapped native parameter is
reduced by the period for numerical evaluation; persistent winding remains
separate sketch state.

Geometry exposes basis values and derivatives through order three for exactly
the selected span's `p + 1` controls. A span-local coordinate `s` is always in
`[0, 1]`, with native parameter `u = a + s(b-a)`. Derivatives with respect to
`s` are the native derivatives multiplied by `(b-a)^r` at order `r`.

### Knot sides and continuity

There is no implicit side at an exact knot. Native evaluation requires `Left` or
`Right`; an unavailable outward side at a clamped endpoint rejects. Span-local
evaluation is unambiguous: `s = 0` is the right-hand jet at the lower knot and
`s = 1` is the left-hand jet at the upper knot. Repeated zero-width knot
intervals never receive spans.

For degree `p` and knot multiplicity `m`, diagnostics report guaranteed
parametric continuity `C^(p-m)`. This is a lower bound from topology, not a claim
that a special control polygon lacks accidental higher continuity. Point contact
requires `C0`; a transition that retains tangency requires `C1`. Later curvature
and parametric-C2 operations may require `C2`. Insufficient continuity is a typed
nondifferentiable outcome and cannot become convergence.

### Persistent span and control state

`CurveSpan.segment` remains serialized as `u32`, but its B-spline meaning is an
opaque family-local semantic span identity, never a knot-array index. A B-spline
definition stores semantic span IDs in positive-interval order and a
never-decreasing allocation high-water mark. Runtime lowering resolves the ID to
the current immutable numerical span.

B-spline controls are ordered persistent `DesignPointId` values. Validation
requires every control reference to exist and rejects duplicate control IDs; the
numerical geometry crate sees only their ordered coordinates. Span, winding,
knot side and control identity remain outside automatic differentiation.

A solve cannot cross a B-spline span implicitly because that changes incidence.
Adjacent-span transitions are explicit structural document edits. Crossing the
periodic seam additionally changes winding by exactly one in the selected
direction. Accepted-state projection never infers or resets either choice.

### Knot insertion

Knot insertion is a structural, atomic document command implemented from the
geometry refinement result. It preserves the parameterized curve before solving
the candidate document. Existing control point IDs are retained in order, one
fresh point ID is allocated for the additional coefficient, and refined control
coordinates are assigned by the reported affine stencils. Because controls are
ordinary shared design points, any other references observe those point edits;
the complete document must solve and independently validate before the command
commits.

Insertion at a new knot splits one positive span. The left child retains the old
semantic span ID and the right child receives a fresh never-reused ID. Increasing
the multiplicity of an existing knot creates no positive interval, so all span
IDs remain unchanged. Insertion that would raise an interior or periodic knot
multiplicity above the degree rejects.

Contacts on a split span migrate atomically by preserving their native parameter
and recomputing the selected child span plus local coordinate. A contact exactly
at the inserted knot is assigned to the retained left child at `s = 1`; that
stored span then makes the one-sided choice explicit. Undo and redo preserve all
allocated point/span IDs through the existing document high-water policy.

### Residuals and validation

The sketch compiler adds only the selected span's `p + 1` control variables and
its latent local parameter to residual incidence. It obtains basis jets from
`geosolve-geometry`; it does not duplicate de Boor or knot equations. Generic
point/contact/tangency residual templates and audit equations remain unchanged.

Degree, knots, selected semantic span, winding, side and neighborhood are fixed
discrete inputs. Every active control and latent parameter is differentiated by
local AD and checked against central finite differences. Independent acceptance
reconstructs immutable geometry from the candidate document and rejects malformed
knots, stale spans, insufficient continuity, zero speed or non-finite results.

## Consequences

- Refinement cannot silently retarget a persisted contact through positional knot
  indexing.
- Periodic curves have one canonical serialized seam without duplicated controls,
  while winding remains explicit traversal state.
- Local support bounds both sparse incidence and AD width by the degree rather
  than total control count.
- Knot insertion may edit shared design points, but its whole-document effect is
  transactional and never commits unless all hard equations and domain checks
  independently validate.
- NURBS can reuse the basis, span and refinement contracts in M22 without adding
  a public curve trait or changing solver-core semantics.
