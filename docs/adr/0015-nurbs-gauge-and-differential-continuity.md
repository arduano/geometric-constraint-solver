# ADR 0015: NURBS weight gauge and differential continuity

Status: accepted

## Context

M22 completes the production 2D sketch curve and continuity surface. Positive
NURBS weights are projective: multiplying every weight by one positive constant
does not change the curve. Compiling every weight independently would therefore
publish one artificial null direction, while enforcing a normalization with a
residual would add a nonphysical equation and connect every otherwise local span.

Curvature and continuity also need explicit semantics. Geometric G2 continuity
must not depend on parameter rate, while parametric C2 intentionally does. Signed
curvature changes under parameter reversal and reflection, and unsigned curvature
is nondifferentiable at zero unless its sign relationship is retained as discrete
state.

## Decision

### NURBS representation

`geosolve-geometry` owns an immutable planar NURBS over the validated
`BSplineBasis` from ADR 0014. A curve stores ordinary controls `P_i` and one
finite strictly positive weight `w_i` per control. Clamped and periodic knot,
span, side and continuity semantics are exactly those of the underlying basis.

For basis derivatives `N_i^(r)` on the selected span, homogeneous jets are

```text
A_r = sum_i N_i^(r) w_i P_i
W_r = sum_i N_i^(r) w_i
```

and Euclidean derivatives through order three are recovered recursively:

```text
C_0 = A_0 / W_0
C_r = (A_r - sum_(j=1..r) choose(r,j) W_j C_(r-j)) / W_0
```

Unit weights therefore reproduce the corresponding non-rational B-spline.
Degree-two clamped knots with weights `[1,w,1]` reproduce the positive-weight
subset of the homogeneous rational-quadratic conic representation.

Knot insertion applies the basis refinement stencils to homogeneous controls
`(w_i P_i, w_i)`, then dehomogenizes. Applying the stencils independently to
ordinary controls and weights is invalid because it does not preserve rational
geometry.

### Persistent weight gauge

A persistent NURBS stores one owned `DesignScalarId` per weight and explicitly
identifies one of them as `gauge_weight`. The selected gauge scalar is exactly
`1`, is serialized, and is omitted from solver variables and residual incidence.
Every other weight is an independently editable positive dimensionless scalar.
The default gauge is the first ordered weight, but identity is stored rather than
inferred from array position.

Changing the gauge is an explicit structural transaction. Selecting a new gauge
with current value `w_g` replaces every weight by `w_i / w_g`; controls, spans,
contacts and parameterized geometry remain unchanged. A solve never changes the
gauge implicitly and no normalization residual is emitted. Direct edits of the
selected gauge scalar reject.

Knot insertion retains all old control and weight identities, allocates one new
control and weight identity, retains the selected gauge identity, and normalizes
the refined homogeneous coefficients back to that exact unit gauge before the
candidate document is solved. Point, scalar and span allocator high-water marks
remain monotonic across undo and divergent history.

### Rational conditioning

Evaluation may divide all active weights by one common finite positive scale,
normally their maximum, before homogeneous accumulation. This is computational
normalization only and does not alter persisted state or solver coordinates.

Let

```text
S_0 = sum_i abs(N_i w_i).
```

The denominator is valid only when `W_0`, `S_0` and every homogeneous and
Euclidean jet component are finite and

```text
W_0 > 64 * EPSILON * S_0.
```

A positive weight that becomes zero under the common normalization, an
unrepresentable active weight ratio, a non-finite homogeneous product, or a
denominator inside this scale-aware band is a typed mixed-scale or rational
denominator outcome. It cannot become a zero residual or convergence. Exact zero
speed remains a separate regularity failure.

The implementation evaluates position relative to one active reference control
and forms derivative quotient numerators from pairwise
`A_r W_0 - A_0 W_r` terms. This is algebraically equivalent to the recurrence
above but avoids subtracting translation-sized or weight-dominated values. Knot
insertion likewise normalizes only the one- or two-control homogeneous stencil;
distant nonincident weights cannot make a local refinement fail.

Independent acceptance reconstructs immutable NURBS geometry from candidate
points and candidate weights. It does not trust the local-AD quotient
implementation and retains the previous accepted state on every definition,
conditioning, regularity, branch or solve failure.

### Local support and derivatives

A NURBS residual includes only the selected span's `degree + 1` controls, its
non-gauge weights, and the latent local parameter. The gauge weight is a fixed
coefficient, not a solver variable. Inactive controls and weights do not enter the
component or Jacobian.

