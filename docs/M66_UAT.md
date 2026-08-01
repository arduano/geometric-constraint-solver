# M66 focused UAT: CAD helper operations

Status: mechanically qualified; supervising-human UAT is pending.

Candidate code source: `f913fb46e14308dc66563d1e602d3ae6ed2f7cb1`.

Tailscale endpoint: `http://100.94.63.83:8080/`.

Mechanical qualification completed on 2026-08-01. The exact candidate passed formatting,
warnings-denied locked workspace/all-target/all-feature Clippy, locked all-feature workspace
tests, the all-feature demo-web WASM check, release Trunk build and `git diff --check`. Every result
below remains Pending until supervising-human UAT is performed.

Use the ordinary GeoSolve Sketch Workbench only. The sample leaves are editable save-like
workspaces; they contain no guided actions or protected state. This scorecard judges whether the
headless Fillet, Line offset and Mirror workflows are understandable, predictable and truthful
after their objective native/WASM qualification passes.

## UAT scorecard

### M66-U1 — local associative fillet authoring

1. Open **Curves & constructions → 2D fillet workshop**.
2. Choose **Modify → Fillet**, then pick the line-line pair near the portions you want to keep.
3. Inspect the accepted scratch preview before applying it. Change the radius and try the
   first-side, second-side and alternate-arc controls.
4. Apply the candidate and edit the resulting radius dimension. For the grounded line-circle pair,
   also edit its source-circle radius. To drag a locked line/Bezier support, delete one of its
   ordinary Fixed constraints first, then Undo and Redo.
5. Repeat for the line-circle and line-Bezier pairs. Also try an obviously ambiguous or singular
   pick and cancel one staged candidate with Escape.

Expected: the preview follows the locally picked retained portions, uses a sensible minor-arc
default and never changes the live document before Apply. Branch correction is explicit rather
than caused by moving seed geometry. Apply creates an ordinary associative fillet with visible
parent trims and one editable driving radius; parent edits remain associative. The workshop's
support locks are ordinary visible, deletable constraints used to make radius editing numerically
stable, not protected scenario state. Unsupported or unresolved local roots show a
typed warning and retain the prior accepted scene. First Escape clears the candidate; a second
Escape exits Fillet mode.

Result: Pending.

Notes:

### M66-U2 — exact and supporting line offsets

1. Open **Curves & constructions → Associative line offsets**.
2. Choose **Modify → Line offset**, select a line or polyline span, choose each side in turn and
   apply the default exact-translated preview.
3. Edit the resulting distance in the ordinary dimension inspector and move the source geometry.
4. Switch to **Supporting line** mode, create another offset and move a target endpoint along the
   supporting direction or change its length.
5. Draw a circle or nonlinear curve in this same workspace and attempt the tool on it, then
   Undo/Redo both valid offset operations.

Expected: the pointer side and numeric distance are clear before Apply. Exact mode preserves one
same-oriented translated segment; supporting mode truthfully permits axial slide and length
freedom while retaining its explicit side-qualified positive distance. The created dimension
remains an ordinary editable driving dimension. Unsupported source families warn without
approximation or allocation.

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
3. Create at least one fillet, offset and mirror, save/reload the ordinary workspace and continue
   editing the created geometry, constraints and dimensions.
4. Deliberately stage an invalid candidate, then return to a valid candidate without reloading.

Expected: preselection and mode-first authoring lead to the same operation. Camera navigation does
not add operands or discard pending state. Only an independently accepted scratch result is shown
as a solved preview, and Apply publishes exactly that result through normal history. Invalid,
stale or rejected work never changes accepted geometry. Workspace reload preserves the ordinary
editable operation output; no sample-specific guided harness, protected mode, old harness or
`/#/dev/lab` appears. Headless per-tool authoring guidance remains available as intended.

Result: Pending.

Notes:

## Finding ledger

No M66 finding has been recorded yet. Assign identifiers `M66-F001`, `M66-F002`, and so on in
discovery order. Objective defects require a direct owning-layer regression before a targeted
human recheck; a presentation-only refinement still requires a rebuilt candidate and affected
retest.

## Approval

M66 closes only after:

1. one exact candidate commit and Tailscale endpoint are recorded above;
2. formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests,
   all-feature demo-web WASM, release Trunk and `git diff --check` pass on that source state;
3. every scorecard item is Pass or an explicitly accepted scoped limitation;
4. every finding has a tested disposition; and
5. the supervising human explicitly approves M66.
