<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 — production-quality constraint annotations

Status: implementation, final clean qualification, immutable Tailscale nomination and scoped human
approval complete; exact GitHub Pages publication is the only open closeout step. No later
milestone is active. This milestone turns the workbench annotations into a polished CAD
demonstration without changing solver, sketch or branch semantics.

## Scope

- Present all seven dimension families with truthful family-specific geometry and compact CAD
  values: `25`, `R12`, `⌀24`, `45°`.
- Present all twenty compact constraint categories with geometry-derived orientation and placement.
  Constraint glyphs are contextual by default; Display may reveal all.
- Place shared-endpoint acute/right line-angle arcs, labels and hit geometry in the actual
  finite-ray interior wedge. Retain the acute supporting-line presentation for obtuse finite joins
  and preserve directed solver branch/value semantics.
- Let the protected X/Y-axis intersection communicate zero on canvas without a duplicate Origin
  ring/cross/text/focus target; retain Origin headlessly and in Reference/inspector interaction.
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

## Closeout evidence

Final candidate source `9b4e7f72dcacefdf4d7847a22eb675c711068d26`, tree
`e0591664fbeb2e353bc880dd826dc39ac1caeec9`, includes feature-refinement commit
`a9fd6f6a71edf5be9d9fb5856074d291192a898d` plus separate M22 property-oracle test hardening. The
angle-side behavior predated M76, so the corrected placement is an M76 feature refinement rather
than an `M76-Fxxx` defect. Its complete clean release gate passes in 569 seconds: editor 353/353,
demo-web 122/122, M76 5/5 natively and under WASM, carried M75 11/11 and M74 5/5 in both
environments, unchanged golden 270/270 and sparse crossover in 127.63 seconds, together with
formatting, warnings-denied Clippy/Rustdoc, all workspace tests, benchmarks, licensing/package
contents and Trunk 0.21.14 assembly.

The exact no-rebuild seven-file snapshot `/tmp/geosolve-m76-uat.ctgYzp` is read-only (directory
`0555`, regular non-symlink files `0444`) with ordered-manifest aggregate
`337b0e6a2ce2b6a9aed979d0a4849e2d0887c092df66efa345d4917929d01dd4`. PID `1455071`, retained
command-runner session `70653`, serves it at `http://100.94.63.83:8080/`;
`/tmp/geosolve-m76-http-verify.CqEufj` proves root plus all files return HTTP 200 with exact
media/length/bytes, no redirects or encoding, root equality and the same fetched aggregate. The
user reviewed the initial candidate, requested the two final feature refinements, and explicitly
authorized closure without separate post-refinement UAT. U1-U4 are accepted for scoped closure
under that approval; individual steps were not replayed or separately logged after the final
refinements. Exact GitHub Pages publication and hosted-byte verification remain the sole closeout
action.

The initial nomination at source `37eade50b566f62905a395655bc80c17d9b6bef4`, tree
`d6ad2f453d672accbcc3848a1a16d2039b3511d1`, snapshot `/tmp/geosolve-m76-uat.puiPgO` and aggregate
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf` is retained as superseded
historical evidence only; its PID `1077092` was retired.

## Non-goals

M76 adds no constraint, residual, geometry primitive, solver behavior, mobile layout, formula
language or canonical sketch persistence field. Exact GitHub Pages publication follows the scoped
human approval of the final frozen Tailscale candidate and completes the milestone.
