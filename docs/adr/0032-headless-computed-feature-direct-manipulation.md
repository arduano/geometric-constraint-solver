<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0032: Headless computed-feature direct manipulation

Status: accepted and completed in M68; implementation, focused direct/release qualification and
explicit supervising-human UAT approval are complete as of 2026-08-09

## Context

ADR 0031 makes the ordinary workbench Fillet a computed feature. It persists explicit corner
intent outside the sketch constraint graph, evaluates revision-local output from one exact
accepted sketch snapshot and keeps the native sketch editable. M66 also established grouped
authoring, numeric radius editing and generated-arc radius dragging.

The accepted limitation `M66-KL001` is not a Fillet-construction or solver-validity defect. The
old radius gesture freezes the previous arc centre and maps the next pointer position by its
distance from that centre. Evaluation moves the centre and contacts as radius changes, so the
gesture can drift, invert or approach a different local root. Completed authoring corners also
carry absolute retained-side, normal-side, winding, neighbourhood, endpoint-order and sweep
intent, but some refresh paths reconstruct them from the original picks and relative option
toggles. Near a branch fold that can silently select a different valid root.

Interaction responsibility is also divided incorrectly. Semantic radius refresh and rollback
partly live in `geosolve-demo-web`; post-publication corner replacement is not exposed through one
coordinator transaction; and pointer release can observe a requested value after the last
independently current preview has been lost. Painted SVG identity is therefore doing more work
than ADR 0029's headless-editor boundary permits.

## Decision

### Preserve absolute branch intent

M68 extends ADR 0031 without changing its persistence schema. Once a Fillet corner has been
accepted, subsequent radius continuation begins from that exact absolute
`NewComputedFilletCorner` intent. It preserves both parent normal sides, retained endpoints,
contact neighbourhoods and windings, output endpoint order, sweep and selected local root. The
relative `FeatureAuthoringOptions` booleans remain defaults for collecting a new corner; they are
not an authority for reconstructing a completed corner.

A same-branch continuation may move contacts and the generated centre only while all of that
absolute intent remains independently valid. At a fold, singular offset intersection, domain
boundary or loss of regularity, continuation stops on the current branch. The last exact
`Current` result remains the solid preview and a typed reason identifies the limit. Neither
pointer motion nor numeric radius entry may silently jump to another root. A different root,
retained direction or complementary arc is admitted only through an explicit, applicable branch
action.

An exact affine/non-affine fold may have no finite rail even though the persisted local branch
cell identifies how its coincident roots separate at a nearby radius. Explicit numeric entry may
depart such a fold only by validating that exact origin and resolving the target inside that
persisted seed-connected cell. This is not a draggable rail or an implicit branch choice: an
absent, tied or remote target still rejects.

Explicit contact movement may reseed one named parent contact. The source hit and intended parent
are validated against the current accepted scene. Candidate enumeration is bounded to the two
selected native parents and their persisted neighbourhoods; tied candidates produce typed
ambiguity instead of an arbitrary choice. M68 does not introduce global root enumeration.

### Use a frozen one-dimensional radius rail

For a regular selected corner at radius `r`, parent `i` contributes an offset point

```text
O_i(t_i, r) = p_i(t_i) + s_i r n_i(t_i),
```

where `s_i` is the persisted normal side. At the accepted intersection `O_1 = O_2`, differentiate
with respect to radius and solve

```text
[ O_1,t  -O_2,t ] [dt_1/dr, dt_2/dr]^T = s_2 n_2 - s_1 n_1.
```

The generated-centre sensitivity is then

```text
dC/dr = O_1,t (dt_1/dr) + s_1 n_1.
```

The implementation must independently form the corresponding second-parent expression and reject
non-finite, ill-conditioned or materially disagreeing results. Central finite differences over
same-branch continuation are the independent oracle for this analytic sensitivity.

Pointer-down freezes the accepted corner, exact scene stamps, start radius, pointer position and
model-space rail sensitivity. Pointer motion maps to radius by

```text
dr = dot(pointer_delta, dC/dr) / |dC/dr|^2.
```

Motion perpendicular to the rail is therefore a no-op. The rail has no arbitrary screen-space or
model-space clamp: the only bounds are finite positive radius, the selected parents' domains and
independently valid same-branch geometry. A shared-radius FilletSet uses the selected corner's rail
as its grip while previewing every corner atomically; all generated arcs affected by that shared
radius are identified in the interaction DTO.

### Put the complete transaction in the headless coordinator

`geosolve-constraint-editor` owns a closed computed-Fillet interaction state machine covering
idle, radius drag, explicit named-parent contact editing and branch preview. Live state records the
pointer ID, stable feature/corner owner, complete origin configuration, exact
sketch/feature/evaluation stamps, frozen rail, and the token and sample for the last exact
`Current` preview.

Authoring and published-feature radius edits use the same transaction rule. A move may replace
only provisional preview state. Pointer-up or direct numeric editing may publish only the exact
last `Current` candidate with matching stamps; release while invalid, cancellation, a foreign or
second pointer, stale work, exhausted work or a camera change cannot publish a requested value.
If no current candidate exists, durable intent and history remain unchanged. A successful radius,
contact, retention or alternative action atomically updates radius and any re-anchored absolute
corner intent in one ordinary feature revision and one Undo step while preserving stable
feature/corner IDs.

