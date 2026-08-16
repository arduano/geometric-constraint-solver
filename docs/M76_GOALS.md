<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 — production-quality constraint annotations

Status: complete (2026-08-17). Implementation, final clean qualification, immutable Tailscale
nomination, scoped human approval, exact GitHub Pages publication and public browser verification
all pass. No later milestone is active. This milestone turns the workbench annotations into a
polished CAD demonstration without changing solver, sketch or branch semantics.

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

Final candidate source `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
`248cba4509a992aeff7a02dd6d57a1a2481380a4`, includes feature-refinement commit
`a9fd6f6a71edf5be9d9fb5856074d291192a898d`, separate M22 property-oracle test hardening and
milestone-neutral shared-runner performance-gate hardening. The angle-side behavior predated M76,
so the corrected placement is an M76 feature refinement rather than an `M76-Fxxx` defect. Its
complete clean release gate passes: editor 353/353, demo-web 122/122, M76 5/5 natively and under
WASM, carried M75 11/11 and M74 5/5 in both environments, unchanged golden 270/270 and sparse
crossover in 151.76 seconds, together with formatting, warnings-denied Clippy/Rustdoc, all
workspace tests, benchmarks, licensing/package contents and Trunk 0.21.14 assembly.

The exact no-rebuild seven-file snapshot `/tmp/geosolve-m76-final-uat.65Y8J1` is read-only
(directory `0555`, regular non-symlink files `0444`) with ordered-manifest aggregate
`967f0c1943c16b9c4a9975aeb973ad0cfe2c6e3dbfab45f414d0dac1bb9088f3`. PID `1780608`, retained
command-runner session `30164`, serves it at `http://100.94.63.83:8080/`;
`/tmp/geosolve-m76-final-http-verify.UwoaMK` proves root plus all files return HTTP 200 with exact
media/length/bytes, no redirects or encoding, root equality and the same fetched aggregate. The
user reviewed the initial candidate, requested the two final feature refinements, and explicitly
authorized closure without separate post-refinement UAT. U1-U4 are accepted for scoped closure
under that approval; individual steps were not replayed or separately logged after the final
refinements.

Pages run `31957299907` failed twice only because the old 180-second wall-clock assertion observed
209.696267408 and 208.757508921 seconds after all semantic assertions passed. No M76 finding was
opened. The milestone-neutral gate retains 180 seconds as its advisory reference and enforces a
240-second shared-runner ceiling without changing solver behavior or workload. Final source
`a7769e4` passes Pages run `31961652265`, qualify job `95200423007` including the
184.090683967-second sparse crossover, deploy job `95204687455` and deployment `5933831093`.
Artifact `9267811418` is 2,164,829 bytes with ZIP/GitHub SHA-256
`dba7e2f5e1b7a51390ec1d840e7869d69968114bcf13250e641448a02d0cb60b`, inner-tar SHA-256
`be18173d61fef8ead3d00cf2dd560f893a7731eff7fa3bdfc0b81aadab6298e5` and exact seven-file
manifest aggregate `41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96`.
Root and every public file byte-match the hosted artifact with expected media types, zero
redirects and repository-prefixed assets. Evidence is retained at
`/tmp/geosolve-m76-pages-verify.ijOz7p`. The unchanged M72 browser check passes at both desktop
sizes; M76-adapted retained M74 and M75 checks pass at both sizes and 6/6 respectively after only
adapting their obsolete Origin-canvas expectations to the approved axis-intersection contract. Pages
is final public-byte authority; its repository-prefixed rebuild is not claimed byte-identical to
the separately frozen Tailscale output.

The intermediate `9b4e7f7` candidate, snapshot `/tmp/geosolve-m76-uat.ctgYzp` and aggregate
`337b0e6a2ce2b6a9aed979d0a4849e2d0887c092df66efa345d4917929d01dd4` are retained as superseded
historical evidence; PID `1455071` was retired before the final snapshot took the shared endpoint.

The initial nomination at source `37eade50b566f62905a395655bc80c17d9b6bef4`, tree
`d6ad2f453d672accbcc3848a1a16d2039b3511d1`, snapshot `/tmp/geosolve-m76-uat.puiPgO` and aggregate
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf` is retained as superseded
historical evidence only; its PID `1077092` was retired.

## Non-goals

M76 adds no constraint, residual, geometry primitive, solver behavior, mobile layout, formula
language or canonical sketch persistence field. Exact GitHub Pages publication followed the
scoped human approval of the final frozen Tailscale candidate and completed the milestone.
