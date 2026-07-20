# ADR 0017: Spatial velocity fields and motion bases

Status: accepted

## Context

M17 established planar velocity over revision-stamped accepted hard
linearizations. M20 and ADR 0013 added spatial position drivers, coordinates,
explicit modes and private six-coordinate gauges; ADR 0016 added spatial
continuation without publishing its gauges or active parameters. M23 requires
several simultaneous driver rates, truthful inconsistency/mobility outcomes,
physical feature velocities and optional motion/nullspace bases without creating
a second rank policy or implying dynamics.

Hinge and translation target derivatives differ. Translation contributes `-1`,
while the smooth hinge row has configuration-dependent target derivative. A
floating physically determinate mechanism also retains six legitimate world
motions in its ungauged hard Jacobian.

## Decision

### Driver rates and component solve

`SpatialAssemblySession::velocity` is revision checked and accepts one finite raw
rate per hinge or translation position-driver `SpatialSourceId`. Requests are
canonicalized by source order; duplicate request entries reject and every
unlisted position driver has zero target rate.

For every requested source, a private parameterized compile extracts the active
normalized scalar column from the executable driver equation at the bitwise
accepted spatial snapshot. Hinge parameter scale is one and translation parameter
scale is model scale. The combined normalized residual-rate vector is solved
independently in each component of the accepted ungauged physical hard
linearization with that component's retained dense-authoritative rank threshold.

Any inconsistent component produces `SpatialVelocityOutcome::Inconsistent` with
finite residual evidence and no body/feature representative. Otherwise certified
domain mobility classifies the result: zero internal mobility is determinate
modulo gauge; positive internal mobility is underdetermined. Core minimum-norm
status alone cannot classify a floating mechanism because world gauge is
physical nullity.

### Representative and feature fields

Raw `Pose3` sensitivity blocks are right/body-local rates. For `T_WB=(R,t)`, the
published body-origin and angular world velocities are

```text
u_W = R v_B
omega_W = R omega_B.
```

The `Pose3` spatial adjoint's translational coordinate is not substituted for
body-origin velocity. For each floating certified component, one common world
twist is subtracted so its configured numerical reference is stationary. This
changes only the representative, not relative differentiated equations.

Concrete point, frame, clocked-axis and clocked-plane records are emitted in
definition order. A feature origin at world offset `r` from its body origin has
`u_feature = u_body + omega cross r`; frame axes, axis direction/clocks and plane
normal/clocks have derivative `omega cross direction`. Hinge winding remains
discrete, while principal phase and axial/planar translation coordinates publish
finite measured rates.

Every success-like representative is independently checked against differentiated
physical ground, joint, mate and driver formulas with normalized tolerance
`min(caller tolerance, 1e-9)`. Mode monitors remain row-free and are not silently
turned into velocity constraints.

### Optional physical motion basis

`velocity_with_options` may request a physical right-nullspace basis. Core uses
the accepted component rank and threshold, forms the right-nullspace projector,
scans deterministic normalized coordinate order with repeated orthogonalization,
sign-canonicalizes each vector, round-trips declared raw tangent scales and
validates `J n = 0`. No linkage-local SVD threshold is introduced.

Basis vectors retain the ungauged physical nullspace. In particular, a floating
determinate assembly publishes all six world-action directions. They are unit and
orthogonal only in normalized tangent coordinates; mixed-unit raw body and
feature fields are not Euclidean-orthonormal. The representative gauge subtraction
is not applied to basis vectors.

## Consequences

- Simultaneous shaft/bearing and block/base rates are source-order invariant and
  scale stable.
- Equal duplicate driver rates are consistent; unequal or omitted duplicate
  rates are explicitly inconsistent without a published least-squares field.
- Alternative floating references differ by one common world twist and static
  common-left `SE(3)` transforms rotate all physical velocity vectors.
- Position finite differences independently reproduce body and point fields.
- Motion bases preserve accepted rank/nullity and physical world gauge.
- These APIs describe instantaneous kinematics only. They expose no mass, force,
  reaction, collision, time integration or dynamics behavior.
