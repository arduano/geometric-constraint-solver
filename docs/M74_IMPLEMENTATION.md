<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M74 implementation — Production-style sketch reference UX

Status: **M74-F001 integrated; replacement qualification in progress as of 2026-08-16**. Focused
native/WASM qualification passes for point-pair symmetry across intrinsic axes. The earlier
candidate predates F001 and is historical; a clean replacement nomination, explicit human approval
and final GitHub Pages deployment remain open.

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
- M74-F001 adds `SymmetricAboutDatumAxis` across document/runtime/lowering, a public
  `Sketch::add_symmetric_about_datum_axis` builder and a two-row constant analytic residual. It
  carries only two point variables plus the closed X/Y axis enum: no hidden line or datum identity.
- `crates/geosolve-sketch/tests/m74_reference_geometry.rs` is the focused domain suite
  for scale behavior, residuals, finite-difference Jacobians, audit descriptors, lifecycle and
  draft-v5 round trips.

### Headless editor and inference

- `SelectionItem::Datum`, `SceneDatum`, `EditorScene::datums` and
  `GeometryVisibility::reference_geometry` expose intrinsic references without document IDs. Scene
  DTOs clip the infinite semantic axes to the current finite viewport for presentation.
- Contextual authoring resolves Origin coincidence, point-on-axis and line-on-axis collinearity in
  either operand order. Parallel/Perpendicular with X/Y axes lower to ordinary Horizontal/Vertical.
- Symmetric resolves two distinct points plus a line or intrinsic X/Y axis. Complete preselection
  is permutation-independent while active authoring remains point → point → reference; repeated
  points and Origin reject with typed, mutation-free failures. Datum symmetry reuses the ordinary
  Symmetry glyph/paired-point anchor and publishes the datum among related operands.
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

Datum-axis symmetry contributes exactly two model-unit hard rows. For X axis they are
`(first.y + second.y)/2` and `second.x - first.x`; for Y axis they are
`(first.x + second.x)/2` and `second.y - first.y`. Each row is divided by document model scale.
The constant analytic Jacobian has `0.5/0.5` normal-coordinate entries and `-1/+1`
tangent-coordinate entries. Two otherwise-free points therefore report numerical rank two and
right nullity two. The focused tests independently recompute both normalized equations at scales
`1e-6`, `1` and `1e6` rather than trusting the solver-owned audit alone.

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
# 9 passed

cargo test --locked -p geosolve-constraint-editor --test m74_reference_geometry --all-features
# 5 passed natively

env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor --test m74_reference_geometry \
   --target wasm32-unknown-unknown'
# the same 5 passed under wasm-bindgen-test-runner

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
# reviewed 270/270 PASS inventory matches exactly

env NO_COLOR=true nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && trunk build --release --locked'
# Trunk 0.21.14 release build passed

M72_BASE_URL=http://127.0.0.1:8094/ node /tmp/m72_full_browser_check.mjs
M74_BASE_URL=http://127.0.0.1:8094/ node /tmp/m74_browser_check.mjs
# Chromium passed at 1440x900 and 1024x720 with no console or page errors
```

The original golden expansion added 27 reviewed rows for Origin coincidence, point-on-datum-axis
and datum-line collinearity. M74-F001 appends exactly nine more PASS rows for datum-axis symmetry;
all prior 261 rows remain byte-identical. The resulting fixture SHA-256 is
`7a4afd4fbd70d0ef6454e5f07f00fde7afb64eec59d329acfba7f761d986e343`.

An independent implementation review found no solver, persistence, accepted-scene or authority
blocker. It identified stale HUD-on-leave, delayed pan cursor and invisible edge-datum hits; all
three received focused corrections before release nomination. Follow-on review then caught the
off-screen-Origin/visible-axis picking interaction and added exact native/WASM evidence for both
the hidden-datum miss and the independently visible-axis hit.

M74-F001 focused qualification passes sketch 9/9, editor 5/5 natively and under WASM, complete
editor 334/334 plus every integration, demo-web 111/111, targeted warnings-denied Clippy and the
270/270 clean golden check. The complete clean replacement gate remains pending at this checkpoint.

The historical initial candidate command passed from committed product source
`7ac3f3b41942a4f4bf5f1a4f06fd59b37caa37a8`, tree
`eff049a7fc0f2df941bcb1360ffb88f60868af21`:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
# exit 0
```

