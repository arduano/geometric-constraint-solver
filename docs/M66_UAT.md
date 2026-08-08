<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 focused UAT: computed 2D Fillet features

Status: post-pivot implementation and mechanical qualification are Pass. Every human result is
Pending; do not use the archived `1034afc` build for this scorecard.

Candidate source: `ac31791` (`Keep Fillet preview drags out of support collection`), extending
editable-playground candidate `02649cc` and resolving `M66-PF004` after
`M66-PF001`/`M66-PF002`/`M66-PF003`.

Tailscale endpoint: `http://100.94.63.83:8080/` (release service live-rebuilt from `ac31791` and
HTTP verified on 2026-08-08; the served HTML contains the scoped non-draggable SVG marker).

Use the ordinary GeoSolve Sketch Workbench only. This scorecard validates the normal computed
Fillet route under ADR 0031. It does not ask the UI to create or edit advanced M28 solver-owned
associations.

Recommended starting point: open **Samples → Curves & constructions → 2D Fillet
playground**. Use the unlocked left polyline for U1/U2, the unlocked short-middle polyline for
conflict/recovery, and the fixed line-circle and line-quadratic-Bezier islands for family checks.
The upper-right three-line junction is intentionally ambiguous at its shared point; click two
branch interiors to choose a pair. For the independent upper-left lines, click each line interior
away from their exact intersection so manual intent is unambiguous.

## UAT scorecard

### M66-U1 — multi-corner authoring and shared radius

1. Draw a four-point open polyline with three spans.
2. Choose ordinary **Fillet**, select both interior corners and inspect preview before Apply.
3. Change the shared radius numerically and by dragging a preview arc/radius grip, including near
   each contact where the generated arc visually meets a parent span.
4. Apply with Enter. Repeat with reverse corner selection and with repeated corner/curve-pair
   clicks instead of preselection.
5. Select each generated arc and adjust the shared radius after publication.

Expected: both selected corners stay grouped and one Apply creates one feature set with two arcs
and one shared radius. The middle span is retained between independently trimmed Start and End
intervals. Reverse selection is visually equivalent. There is no final radius-confirmation canvas
click, Driving/Reference choice or radius dimension. Generated-arc/grip drag changes feature radius
without moving the underlying sketch or changing its DOF.

Result: Pending.

Notes:

### M66-U2 — sequential composition, delete and suppression

1. On a fresh four-point/three-span polyline, apply one Fillet to the first corner.
2. Start Fillet again and apply a separate set to the second corner.
3. Give both sets the same radius as U1 and compare the visible result.
4. Delete one generated arc/set, Undo/Redo, then suppress and unsuppress either set.
5. In a multi-corner set, delete one arc and then the final remaining arc.

Expected: the second Fillet is authorable even though it shares the middle source span. Opposite
endpoint claims compose, and equal sequential intent matches batch visible geometry. Separate sets
keep separate radii and identities. Deleting/suppressing one does not damage the other. Deleting a
multi-corner arc removes only that corner; deleting the final corner removes the set. History
restores stable feature/corner identities.

Result: Pending.

Notes:

### M66-U3 — source editability, invalid output and recovery

1. Author adjacent Fillets, then drag every original source-polyline point independently.
2. Move a source through a configuration where a Fillet is impossible, singular or exceeds the
   available middle interval.
3. Inspect the canvas, **Features** tree and Problems presentation; then move back to a valid
   configuration.
4. Delete one referenced source span and Undo the deletion.
5. Deliberately create overlapping, duplicate or consumed endpoint claims between sets, then
   reduce a radius or delete/suppress one participant to recover.

Expected: every valid sketch edit is accepted even if computed output becomes invalid. A failed set
shows no stale arc/ghost, does not lock any source point and exposes feature/corner/source-
attributed errors where safe. Unrelated valid sets remain visible. Source motion and Undo recover
the same feature intent. Claim conflicts fail all participants deterministically and recover without
silent branch changes.

Result: Pending.

Notes:

### M66-U4 — feature interaction and sketch-boundary trust

