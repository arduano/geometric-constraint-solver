<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M72 — Public workbench bulk fixes

Status: activated by the supervising caller on 2026-08-14. This milestone replaces the previously
prepared semantic-consolidation proposal, which moves without implementation to
`docs/M73_GOALS.md`.

## Goal

Repair three bounded authoring/workbench defects, consolidate option surfaces into the canvas, and
publish the qualified desktop workbench at
`https://arduano.github.io/geometric-constraint-solver/` from the public repository.

## Accepted work

### M72-F001 — stale Problems after recovery

- Clear obsolete native and computed-feature setup errors after successful Undo, Redo, repair,
  reset or reload while retaining genuine failures under the current host inputs.
- Rebuild a same-sketch checkpoint when the live attempt is rejected instead of preserving that
  rejected attempt through the identity fast path.
- Let the Problems card dismiss only the exact current rendered problem set. Diagnostics, tree
  state and canvas markers remain authoritative; a changed problem opens automatically.

### M72-F002 — interactive rectangle freedom

- Keep the ordinary constrained rectangle macro unchanged.
- Remove only the interactive proposal's generated anchor and two dimensions, including their
  owned private scalars. Retain shared corners, explicit edge branches and four H/V constraints.
- Require finite validated geometry, rank four, four local equality DOF and one-step Undo/Redo.

### M72-F003 — canvas-owned tool options

- Use one nonmodal bottom-left canvas overlay for Equal, Tangent, Continuity, every dimension,
  Fillet, Conic-family construction, NURBS and Construction display.
- Open the relevant options when an option-bearing tool is selected; show and validate only the
  active family/subtype; remember valid values only for the current process.
- Provide one-open-at-a-time, bounded scrolling, keyboard focus/return, Escape, explicit close and
  light-dismiss behavior at desktop and compact-desktop sizes.

### M72-F004 — public Pages release

- Add an artifact-based, pinned GitHub Pages workflow that runs the complete release gate before a
  separate repository-prefixed Trunk build and deployment.
- Recheck the full Git history for secrets before making it public, enable workflow Pages through
  GitHub CLI/API, push `main`, set the repository homepage and verify the public files and runtime.
- Link the live demo from the README and expose repository source/licence links in the workbench.

## Acceptance

- Focused owner regressions independently validate F001 and F002 without changing residuals,
  Jacobians, branches, persistence schemas or the reviewed 234-row golden inventory.
- Native and WASM/web tests cover overlay state, family-local validation, Problems visibility and
  existing adapter behavior; the complete clean release gate passes.
- Chromium UAT at `1440x900` and approximately `1024x720` confirms the repairs, accessibility and
  containment. The public site loads with correct prefixed assets and preserves a browser-local
  scene across reload.
- The supervising caller approves the focused M72 UAT before final milestone closure.

## Non-goals

M72 does not implement M73 semantic consolidation, broaden constraint/inference families, add
rectangle snapping, change sketch-operation identity reporting, make Finish/Cancel sticky, add a
custom domain, restore browser E2E infrastructure, or add tablet/mobile support.
