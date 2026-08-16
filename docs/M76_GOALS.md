<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 — production-quality constraint annotations

Status: active. This milestone turns the workbench annotations into a polished CAD demonstration
without changing solver, sketch or branch semantics.

## Scope

- Present all seven dimension families with truthful family-specific geometry and compact CAD
  values: `25`, `R12`, `⌀24`, `45°`.
- Present all twenty compact constraint categories with geometry-derived orientation and placement.
  Constraint glyphs are contextual by default; Display may reveal all.
- Make every dimension and compact glyph movable. Geometry-locked right-angle squares remain fixed.
- Retain placement in an editor-owned presentation store, support selected/global reset, and keep
  movement outside solving, sketch revisions and sketch Undo/Redo.
- Persist an optional self-versioned annotation cache in workspace v6. Malformed, stale or
  incompatible cache state is disposable and may never reject valid sketch geometry.

## Architecture

The retained `ConstraintEditor` owns `AnnotationLayoutState`. A transient `EditorScene` receives
resolved layout and exact paint/hit primitives: baselines, witnesses, leaders, arcs, arrowheads,
label bounds and glyph bounds. Automatic layout deterministically reserves geometry, points,
existing annotations, viewport margins, fixed marks and manual placements.

Placement forms are signed perpendicular offset with a continuity frame for linear dimensions,
radial direction plus leader clearance for full circles, canonical on-arc direction plus clearance
for bounded arcs, angular radius, and free screen offset for label-only legacy annotations and
compact glyphs. Select-mode label/glyph drag uses a 3 px threshold. Escape, capture loss,
camera/tool change or cancellation restores its origin. A successful move performs no solve,
sketch revision or history mutation.

## Compatibility and lifecycle

Workspace v1-v5 migrate with an empty cache. New documents and samples clear layout; Delete and
normal same-document Undo/Redo retain surviving entries. The optional cache remains outside
canonical sketch data and reproduction geometry authority. Canonical sketch v1-v4, unsupported
draft-v5, `GEOSOLVE_REPRO_V1`, equations, solver history and branch semantics are unchanged.

## Formatting and accessibility

Use four significant digits, strip trailing zeros and negative zero, and use scientific notation
below `1e-3` or at/above `1e5`. A reference value wraps the whole compact notation in parentheses
and uses muted/dashed non-colour styling. Full semantic names remain in inspector, tooltip and ARIA
text.

## Non-goals

M76 adds no constraint, residual, geometry primitive, solver behavior, mobile layout, formula
language or canonical sketch persistence field. GitHub Pages publication follows explicit human
approval of the frozen Tailscale candidate.
