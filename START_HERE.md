<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Start here

GeoSolve is an embeddable Rust constraint solver with a headless interaction layer,
bidirectional TypeScript authoring, and browser/local-server integration examples.
Start with the [README](README.md) for the product overview or the
[documentation index](docs/README.md) for a guide by task.

## Current work

[M99 is accepted and closed](docs/M99_CLOSURE.md). [M100](docs/M100_FINAL_CLEANUP.md)
is the final maintenance milestone before a project pause. Its documentation pass
is complete; the remaining code, tooling, retention and restart work is tracked in
[PLAN.md](PLAN.md). Historical handoffs describe their original checkpoints and
are not current startup instructions.

M98's mechanical qualification and human review are separate: U02 remains **Fail
pending human recheck**, and unperformed human rows remain **Not run**. See the
[current acceptance status](ACCEPTANCE.md#current-limits-and-review-status).

## Before changing code

Read [AGENTS.md](AGENTS.md), [ARCHITECTURE.md](ARCHITECTURE.md), [PLAN.md](PLAN.md),
[ACCEPTANCE.md](ACCEPTANCE.md) and [the scenario inventory](docs/SCENARIOS.md), then
the active milestone and owning subsystem's contracts.

Preserve separate sketch/linkage domain models, explicit branch state, independent
residual validation and transactional failure retention. New residuals need a
finite-difference Jacobian test and readable audit descriptor. Use the repository's
[defect-hardening workflow](.agents/skills/geosolve-harden-defect/SKILL.md) for solver,
domain or headless-interaction defects and golden-oracle expansion.

## Build, inspect and qualify

- [Getting started](docs/GETTING_STARTED.md): prerequisites, source build and local server.
- [Authoring](docs/AUTHORING.md): code/UI workflows, parameters and shared editing.
- [Development](docs/DEVELOPMENT.md): ownership, focused checks and contribution workflow.
- [Release qualification](docs/RELEASE_QUALIFICATION.md): integrated gates and authenticated evidence.
- [Compatibility](docs/API_COMPATIBILITY.md): persistence, public APIs and installed packages.

Use focused checks while implementing and the integrated gate when nominating a
product. The gate owns format, Clippy, native tests, optimized WASM, browser,
package and performance obligations. Only its authenticated unchanged-input
receipts permit reuse. Documentation-only closeout preserves the previously
qualified product; it does not qualify new package bytes.

Record files/APIs, mathematical behavior, exact commands and outcomes, acceptance
criteria, and remaining limitations. Keep credentials, local deployment inventories
and private artifact locations outside tracked documentation. Back up complete
project state before restart or storage maintenance; source text alone does not
represent shared history and pending operations.
