# OpenCode handoff

## Objective

Implement a robust pure-Rust geometric constraint solver following the staged plan in this repository. The first useful vertical slice must solve and display both:

1. an underconstrained 2D sketch triangle with interactive dragging;
2. a driven planar four-bar linkage that preserves its assembly mode.

Do not attempt every CAD constraint in one pass.

## Required reading

- `ARCHITECTURE.md`
- `PLAN.md`
- `ACCEPTANCE.md`
- `REFERENCES.md`
- `docs/SCENARIOS.md`
- `AGENTS.md`

## First assignment

Complete **M1 and M2** in `PLAN.md`, then stop for review.

The expected M1/M2 result is:

- packed scalar/vector variable blocks with stable IDs;
- residual blocks with declared variable incidence and source IDs;
- structured human-readable audit descriptors for source constraints and scalar residual rows;
- dimensionless residual scaling;
- dense residual/Jacobian assembly;
- central finite-difference Jacobian verification;
- manifold-aware `Pose2` local updates;
- a damped Gauss-Newton or Levenberg-Marquardt loop;
- rank/DOF calculation through dense QR/SVD;
- strict independent residual validation;
- explicit `SolveTermination`/`SolveReport` outcomes plus accepted-state equation audit snapshots;
- unit tests covering exact, underdetermined, inconsistent, rank-deficient and badly scaled systems.

## Boundaries for the first assignment

Do not yet implement:

- sparse `faer` solving;
- circles/arcs/tangency;
- conflict set minimization;
- pseudo-arclength continuation;
- spatial `SE(3)` mechanisms;
- a JavaScript framework.

It is acceptable for M1/M2 APIs to remain `pub(crate)` while they settle. The public contract is the behavior and reports, not a frozen generic abstraction.

## Required verification before handoff

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Include exact test output and call out any acceptance criterion not yet met.