The gate repeats formatting, warnings-denied Clippy, locked all-feature workspace tests, the exact
261/261 golden check, native/WASM transition and M74 parity, Rustdoc and benchmark compilation. Its
M14/M32 performance examples passed, the release 256-moving-body sparse crossover passed in
88.91 seconds, licensing and package contents passed, and Trunk 0.21.14 assembled the release
distribution successfully.

## 4. Historical initial UAT candidate

The seven files produced by that successful gate were copied without rebuilding to
`/tmp/geosolve-m74-uat.MpvYrl`. The directory is mode `0555`; every file is mode `0444`; all entries
are regular, non-symlink files.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 16,385 | `180092f5db68423f14760db12265d06b81786df5ed3d3ba6f5ecd745e36ad567` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-90bff110a4eada3.js` | 33,221 | `115760f338139851520a5978ddaad4acb7441a5ec81a83d793885f81651eff16` |
| `geosolve-demo-web-90bff110a4eada3_bg.wasm` | 6,091,298 | `f8389efd2c34519f38b0b3195a1efffe9a822c7641c124361997ab9131936b92` |
| `index.html` | 27,474 | `a53bd7f661e92e5ba856ebdaca686c53ab3e1566d5c1ad32cc2a90065930c56a` |
| `styles-711a681b653e6d49.css` | 30,861 | `d75f830c2e0af21399fd94f31dda74888a4ce82bbe7527521c7d5f5a1c948532` |

The C-locale `sha256sum * | sha256sum` aggregate is
`2ceaa9f8707a54aa9bcbf62771a5cd0c3f6dd594bd5ba2829ffc370ee7588546`.

PID `969003` serves only this snapshot at `http://100.94.63.83:8080/` with exact argv:

```text
python3 -u -m http.server 8080 --bind 100.94.63.83 --directory /tmp/geosolve-m74-uat.MpvYrl
```

Its executable is
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`.
The former M73 server PID `3870531` exited only after the M74 snapshot was complete. Historical
snapshot `/tmp/geosolve-m73-uat.JKAWtJ` remains read-only with its unchanged aggregate
`3153f3b7b75e55ecc27c8798f4f26c6368c5b1e8db8422ee44c8840612d7ba8e`.

Proxy- and cache-bypassed, identity-encoded Tailscale requests for `/` and all seven named files
returned HTTP 200 directly from `100.94.63.83`. Every response had the exact expected length and
media type (`text/html`, `text/markdown`, `application/octet-stream`, `text/javascript`,
`application/wasm` or `text/css`), matched the frozen file byte-for-byte, and had no redirect or
compressed encoding. `/` equals `index.html`; the fetched seven-file aggregate equals the frozen
aggregate. The retained HTTP evidence directory is `/tmp/geosolve-m74-http-verify.EiuhSE`.

The candidate also passed both browser scripts directly over Tailscale:

```text
M72_BASE_URL=http://100.94.63.83:8080/ node /tmp/m72_full_browser_check.mjs
M74_BASE_URL=http://100.94.63.83:8080/ node /tmp/m74_browser_check.mjs
# both passed at 1440x900 and 1024x720 with no console or page errors
```

The reviewed script SHA-256 values are respectively
`4fdf48db8a39c5f10e42bbd6da34421bf1f1a4450d3bd92e7b04bc1ec6f87b44` and
`e6606f7756d33fff091b228dfd5b6395ceda5deb5e014946635fefb1cc539bcc`.

M74-F001 changes product bytes, so this snapshot is retained only as historical evidence and is no
longer current UAT authority. It must not receive human approval for the replacement scope.

## 5. Open completion gates

- Pass the complete clean replacement release gate from committed F001 source.
- Copy its exact gate output without rebuilding, replace the live Tailscale candidate, byte-verify
  every asset and complete `docs/M74_UAT.md` over those replacement bytes.
- Receive explicit supervising-human approval.
- Deploy the exact accepted source through GitHub Pages and verify every hosted byte/media type.

## 6. Compatibility result so far

The in-progress APIs are additive pre-1.0 sketch/editor surface. They do not modify a released
persistence language: canonical sketch v1-v4 remains the only supported sketch wire contract and
rejects datum relations with `UnsupportedM74State`; the representations in draft-v5 side records
remain unsupported. Intrinsic datums have no persistent identity, so hosts must not serialize a
scene-clipped `SceneDatum` or treat it as application identity.
