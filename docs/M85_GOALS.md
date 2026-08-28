<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 — Responsive retained workbench presentation

Status: **accepted at milestone level on 2026-08-28; M85-F001 through M85-F003 are repaired, the
exact final source passes the clean release gate, and its immutable local/Tailscale candidate passes
the frozen-byte browser profile plus native flat-adapter evidence**. The supervising user's close
decision accepts M85-U1 through M85-U12 without claiming a separately logged row-by-row replay.
Pages publication and final closure remain pending. Accepted M84 product source `84dd768`, immutable snapshot
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

## Current mechanically nominated candidate

Exact committed source `5c265e211e20dabc8a27f6402d530f5d645ff15c`, tree
`b55d012443f4dbf7551e30041da2912e666de9db`, implements retained camera/hover paths, actual
interaction/code receipts and the history-neutral unchanged-host code terminal. M85-F003 remains
repaired without changing any pre-M85 public incremental-code signature: the unaudited adapter
calls the receipt-aware worker directly, while six large optional projectional Fillet/Offset
preview and gesture states are privately heap-owned. `ProjectionalEditorSession` shrinks from
`35,488` to `15,296` bytes and its enclosing `MaterializedCodeProject` from `36,320` to `16,128`.

Implementation-checkpoint affected-crate evidence passes format and diff hygiene, the explicit 2 MiB
deletion pair (2/2 in approximately `2.04 s`), the Compass Rose exact terminal and adaptive-
polyline insertion regressions, constraint-editor (756 passed, 3 ignored), the complete sketch-code
crate, the ordinary-stack PC Water Manifold M85-F002 sentinel (`55.45 s`), demo-web (300/300 in
`98.61 s`) and warnings-denied all-target Clippy for constraint-editor, sketch-code and demo-web.

A five-test browser profile from an unpinned pre-F003 implementation ancestor did pass 5/5,
recording 1,200/1,200 camera-only admissions, zero forbidden admissions or navigation long tasks,
approximately `0.8 ms` worst completed-presentation RAF p95 and `59.6–61.3 fps` sustained
navigation. It is provisional directional evidence only, not final-source or frozen-byte evidence.

That historical profile is now superseded for nomination by exact frozen-byte evidence. The clean
gate ran from 06:40:49 through 07:13:16 AEST on 2026-08-28, exited `0`, and produced a 6,993-line,
456,580-byte log with SHA-256
`0b09720dfd4491575ab10bd3baba2f8f6e7fae9e8e90954ff0026a64de4458eb`. Its no-rebuild seven-file
output is frozen at `/tmp/geosolve-m85-uat.QX8fU3Q6`, with directories/files `0555`/`0444`, zero
symlinks and ordered-manifest aggregate
`dc729ce5fa28929dba2aa086e49246aac7d5173a0583d3b0b64748ffbbb7fda5`. Local and retained
Tailscale HTTP ledgers are byte-identical at SHA-256
`305eccfc8fa60786aabfae59edd612e695ce3c15b7224abbf3be0d0852ae0d27`.

The final frozen profile passes 5/5 in `2.2m`: 1,200/1,200 camera-only admissions, zero forbidden
admissions and zero navigation long tasks; worst callback/camera-RAF/completed-presentation p95 is
`0.2`/`0.6`/`0.8 ms`, worst frame-gap p95 is `16.8 ms`, minimum sustained rate is
`59.8249 fps`, and worst hover p95 is `3.5 ms`. Ordinary, Compass Rose and Rounded Polyline drag
preview/terminal results are respectively `5.4`/`87.23 ms`, `8.7`/`175.50 ms` and
`6.6`/`184.93 ms`, all with preview/terminal parity, no delayed motion and exactly one save per
mutating release. The captured profile log SHA-256 is
`d1174515c320e1f3a0e006bbaad47cc47ba0aeda52fe9bf05c956d91750951c7`; its summary SHA-256 is
`db508a28f46774fbc74f9bdebfc20c513c46934bfd29a7b0775d96962caa63e6`.

Final-source native M85-U10 evidence passes 19/19 exact tests for flat/shared camera, hover,
interaction, history/work-neutrality and v1-v6 normalization contracts. Evidence is
`/tmp/geosolve-m85-u10-final.D1auvz5d`; command/result/manifest SHA-256 values are
`5da8bf46936228d22034f4195e9e571f6f0227510db257f943ef27686dee6545`,
`5efaa7d874d1c07189ab8ec7398863945abd1d15bb50933e55f47bac888f7f79` and
`ec2b205710bf6d79c09e696fb6023b01ebcac1f922bcae04c2e8486c80702189`. Human UAT and approval now
pass at milestone level; only post-approval Pages publication, hosted-byte verification and service
retirement remain. `docs/M85_IMPLEMENTATION.md` owns the full evidence record.

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