1. Select generated arcs, their radius grips and source spans/points in crowded geometry.
2. Try to use a generated arc as an operand for several sketch constraints.
3. Change a set radius repeatedly while watching sketch diagnostics, rank and DOF.
4. Exercise an affine/affine and affine/non-affine corner, then try two non-affine sources.
5. Open any profile/fill presentation while computed Fillets are active.

Expected: generated output selects stable feature/corner provenance, while native source geometry
keeps normal authoring and drag priority. Computed arcs are not constraint operands. Radius edits do
not alter sketch residual/rank/DOF evidence. Supported corners retain explicit intended branches;
two non-affine parents fail with a clear typed limitation without changing the sketch. A base-only
profile/fill is withheld with “computed geometry not yet included” rather than presenting a
misleading result.

Result: Pending.

Notes:

### M66-U5 — persistence, stale work and compatibility

1. Create several auto-labelled FilletSets, suppress some of them, then Undo/Redo and reload the
   workspace.
2. Continue editing source points and feature radii after reload.
3. Confirm the ordinary Fillet UI does not expose an Offset placeholder, Bake/Explode,
   computed-on-computed selection, legacy harness or `/#/dev/lab`.

Direct evidence already passed for the non-interactive boundaries: cancellation, deterministic
work exhaustion and stale sketch/feature/policy results cannot publish; workspace v1-v3 migrates
to an empty feature sidecar; a legacy M28 associative Fillet retains its existing meaning; and a
real encode/decode/fresh-process restore after Undo plus a cancelled preview preserves all live
allocator high-water without reusing feature, corner, sketch-revision or computed-edge identities.

Expected: workspace v4 preserves feature intent and stable IDs, regenerates fresh output IDs and
continues editing normally. Older workspaces receive an empty feature sidecar; existing M28 Fillets
retain their old meaning and are not migrated. Stale/cancelled/exhausted output never replaces the
current snapshot. Ordinary UI Fillet creates computed features only and leaves the advanced
M27/M28/M58 compatibility surface intact.

Result: Pending.

Notes:

## Finding ledger

### M66-PF001 — second Fillet line appeared unselectable

Observed: in ordinary Fillet mode, the first line entered pending state but clicking a distinct
second line could appear to do nothing.

Root cause: the workbench's blank optional radius value was translated to `None`, which erased the
headless collector's initialized `0.1 * model_scale` radius before the second pick attempted to
resolve a corner. The recovery audit also found that point corners could be flattened across a
pending support, overlapping hits were not resolved through one bounded domain-aware transaction,
and option/radius refresh rejection could advance state or retain a non-current preview.

Disposition on `b53a451`: absence of an explicit radius now preserves the initialized/remembered
value; point corners are atomic semantic targets; native hits are bounded and deterministic;
high-valence ambiguity cannot fall through to an arbitrary line; and picks/options commit only with
a freshly `Current` coordinator-held preview. Refresh and Apply defensively reject failed feature
evaluations. The direct Rust suites `m66_feature_authoring.rs` (14 tests) and
`m66_feature_authoring_matrix.rs` (15 tests) cover both pick orders, exact screen picks, shared
endpoints, overlap/crowding, stale and rejected retry paths, preview lifetime, transactional option
changes and full sequential adjacent-set publication with Undo/Redo.

Status: mechanically resolved; focused human retest Pending. Recheck ordinary line-line selection,
then M66-U1 and M66-U2 sequential/multi-corner authoring.

### M66-PF002 — curves and generated Fillets appeared visibly faceted

Observed: native curves, construction previews and especially small generated Fillet arcs used too
few visible subdivisions for comfortable close inspection.

Root cause: the workbench requested a relatively loose 0.8 px chord tolerance. Native curve
tessellation also relied only on midpoint-to-chord deviation, so an inflected cubic whose parameter
midpoint landed on its endpoint chord could collapse to one rendered and pickable segment even when
its quarter points departed materially from that chord.

