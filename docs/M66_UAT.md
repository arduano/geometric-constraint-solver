# M66 focused UAT: polished associative 2D Fillet authoring

Status: `M66-F012` replacement qualification is pending. Every human result remains Pending.

Candidate source: pending replacement qualification after `M66-F012`.

Tailscale endpoint: `http://100.94.63.83:8080/` (release service restarted and HTTP 200 verified
2026-08-02).

Use the ordinary GeoSolve Sketch Workbench only. The **2D fillet workshop** is an editable
save-like workspace with no guided actions or protected state. This scorecard judges whether one
headless Fillet workflow is exceptionally predictable, recoverable and truthful after its direct
native/WASM qualification passes.

## UAT scorecard

### M66-U1 — parents, corner shortcut and branch intent

1. Open **Curves & constructions → 2D fillet workshop**.
2. Choose **Modify → Fillet**, then pick the line-line pair near the portions you want to keep.
3. After the second parent, move the pointer to place the radius. Inspect the scratch preview and
   exercise first-side, second-side and alternate-arc controls before Apply.
4. Repeat with line-circle and line-Bezier pairs. Also try a pair containing two non-affine
   parents, such as circle-Bezier.
5. Draw or use an open polyline. First choose its two adjacent spans, then repeat by choosing only
   their unambiguous interior corner. Try an endpoint, unrelated same-support pair and ambiguous or
   singular input.

Expected: the preview follows the locally picked retained portions, uses a sensible minor-arc
default and never changes live design before Apply. Branch corrections are explicit rather than
accidental consequences of seed coordinates. Direct-span and corner-shortcut paths resolve the
same ordered parents with `End`/`Start` trim ownership. The current authoring abstraction accepts
affine/affine and affine/non-affine pairs; two non-affine parents give a clear typed unsupported
warning until pairwise continuation exists. That scoped authoring warning does not imply that the
underlying M28 generic Fillet API was removed. Unsupported or unresolved geometry warns without
partial mutation or global search.

Result: Pending.

Notes:

### M66-U2 — placement controls, acquisition and invalid-hover recovery

1. Start Fillet from compatible preselection and again from no selection.
2. With both parents selected, move across valid and invalid radius positions repeatedly. Return
   to a valid position without reselecting either parent.
3. Approach several parent curves from increasing distances and verify hover matches click. Check
   the exact 12-pixel acquisition boundary if practical.
4. Deliberately move and click across an accepted preview arc where it covers or closely approaches
   a source curve.
5. Open Fillet controls with the palette scrolled, then resize the window and place the trigger
   near every canvas edge.

Expected: an invalid unconfirmed hover clears only transient candidate/preview feedback; both
parents remain selected and radius placement recovers immediately on later valid motion. Hover and
click use the same nearest-geometry result and inclusive tolerance. Preview-only foreground
geometry blocks hidden-source click-through but never becomes a hover or third operand. Controls
remain legible, clickable and viewport-clamped over the canvas; palette overflow never clips them.

Result: Pending.

Notes:

### M66-U3 — Reference mobility and certified contact branches

1. Place and Apply the default Reference Fillet. Confirm Fillet mode exits automatically and
   ordinary Select becomes active.
2. Drag its visible arc body and center through small and large valid motions. Observe radius,
   contacts and both parent trim endpoints.
3. Delete the Reference radius dimension and repeat those drags. For a line-line/polyline Fillet,
   verify that both affine contacts can use their full interiors. For line-circle and line-Bezier,
   move substantially within the selected branch and verify that neither curved contact jumps
   across its nearest tangent-parallel barrier to a remote root.
4. Repeat with an explicitly Driving radius and compare its truthful remaining mobility.
5. Edit one source point successfully, then immediately start or continue Fillet authoring without
   refreshing the workspace.

Expected: Reference is flexible and only explicit Driving intent locks radius. Successful Apply
returns to Select, and arc-body/center drag works immediately with the Reference dimension still
present. Drag keeps the Fillet association, contacts and trims coherent. Deleting the dimension
neither explodes nor immobilizes the association. Affine pairs are not caged by a seed-centred
parameter window. A line/curved pair remains inside a strict outward-rounded `Local` cell over the
full bounded support or one explicit period, stopping conservatively at a real tangent-parallel
branch barrier rather than jumping to a remote root. A successful point edit does not falsely trigger
“helper operations require current accepted geometry.” Genuinely rejected or stale state still
cannot preview or publish.

Result: Pending.

Notes:

### M66-U4 — publication, persistence and trust

1. While a candidate is pending, use wheel zoom, middle-button pan, Fit and ordinary canvas
   inspection; then Apply with Enter and cancel with Escape.
2. Before Apply, confirm first Escape clears a candidate and second Escape exits Fillet mode.
3. Create both a free and Driving Fillet, Undo/Redo each, save/reload the ordinary workspace and
   continue editing the retained output.
4. Deliberately stage invalid, rejected and stale candidates and verify the last independently
   accepted live scene never changes.

