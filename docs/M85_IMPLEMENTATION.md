<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 implementation ledger — Responsive retained workbench presentation

Status: **active; diagnosis complete; implementation and qualification pending**. No M85 product
candidate, clean gate, frozen artifact, Tailscale nomination, human acceptance or Pages publication
is claimed. `docs/M85_GOALS.md` owns the contract.

## Baseline and finding

### M85-F001 — Camera events rebuild the complete canvas

Disposition: **confirmed DEFECT; repair pending**.

Reproduction authority is exact M84-F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`,
tree `429ed56d2a5b3988d6604079d19e1002f9049d64` and immutable snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM`. Open the PC Water Manifold with annotations visible, then
middle-button pan or issue a wheel burst. A focused ephemeral Chromium harness records:

| Operation | Raw samples | Elapsed | Viewport child replacements | p95 frame gap |
|---|---:|---:|---:|---:|
| Pan, annotations visible | 30 | 14,933 ms | 33 | 583.3 ms |
| Wheel, annotations visible | 18 | 9,976 ms | 19 | 583.2 ms |
| Pan, annotations hidden diagnostic | 30 | 16,937 ms | 33 | 733.3 ms |

The first owner is `geosolve-demo-web` presentation. Projectional camera callbacks call
`render_projectional_canvas`; that rebuilds `ProjectionalEditorSession::scene`, screen-space curve
tessellation, computed/annotation presentation, full SVG text and `#wb-viewport.innerHTML` for
each raw event. The flat compatibility callbacks perform the same class of full transient/durable
canvas reconstruction. This is not a solver equation or accepted-scene defect, and it does not
warrant golden-authoring expansion.

## Baseline commands actually run

```bash
cargo run --locked --release -p geosolve-constraint-editor --example m83_performance
cargo test --locked --release -p geosolve-constraint-editor \
  --test m83_interaction_performance -- --ignored --nocapture --test-threads=1
```

Both exit 0. Results are: retained preview `4.340 ms` p95 and terminal `66.619 ms`;
relation-heavy curve control `17.094 ms` p95 and terminal `50.118 ms`; Fillet radius `2.313 ms`
p95 and terminal `0.810 ms`; Offset distance `5.743 ms` p95 and terminal `1.085 ms`.

The focused browser measurement is an ephemeral diagnosis harness over the exact frozen bytes. It
is intentionally not checked into the repository or added to broad PR CI.

## Planned implementation slices

### I1 — Actual work ledger and queues

- [ ] Add deterministic actual-work counters for camera frames, scene composition, SVG
  serialization, viewport replacement, solver/preview, materialization, computed evaluation,
  code parse/expansion, persistence and durable panels.
- [ ] Add independent camera RAF queues for projectional and flat routes. Prove one scheduled RAF,
  newest desired state, stale-generation rejection and exact ordered wheel folding.
- [ ] Retain the existing semantic pointer queues and terminal draining; do not merge camera and
  semantic gesture authority.

Proposed owner tests (not yet claimed as existing commands):
`camera_burst_coalesces_to_latest_single_frame`,
`camera_frames_admit_only_camera_presentation_work`, and
`camera_terminal_does_not_save_or_rebuild_panels`.

### I2 — Retained camera presentation

- [ ] Introduce a browser-owned retained camera layer that can pan/zoom accepted SVG and lightweight
  grid/HUD presentation without rebuilding scene DTOs or replacing viewport children.
- [ ] Preserve exact screen mapping, accumulated wheel anchors, point/line stroke and hit sizes,
  datum/annotation presentation, stable DOM identity and post-navigation hit testing.
- [ ] Route wheel, pan, Fit, Origin and toolbar zoom through the same camera authority in both
  workbench routes.
- [ ] Add semantic comparisons between retained camera output and a cold exact canvas across fixed
  pan/zoom states.

### I3 — Pointer-frame follow-up

- [ ] Re-profile hover and every drag family after camera repair using ordinary, Compass Rose and
  Rounded Polyline fixtures.
- [ ] Remove only measured repeated scene/markup/code work from transient frames; keep exact native
  preview and independent hard-residual validation unchanged.
- [ ] Prove exactly one durable publication on mutating release, none on cancel/no motion and no
  delayed post-release geometry change.

### I4 — Qualification and release

- [ ] Pass every timing and admitted-work budget in `docs/M85_GOALS.md` and `ACCEPTANCE.md`.
- [ ] Run format, warnings-denied Clippy, workspace tests, relevant release performance tests,
  actual WASM/Trunk, clean golden checks and the complete clean release gate.
- [ ] Freeze without rebuild, exact-verify local and retained Tailscale bytes, run focused Chromium
  timing and complete human UAT.
- [ ] Publish and exact-verify GitHub Pages only after explicit UAT approval; then retire the
  retained candidate service and close M85.

## Semantic-preservation ledger

Every implementation slice must demonstrate that camera-only input leaves accepted/current
identity, retained document, Intent graph, code session, history, selection, annotation layout and
canonical persistence bytes unchanged. M85 changes no native equation, residual, Jacobian,
priority, branch, tolerance, persistence wire or managed-code meaning. A fast path that bypasses
current-camera hit testing, independent terminal validation or transactional rejection fails even
if it meets timing budgets.
