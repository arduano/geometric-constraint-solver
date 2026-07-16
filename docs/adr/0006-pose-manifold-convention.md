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

M15 sets the quaternion norm validation band to `abs(norm(q) - 1) <= 1e-6`. Accepted values in that band are normalized and then sign-canonicalized. The sign tie tolerance is `32 * f64::EPSILON`: prefer `q_w > 0`, and when `q_w` is within that tie band use the first materially nonzero component of `(q_x, q_y, q_z)` as positive. `Log` uses the principal rotation in `[-pi, pi]`; exact half turns are selected by the canonical quaternion vector sign, while callers must not differentiate exactly across that principal-log cut.

Because `q` and `-q` represent the same rotation, accepted `Pose3` values use a deterministic sign: prefer `q_w > 0`; when `q_w == 0` within canonicalization tolerance, the first nonzero value among `q_x`, `q_y`, `q_z` is positive. Quaternion sign is representation state, not an assembly branch.

`Exp`, `Log`, adjoint and Jacobian helpers use numerically stable small-angle series. `Log` reports the principal local rotation; multi-turn driver winding is separate explicit domain state. Branch and assembly-mode values stay outside manifold arithmetic and outside AD.

Velocity APIs must name their frame. The default reduced tangent and nullspace basis use body-local/right-trivialized coordinates matching retraction; world/spatial velocities are explicit conversions through the adjoint.

The existing `CoordinateBound` contract remains an additive-coordinate box bound. M15 supports it for scalar, `Vec2` and `Vec3` blocks and rejects it for `Pose2` and `Pose3`; a future pose limit requires an explicitly named scalar chart/function and current tangent gradient rather than indexing quaternion or world-translation storage.

Frames and workplanes are valid only when constructed from finite right-handed orthonormal axes through checked constructors. Point/vector forward and inverse transforms reject non-finite data, invalid frames and off-workplane inverse requests rather than silently normalizing materially invalid input.

Accepted-state linearization is exposed only from a revision-stamped `SolveSession`, never from an arbitrary unvalidated `Problem` state. It returns deterministic component-local normalized hard matrices, row identities and reduced root/member tangent mappings without exposing private compiler IR. Sensitivity solves use the accepted component rank threshold, return body-local raw tangent blocks, distinguish unique/minimum-norm/inconsistent outcomes and independently validate `J * delta + rhs` before any success-like status.

The implementation is pure safe Rust, with no `unsafe` code or native manifold FFI. Manifold evaluator traits remain internal initially.

## Transition

M15 completed the migration from additive `Pose2` solving to this retraction and added `Pose3`. Planar accepted geometry and branch behavior are preserved through property, finite-difference and global-transform tests; identical internal increments or iteration counts are not required. Accepted sensitivity is limited to reduced hard equalities in body-local coordinates until later milestones define active-bound, secondary-objective and world-frame policies.

## Consequences

- Fixed/alias elimination and numerical gauges operate through manifold local difference rather than ambient subtraction.
- Pose residuals share one frame convention across geometry, core and linkage crates.
- Persistence stores canonical ambient values plus explicit winding/assembly state.
- This ADR defines kinematics only and introduces no mass, force, reaction or dynamics semantics.
