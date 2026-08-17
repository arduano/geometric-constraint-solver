<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 implementation — CAD curve handles and implicit parameters

Status: **active (2026-08-17); implementation, owner regressions and pre-nomination qualification
pass. The clean committed-source gate, immutable Tailscale candidate, human UAT, GitHub Pages
publication and closeout remain open.**

## Approved architecture

- `geosolve-sketch` owns the semantic control catalog, inverse configuration projections,
  rational ordinary/projective conversion and independently validated document edits.
- Prepared work exposes only an immutable accepted preview view. The live retained session changes
  solely when an exact patch wins compare-and-swap publication.
- `geosolve-constraint-editor` owns selected-only control identities, guides, paint/hit geometry,
  hover/click priority, pointer gestures, last-valid preview and exact property metadata.
- `geosolve-demo-web` renders headless DTOs and forwards typed inputs. It contains no curve
  equation, inverse projection, branch choice or independent hit policy.

## Files and public APIs

- `geosolve-sketch::SketchDocument` now publishes the typed `DocumentCurveControl*` catalog,
  `curve_controls`, `project_curve_control`, `project_curve_trim_endpoint`,
  `rational_conic_control` and atomic `set_rational_conic_control` boundaries. The public
  ordinary/projective rational DTOs preserve one unambiguous meaning for the middle control.
- `DocumentEdit::SetRationalConicControl` and `PreparedSketchPreview` extend existing prepared
  transaction machinery. The preview exposes only immutable candidate input/design/accepted
  views; exact compare-and-swap publication still consumes the opaque patch.
- `geosolve-constraint-editor` adds `SceneCurveControl*` handles, guides, rails and shared finite
  paint/hit geometry; `CurveNumericProperty*` and `SelectedCurvePropertyMetadata`; selected-only
  scene population; one hover/pointer gesture; and retained-coordinator preview/commit plus exact
  numeric property setters.
- `geosolve-demo-web` renders the published cage, handles, rails, hover state, property inspector
  and pointer lifecycle. It forwards typed requests and contains no curve equation, inverse
  projection, branch choice or competing hit policy.

The principal implementation and owning regressions are in:

- `crates/geosolve-sketch/src/{document.rs,document_session.rs}` and
  `crates/geosolve-sketch/tests/m77_curve_controls.rs`;
- `crates/geosolve-constraint-editor/src/{curve_controls.rs,lib.rs,coordinator.rs}` and the five
  `m77_*` integration targets; and
- `crates/geosolve-demo-web/src/workbench/{mod.rs,scene.rs}`, `index.html` and `styles.css`.

## Mathematical and transactional behavior

- Circular and elliptical arcs retain explicit sweep while Start/End angles unwrap near their
  own accepted seeds. Parabola and hyperbola inverse projections retain the sign of
  `trim_end - trim_start`; equality or a crossing is rejected rather than swapping endpoints or
  reversing orientation. Hyperbola branch remains separate explicit state.
- Circle/arc radius and hyperbola semi-conjugate size project on positive scalar rails. Ellipse
  minor size projects to the existing finite ratio domain `0 < ratio <= 1`, without swapping axes
  or changing family.
- For nonzero rational weight, the spatial middle is `P1 = Qh / w` and a spatial edit stores
  `Qh = w·P1`. That round trip must remain finite and preserve each ordinary component within
  `64·f64::EPSILON·|P1|`; lossy underflow rejects atomically. Exact zero weight stays explicit
  projective `Qh` state. A host-bound effective weight may shape `Qh`, but a spatial edit never
  overwrites the persistent fallback weight.
- Point-backed cage controls remain aliases of their stored point owner and use ordinary point
  dragging. Derived direct grips own the selected curve. Inside the acquisition region a stored
  point alias outranks a guide originating at the same coordinate; the guide remains hittable
  beyond it.
