<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M74 implementation — Production-style sketch reference UX

Status: **mechanically implemented; release nomination in progress as of 2026-08-16**. Focused and
workspace-wide development qualification passes on the shared working tree. The clean committed
release gate, immutable Tailscale candidate, human approval and final GitHub Pages deployment have
not yet been recorded.

Architecture decision: no new ADR is currently required. Intrinsic datums extend the ordinary
sketch/editor model within the retained-authoring and accepted-scene boundaries. Canonical sketch
v1-v4 remains frozen, and the browser continues to consume public scene/audit APIs.

## 1. Current files and APIs

### Sketch domain

- `crates/geosolve-sketch/src/document.rs` and `src/lib.rs` add and export
  `SketchDatum::{Origin, XAxis, YAxis}`.
- `crates/geosolve-sketch/src/document.rs` and `src/document_lowering.rs` add
  `CoincidentWithOrigin`, `PointOnDatumAxis` and `CollinearWithDatumAxis` definitions, validation,
  dependency/lifecycle handling and runtime lowering. Canonical-v4 encoding rejects these states
  with `DocumentError::UnsupportedM74State`; only unsupported draft-v5 side records represent them.
- `crates/geosolve-sketch/src/compiler.rs` lowers Origin and axis rows through public solver
  constraints with datum-specific source labels and audit bindings. Datum-line collinearity uses a
  datum-specific signed-angle/support residual plus an ordinary length-retention preference that
  selects a non-degenerate solution without adding a hard dimension.
- `crates/geosolve-sketch/tests/m74_reference_geometry.rs` is the focused domain suite
  for scale behavior, residuals, finite-difference Jacobians, audit descriptors, lifecycle and
  draft-v5 round trips.

### Headless editor and inference

- `SelectionItem::Datum`, `SceneDatum`, `EditorScene::datums` and
  `GeometryVisibility::reference_geometry` expose intrinsic references without document IDs. Scene
  DTOs clip the infinite semantic axes to the current finite viewport for presentation.
- Contextual authoring resolves Origin coincidence, point-on-axis and line-on-axis collinearity in
  either operand order. Parallel/Perpendicular with X/Y axes lower to ordinary Horizontal/Vertical.
- `DisabledReason::ProtectedDatum` owns datum mutation rejection. The coordinator
  guards deletion, suppression/reactivation, geometry-role conversion, Lock and drag startup; a
  datum drag is selection-only and creates no gesture, problem or history entry.
- Draft inference carries `CoincidentWithOrigin` and `PointOnDatumAxis` through candidate
  resolution and atomic construction-plan lowering. The policy adds Origin `6/9 px` and axis
  `4/7 px` hysteresis, native-before-datum priority, Origin-before-axis priority, reference
  visibility and Shift suppression, point-stage/circle exclusion, live-span same-axis suppression
  and orthogonal datum/direction bundles.

### Demo presentation

- The web workbench adds a Reference geometry tree group, protected datum
  inspector, viewport-clipped axis/Origin SVG presentation, related/hover/selection styling and
  independent References/Grid controls.
- The fixed CSS grid is replaced by a presentation-only Origin-aligned adaptive SVG grid using a
  `1–2–5` major-step sequence. Reference geometry paints before native geometry and remains outside
  Fit bounds.
- The workbench patch also adds Origin recentering, canonical empty-Fit reset, an inference-aware
  coordinate HUD, contextual cursor state, isolated Undo/Redo shortcuts and letterbox-aware pointer,
  double-click and wheel translation.

Picking and painting share `SceneDatum::is_visible_in_viewport`, so a datum just outside the mapped
plane cannot expose an invisible edge hit while an independently visible axis remains pickable when
Origin is off-screen. Pointer-leave clears and immediately rerenders the coordinate HUD even when
the headless editor has no hover effect, and middle-button press renders the grabbing cursor before
the first pan move.

## 2. Mathematical and lifecycle behavior