The generic sketch local-AD curve adapter carries dual-valued position, first and
second parameter derivatives. For a free parameter it lifts basis terms as

```text
C   uses N   + delta N'
C'  uses N'  + delta N''
C'' uses N'' + delta N'''
```

so third-order immutable basis jets are sufficient for first-order Jacobians of
all M22 curvature and C2 residuals. M22 does not add curvature-gradient, G3 or C3
constraints, which would require fourth-order parameter jets.

### Differential geometry

For a regular planar jet with `v = length(C')`, define

```text
T       = C' / v
N_left  = (-T.y, T.x)
kappa   = cross(C', C'') / v^3
K       = kappa N_left
rho     = 1 / abs(kappa)
```

Signed curvature `kappa`, unsigned curvature `abs(kappa)`, and osculating radius
`rho` are public immutable/document measurements. Zero curvature is a valid
signed and unsigned measurement. Osculating radius at zero curvature is a typed
undefined result and never returns infinity. Parameter reversal flips `T`,
`N_left` and signed curvature, but preserves unsigned curvature and curvature
vector. A reflection flips signed curvature; a positive similarity of scale `s`
divides curvature by `s` and multiplies osculating radius by `s`.

Finite evaluation may use the equivalent normal-acceleration form
`kappa = dot(N_left, C'') / v^2`. Compensated raw determinant, unscaled normal
projection and acceleration-scaled normal projection paths are tried in that
order so cancellation, overflow or underflow cannot silently become zero.

Generic tangent and normal constraints consume the same differential curve jet.
Normal direction stores explicit left/right side relative to increasing parameter
and independently validates the selected dot-product sign. It is not inferred
from initial coordinates after acceptance.

Equal-curvature constraints are explicitly either signed or magnitude-based.
Magnitude equality stores `Same` or `Opposite` signed-curvature relation and uses
the corresponding smooth signed equation; it does not differentiate `abs` at
zero or permit a silent sign change. A zero-curvature magnitude relation is an
ambiguous branch outcome unless the signed form was requested.

### Endpoint continuity

An endpoint continuity source stores ordered first and second curve contacts,
each fixed to an explicit start or end side. The first curve is incoming and the
second outgoing. Their path traversal signs are

```text
first Start  = -1    first End  = +1
second Start = +1    second End = -1.
```

G0 equates positions. G1 adds aligned path tangents. G2 adds equality of the
path-oriented signed curvatures and is invariant under positive affine
reparameterization. The G2 curvature row is multiplied by the finite positive
model scale before normalization.

Parametric C2 is a separately named source. It stores positive finite fixed
affine rates `a_1` and `a_2` and emits

```text
q_1 a_1 C_1'      - q_2 a_2 C_2'      = 0
a_1^2 C_1''       - a_2^2 C_2''       = 0.
```

The rates are source parameters, not solved variables, so they introduce no
second scale gauge and cannot silently turn parametric C2 into geometric G2.
Position and derivative vector rows use the source model length scale; tangent
rows are dimensionless.

A contact transition requires the highest guaranteed knot continuity consumed by
its sources: C0 for position, C1 for tangent/normal, and C2 for curvature, G2 or
parametric C2. One-sided measurements at an explicitly selected side remain valid
without claiming continuity across the knot.

### Persistence and audit

NURBS weights, gauge identity, semantic spans, winding, knot side, neighborhoods,
normal side, curvature relation, endpoint order and parametric rates are explicit
versioned document state. M22 extends the existing pre-1.0 document envelope;
M29 remains responsible for the final migration and compatibility policy; M24
first freezes the version-1 wire boundary needed by M25-M28 extensions.

All advanced constraints lower through one generic differential-curve row model
and produce structured row-specific audit descriptors. No curve-family-pair
equations or sampling equations are added to `geosolve-demo-web`.

## Consequences

- NURBS rank and mobility contain no projective weight-gauge artifact.
- Weight editing and knot insertion preserve sparse span-local incidence.
- Unit-weight and canonical-conic equivalence provide independent cross-family
  geometry oracles.
- G2 and parametric C2 have distinct, testable behavior instead of sharing an
  ambiguous continuity label.
- Zero speed, zero curvature, denominator conditioning and unsigned-curvature
  branch ambiguity remain separate typed outcomes.
- No M22 behavior requires a solver-core change, public curve trait, fourth-order
  jet, native FFI or browser-owned equation.
