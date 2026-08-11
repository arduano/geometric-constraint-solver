---
name: geosolve-harden-defect
description: Reproduce, classify, regress, and qualify defects in GeoSolve's solver and presentation-independent interaction stack. Use whenever investigating, diagnosing, test-hardening, or fixing suspected failures in geosolve-core, geosolve-sketch, geosolve-linkage, geosolve-constraint-editor, the retained coordinator, accepted-scene authority, payload restoration, authoring, selection, picking, dragging, constraints, dimensions, convergence, rank or degrees of freedom, scaling, explicit branches, invalid geometry, or the golden authoring/scene corpus. Do not use for a purely browser, CSS, layout, text, or icon defect unless evidence indicates a Rust headless or scene-authority contract failure.
---
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Harden a GeoSolve Defect

## Establish scope

Read the repository `AGENTS.md` and
[`references/testing-layers.md`](references/testing-layers.md) completely. In diagnosis-only mode,
read the active-status and owning-subsystem sections needed to ground the report. Before changing
tests or implementation, read every project document required by `AGENTS.md`.

Preserve the user's exact reproduction steps, payload or scene fingerprint, observed outcome, and
source revision. Distinguish mathematical correctness from browser presentation or interaction
feel.

Honor the requested work mode:

- For diagnosis, inspect and report without changing production code or tests.
- For test-only hardening or discovery, add or run tests and evidence without fixing production
  behavior.
- For an authorized fix, reproduce first, add the smallest owning-layer regression, implement the
  correction, and qualify it.

Never infer permission to move from diagnosis or test-only discovery into a production fix.

## Reproduce and route

Reproduce through the narrowest public Rust boundary that still exhibits the reported behavior.
Use the exact supplied payload through the ordinary decoder/coordinator path when payload identity
is part of the failure. Do not duplicate equations or accepted-state logic in a test adapter.

Assign a milestone finding ID only after independent reproduction. Deduplicate reports with the
same root cause before opening another finding. Route the regression to the owner identified in the
testing-layers reference; add a thin adapter test only when the defect crosses that adapter.

During discovery, isolate cases with bounded execution and continue after a defect, panic, timeout,
or harness error. Produce a complete checklist instead of stopping at the first failure.

## Freeze the contract

Add a minimal exact regression at the owning layer and validate semantic invariants independently
of the implementation's success status. Require finite accepted geometry, independently validated
hard residuals, explicit branch state, transactional failure retention, and relevant rank/DOF or
locality evidence.

For any residual or equation change, add a central finite-difference Jacobian check and a structured
human-readable audit descriptor. For convergence, rank, scaling, or branch defects, retain a named
regression scenario.

Expand the broad golden authoring/scene matrix only when the defect exposes a missing systemic
dimension. Keep isolated defects in focused owner regressions. Review every golden change row by
row; never bless changed bytes merely to make a check pass.

## Repair and qualify

When a production fix is authorized, keep it behind the failing regression, preserve explicit
branches and hard/soft semantics, and re-run the focused owner test before collateral suites.
Run the generic golden survey/check and the milestone-appropriate native, WASM, Clippy, formatting,
and release gates described in the reference.

Report the reproduction identity, owner and root cause, regression added, production behavior
changed or deliberately unchanged, exact commands and outcomes, acceptance criteria, and remaining
limitations. Distinguish commands that actually ran from proposed test targets; never present an
invented or unverified test name as an existing command.
