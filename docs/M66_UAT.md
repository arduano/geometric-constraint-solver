<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 focused UAT: computed 2D Fillet features

Status: post-pivot implementation and mechanical qualification are Pass. Every human result is
Pending; do not use the archived `1034afc` build for this scorecard.

Candidate source: `941177c` (`Expose computed Fillets in the workbench`), with the prerequisite
feature/editor integration at `6e710e8`.

Tailscale endpoint: `http://100.94.63.83:8080/` (service restarted and HTTP verified on
2026-08-07).

Use the ordinary GeoSolve Sketch Workbench only. This scorecard validates the normal computed
Fillet route under ADR 0031. It does not ask the UI to create or edit advanced M28 solver-owned
associations.

## UAT scorecard

### M66-U1 — multi-corner authoring and shared radius

1. Draw a four-point open polyline with three spans.
2. Choose ordinary **Fillet**, select both interior corners and inspect preview before Apply.
3. Change the shared radius numerically and by dragging a preview arc/radius grip.
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

No post-pivot UAT finding is recorded yet. Add each objective finding with a stable `M66-PFxxx`
identifier, owning-layer regression and targeted human retest before approval.

The old `M66-F002` through `M66-F013` ledger belongs to the archived solver-owned UI architecture
at `origin/archive/m66-associative-fillet-2026-08-07` (`1034afc`). Those regressions remain useful
compatibility evidence, but their mechanically qualified disposition does not qualify this UAT.

## Approval

M66 closes only after:

1. one exact ADR 0031 candidate source and verified Tailscale endpoint are recorded above;
2. formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests,
   all-feature demo-web WASM, release Trunk and `git diff --check` pass on that source (satisfied);
3. every scorecard item is Pass or an explicitly accepted scoped limitation;
4. every post-pivot finding has a direct tested disposition and human retest; and
5. the supervising human explicitly approves M66.
