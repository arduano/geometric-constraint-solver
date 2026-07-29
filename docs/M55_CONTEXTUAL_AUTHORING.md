# M55 contextual constraint authoring follow-up

Status: complete as of 2026-07-29 during the active M61 remediation.

## Objective

Replace the workbench's equation-shaped relation list with a small set of
selection-sensitive authoring intents. The headless editor, not the browser,
resolves each intent to one existing persistent sketch constraint definition.
The resolver uses only typed operand capabilities and explicit user choices; it
never chooses a discrete mathematical branch from the current coordinates.

This follow-up does not reopen the completed M55 parity gate. It preserves the
underlying M55 relations while improving their reusable authoring policy before
M61 UAT resumes.

## Contextual intent vocabulary

| Intent | Typed selection | Persistent definition |
| --- | --- | --- |
| Lock | point | `FixedPoint` |
| Coincident | point + point | `Coincident` |
| Coincident | point + curve | `PointOnCurve` |
| Coincident | curve + curve | `CurveCurveContact` |
| Horizontal | linear span | `Horizontal` |
| Vertical | linear span | `Vertical` |
| Parallel | two linear spans | `Parallel` |
| Perpendicular | two linear spans | `Perpendicular` |
| Perpendicular / Normal | linear span + circle or circular arc | `PointOnCurve` placing the circular centre on the line |
| Equal | two linear spans | `EqualLength` |
| Equal | two circles/arcs | `EqualRadius` |
| Equal | two other regular curves | branch-explicit `EqualCurvature` |
| Midpoint | point + linear span | `Midpoint` |
| Symmetric | point + point + linear span | `SymmetricAboutLine` |
| Tangent | two regular curves | branch-explicit `CurveCurveTangency` |
| Continuity | two curve endpoints | ordered G0/G1/G2 or rate-explicit parametric C2 `EndpointContinuity` |

The presentation label may become more specific after resolution—for example,
Coincident, Point on curve, or Curve contact—but the stable action identity is
the contextual intent.

Specialized line/circle, circle/circle and circle/arc tangent definitions remain
public domain constructors. The contextual editor initially uses the generic
curve-jet tangent definition so every supported curve family has one consistent
contact/branch workflow. Moving an individual family to a specialized definition
requires an equally explicit side/containment/center-direction control and direct
parity evidence; it must never be a coordinate-derived substitution.

## Explicit choices

Contact-bearing actions carry the complete semantic span, parameter domain and
value, winding, neighborhood and optional aligned/opposed tangent orientation.
Equal curvature carries signed, magnitude-same-sign or magnitude-opposite-sign
state. Continuity carries order and, for parametric C2, positive finite
first/second rates. A circular normal needs no left/right branch: centre-on-line
incidence makes the selected line radial, and its two circle intersections are
the two possible normal contact locations.

The resolver must not infer internal/external containment, side, orientation,
contact span, winding, neighborhood, curvature sign or continuity order from
geometry. Unsupported combinations return a typed disabled reason.

## Deliberate retained-lifecycle boundary

The separately persisted M36/M37 semantic catalog contains Concentric,
Collinear, point-pair Horizontal/Vertical, Point/Entity Symmetry, EqualDistance,
EqualAngle and BlockEntity relations. It is not part of
`RetainedSketchDocumentSession`, its undo/redo history, prepared-input stamp or
workspace envelope. Those relations must not be presented as ordinary retained
actions until M62 deliberately freezes their lifecycle and schema integration.

The public sketch domain retains `CurveDirection` for callers that deliberately
need a direction at an explicit curve contact. Compact authoring does not expose
it as Parallel, Tangent, Perpendicular, or Normal. On a full circle a free
direction contact can move to wherever the requested direction occurs, so that
relation does not establish either contact with the selected line or a radial
normal. Arbitrary curve-pair direction-only parallel/perpendicular therefore
remains outside this authoring vocabulary. A future general curve-pair angle
residual would require structured audit metadata, an independent
finite-difference Jacobian test and explicit contact/orientation regression
scenarios.

## Qualification

- Headless tests freeze the intent-to-definition matrix, typed disabled reasons,
  explicit branch choices and accepted/rejected lifecycle behavior.
- Workbench tests freeze the compact identity catalog, contextual labels and
  absence of the former Point-on-curve/Equal-length/Equal-radius/Generic-contact/
  Generic-tangency action identities.
- Reusable scenarios demonstrate point/curve contact, true curve/curve
  tangency, circular radial normal incidence, equal curvature and endpoint
  continuity.
- Format, warnings-denied workspace Clippy, complete locked workspace tests,
  all-feature demo-web WASM check and release Trunk build must pass.

No browser E2E, CDP, `/#/dev/lab`, legacy playground or mobile acceptance is
restored.
