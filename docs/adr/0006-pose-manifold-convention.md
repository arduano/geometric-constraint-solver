# ADR 0006: Pose2 and Pose3 manifold convention

Status: accepted

## Context

The baseline `Pose2` is stored as translation plus an unwrapped angle and applies additive coordinate increments. Spatial kinematics needs one explicit transform, tangent, retraction and quaternion convention so residuals, finite differences, velocities, gauges and persistence cannot silently mix frames.

## Decision

A pose `T_WB` maps coordinates from body/local frame `B` into parent/world frame `W`:

```text
p_W = R_WB * p_B + t_WB
T_WC = T_WB * T_BC
```

Pose increments use right retraction in the current body frame:

```text
T_new = T * Exp(delta_body)
local_difference(T_reference, T) = Log(inverse(T_reference) * T)
```

`Pose2` tangent order is `[v_x, v_y, omega]`. `Pose3` tangent order is `[v_x, v_y, v_z, omega_x, omega_y, omega_z]`. Translational and angular step scales remain separate positive finite values. Jacobians and finite differences differentiate these tangent coordinates through right retraction.

`Pose2` ambient persistence is translation plus an unwrapped angle. `Pose3` ambient storage is `[t_x, t_y, t_z, q_w, q_x, q_y, q_z]`, with a finite unit quaternion. Quaternion multiplication follows transform composition order. Imported and committed quaternions are normalized only within a documented small validation band; zero or materially non-unit quaternions reject.

Because `q` and `-q` represent the same rotation, accepted `Pose3` values use a deterministic sign: prefer `q_w > 0`; when `q_w == 0` within canonicalization tolerance, the first nonzero value among `q_x`, `q_y`, `q_z` is positive. Quaternion sign is representation state, not an assembly branch.

`Exp`, `Log`, adjoint and Jacobian helpers use numerically stable small-angle series. `Log` reports the principal local rotation; multi-turn driver winding is separate explicit domain state. Branch and assembly-mode values stay outside manifold arithmetic and outside AD.

Velocity APIs must name their frame. The default reduced tangent and nullspace basis use body-local/right-trivialized coordinates matching retraction; world/spatial velocities are explicit conversions through the adjoint.

The implementation is pure safe Rust, with no `unsafe` code or native manifold FFI. Manifold evaluator traits remain internal initially.

## Transition

M11 migrates baseline additive `Pose2` solving to this retraction and adds `Pose3`. Before that gate passes, public reports must describe `Pose2` increments as the additive baseline. M11 must preserve planar accepted geometry and branch behavior through property, finite-difference and global-transform tests; identical internal increments or iteration counts are not required.

## Consequences

- Fixed/alias elimination and numerical gauges operate through manifold local difference rather than ambient subtraction.
- Pose residuals share one frame convention across geometry, core and linkage crates.
- Persistence stores canonical ambient values plus explicit winding/assembly state.
- This ADR defines kinematics only and introduces no mass, force, reaction or dynamics semantics.
