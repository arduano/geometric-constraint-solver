<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 — CAD geometry tool families and authoring variants

Status: **active (2026-08-17); contract approved, implementation, qualification, UAT,
publication and closeout remain open**. M78 turns the demo's flat collection of geometry buttons
into a polished CAD-style family palette while keeping recipe meaning, inference, branch state and
atomic publication in reusable headless Rust.

## Product contract

M78 exposes exactly nine geometry families and 25 authoring variants. A family button activates
its last-used session variant and opens one persistent bottom-left overlay; selecting another
variant changes the exact active recipe without creating geometry.

| Family | Variants | Authoring intent |
| --- | --- | --- |
| Point | Sketch Point | Place or reuse one persistent point. |
| Lines | Segment; Polyline; Midpoint Line | Two-endpoint segment; open/closed connected chain; centre-to-end symmetric segment with an intrinsic midpoint. |
| Rectangles | 2-Point Aligned; 3-Point Corner; Center Rectangle; 3-Point Center Rectangle | Axis-aligned diagonal; oriented corner/baseline/height; aligned centre/corner; oriented centre/half-width/half-height. |
| Circles | Center–Radius; 2-Point Diameter; 3-Point Circle | Centre and rim; diameter endpoints; three non-collinear rim samples. |
| Arcs | Center Arc; 3-Point Arc; Tangent Arc | Centre/Start/End; Start/Through/End; outgoing arc from an eligible native open-curve endpoint. |
| Ellipses | Center–Axes Ellipse; Axis-Endpoints Ellipse; Center–Axes Elliptical Arc; Axis-Endpoints Elliptical Arc | Centre- or major-axis-endpoint construction, with full-ellipse or explicit Start/End trim output. |
| Béziers | Quadratic; Cubic | Start/control/End and Start/control-1/control-2/End. |
| Conics | Rational Quadratic; Parabola; Hyperbola | Existing ordinary-middle rational, vertex/focus parabola and centre/transverse-axis hyperbola recipes with their typed numeric options. |
| Splines | Open Control NURBS; Periodic Control NURBS | Variable-length control-point construction with explicit open or periodic topology. |

Family keys are `point`, `lines`, `rectangles`, `circles`, `arcs`, `ellipses`, `beziers`,
`conics` and `splines`. Variant keys are, in the family order above:
`sketch-point`; `segment`, `polyline`, `midpoint-line`; `two-point-aligned-rectangle`,
`three-point-corner-rectangle`, `center-rectangle`, `three-point-center-rectangle`;
`center-radius-circle`, `two-point-diameter-circle`, `three-point-circle`; `center-arc`,
`three-point-arc`, `tangent-arc`; `center-axes-ellipse`, `axis-endpoints-ellipse`,
`center-axes-elliptical-arc`, `axis-endpoints-elliptical-arc`; `quadratic-bezier`,
`cubic-bezier`; `rational-quadratic-conic`, `parabola`, `hyperbola`; and
`open-control-nurbs`, `periodic-control-nurbs`.

Variant keys and family membership are stable public metadata. `EditorTool` remains a coarse
compatibility projection for existing hosts; it is not expanded into 25 unrelated legacy cases.
New hosts activate and inspect the exact variant through `GeometryToolFamily` and
`GeometryToolVariant`.

## Rectangle and line recipes

Every rectangle creates four explicit line curves sharing four ordinary corner point identities.
No variant silently adds a lock, driving/reference dimension or target scalar. Aligned variants
commit the ordinary Horizontal/Vertical intent needed to remain aligned. Three-point variants
commit the ordinary perpendicular/parallel intent needed to remain rectangular. Center variants
also create one visible Construction helper diagonal through the chosen centre and commit one
ordinary Midpoint relation; that helper participates in the normal role, selection and lifecycle
rules rather than becoming hidden state.

Holding Shift during any rectangle recipe regularizes the live preview to a square and commits one
ordinary adjacent-edge `EqualLength` relation with the rectangle's intrinsic relations. The square
remains square after release, Undo/Redo and reload. Shift is recipe intent, not an ambient inference
candidate, so Shift+Ctrl/Cmd still produces the intrinsic square while suppressing unrelated
automatic snapping.

Midpoint Line retains the selected centre as an ordinary persistent point operand, reflects the
sampled endpoint to create the other endpoint and commits a `Midpoint` relation against the created
segment. Segment and each live Polyline edge continue to use ordinary positional and directional
inference. Clicking a Polyline's first persistent vertex closes it without allocating a duplicate
point; Enter or double-click finishes it open.

## Circle, arc and ellipse recipes

Center–Radius and Center Arc retain the current spatial centre/rim and centre/Start/End language.
Two-point diameter derives its centre and radius from the two diameter samples. Three-point circle
and 3-Point Arc derive one finite circumcircle from three scale-aware non-collinear samples and
reject coincident or near-collinear terminal input without replacing the last valid draft. The
3-Point Arc retains the ordered Start/Through/End span explicitly. `F` flips the complementary
Center Arc sweep before commit; the chosen sweep is durable branch state.

Diameter and three-point rim samples are coordinates, not promises to create synthetic sketch
points. When such a sample resolves to an existing persistent point, the atomic recipe adds an
ordinary `PointOnCurve` relation from that point to the newly created curve. Otherwise it creates
no rim point. The same rule applies to spatial arc trim samples. Intrinsic recipe relations are
lowered before ambient inference, so a recipe never depends on accidental point allocation or
source order.

