<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Acceptance criteria

These are behavioral gates. [PLAN.md](PLAN.md) owns milestone order; the
[scenario inventory](docs/SCENARIOS.md) and [detailed acceptance reference](docs/reference/ACCEPTANCE_DETAILS.md)
retain the exact subsystem and historical regression requirements. Performance
results never weaken correctness thresholds.

## Global quality gates

Every accepted change must preserve:

- Finite accepted state, residuals, Jacobians, factorization inputs and reports.
- Independent hard-residual and domain/branch validation before success or commit.
- Transactional retention of accepted geometry, source authority and history on rejection.
- Deterministic result, source, component and diagnostic ordering for identical input.
- A central finite-difference comparison and structured audit for every residual implementation.
- Explicit branch/orientation state and documented hard/soft priority semantics.
- Pure Rust solver dependencies, no unauthorized `unsafe`, and GPL-3.0-or-later metadata.

Use the [release runner](docs/RELEASE_QUALIFICATION.md) for integrated qualification.
Its obligations include the following commands plus optimized WASM, package,
golden, browser, headless and bounded performance coverage:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
```

See [Getting started](docs/GETTING_STARTED.md) for the complete frontend/engine
preparation order. Focused development results are not full release qualification.
A documentation-only closeout retains the qualified product identity and does
not nominate newly packaged documentation.

## Numerical and interaction policy

The established baseline requires maximum normalized hard residual `<= 1e-9` and
analytic/local-AD Jacobian relative error `<= 1e-6` away from singular or
nondifferentiable states. Model scales `1e-6`, `1` and `1e6` preserve topology,
branch labels, rank/mobility classification and source diagnosis, unless an owning
scenario explicitly specifies a stricter or conditioning-justified alternative.

Current reports separate hard validity, nonlinear termination and secondary
optimization. Component-local machine-floor thresholds govern numerical rank;
left/right nullity, near-singular warnings, active-bound mobility and structural
partitions follow the [architecture reference](docs/reference/ARCHITECTURE_DETAILS.md).
Historical milestone rules are retained in the detailed acceptance reference;
they must not be applied retroactively to different solver versions.

Source edits, UI commands, persistence and shared publication require exact
identities and stale-work rejection. Failed candidates retain the complete prior
accepted state. Local navigation and selection cannot depend on a server solve.
Provisional geometry has no authority to publish or replace accepted picking.
Native/WASM, restoration, failure retention and actual browser regressions cover
these boundaries.

## Regression and oracle policy

Owning-layer regressions remain mandatory after later milestones. Every reported
convergence, rank, scaling or branch failure gets a reproducible scenario. Golden
changes require the repository's [defect-hardening workflow](.agents/skills/geosolve-harden-defect/SKILL.md)
and independent review; qualification must not regenerate expected values to make
a failure pass. Use exact equality where the contract requires it, including
canonical persistence and accepted-scene provenance.

Human acceptance and automated qualification are separate evidence. An automated
replay cannot change a human **Fail** or **Not run** row into **Pass**.

## Current limits and review status

| Area | Current record |
| --- | --- |
| M99 | Accepted and closed on 2026-09-13. Product `eb4d2e06933a7ed5e02533f0359abff28040991a`; run `20260913T155413-41e9f715`; 297/297 obligations, with 32 fresh and 265 authenticated reused. |
| Installed M99 product | Four matching offline archives; all 285 installed files verified. The reviewed 271-case golden is unchanged. |
| M98 human review | U02 remains **Fail pending human recheck** after the corner-drag repair. Other unperformed human rows remain **Not run**. M99 closure does not change them. |
| M100 | Documentation cleanup is complete. Code/tooling cleanup, restart exercises and final nomination remain open. |
| Dense startup | Earlier focused manifold opening took 6.484 s. Dense solving can take seconds; no five-second startup or universal frame-rate guarantee is claimed. |
| Shared responsiveness | Qualified held-manifold navigation measured 264.2 ms p95 and durable text acknowledgement 335.2 ms, with zero navigation RPCs. These are bounded test results, not arbitrary-scale guarantees. |
| Demo size | Qualified M99 demo WASM is 20,940,556 bytes, 30,964 bytes below the 20 MiB gate. |
| Scope | Desktop sketch/kinematics foundation. No solid modeler, dynamics, production hosted collaboration service or mobile workbench commitment. |

See [M99 qualification](docs/M99_QUALIFICATION.md), [M99 closure](docs/M99_CLOSURE.md),
[M98 UAT](docs/M98_UAT.md) and [M100's remaining criteria](docs/M100_FINAL_CLEANUP.md).
The published GitHub Pages snapshot may differ from the latest qualified source.
