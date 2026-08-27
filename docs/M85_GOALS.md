<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 — Responsive retained workbench presentation

Status: **active and unaccepted; M85-F001 through M85-F003 are repaired at committed affected-
crate-qualified implementation checkpoint `fd2c560`, while the clean release gate, frozen-byte
browser profile, immutable nomination, human UAT and publication remain pending**. Accepted M84
product source `84dd768`, immutable snapshot
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
- After a completed burst, one authenticated exact-reprojection reconciliation may rebase the
  retained SVG to the desired camera. It is a separately counted boundary, never a raw camera RAF,
  and may not solve, materialize Intent, evaluate computed features, parse/expand code, publish
  history, persist or rebuild durable panels. It cannot recur per raw sample.
- Keep accepted/current identity, document bytes, Intent/code-session identity, history, selection,
  annotation layout and canonical persistence bytes unchanged.

### G2 — Exact presentation parity

- Retained camera paint must be semantically equivalent to a cold exact scene at the same camera:
  finite model-to-screen positions, pan direction, zoom anchor, visibility, ordering and stable
  semantic DOM IDs all agree. Retained nodes stay in place throughout a burst; the separately
  authenticated exact reconciliation may replace them once without changing their semantic IDs.
- Screen-sized strokes, points, labels, hit envelopes, datum labels and annotation interaction
  remain usable across pan and zoom. Any lightweight normalization must stay within the camera
  work boundary rather than triggering a full scene rebuild.
- Pointer coordinates and hit testing use the current desired camera even when the retained paint
  originated at an earlier camera. The first hover, click or drag after navigation cannot target
  stale screen coordinates or cause a delayed visual jump.
- Fit, Origin, toolbar zoom, wheel zoom and middle-button pan converge to the same exact camera
  semantics in projectional and flat routes.

### G3 — Auditable work and timing

- Replace policy-only presentation counters with actual owner receipts and one browser work ledger.
  Distinguish camera presentation, exact reprojection, retained hover, scene composition, SVG
  serialization, viewport replacement, native preview/solve attempts, Intent materialization,
  computed evaluation, native-history publication, managed-code parse, code expansion, outer-code
  publication, persistence and durable-panel work. Attempt counters survive rejection; an attached
  code project records no code work unless its owner issues a receipt.
- Unit tests own deterministic coalescing, newest-sample, exact-final-camera and zero-forbidden-work
  invariants. They run in ordinary CI without a browser or wall-clock assumptions.
- A focused candidate-only Chromium trace owns real input/callback/RAF/paint/long-task timing. It is
  run against local/frozen Tailscale bytes, not restored as broad PR integration CI.

### G4 — Hover, drag and terminal responsiveness

- Preserve existing newest-sample RAF coalescing and synchronous exact terminal draining.
- Pointer preview frames may cross only the native preview, Intent materialization and computed
  evaluation boundaries actually required by their owner. They never parse/expand or publish code,
  publish history, save workspaces or rebuild durable panels.
- A native mutating release publishes one native history transaction. A code-owned release may
  absorb that accepted native transaction into one outer code publication, but exposes only one
  user-visible Undo step, one save and one durable render. Cancel and no-motion release publish
  none. No geometry may change after the terminal presentation.
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

## Current implementation checkpoint

Committed source `fd2c560c5c61338a96f145ecb87106af49e93749`, tree
`97591f3d8c268e36db2e3e728c52163dca87d055`, implements retained camera/hover paths, actual
interaction/code receipts and the history-neutral unchanged-host code terminal. M85-F003 is
repaired without changing any pre-M85 public incremental-code signature: the unaudited adapter
calls the receipt-aware worker directly, while six large optional projectional Fillet/Offset
preview and gesture states are privately heap-owned. `ProjectionalEditorSession` shrinks from
`35,488` to `15,296` bytes and its enclosing `MaterializedCodeProject` from `36,320` to `16,128`.

Final-checkpoint affected-crate evidence passes format and diff hygiene, the explicit 2 MiB
deletion pair (2/2 in approximately `2.04 s`), the Compass Rose exact terminal and adaptive-
polyline insertion regressions, constraint-editor (756 passed, 3 ignored), the complete sketch-code
crate, the ordinary-stack PC Water Manifold M85-F002 sentinel (`55.45 s`), demo-web (300/300 in
`98.61 s`) and warnings-denied all-target Clippy for constraint-editor, sketch-code and demo-web.

A five-test browser profile from an unpinned pre-F003 implementation ancestor did pass 5/5,
recording 1,200/1,200 camera-only admissions, zero forbidden admissions or navigation long tasks,
approximately `0.8 ms` worst completed-presentation RAF p95 and `59.6–61.3 fps` sustained
navigation. It is provisional directional evidence only, not final-source or frozen-byte evidence.
The clean release gate, immutable freeze, exact local/Tailscale verification, frozen-byte browser
profile, UAT and Pages publication remain pending. `docs/M85_IMPLEMENTATION.md` owns exact commands
and outcomes.

## Release sequence

1. Freeze deterministic work-ledger and camera-parity regressions for both presentation adapters.
2. Implement and qualify camera navigation, then re-profile and harden hover/drag/terminal paths.
3. Pass format, warnings-denied Clippy, workspace tests, relevant release performance tests, WASM
   build, clean golden checks and the complete clean release gate.
4. Freeze the exact no-rebuild candidate, serve only those bytes on Tailscale, run the focused
   Chromium timing trace and complete the human scorecard plus native flat-adapter evidence. The
   flat retained-coordinator compatibility route has no ordinary persisted browser bootstrap.
5. Only after explicit supervising-user UAT approval publish the accepted descendant to GitHub
   Pages, exact-verify hosted bytes, retire the retained service and close M85.
