<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 handover — Workflow-led authoring workbench redesign

Status: **audit/contract complete; implementation and UAT have not started**. Do not reconstruct
scope from chat history.

## Incoming authority

- Repository: `/home/arduano/programming/geometric-constraint-solver`.
- M87 clean-qualified source:
  `32c72892772ee09f8b904153484b02fd9923dc25`.
- M87 clean-qualified tree:
  `38f7175f93c87d11422f5de00e78208f8cf315bb`.
- M88 currently consists only of its audit, goals, prospective implementation ledger, pending UAT
  and this handover. No production code, test, build, artifact, service or publication change is
  implied by these documents.

Preserve the twelve bundled code projects, especially the CNC joinery fit coupon and fully
constrained Gridfinity section added in M87. Preserve managed controls, exact-CAS outer history,
solver-instance overlays, shared rendering, headless inspect/edit/render, complete scene paint and
the full LOD removal.

## Mandatory read order

1. `AGENTS.md`, `START_HERE.md`, `ARCHITECTURE.md`, `PLAN.md`, `ACCEPTANCE.md` and
   `docs/SCENARIOS.md`.
2. ADR 0041 and ADR 0042 for optional managed-code and headless authority.
3. `docs/M88_AUDIT.md` for measured current behavior and workflow traces.
4. `docs/M88_GOALS.md` for required scope, authority boundaries and non-goals.
5. `docs/M88_IMPLEMENTATION.md` for the ordered unchecked phases.
6. `docs/M88_UAT.md` before changing markup, focus behavior or presentation persistence.

## Exact resume order

1. Establish an optimized/release browser build identity and repeatable Gridfinity timing.
2. Use the defect-hardening workflow to freeze the actual browser/WASM stack contract and a
   separate one-MiB native proxy regression; reduce stack use at their shared owning path without
   changing mathematical acceptance or treating either result as proof of the other.
3. Remove independently proven duplicate checkpoint restore/validation and no-op post-open
   encoding, with persistence/history/accepted-scene equivalence.
4. Only then consider the full-hard-row-rank redundancy shortcut at `geosolve-core`, behind an
   owner regression and unchanged independent residual validation.
5. Freeze command frequency, focus order and the three-mode layout contract.
6. Implement shell/layout before relocating individual panels; then navigation, Code, ownership/
   Problems, browser/headless handoff and cleanup in ledger order.
7. Run every pending UAT row and complete qualification before any nomination or publication claim.

## Frozen UX target

- Compact File/project app bar with Undo/Redo, true title/status, Design/Split/Code and Export;
  repro/trace under Diagnostics.
- Narrow Select/Sketch/Constraint/Dimension/Modify rail with last-used state and click/keyboard
  submenus for every current variant.
- One collapsible Explorer instead of simultaneous tree and Design hierarchy.
- Design canvas, resizable Split and first-class central Code modes.
- Right Inspector, Parameters and Problems tabs in Design/Split; the latter two rehost their one
  logical state into Code mode rather than creating duplicate views.
- Sticky Apply/Revert/status, at least 12 px source text, at least `720 x 500` CSS px of editor at
  `1024 x 720`, source tabs and secondary Parameters, Problems, Generated and Artifacts surfaces.
- Searchable New/Start from code/sample/import/recents surface covering all 37 samples.
- Exact managed-owner Open in code and canonical `project.json`/`sketch.ts`
  browser-to-headless-to-browser handoff.

## Performance evidence to retain

Gridfinity is a valid 62x62 full-rank, zero-DoF system with no `FixedPoint` and one Y
`FixedCoordinate`. Its 35.6-to-20 width edit succeeds in native/release WASM with hard residual
`4.85e-12`. Debug cold open is 23.3–37.7 seconds and debug edit traps memory OOB; release open/edit
are 2.67/1.75 seconds. A one-MiB native proxy stack fails and 2 MiB passes; this does not replace
actual browser/WASM stack evidence. The post-prerequisite recorded-machine budgets are five-run
cold-open median/max at most 2.0/2.5 s and edit median/max at most 1.25/1.75 s. Timing runs span
dispatch to replacement accepted-frame presentation. The cold-open profile is
`/tmp/geosolve-gridfinity-open.cpuprofile.json`, SHA-256
`d7be3dcaad3fcf63342de0263b44c79bd7f95927b28ee5d59ab656244b0601f`.
These facts justify the ordered prerequisites; they are not repair or qualification evidence.

## Boundaries and cautions

- Presentation preferences never enter canonical project, repro, solver, accepted-scene or history
  authority. Resizing and mode changes do no semantic work.
- Invalid code retains prior accepted paint and one outer transaction boundary.
- Canonical project export returns a typed refusal while any unapplied draft exists; raw draft-
  source download remains separate and claims no accepted-project authority.
- Do not execute custom TypeScript in Rust/WASM or add an AI chat/stateful service.
- Do not build a general IDE, add new sketch mathematics or weaken independent residual checks.
- Do not restore LOD or hide accepted geometry for performance.
- Preserve existing dirty user work if implementation resumes in a non-clean tree; inspect before
  editing and make small, owning-boundary changes.

## Files introduced by the planning pass

- `docs/M88_AUDIT.md`
- `docs/M88_GOALS.md`
- `docs/M88_IMPLEMENTATION.md`
- `docs/M88_UAT.md`
- `docs/M88_HANDOVER.md`