- Every sample starts from exact accepted authority. Only a finite independently accepted prepared
  candidate may preview; an invalid later sample retains the last valid preview. Release consumes
  that exact patch as one history step. Cancellation, rejection, staleness and no-op gestures
  publish neither geometry nor history.

No solver equation, residual, constraint, dimension, rank/DOF rule, canonical sketch schema or
annotation-cache field changed.

## Review findings

- `M77-F008` — stored-point aliases were initially hidden by guide-only hits at a shared origin.
  Grip ownership now wins inside the point region while the guide stays available beyond it.
- `M77-F009` — a spatial rational-middle edit initially persisted a host-effective weight into the
  fallback scalar. Spatial edits now lower to `SetConicWeightedMiddle`; explicit numeric/mode edits
  retain `SetRationalConicControl`.
- `M77-F010` — non-periodic trim handles could initially cross the opposite endpoint. Ascending and
  descending parabola/hyperbola trims now reject crossings transactionally through projection and
  retained publication.
- `M77-F011` — a finite nonzero weight could underflow `Qh` and lose material `P1` information.
  Precision-preserving homogeneous representability is now validated before any atomic edit.

These are isolated owner defects, so no golden-oracle expansion was warranted. Qualification also
found one stale M19 expectation: endpoint equality is now rejected during projection rather than
only by the later scalar setter. Test-only commit `20ae036` preserves both typed projection
rejection and byte-identical setter rejection; it changes no product behavior and received no
finding ID.

## Focused and pre-nomination qualification

- Sketch control suite: 10/10 passed.
- Editor scene/control suite: 8/8 passed.
- Exact curve-property suite: 6/6 passed.
- Retained coordinator suite: 14/14 passed.
- Native and WASM curve-control parity: 4/4 passed in each target.
- Rational replay: 1/1 passed.
- Demo-web library: 127/127 passed.
- Full M19 compatibility suite after `20ae036`: 24/24 passed.
- Demo WASM check, warnings-denied Rustdoc and Trunk 0.21.14 release assembly pass.
- Golden survey, check and require-clean each pass the unchanged 270/270-`PASS` inventory.
- The post-`20ae036` locked all-feature workspace suite passes. Formatting, diff hygiene and
  focused warnings-denied Clippy pass. One clean committed-source release gate remains nomination
  authority.

Exact notable commands already run successfully include:

```text
cargo test --locked -p geosolve-sketch --test m77_curve_controls
cargo test --locked -p geosolve-constraint-editor --test m77_curve_controls
cargo test --locked -p geosolve-constraint-editor --test m77_curve_properties
cargo test --locked -p geosolve-constraint-editor --test m77_curve_control_coordinator
cargo test --locked -p geosolve-constraint-editor --test m77_curve_control_parity
cargo test --locked -p geosolve-constraint-editor --test m77_rational_control_replay
cargo test --locked -p geosolve-demo-web --lib
cargo test --locked -p geosolve-sketch --test m19
cargo test --locked --workspace --all-features
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
```

The WASM parity command requires the project `shell.nix` in the current ambient environment so
`wasm-bindgen-test-runner` is available. A direct ambient invocation failed only for that missing
runner; the exact project-shell invocation passed 4/4 and is the recorded result.

## Acceptance and known limits

The approved family inventory, selected-only ownership, exact projection, rational semantics,
properties, preview/cancellation/staleness, one-step history and persistence contracts have direct
native evidence. Weight rails, knot/degree/topology editing, generalized derived-point constraint
targets, automatic trim/branch changes and mobile layout remain deliberate non-goals.

Mechanical nomination still requires one clean committed-source `scripts/release-gate.sh` pass,
then a no-rebuild seven-file freeze and served-byte verification. U1-U6 remain genuine human UAT;
no item is accepted by automation alone.

## Closeout evidence

Pending clean release nomination, immutable candidate identity, explicit human UAT disposition,
accepted-source GitHub Pages publication, hosted-byte verification and a clean final worktree.