The headless layer publishes model-space interaction DTOs for the radius grip, spoke and rail;
named-parent contact metadata; solid current retained-direction arrows; outlined applicable
alternatives; and dashed complementary/local branch previews. Stable action IDs carry labels,
applicability, disabled reasons, attribution and affected-corner identities. The same resolver and
priority order serve hover and click:

```text
explicit radius grip or generated arc > native support
```

Painted SVG identity remains only a hint. The coordinator must still validate exact owner,
provenance and model-space proximity, preserving the `M66-PF004` trust boundary.

A locally resolvable alternative is not sufficient authority to advertise an action. Before the
coordinator publishes a Fillet action, it replaces that corner in a cloned complete feature
document and requires the owning feature to evaluate `Current` under ordinary composition. An
alternative that conflicts with another corner's endpoint claim is omitted from both canvas and
accessible action surfaces; it is not presented as a control that can never commit.

Full-period periodic parents contribute contact, tangent, normal and continuation data to the
Fillet arc but do not contribute visual source-trim claims. Their native closed loop therefore
remains complete, and retained-direction reversal is not advertised because it has no visible
retention meaning. Bounded/open parents and explicitly open trim views of periodic supports retain
the existing source-fragment composition and retained-direction behavior. This policy is based on
visible source topology rather than curve-family names, so it applies equally to full circles,
ellipses and future full-period supports while preserving arcs and other open curves.

Where multiple transparent action corridors overlap, SVG paint order is not semantic priority.
The adapter may collect every exact-stamped painted action under the pointer, but only the unique
action independently selected as nearest by the headless model-space resolver may preview or
commit. A validated visible arrow outranks an overlapping Fillet radius surface; the central
radius grip retains priority where it visibly covers the arrow. Stale, foreign, disabled, far or
spoofed painted targets still fall through to ordinary editor interaction.

### Keep the web adapter thin

The sole workbench renders the headless central radius handle, arrows, rail and ghost alternatives
and exposes the same stable actions in a compact accessible panel. It owns layout, focus and event
translation, not branch applicability, hit resolution, radius mathematics or rollback.

The visible radius grip sits at the generated arc midpoint with a spoke and rail; dragging the arc
body is a convenience entry into the same headless gesture. Named contact state remains available
through the headless interface and internal continuation seam but has no endpoint circle, canvas
hit zone or compact-panel control. Retained-direction and branch choices use lightweight canvas
icons and arrows without circular handle-like backplates; they preview on hover/focus and commit
only through their headless action ID. The old raw Flip-first, Flip-second and Alternate-arc
checkboxes are removed from ordinary presentation.

The browser adapter captures the initiating pointer for point drag, Fillet direct manipulation and
canvas pan, and releases capture on completion or cancellation. A camera change during a live
Fillet manipulation first cancels and restores that gesture. Pan and zoom remain available while
Fillet authoring is only collecting or being inspected. These platform mechanics do not replace
the headless pointer-ID and exact-stamp checks.

Problem detail that appears during a gesture is an accessible live-region overlay inside the
position-stable canvas panel, not a workbench grid row. Showing or hiding it may not resize the
viewport, alter pointer-to-model mapping or intercept the gesture whose failure it reports.

### Scope boundary

M68 is a focused Fillet interaction and shared-canvas-foundation cut. It adds no Offset or Mirror
authoring, no two-non-affine-parent Fillet, no computed-on-computed chain, no Bake/Explode, no
profile or production-topology consumption, no cross-revision topological naming, no computed arc
as a sketch-constraint operand, no persistence/schema migration, no global root enumeration and no
browser E2E, mobile or legacy-UI claim.

## Verification

Direct `geosolve-sketch-features` tests are authoritative for same-branch continuation, analytic
rail sensitivity versus central finite differences, bounded alternatives, fold/singularity
rejection and atomic configuration replacement. They cover orthogonal, acute and reversed
line-line corners; line-circle roots, retained directions and folds; line-Bezier; transforms and
scales; forward/reverse motion; and grouped-radius conflicts.

Direct `geosolve-constraint-editor` tests are authoritative for the complete pointer
down/move/up/cancel matrices, radial and tangential motion, viewport scales, sampling invariance,
invalid-to-valid recovery, release while invalid, second pointers and modifiers, stale/foreign
owners, overlap priority, hover/click parity, authoring/published and numeric/drag parity, branch
and contact actions, one-step history, Undo/Redo and reload. A bounded deterministic transition
model must prove that no unaccepted preview can publish or survive cancellation.

Every feature edit regression records that native sketch identity, coordinates, independently
validated residuals, numerical rank and DOF remain unchanged. M66's `M66-PF001` through
`M66-PF004` regressions remain mandatory. Workbench tests stay thin: DTO rendering,
accessibility/action metadata, event translation, pointer capture/release, overlay layout and
browser-default suppression. No browser E2E suite is restored.

Mechanical qualification preceded a fresh human UAT over the release build served through
Tailscale. The supervising human explicitly approved that scorecard and closed M68 on 2026-08-09.

## Consequences

- Radius dragging follows a stable local degree of freedom instead of chasing a moving centre.
- Absolute persisted branch intent survives authoring refresh, published editing and history.
- Branch folds are visible limits, never opportunities for an automatic root switch.
- Direct manipulation, the accessible panel and numeric editing share one headless action and
  transaction authority.
- Invalid release cannot persist a value that was never represented by a current validated
  preview.
- The rail, action, transaction and revision-local-output seams can support future computed
  geometry, but M68 deliberately proves them only for Fillets.
