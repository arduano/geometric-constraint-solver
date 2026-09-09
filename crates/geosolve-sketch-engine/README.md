<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# `GeoSolve` sketch engine

Headless evaluation of managed code projects and independently admitted generator
records. Both use the existing equation-free authoring lowering, native geometry,
explicit computed-feature authoring and independent hard residual validation.
The engine has no browser, renderer, filesystem or TypeScript execution dependency.

`SketchEngine::evaluate_generated_json` admits an evaluated record; `evaluate_managed_json`
admits the existing authenticated editable project. Both return immutable accepted
model-space data. Failure preserves `last_accepted`. Cloned `AcceptedEvaluation`
handles own the exact native authority and remain exportable after later evaluations.
A generator cannot obtain managed source-edit tokens from this API.

Profile export uses production topology and bounded model-space sampling. Current
computed line/circle/arc boundaries are projected into a private native query
document with authenticated endpoint joins. Independent acceptance must preserve
every projected coordinate and scalar exactly. Unsupported or incomplete geometry
fails export while the original accepted evaluation remains intact.
Mixed computed export supports lines, polyline spans, circles and circular arcs.
Native joins come from complete-span endpoint ownership or exact computed fillet
contact boundaries. Pre-trimmed native contacts at interior parameters can fail
closed when combined with computed features; no coordinate-based joining is used.

`export_profiles_for_output` selects complete regions containing at least one
outer-boundary fragment owned by the named output and preserves their holes. An
individual edge can select its whole containing region. Outputs use JSON Pointer paths in generator
mode. The export contains bounded arrangement faces: nested circles yield an
annulus and its interior disk. Selecting the outer circle yields only the annulus;
an output owning both circles may select both faces. No material/cut interpretation
is inferred from those boundaries.

The `profile_export` capability means the operation is available, not that any
geometry or tolerance is exportable. Evaluation currently uses a 1 mm model scale.
The native topology query can reject much larger geometry when it cannot certify
area at that scale; export preserves that diagnostic instead of changing the
geometry or relaxing validation.

`compile_project_json` assembles the existing validated offline managed project
from complete compiler authority, local files, artifacts and pins. Local module
paths may use ordinary `.ts`/`.js` and ESM/CommonJS extensions, must remain
relative and canonical, and cannot contain traversal or absolute-path components.

`geometry` exposes exact native curve definitions and their current visible
intervals plus current computed fragments/arcs in model coordinates.
`named_geometry` maps each output to native point/span IDs and explicit computed
edge ordinals. Generated metadata preserves names, groups, applications and
original parameter units. Standalone engine evaluation is read-only in both modes.

`EditableSession::open` accepts a complete managed project and optional
`EditableDesign`. `apply_project`, `apply_overlay`, `undo` and `redo` require the
exact current `CodeSessionIdentity`, including its session and revision. They use
the existing `SketchCodeSession` transactions and shared native materialization.
Failures preserve accepted geometry, semantic state and history; old accepted
results remain exportable. History checkpoints reference private immutable native
authorities. They cannot be imported as external state.

`design()` exports `geosolve-design-v1`: project identity, keyed reconciliation and
authored semantic overrides. Reopening reconstructs and independently validates
source plus those overrides without serialized solved geometry or hidden history.
Complete source/dependency replacements are the initial editing interface; this API
does not manufacture lexical edit tokens or expose a canvas interaction session.