Tangent Arc accepts only a certified endpoint of a native open curve with a finite nonzero endpoint
jet, followed by an End sample. The headless recipe constructs the unique outgoing tangent circle,
using the source tangent and chord rather than a browser heuristic, and commits generic curve
tangency with explicit contact, tangent orientation, source endpoint neighbourhood and arc sweep.
Zero chord, a zero-speed jet, the tangent-line/infinite-radius limit, non-finite radius and vanishing
sweep reject locally and retain correction-ready draft state. Interior, periodic and computed-only
contacts are unavailable rather than approximated.

Ellipse variants differ only in how their centre and principal-axis frame are established. The
centre forms collect Centre then Major axis endpoint; axis-endpoint forms collect Major endpoint 1
then Major endpoint 2 and derive their centre. Existing validated minor-axis ratio/options remain
explicit during construction. Elliptical-arc forms then collect spatial Start and End samples,
inverse-project them through the headless support ellipse and retain an explicit sweep. `F` flips
the complementary elliptical-arc sweep. The browser never reconstructs an ellipse equation,
projection or branch from SVG geometry.

## Variable-length and advanced recipes

Quadratic/cubic Bézier, rational quadratic, parabola and hyperbola retain their established point
meaning, typed scalar options and explicit branch/domain state while moving under coherent family
overlays. The rational middle click continues to mean ordinary Euclidean `P1` for nonzero weight
and explicit projective `Qh` at zero weight, as established by M77.

Open and Periodic Control NURBS share one variable-length stage model. Enter or double-click
finishes when the exact current degree/topology options make the draft finishable; Backspace or
Undo removes the latest unfinished control before touching accepted history. Periodic construction
is explicit topology, not an implicit close-by-proximity rule. Invalid degree, knot, weight or
control-count input remains family-local and cannot block another tool.

## Shared authoring interaction

The exact variant publishes a semantic stage, completed/required progress, finishability, explicit
branch choice and typed live measurements such as width/height, radius/diameter, sweep or control
count. Ordinal-only prompts and web-owned `positions.len()` logic are not authoritative.

- Ctrl on Windows/Linux or Cmd on macOS suppresses ambient inference for the current sample. It
  does not disable intrinsic recipe relations or Shift regularization.
- Shift regularizes all four rectangle variants to persistent squares. It has no undocumented
  alternate meaning for other M78 recipes.
- Tab cycles the bounded, ranked ambiguous inference candidates returned for the current stage.
- `F` flips only the complementary Center Arc or Elliptical Arc sweep and records the result as
  explicit draft branch state.
- Backspace, or Undo while a draft has unfinished stages, removes only the latest draft stage.
  Accepted document history is reached only when no unfinished stage remains.
- Escape cancels the current shape and stays in the active variant. A second Escape with no draft
  activates Select. Closing a family overlay also activates and focuses Select.
- Enter or double-click finishes a finishable Polyline or NURBS draft. Clicking the first Polyline
  vertex closes the chain without duplicating it.
- Every geometry variant remains active after successful creation so repeated authoring is direct.

The active Profile/Construction choice applies to main geometry created by every recipe. A centre-
rectangle helper is always Construction. Tool and family overlays persist through blur, canvas
clicks, pan and zoom, remember their session-local variant/options and close only on explicit close,
Escape-to-Select or a tool/family switch.

## Atomic construction and recovery

Every interactive recipe, including geometry-only construction, lowers to one authenticated
`CommitConstructionPlan`. A plan contains typed prospective geometry, point/contact operands,
per-curve roles, explicit branch choices and ordered relation definitions with provenance:
`RecipeIntrinsic`, `RecipeRegularization` or `AutoInference`.

Recipe-intrinsic and regularization relations are applied before compatible ambient inference.
Conflicting or already-implied ambient candidates yield to the recipe instead of making a valid
shape fail. Every accepted recipe solves once, independently validates finite geometry and hard
residuals, publishes once through exact retained compare-and-swap and creates exactly one history
entry. A rejection preserves the complete document, accepted scene, history, persistent allocator,
preview and terminal draft. The visible problem is attached to the active draft/field; it does not
become a stale global error after correction, cancellation or Undo.

## Qualification and closeout

Focused editor/coordinator tests own the exact 25-variant catalog, stage progression, modifier
semantics, typed proposals, relation provenance, branch retention, point/contact identity,
redundancy precedence, invalid-terminal correction, one-step history and native/WASM parity. Thin
demo tests own family overlay persistence, event/modifier mapping, accessible labels and rendering
of published previews. Any broad golden authoring/scene expansion must represent a reviewed
systemic family or lifecycle dimension; isolated defects stay in focused owning-layer regressions.

Formatting, warnings-denied Clippy/Rustdoc, locked all-feature workspace tests, relevant native and
WASM parity, golden survey/check/require-clean, demo WASM and Trunk release assembly must pass
before an exact no-rebuild artifact is frozen and byte-verified over the retained Tailscale
endpoint. `docs/M78_UAT.md` remains open until explicit supervising-human approval. GitHub Pages
publication and hosted-byte verification happen only after that approval and are required before
M78 closes.

## Explicit deferrals

M78 does not add two-/three-tangent or tangent-tangent-radius circles, interior/periodic Tangent
Arc, curve/curve intersection Point inference, fit-point splines, polygons or slots. It does not
duplicate Center–Radius with separate radius/diameter buttons; one live `R`/`Ø` readout is enough.
It adds no new solver residual, curve family, canonical persistence version, browser-owned
geometry, mobile layout, B-rep feature or hidden construction point.
