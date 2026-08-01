# M66 focused UAT: CAD helper operations

Status: mechanically qualified replacement; supervising-human UAT is pending.

Candidate code source: `77eda3ec5f7fc49b69eaf6a70124b9596f4ab796`. First replacement
`92e6ddce1e37d6508b5dd8568078146ac2822aa7` and initial source
`f913fb46e14308dc66563d1e602d3ae6ed2f7cb1` are superseded.

Tailscale endpoint: `http://100.94.63.83:8080/`.

Replacement mechanical qualification completed on 2026-08-01. The exact candidate above passed
formatting, warnings-denied locked workspace/all-target/all-feature Clippy, locked all-feature
workspace tests, the all-feature demo-web WASM check, release Trunk build and `git diff --check`.
Every human result below remains Pending.

Use the ordinary GeoSolve Sketch Workbench only. The sample leaves are editable save-like
workspaces; they contain no guided actions or protected state. This scorecard judges whether the
headless Fillet, Line offset and Mirror workflows are understandable, predictable and truthful
after their objective native/WASM qualification passes.

## UAT scorecard

### M66-U1 — local associative fillet authoring

1. Open **Curves & constructions → 2D fillet workshop**.
2. Choose **Modify → Fillet**, then pick the line-line pair near the portions you want to keep.
3. After the second parent, move the pointer to place the radius, click to confirm it, and inspect
   the accepted scratch preview before Apply. Try the first-side, second-side and alternate-arc
   controls. Deliberately click directly on the preview arc as well as empty canvas; preview-only
   geometry must not be consumed as a third operand or expose a hidden parent behind it.
4. Apply the default Reference candidate. Drag its visible arc body and its center; the radius
   measurement, contacts and parent trim endpoints should move together. Delete the reference
   dimension and repeat the arc drag—the association must remain intact and mobile.
5. Repeat with an explicitly Driving radius, then with the line-circle and line-Bezier pairs. For
   the grounded line-circle pair, also edit its source-circle radius. To drag a locked line/Bezier
   support, delete one of its ordinary Fixed constraints first, then Undo and Redo.
6. Draw or use an open polyline. First click its two adjacent spans, then repeat by selecting only
   their unambiguous interior corner. Both paths should resolve the same spans in visible order.
   Try an endpoint, ambiguous/singular input and one staged candidate cancelled with Escape.

Expected: the preview follows the locally picked retained portions, uses a sensible minor-arc
default and never changes the live document before Apply. Radius placement is a visible headless
stage: its fallback is not silently persisted as a hard equation, Reference is flexible and only
an explicit Driving choice locks the radius. Branch correction remains explicit rather than
caused by moving seed geometry. Apply creates an ordinary associative fillet with visible parent
trims. A free arc drag routes to its stored center and updates radius, contacts and trims through
the normal projected-drag lifecycle; deleting only the dimension neither explodes nor immobilizes
the association. The polyline-corner shortcut owns exactly the ordered adjacent `End`/`Start`
spans. The workshop's support locks are ordinary visible, deletable constraints, not protected
scenario state. Unsupported or unresolved local roots show a typed warning and retain the prior
accepted scene. First Escape clears the candidate; a second Escape exits Fillet mode.

Result: Pending.

Notes:

### M66-U2 — exact and supporting line offsets

1. Open **Curves & constructions → Associative line offsets**.
2. Choose **Modify → Line offset**, select one line or polyline span, choose each side in turn and
   apply the default exact-translated preview.
3. Edit the resulting distance in the ordinary dimension inspector and move the source geometry.
4. Switch to **Supporting line** mode, create another offset and move a target endpoint along the
   supporting direction or change its length.
5. Start Line offset again and explicitly click two, then three, unique endpoint-connected spans in
   path order. Click empty canvas to finish collection/confirm the side, inspect the mitered joined
   preview and Apply it. Also confirm by clicking directly on the preview offset where it covers or
   approaches a source; the preview must not intercept the side-placement click.
6. Verify the joined result is one ordinary polyline with no offset dimension or association. Move
   a source afterward and confirm the one-shot output does not follow. Undo/Redo and save/reload it.
7. Try a duplicate, disconnected and over-budget path plus a circle/nonlinear curve; each must warn
   without recursive neighbor discovery, partial output or mutation.

Expected: the pointer side and numeric distance are clear before Apply. Exact mode preserves one
same-oriented translated segment; supporting mode truthfully permits axial slide and length
freedom while retaining its explicit side-qualified positive distance. Those single-span results
retain an ordinary editable driving dimension, and guidance states that source edits propagate. A
multi-span path has separate bounded one-shot semantics: only explicitly clicked unique connected
spans participate, interior vertices are
supporting-line miters, and Apply creates one plain non-associative polyline with no persistent
distance source; guidance states that source edits will not propagate. Persistent associative
multi-span offset remains deferred. Preview-only foreground geometry blocks click-through without
becoming an operand. Unsupported, disconnected, duplicate, over-budget or unresolved input warns
without approximation or allocation.

Result: Pending.

Notes:

### M66-U3 — repeated exact mirror authoring

