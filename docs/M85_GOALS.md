<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 — Responsive retained workbench presentation

Status: **active; M85-F001 is reproduced; implementation, qualification and human UAT are
pending**. Accepted M84 product source `84dd768`, immutable snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM` and Pages run `33068058169` remain product and public-byte
authority until M85 passes every gate below.

## Goal

Make ordinary canvas navigation, hover and direct manipulation feel immediate in both the
projectional/code workbench and the retained flat compatibility workbench, without changing any
geometry, constraint, solver, history, persistence or code-authoring semantics. Camera motion is a
presentation operation: it must reuse retained paint, coalesce raw input at animation-frame
boundaries and never rebuild the mathematical scene merely because the view changed.

## Confirmed baseline

M85-F001 is reproduced against the exact immutable M84-F012 candidate at `1440x900` in headless
Chromium. On the visible-annotation PC Water Manifold, a 30-sample middle-button pan took
`14,933 ms`, replaced viewport children 33 times and produced a `583.3 ms` p95 frame gap. An
18-event wheel burst took `9,976 ms`, replaced viewport children 19 times and also produced a
`583.2 ms` p95 frame gap. Hiding annotations did not remove the defect: the same pan took
`16,937 ms` with a `733.3 ms` p95 gap.

The native retained paths are not the primary cause. Current release measurements report the
96-point/95-curve M83 retained preview at `4.340 ms` p95 and exact terminal publication at
`66.619 ms`; relation-heavy curve control at `17.094 ms` p95 and `50.118 ms` terminal; Fillet
radius at `2.313 ms` p95 and `0.810 ms` terminal; and Offset distance at `5.743 ms` p95 and
`1.085 ms` terminal. Source inspection instead shows each raw camera event composing an entire
screen-space scene, laying out annotations, serializing full SVG and replacing `#wb-viewport`.

## Required architecture

### G1 — Camera-only admission boundary

- Maintain a desired camera independently from the exact retained scene and paint at most one
  newest-camera presentation per `requestAnimationFrame`.
- Fold every wheel delta in order and preserve its screen anchor. A pan may discard intermediate
  absolute samples, but its latest sample and exact final center must win.
- Update retained camera presentation, the adaptive grid and camera HUD without calling solver or
  projected-preview paths, materializing Intent, evaluating computed features, parsing/expanding
  managed code, encoding/saving a workspace, rebuilding durable panels, composing a full
  `ProjectionalEditorSession::scene`, serializing full SVG or replacing viewport `innerHTML`.
- A camera burst may retire an already active semantic gesture once at admission. That cancellation
  is separately audited; it cannot recur for every wheel or pan sample.
- Keep accepted/current identity, document bytes, Intent/code-session identity, history, selection,
  annotation layout and canonical persistence bytes unchanged.

### G2 — Exact presentation parity

- Retained camera paint must be semantically equivalent to a cold exact scene at the same camera:
  finite model-to-screen positions, pan direction, zoom anchor, visibility, ordering and stable DOM
  identities all agree.
- Screen-sized strokes, points, labels, hit envelopes, datum labels and annotation interaction
  remain usable across pan and zoom. Any lightweight normalization must stay within the camera
  work boundary rather than triggering a full scene rebuild.
- Pointer coordinates and hit testing use the current desired camera even when the retained paint
  originated at an earlier camera. The first hover, click or drag after navigation cannot target
  stale screen coordinates or cause a delayed visual jump.
- Fit, Origin, toolbar zoom, wheel zoom and middle-button pan converge to the same exact camera
  semantics in projectional and flat routes.

### G3 — Auditable work and timing

- Replace policy-only presentation counters with an actual work ledger that distinguishes camera
  presentation, scene composition, SVG serialization, viewport replacement, solver/preview,
  materialization, computed evaluation, code parse/expansion, persistence and panel work.
- Unit tests own deterministic coalescing, newest-sample, exact-final-camera and zero-forbidden-work
  invariants. They run in ordinary CI without a browser or wall-clock assumptions.
- A focused candidate-only Chromium trace owns real input/callback/RAF/paint/long-task timing. It is
  run against local/frozen Tailscale bytes, not restored as broad PR integration CI.

### G4 — Hover, drag and terminal responsiveness

- Preserve existing newest-sample RAF coalescing and synchronous exact terminal draining.
- Pointer preview frames never parse/expand code, save workspaces or rebuild durable panels.
- A mutating release publishes exactly one history/save/durable-render boundary; cancel and
  no-motion release publish none. No geometry may change after the terminal presentation.
- Profile and optimize only measured residual hot paths after the camera defect is removed; do not
  weaken solver validation or replace exact terminal publication with stale predictive state.

## Fixed qualification scenes and budgets

Use the ordinary authored rectangle-plus-diagonal scene, the exact `pc-water-manifold` M84 dogfood
with annotations visible, the same manifold with annotations hidden as a diagnostic only, and
Compass Rose plus Rounded Polyline for coupled code-owned dragging.

- Raw camera callback CPU p95: at most `1 ms`.
- Camera RAF callback CPU p95: at most `8 ms`; no more than one camera paint per animation frame.
- Ordinary input-to-next-paint p95: at most `16.7 ms` and at least 55 sustained frames/second.
- Visible-annotation manifold input-to-next-paint p95: at most `33.3 ms` and at least 30 sustained
  frames/second.
- No task over `50 ms` during a two-second pan or wheel burst, excluding sample load/cold startup.
- The newest admitted camera is visible on the next RAF and never later than two frames.
- Ordinary/Compass/Rounded-Polyline hover and drag browser preview p95: at most `33.3 ms`.
- Existing ordinary native retained-preview p95 gate remains at most `16 ms`.
- Exact ordinary release becomes visible and durable within `250 ms`; code-coupled release within
  `500 ms`, with no delayed post-release movement.

Collect at least three warmed bursts and 120 measured samples per scene/action class. Report p50,
p95, maximum, long-task count and actual admitted-work counts; do not hide a failed visible-
annotation result behind the annotations-hidden diagnostic.

## Bounds and non-goals

M85 adds no primitive, constraint, residual, Jacobian, solver priority, tolerance, branch rule,
Intent/code meaning, persistence schema or topological claim. It does not replace SVG with Canvas,
WebGL or a production renderer; introduce semantic approximation, geometry LOD or skipped terminal
validation; restore the retired broad browser E2E/CDP stack; or move camera/DOM policy into
`geosolve-core`, `geosolve-sketch`, `geosolve-linkage` or `geosolve-constraint-editor`.

## Release sequence

1. Freeze deterministic work-ledger and camera-parity regressions for both workbench routes.
2. Implement and qualify camera navigation, then re-profile and harden hover/drag/terminal paths.
3. Pass format, warnings-denied Clippy, workspace tests, relevant release performance tests, WASM
   build, clean golden checks and the complete clean release gate.
4. Freeze the exact no-rebuild candidate, serve only those bytes on Tailscale and run the focused
   Chromium timing trace plus the M85 human UAT scorecard.
5. Only after explicit supervising-user UAT approval publish the accepted descendant to GitHub
   Pages, exact-verify hosted bytes, retire the retained service and close M85.