Expected: only an independently accepted scratch result is shown as solved preview, and Apply
publishes exactly that proposal through one normal history step. Navigation neither adds operands
nor discards pending parents. Reload preserves ordinary editable Fillets and explicit branch
intent. No M66 Offset/Mirror tool or sample, guided harness, protected mode, legacy harness or
`/#/dev/lab` appears.

Result: Pending.

Notes:

## Finding ledger

| Finding | UAT observation | Required disposition | Status |
| --- | --- | --- | --- |
| `M66-F002` | Selecting an open-polyline corner did not author a Fillet across its two adjacent spans. | Resolve one unambiguous interior point headlessly to ordered adjacent spans and explicit `End`/`Start` trim ownership; directly test accepted M28 geometry and endpoint/ambiguous rejection. | Implemented and mechanically qualified; U1 human retest Pending. |
| `M66-F003` | Fillet creation used a hidden Driving fallback and deleting that dimension left the visible arc without a drag route. | Use pointer radius placement with Reference default and explicit Driving intent; expose semantic arc-center drag and prove retained association/mobility. | Implemented and mechanically qualified, including a multi-sample pointer gesture; U3 human retest Pending. |
| `M66-F004` | Scratch preview geometry could intercept a later Fillet placement click. | Preview-only foreground geometry blocks hidden sources but is neither hoverable nor forwarded as a live operand. | Implemented and mechanically qualified; U2 human retest Pending. |
| `M66-F005` | Curve picking was too narrow for predictable authoring. | Use one headless nearest-curve acquisition with an inclusive 12-pixel boundary and stable persistent-identity ties. | Implemented and mechanically qualified with F009 parity; U2 human retest Pending. |
| `M66-F007` | Fillet configuration controls are broken/clipped when rendered beside the scrolling tool selector. | Render a viewport-clamped canvas overlay outside palette overflow; directly test edges, resize, scroll and palette disclosure reflow. | Implemented and mechanically qualified; U2 human retest Pending. |
| `M66-F008` | An invalid exploratory Fillet hover exits placement and forces both parents to be selected again. | Clear only the unconfirmed candidate/preview and retain both parents plus radius-placement mode; keep confirmed terminal failure semantics separate. | Implemented and mechanically qualified; U2 human retest Pending. |
| `M66-F009` | Click acquisition became wider but CSS stroke hover still used the painted line, so visible hover and actual click disagreed. | Route operation hover and click through the same preview-aware headless hit test, including exact 12-pixel boundary and preview barriers. | Implemented and mechanically qualified; U2 human retest Pending. |
| `M66-F010` | A successful point drag left accepted geometry usable but literal input equality falsely disabled helper authoring because the one-shot candidate request was no longer retained. | Add sketch-owned current-publication compatibility that ignores only `candidate_request`; retain all other input/attempt identity checks and exact proposal CAS. | Implemented and mechanically qualified; U3 human retest Pending. |
| `M66-F011` | After deleting the radius dimension, a free Fillet still had an arbitrary apparent minimum/maximum size. | Persist full `Interior` support for affine pairs. For exactly one non-affine parent, certify a strict curved `Local` cell over the full bounded support or one explicit unwrapped period using outward-rounded tangent/line cross-product intervals; never cross a tangent-parallel barrier. Keep two-non-affine authoring typed unsupported until pairwise continuation, without narrowing M28. | Implemented and mechanically qualified with exact hostile-root and mobility regressions; U3 human retest Pending. |
| `M66-F012` | After Apply, the new Reference Fillet could not be resized; deleting its displayed dimension did not help. | After successful publication, use one tested host completion handoff to exit the headless Fillet collector and explicitly restore ordinary Select. Directly prove default Reference publication immediately routes arc-body drag to the semantic center and accepts radius changes both before and after dimension deletion. Keep failed Apply attempts recoverable. | Implemented; replacement mechanical qualification and U3 human retest Pending. |

Every objective defect requires a direct owning-layer regression before targeted human recheck. A
replacement candidate must be rebuilt and fully qualified even when a remaining change is
presentation-only.

## Archived three-tool history

The superseded, unapproved Fillet/Offset/Mirror candidate is preserved at
`origin/archive/m66-three-helper-tools-2026-08-02`, commit `80d4939`. Its former Offset/Mirror UAT
sections and Offset findings `M66-F001`/`M66-F006` are archived history, not active checks. Removing
their M66 authoring/UI/samples and M66-only offset requests does not remove the completed M25 signed
Offset constraints or M58 exact Mirror operation-companion API.

## Approval

M66 closes only after:

1. one exact post-pivot candidate source and Tailscale endpoint are recorded above;
2. formatting, warnings-denied locked workspace Clippy, locked all-feature workspace tests,
   all-feature demo-web WASM, release Trunk and `git diff --check` pass on that source;
3. every active scorecard item is Pass or an explicitly accepted scoped limitation;
4. every active finding has a tested disposition; and
5. the supervising human explicitly approves M66.
