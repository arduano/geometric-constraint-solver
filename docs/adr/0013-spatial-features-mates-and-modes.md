# ADR 0013: Spatial features, mates and assembly modes

Status: accepted

## Context

M18 established one checked `Pose3` per spatial body, body-local point and frame
features, physical grounding, private six-coordinate gauges, and the first ball,
fixed-frame and revolute relations. M20 adds the common joint and mate catalog,
position coordinates, simultaneous drivers and explicit assembly modes. The
roadmap named those primitives but did not freeze their feature signatures,
equations, coordinate signs or branch boundaries.

Spatial equations use `T_WB`, right/body-local pose increments and dense-SVD rank
authority from ADRs 0006, 0009 and 0012. Every success remains subject to fresh
hard-row and domain validation capped at normalized residual `1e-9`.

## Decision

### Clocked features

An axis and a plane each store a complete checked body-local `Frame3`.

- The frame origin is the axis or plane origin.
- The directed `z` axis is the axis direction or plane normal.
- The directed `x` and `y` axes are persistent transverse or in-plane clocks.
- Clock directions are never regenerated from world coordinates or current poses.

Axis and plane identities remain separate concrete types. No public generic
feature trait is introduced.

Every public spatial ID contains a private nonzero assembly namespace and a
local ordinal. A safe atomic allocator assigns the namespace when
`SpatialAssembly::new` succeeds, and assembly clones retain it. Equality,
hashing and ordering include both fields, while `as_u64`, `Display`, `Debug`
and audit bindings expose only the deterministic local ordinal. Namespace data
is not serialized or otherwise public; complete persistence remains M23. Every
constructor, gauge policy, coordinate, monitor and transaction lookup checks
the complete ID, so a same-ordinal ID from another assembly is typed `Unknown*`.

For transformed feature frames `F_i = (p_i, x_i, y_i, z_i)`, define
`d = p_2 - p_1` and `b = parity * z_2`, where aligned parity is `+1` and opposed
parity is `-1`.

### Joint equations

The M20 joint rows are:

```text
prismatic:
    x_1 dot d
    y_1 dot d
    x_1 dot b
    y_1 dot b
    y_1 dot x_2

cylindrical:
    x_1 dot d
    y_1 dot d
    x_1 dot b
    y_1 dot b

planar:
    z_1 dot d
    x_1 dot b
    y_1 dot b

universal:
    p_2 - p_1
    z_1 dot z_2
```

Length rows use the immutable assembly model scale. Orientation rows are
dimensionless. At regular configurations the ranks are respectively `5`, `4`,
`3` and `4`, leaving grounded relative mobility `1`, `2`, `3` and `2`.

Prismatic independent validation additionally requires positive retained clock
alignment. Axis and normal alignment independently require the selected parity
dot product to exceed the documented orientation branch margin.

### Mate equations

The public mate meanings are:

- point distance: one strictly positive point-to-point distance row;
- axis angle: one directed-axis `dot(z_1,z_2) - cos(target)` row for targets
  strictly inside `(0, pi)`;
- axis alignment: direction-only two-row alignment with explicit parity;
- frame offset: a complete six-row relative `SE(3)` target.

Direction-only axis alignment is deliberately distinct from cylindrical/coaxial
placement. Zero distance lowers to point coincidence rather than changing the
codimension of a distance row. Angle targets zero and pi lower to explicit-parity
axis alignment rather than using a rank-deficient interior-angle row.

Frame offset uses the first world feature frame composed with one checked fixed
relative pose as its expected second frame. Position uses three model-unit rows;
orientation uses the M18 three off-diagonal frame rows. Independent positive
diagonal checks reject the unwanted half-turn roots relative to the expected
frame.

### Coordinates and drivers

A hinge coordinate belongs to a revolute, cylindrical or planar relation. Its
positive phase is measured from the first axis/plane x-clock to the second
x-clock about the first directed axis/plane normal under the retained parity.
State stores an integer winding separately from a canonical principal phase;
quaternion sign is not winding.

Translation coordinate kinds are explicit rather than overloaded:

```text
axial, prismatic/cylindrical: q = z_1 dot (p_2 - p_1)
planar X:                       q = x_1 dot (p_2 - p_1)
planar Y:                       q = y_1 dot (p_2 - p_1)
```

The public definitions are `SpatialCoordinateKind::AxialTranslation` and
`SpatialCoordinateKind::PlanarTranslation { axis }`, where
`SpatialPlanarTranslationAxis` is `X` or `Y`. Construction uses
`add_axial_translation_coordinate` or `add_planar_translation_coordinate` and
the accepted value variant repeats the axial or selected planar axis kind.

Each position driver is its own hard source. The executable hinge row is the
raw clock-dot expression

```text
cos(target) * (y_1 dot x_2) - sin(target) * (x_1 dot x_2)
```

which is projection magnitude times the sine phase error off the parent
manifold. Independent validation checks the positive cosine root, exact winding
and parent parity. A translation driver uses the selected `q - target` with a
model-unit scale and an exact right-local analytic Jacobian. Several drivers
compile simultaneously; they are never replaced by a weighted objective.
Spatial continuation and multi-driver velocity remain deferred to M23.

### Assembly modes

Axis parity, hinge winding, plane side and signed volume are explicit state
outside automatic differentiation.

```text
axis parity:   selected_sign * dot(z_1, z_2)
plane side:    selected_sign * dot(z_plane, p_observed - p_plane) / model_scale
signed volume: selected_sign * dot(unit(B-A) x unit(C-A), unit(D-A))
```

Each retained metric must be finite and greater than the documented branch
margin. Zero or ambiguous metrics reject. Monitor-only relationships contribute
domain connectivity and structured mode evaluations but no synthetic core rows,
rank or mobility.

### Transactions and validation

One revision-checked assembly-mode transaction may stage multiple driver targets,
feature edits, parity/winding/side/volume changes and pose guesses. Duplicate IDs,
stale revisions and invalid values reject before solving. The candidate is solved
once through the private-gauge and separately published physical stages, then all
equations and modes are independently validated. Only the complete candidate is
swapped; failure retains every accepted view.

All non-ground M20 relationships are relative and common-left `SE(3)` invariant,
so the M18 six-DOF floating gauge certification remains valid.

## Consequences

- Stable clocks eliminate state-dependent perpendicular-vector choices.
- Private assembly provenance prevents same-ordinal IDs from resolving across
  independent in-memory assemblies without exposing persistence metadata.
- Joint and mate rank expectations are literal and testable.
- Cylindrical placement and direction-only axis alignment have distinct APIs.
- Axial, plane-X and plane-Y translation coordinates remain distinct in public
  definitions and accepted state.
- Endpoint angle and zero-distance codimension changes are explicit lowering
  choices rather than numerical special cases.
- Branch monitors cannot corrupt equality rank or diagnostics.
- M20 may use private common residual atoms, but every public source retains one
  semantic source identity, structured audit rows and an independent oracle.
- Complete spatial persistence, continuation and multi-driver velocity remain M23
  work.
