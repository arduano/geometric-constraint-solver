<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 — focused dimensions and stable annotation placement

Implementation authorized on 2026-09-08 from the approved plan. M96 remains the
accepted product. The workbench should convey relevant measurements without
filling general navigation with dimension widgets.

## Approved interaction

- Canvas control: Dimensions **Focused / All / Hidden**; Focused is the default.
- Without selection, Focused shows only pins. A selected object reveals related
  measurements, with no more than six ordinary canvas callouts in total.
- A 250 ms stationary geometry hover previews one related measurement and retains
  the geometry-to-label transit corridor. Pan, zoom and drawing suppress hover previews.
- Inspector Dimensions lists every related measurement, even if not drawn. Rows
  show names, values, units and truthful editing authority; focusing a row reveals
  its callout and supported edits use the existing owner transaction.
- At most four pins survive selection changes and reload, outside design history.
  Individual unpin and Clear pins actions remain available.
- Priority: actively inspected/edited item, pins, authored related dimensions,
  then generated dimensions. Stable source order breaks ties; groups retain the cap.
- Patch public dimensional parameters precede a collapsed Generated dimensions
  disclosure. A 12 mm channel parameter must not be presented as a 6 mm offset.
- Fixed-size readable text and subdued lines retain existing selection and
  reference notation. All is deliberate full inspection; Hidden preserves only
  necessary active authoring feedback.

## Ownership and stability

Rust owns accepted dimension metadata, relevance, visibility, placement and picking.
One resolved decision feeds numeric drawing, SVG and pointer behavior. Hidden
dimensions neither reserve layout space nor intercept clicks. Relevance uses
accepted direct operands, curve endpoints and authenticated producer ownership;
it does not traverse the entire constraint graph.

Automatic positions are cached separately from manual placements and retained
across pan/zoom, selection-only rebuilds, Fit, centering and resize. Geometry edits
invalidate affected positions. Manual positions have priority. Automatic displacement
is bounded to 96 CSS pixels; a measurement without a readable slot remains in the
Inspector. Navigation freezes membership and slots; lower-priority overlaps are
suppressed after navigation, with 6 px separation and 12 px restoration clearance.
Zoom alone does not promote additional measurements.

Mode and pins are presentation preferences. Older workspaces default to Focused
while preserving manual placements; stale identities must never retarget another
dimension. Workbench exports follow the chosen policy; standalone native exports
retain their default unless explicitly configured. Solver behavior is unchanged.

## Reproduced report and qualification

At accepted source `41ad6c7a89470bef22a8635fc43c73dae5ae4db2`, the frozen M96
manifold has 82 dimension texts. Chromium at 1440×900, four wheel samples of -90
followed by an empty-canvas click relocates 45 labels, with a maximum 628 px jump.
Wheel reprojection itself retains slots; the subsequent cold scene rebuild selects
different automatic slots. This is M97-F001, a presentation-independent layout
retention defect, with a native regression required before repair.

Focused native tests must cover all existing dimension families, bounded visibility,
complete metadata, hidden picking, label transit, manual placement and source/history
preservation. Bridge/frontend coverage includes mode, pins, patch parameters,
native and source editing, groups, failed drafts and reload. Capture simple/manifold/
dense screenshots and compare navigation performance. Run the integrated clean-source
gate once at nomination, authenticating reuse under RELEASE_QUALIFICATION.md; review
any golden changes individually. Final supervising-user acceptance remains pending.