`Origin` is the immutable model-space point `[0, 0]`. `XAxis` has supporting-line coefficients
equivalent to `y = 0`; `YAxis` is `x = 0`. `CoincidentWithOrigin` therefore contributes two scalar
rows. `PointOnDatumAxis` contributes one normal-coordinate row. Datum-line collinearity contributes
one signed-angle row against the datum direction selected from the line's explicit retained branch
and one scaled support-through-Origin row. Those two hard rows establish direction and position
while preserving two geometric degrees of freedom. A same-source Preference row retains the
pre-authoring line length so an exactly perpendicular underdetermined seed cannot minimize point
motion by collapsing toward zero; it is not a dimension or hard relation. The analytic Jacobian
must match a central finite-difference oracle and every success-like solve must pass independent
finite residual validation.

Datums themselves never enter the document allocator, coordinate vector, persistent graph or
history. A relation that refers to a datum is ordinary design intent: it owns a constraint ID,
participates in dependency deletion and suppression, and may be removed without removing the
intrinsic datum. This distinction is also the interaction rule: selecting a datum is legal, but any
object mutation over a selection containing one datum rejects atomically.

Inference uses pixels rather than model distance so capture feel is zoom-independent. Origin uses
Euclidean `6 px` entry and `9 px` exit. Axes use perpendicular `4 px` entry and `7 px` exit. Native
geometry outranks datums, and Origin outranks either axis at the shared intersection. A durable
Horizontal live span already owns Y and suppresses X-axis inference; Vertical owns X and suppresses
Y-axis inference. The opposite-axis combination controls the other coordinate and remains a legal
two-relation candidate.

The adaptive grid and camera/HUD/cursor treatments are presentation state only. The grid has no
editor item, inference anchor, retained relation or persistence field. The HUD reports the same
adjusted coordinate returned by headless inference rather than recomputing a snap in the browser.

## 3. Qualification ledger

The current implementation has passed:

```text
cargo test --locked -p geosolve-sketch --test m74_reference_geometry --all-features
# 6 passed

cargo test --locked -p geosolve-constraint-editor --test m74_reference_geometry --all-features
# 3 passed natively

env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor --test m74_reference_geometry \
   --target wasm32-unknown-unknown'
# the same 3 passed under wasm-bindgen-test-runner

cargo test --locked -p geosolve-constraint-editor --all-features
# 334 unit tests plus every integration and doc-test target passed

cargo test --locked -p geosolve-demo-web --lib --all-features
# 111 passed

cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
# all passed

./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
# reviewed 261/261 PASS inventory matches exactly

env NO_COLOR=true nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && trunk build --release --locked'
# Trunk 0.21.14 release build passed

M72_BASE_URL=http://127.0.0.1:8094/ node /tmp/m72_full_browser_check.mjs
M74_BASE_URL=http://127.0.0.1:8094/ node /tmp/m74_browser_check.mjs
# Chromium passed at 1440x900 and 1024x720 with no console or page errors
```

The golden expansion adds 27 reviewed rows: deterministic plus eight seeded cases for Origin
coincidence, point-on-datum-axis and datum-line collinearity. Its SHA-256 is
`805dc2a9bde96d3c7980e7ee314527d0406b6e88fbd370fb97eff760224b3c84`.

An independent implementation review found no solver, persistence, accepted-scene or authority
blocker. It identified stale HUD-on-leave, delayed pan cursor and invisible edge-datum hits; all
three received focused corrections before release nomination. Follow-on review then caught the
off-screen-Origin/visible-axis picking interaction and added exact native/WASM evidence for both
the hidden-datum miss and the independently visible-axis hit.

Still required before nomination:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That command must run from a clean committed source. Its exact output distribution will then be
copied without rebuilding and byte-verified before Tailscale publication.

## 4. Open completion gates

- Pass the warnings-denied clean release gate and freeze its exact output without rebuilding.
- Serve and byte-verify the immutable candidate over Tailscale, then complete `docs/M74_UAT.md`.
- Receive explicit supervising-human approval.
- Deploy the exact accepted source through GitHub Pages and verify every hosted byte/media type.

## 5. Compatibility result so far

The in-progress APIs are additive pre-1.0 sketch/editor surface. They do not modify a released
persistence language: canonical sketch v1-v4 remains the only supported sketch wire contract and
rejects datum relations with `UnsupportedM74State`; the representations in draft-v5 side records
remain unsupported. Intrinsic datums have no persistent identity, so hosts must not serialize a
scene-clipped `SceneDatum` or treat it as application identity.
