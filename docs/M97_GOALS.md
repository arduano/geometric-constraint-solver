<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 — focused dimensions and stable annotation placement

Implementation authorized on 2026-09-08 from the approved plan. M96 remains the
accepted product. The workbench should convey relevant measurements without
filling general navigation with dimension widgets.

The subsequent source-native authoring request has a completed
[design and implementation plan](M97_AUTHORING_METADATA.md). That amendment is
implemented locally and undergoing replacement qualification. The interactions below describe the current
qualified preview; the addendum replaces catalog priority ownership when implemented.

## Approved interaction

- Canvas control: Dimensions **Focused / All / Hidden**; Focused is the default.
- Focused keeps sample design-intent priorities eligible without selection.
  Gridfinity prioritizes all 20 authored measurements; the manifold prioritizes
  plate/reservoir sizes and representative outlet/fastener sizes. Other samples
  curate a small overview set. In the source-native amendment, unmarked source has no implicit
  overview measurements; All measurements provides discovery and explicit promotion. These defaults do not consume user pins.
- A selected object reveals related measurements, with no more than six ordinary
  contextual callouts plus the default priorities. Every callout still needs a
  readable retained slot; crowded priority measurements remain in the Inspector.
- A 250 ms stationary geometry hover previews one related measurement and retains
  the geometry-to-label transit corridor. Pan, zoom and drawing suppress hover previews.
- Inspector Dimensions lists every related measurement, even if not drawn. Rows
  show names, values, units and truthful editing authority; focusing a row reveals
  its callout and supported edits use the existing owner transaction.
- At most four pins survive selection changes and reload, outside design history.
  Individual unpin and Clear pins actions remain available.
- Priority: actively inspected/edited item, pins, hovered measurement, selected
  authored/generated measurements, then idle defaults. Stable source order breaks
  ties; explicit contextual interest remains reachable among default priorities.
- Patch public dimensional parameters precede a collapsed Generated dimensions
  disclosure. The manifold's full 12 mm channel width and 2.4 mm groove width are
  default Inspector parameters. A 12 mm
  channel parameter must not be presented as a 6 mm offset.
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
Zoom alone does not promote additional measurements. Explicit Fit and the first
measured host layout reconsider hidden callouts, reserving existing visible
positions before a bounded search for newly readable measurements.

In the current qualified preview, sample manifests own checked declaration/parameter selectors for design-intent
priorities. Rust resolves them through accepted declaration and control provenance,
never rendered labels. Default priorities stay in the Inspector even when occluded.

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

## Default-priority amendment

On 2026-09-08 the supervising user approved the general direction and requested
important shape-intent measurements by default, explicitly including every
Gridfinity measurement. This amendment changes presentation only. Qualification
of the prior preview remains historical evidence until the amended product is
qualified; M97 is open.

## Source-native authoring amendment — implemented, qualification underway

The supervising user requested native/intuitive authoring and an audit of adjacent
metadata that should belong in code. [M97_AUTHORING_METADATA.md](M97_AUTHORING_METADATA.md)
defines one source owner for dimension `isKeyConstraint`, named public parameters, display
labels/help and document title/description. Inspector actions rewrite these same
properties through authenticated source transactions, with Undo/Redo and restoration.
Gridfinity uses an explicit document dimension default; the manifold keeps its exact
selected dimensions and full channel/groove values. Shared parameters appear once;
generated offsets retain their owning patch and existing editing restrictions.

Live catalog priorities, titles and descriptions migrate into source. Runtime
selection by sample identity and the first-six heuristic retire. Existing groups
remain source-owned, with catalog projections derived from them. Catalog ordering,
categories, legal records and independent qualification expectations retain their
existing purpose. Personal mode/pins/visibility/camera remain outside design source.

This adds implementation and qualification obligations to the open M97 milestone;
it does not claim a replacement qualified product or supervising-user closure.