1. Open **Curves & constructions → Mirror construction workshop**.
2. Choose **Modify → Mirror**, pick the line source and then the line axis; inspect and Apply the
   preview.
3. Keep Mirror active and repeat for the cubic Bezier and non-rational B-spline sources.
4. Drag several original control points and verify their mirrored counterparts remain associated.
5. Draw a circle in this same workspace and try it as an unsupported source; optionally repeat with
   a conic or NURBS, then cancel a partially collected mirror.

Expected: source-then-axis progression is obvious, repeated mode re-arms after each terminal
attempt and every preview is exact. Supported mirrored controls follow subsequent source edits.
Unsupported families show a typed warning without tessellation, approximation or partial
geometry. Apply selects the primary created curve and each operation is one Undo step.

Result: Pending.

Notes:

### M66-U4 — shared interaction, persistence and trust

1. Start each Modify tool both from compatible preselection and from no selection.
2. While a candidate is pending, use wheel zoom, middle-button pan, Fit and ordinary canvas
   inspection; then Apply with Enter and cancel with Escape.
3. Create at least one flexible fillet, single-span associative offset, one-shot joined offset and
   mirror, save/reload the ordinary workspace and continue editing their ordinary retained output.
4. Deliberately stage an invalid candidate, then return to a valid candidate without reloading.
5. Acquire several curves near, rather than exactly on, their strokes. The shared curve acquisition
   radius is inclusive at 12 screen pixels; the nearest curve wins and exact ties are stable.

Expected: preselection and mode-first authoring lead to the same operation. Camera navigation does
not add operands or discard pending state. Only an independently accepted scratch result is shown
as a solved preview, and Apply publishes exactly that result through normal history. Invalid,
stale or rejected work never changes accepted geometry. Workspace reload preserves the ordinary
editable operation output; no sample-specific guided harness, protected mode, old harness or
`/#/dev/lab` appears. Headless per-tool authoring guidance remains available as intended.

Result: Pending.

Notes:

## Finding ledger

| Finding | UAT observation | Required disposition | Status |
| --- | --- | --- | --- |
| `M66-F001` | Line offset could not collect connected spans into one useful joined result; a subsequent click was consumed as side confirmation. | Add a max-32 explicitly clicked, unique, endpoint-connected ordered-chain request that emits one atomic one-shot mitered polyline with no dimension/association or automatic discovery. Directly test valid expansion, invalid retention, Undo/Redo and persistence; recheck M66-U2. | Implemented and directly qualified; focused M66-U2 retest pending. |
| `M66-F002` | Selecting an open-polyline corner did not author a fillet across its two adjacent spans. | Resolve one unambiguous interior point headlessly to its ordered adjacent spans and explicit `End`/`Start` trim ownership. Directly test accepted M28 geometry plus endpoint/ambiguous rejection; recheck M66-U1. | Implemented and directly qualified; focused M66-U1 retest pending. |
| `M66-F003` | Fillet creation silently used a driving fallback radius, and deleting that dimension still left the visible arc body without a drag route. | Add pointer radius placement with Reference default and explicit Driving intent; expose CircularArc center drag metadata; prove mathematical DOF, association retention, projected drag/rejection and exact preview publication; recheck M66-U1/U4. | Implemented and directly qualified; focused M66-U1/U4 retest pending. |
| `M66-F004` | Accepted scratch preview geometry intercepted later Fillet radius and Line offset side-placement clicks, so the preview could be treated as another operand instead of completing the stage. | Resolve the best visible hit once; if its identity exists only in preview, block click-through to hidden source geometry but forward no live operand so the headless placement stage owns the click. Directly regress preview/source overlaps and recheck M66-U1/U2/U4. | Implemented and directly qualified; focused M66-U1/U2/U4 retest pending. |
| `M66-F005` | Curve picking during repeated helper authoring was too narrow and made otherwise clear selections unnecessarily difficult. | Use the editor's shared nearest-curve acquisition with an inclusive 12-pixel radius and stable persistent-identity ties; qualify the exact inside/boundary/outside cases and recheck all operation tools. | Implemented and directly qualified; focused operation-tool retest pending. |
| `M66-F006` | The Line offset tool did not make it sufficiently clear that adding a second connected span changes from an associative result to one-shot geometry. | Keep one-span exact/supporting offsets associative, keep two-or-more-span joined offsets explicitly one-shot, state propagation semantics in headless guidance and defer persistent associative multi-span offsets. Recheck M66-U2/U4. | Implemented and directly qualified; focused M66-U2/U4 retest pending. |

Every objective defect requires a direct owning-layer regression before targeted human recheck. A
replacement candidate must be rebuilt and fully qualified even when one remaining change is
presentation-only.

## Approval

M66 closes only after:

1. one exact candidate commit and Tailscale endpoint are recorded above;
2. formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests,
   all-feature demo-web WASM, release Trunk and `git diff --check` pass on that source state;
3. every scorecard item is Pass or an explicitly accepted scoped limitation;
4. every finding has a tested disposition; and
5. the supervising human explicitly approves M66.