Disposition on `a34d137`: both workbench scene branches use one 0.25 px policy; every non-linear
native/source-fragment span receives eight seed segments before bounded adaptive refinement;
generated Fillet arcs retain at least eight segments; and advanced drafting previews use 64 samples
per semantic span. Straight spans remain two-point polylines. Direct headless regressions prove the
analytic quarter point of the midpoint-aliasing cubic remains pickable and a small computed arc
meets the intended baseline between vertices.

Status: mechanically resolved; focused human visual retest Pending. Inspect ordinary Bézier/conic/
NURBS curves and small/large Fillet previews at several zoom levels while continuing M66 UAT.

### M66-PF003 — repetitive Fillet setup and canvas gestures obstructed focused UAT

Observed: the former workshop already provided basic line-line, line-circle and line-Bezier
references, but its only polyline was one fixed corner; it did not colocate editable
batch/sequential, high-valence ambiguity and short-middle claim-conflict cases. Canvas gestures
could also trigger native browser text selection or element dragging around the page.

Disposition on `02649cc`: the stable sample key now opens the ordinary editable **2D Fillet
playground** described above. Fixed reference islands and unlocked polylines remain normal save-like
geometry with no guide or alternate coordinator. Only the SVG canvas boundary suppresses
`selectstart`/`dragstart`, native user selection and element dragging; the Fillet radius input,
sidebar and other HTML remain selectable/editable. Direct Rust tests cover the real
screen/coordinator fixture transactions and focused presentation scoping. No browser E2E claim is
made. The full native/Clippy/workspace/WASM/release gate and 73/73 demo-web tests pass.

Status: mechanically resolved; focused human retest Pending. Exercise each playground region,
confirm canvas drags do not select page text, and confirm text plus the Fillet radius input still
work normally outside the SVG.

### M66-PF004 — preview-radius drag could consume a parent and strand authoring

Observed: after two valid line selections produced a Fillet preview, pressing or beginning to drag
the preview arc near a contact could deselect one parent and clear the preview. The consumed parent
then became the first support of an unintended new corner; selecting it again reported duplicate
support, so cancellation appeared to be the only recovery.

Root cause: active Fillet pointer-down always ran the native authoring collector first. The
generated arc's transparent painted hit stroke and its native parent intentionally overlap near a
Fillet contact, so the native parent won semantic collection before the old radius fallback could
run. Existing radius tests entered below this web/coordinator arbitration seam and did not exercise
the overlap.

Disposition on `ac31791`: the painted stable `FeatureCorner` now routes through one coordinator-
owned pointer transaction before native collection. Painted identity is only a hint: the exact held
candidate and current accepted/computed scene must match, and the headless editor independently
hits that owner's generated curve. Stale/foreign owners and a second live radius press reject
state-neutrally; the original gesture remains usable. Shift/Control/Command cannot toggle the
explicit radius owner away, while ordinary selection keeps its prior modifier semantics. A direct
screen/coordinator regression constructs a point where both arc and parent are in tolerance and
covers pointer-down, move, release, invalid-owner rejection, second-pointer survival and modified
press behavior. The full formatting, warnings-denied workspace Clippy/test, WASM and release Trunk
gate passes.

Status: mechanically resolved; focused human retest Pending. In the playground, select both
parents of a corner, then drag the preview arc at its midpoint and near both contacts. Confirm the
preview never turns back into a one-line pending selection and that radius dragging continues
normally after a rerender.

The old `M66-F002` through `M66-F013` ledger belongs to the archived solver-owned UI architecture
at `origin/archive/m66-associative-fillet-2026-08-07` (`1034afc`). Those regressions remain useful
compatibility evidence, but their mechanically qualified disposition does not qualify this UAT.

## Approval

M66 closes only after:

1. one exact ADR 0031 candidate source and verified Tailscale endpoint are recorded above;
2. formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests,
   all-feature demo-web WASM, release Trunk and `git diff --check` pass on that source (satisfied);
3. every scorecard item is Pass or an explicitly accepted scoped limitation;
4. every post-pivot finding (`M66-PF001` through `M66-PF004`) has a direct tested disposition and
   human retest; and
5. the supervising human explicitly approves M66.
