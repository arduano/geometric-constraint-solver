# ADR 0009: Physical grounding and numerical gauge are distinct

Status: accepted

## Context

A floating rigid assembly has a free world-frame action even when all relative geometry is fully determined. Dense solving often fixes one body numerically to remove that nullspace. Treating this coordinate choice as a physical ground gives false mobility and design-intent reports.

## Decision

`geosolve-linkage` represents physical grounding as persistent domain intent. A ground, world-frame mate or world-relative driver is a real source that changes admissible physical motion, participates in validation and diagnostics, and round-trips in the assembly document.

A numerical gauge is solver metadata used only to choose coordinates for a domain-certified floating component. It adds no physical source constraint, is not a conflict/redundancy candidate and does not reduce reported physical nullity. Alternative valid gauges must preserve relative geometry, hard validity, internal mobility and source diagnostics.

M14 introduces persistent body, feature and source IDs for `geosolve-linkage`, separate from runtime generational keys and consistent with the sketch identity policy introduced in M13. The default deterministic gauge anchors the body with the lowest persistent body ID to its last accepted world pose. A caller may request another documented gauge policy, but a gauge change is an explicit session operation and cannot change physical grounding. Manifold gauges use local difference and the pose convention in ADR 0006.

For each disconnected kinematic component, the domain certifies whether all hard relationships are invariant under a common world action. Only a certified floating planar component contributes three gauge DOF; only a certified floating spatial component contributes six. World-relative features, drivers or physical grounds can remove some or all of that action. The solver never infers `gauge_dof` by blindly subtracting three or six from numerical nullity.

Reports expose:

- total numerical equality right nullity;
- domain-certified gauge DOF;
- internal mobility after gauge separation;
- physical grounding sources;
- numerical gauge policy/reference;
- active-bound mobility separately.

Velocity/nullspace queries use the same split. A numerical gauge may choose one representative velocity, while optional bases and mobility reports retain physical global rigid motion.

The implementation is pure safe Rust with no `unsafe` code or native solver FFI. Gauge policy traits remain internal initially. This ADR defines coordinate handling for kinematics only; it adds no support reactions, forces, masses or dynamics.

## Transition

The planar baseline represents its ground body through fixed pose behavior, has runtime body identity only and does not report gauge/internal mobility separately. M14 adds persistent linkage identity before selecting the default gauge, performs the planar transition and must show three gauge DOF for a fully floating planar assembly. M16 applies the contract to spatial assemblies and must show six for a fully floating spatial assembly. Existing explicitly grounded L1-L3 behavior remains unchanged.

## Consequences

- Numerical conditioning choices cannot masquerade as user constraints.
- Relative assemblies solve reproducibly without losing truthful mobility.
- Persistence records physical ground and an optional gauge preference as different fields.
- Gauge-invariance metamorphic tests are required for planar and spatial migrations.
