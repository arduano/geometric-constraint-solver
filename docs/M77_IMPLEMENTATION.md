<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M77 implementation — CAD curve handles and implicit parameters

Status: **active (2026-08-17); the M77-F012/F013 replacement is clean-qualified, immutably frozen
and byte-verified on Tailscale. Human UAT, GitHub Pages publication and closeout remain open.**

## Approved architecture

- `geosolve-sketch` owns the semantic control catalog, inverse configuration projections,
  rational ordinary/projective conversion and independently validated document edits.
- Prepared work exposes only an immutable accepted preview view. The live retained session changes
  solely when an exact patch wins compare-and-swap publication.
- A prepared curve-control scene keeps truthful candidate design/revision/computed provenance. A
  private gesture-local seal separately binds the pointer-down origin to the exact accepted preview
  request and control surface; older candidate generations cannot publish newer unseen geometry.
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
  and pointer lifecycle. Circular and elliptical arcs share spatial Start/End construction, with
  the headless editor supplying projected support-curve previews. The adapter forwards typed
  requests and contains no curve equation, inverse projection, branch choice or competing hit
  policy.

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
- Circular-arc authoring remains Centre, Start, End. Elliptical-arc authoring is Centre, Major axis,
  Start, End; both trim clicks are radially projected in normalized ellipse space and incomplete
  stages consume a headless-evaluated support ellipse.
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
  beyond it. Elliptical-arc minor-size grips choose the clearer signed pole, and crowded derived
  grips shift by a zoom-independent 16 screen pixels while retaining their exact model target and
  one-dimensional rail.
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
- `M77-F012` — dragging a curve control composed candidate geometry with pointer-down revision and
  design stamps, so the scene disappeared during movement and release committed nothing. Candidate
  scenes now keep truthful candidate provenance while a private origin authenticates the live
  gesture; point-alias and direct-trim transactions for both arc families remain visible and commit
  exactly one history step.
- `M77-F013` — approved parity enhancement: elliptical-arc construction now uses four spatial
  stages (Centre, Major axis, Start, End) with exact radial trim projection, matching the circular
  arc's spatial trim language without pretending the two families have the same axis inputs.
- `M77-F014` — a trim at an elliptical arc's stored major-axis pole could win the coincident hit.
  The stored point now owns the physical pole and the derived trim moves tangentially 16 screen
  pixels while remaining independently hittable.
- `M77-F015` — at exactly 16 px per model unit, shifting a crowded minor-size grip could cancel its
  rail direction. Rail direction now derives from the unshifted model projection.
- `M77-F016` — an older privately authenticated candidate scene could release a newer unseen
  request. The seal now binds the accepted request ID and model position; stale generations cancel
  without geometry, transcript or history publication.

These are isolated owner defects, so no golden-oracle expansion was warranted. Qualification also
found one stale M19 expectation: endpoint equality is now rejected during projection rather than
only by the later scalar setter. Test-only commit `20ae036` preserves both typed projection
rejection and byte-identical setter rejection; it changes no product behavior and received no
finding ID.

## Focused and pre-nomination qualification

- Sketch control suite: 11/11 passed.
- Editor scene/control suite: 11/11 passed.
- Exact curve-property suite: 6/6 passed.
- Retained coordinator suite: 16/16 passed.
- Native and WASM curve-control parity: 5/5 passed in each target.
- Rational replay: 1/1 passed.
- Demo-web library: 131/131 passed.
- Editor library: 353/353 passed.
- Full M19 compatibility suite after `20ae036`: 24/24 passed.
- Demo WASM check, warnings-denied Rustdoc and Trunk 0.21.14 release assembly pass.
- Golden survey, check and require-clean each pass the unchanged 270/270-`PASS` inventory.
- The post-`20ae036` locked all-feature workspace suite passes. Formatting, diff hygiene and
  focused warnings-denied Clippy pass. The exact clean committed-source release gate below is
  nomination authority.

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
runner; the exact project-shell invocation passes 5/5.

## Replacement qualification and nomination

Implementation commit `f53934f` is contained in exact clean product source
`cc99b11071dc62732e02b630ba7a1381d754b04c`, tree
`3315a2bdd0137f59657ea2500962ef971a23ea15`. From a clean worktree, the exact command

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

ran from 18:21:03 through 18:33:22 AEST and exited successfully without changing HEAD, tree or
worktree. The retained 243,128-byte log is
`/tmp/geosolve-m77-replacement-clean-gate.cc99b11.log`, SHA-256
`0da2456b69951a50ba41cfe939f37764fcc211f2af45f4b9526cd5c974829301`. It passes formatting/diff,
warnings-denied workspace Clippy and Rustdoc, every locked all-feature workspace test, unchanged
270/270 clean golden authority, every carried native/WASM parity target through M77, demo WASM,
benchmark compilation, M14/M32 workloads, the 138.34-second 256-body sparse crossover,
licensing/package contents and Trunk 0.21.14 release assembly. The only diagnostics are the
longstanding non-failing Cargo advisories for packages declaring both `license` and `license-file`.

Without rebuilding, the exact gate output was byte-compared and frozen at
`/tmp/geosolve-m77-uat.ARrQFw`, directory mode `0555`, seven regular non-symlink files mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 23,216 | `841936f73b5d21fbee999ec2bc4140ae0869cd2821429816e3766bd026ad771b` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-74621b73a35eab86.js` | 33,221 | `9f28eed1331a570a1fa894f16834a40be0593ef9bd673ca80db8fbea4017eef1` |
| `geosolve-demo-web-74621b73a35eab86_bg.wasm` | 6,426,513 | `1c4701e10d4ca672b0aa2511ff3fc4067be5c03965274de4925a711b5414e3f1` |
| `index.html` | 28,940 | `f3740f54742d6895e204cc41c08e031d0f2b639e6dd30df30c3e08b1b878527d` |
| `styles-d7435a6d60dc3430.css` | 34,689 | `870bde7d758fe95f4323bedc6588ff2cffaf3c826549e684718ebfd818eebcd6` |

Its C-locale ordered-manifest aggregate is
`abfa7ef6b75f127fa6d93ff6ad6960c7f5df7d4c799a578c785e1192c2b7ee94`; freeze evidence is
`/tmp/geosolve-m77-replacement-freeze-evidence.2kfhjk`. PID `284248`, retained command-runner
session `5213`, serves only that snapshot at `http://100.94.63.83:8080/`. Proxy-disabled,
cache-bypassed identity requests for `/` and every file return HTTP 200 with zero redirects, no
content encoding, expected media types/lengths and exact bytes; `/` equals `index.html` and the
fetched aggregate matches. HTTP evidence is
`/tmp/geosolve-m77-replacement-http-verify.yxgjkL`. Withdrawn PID `3912158` remained live until the
replacement freeze was complete, then was retired before the verified replacement listener began.

The later evidence-ledger commit is a documentation descendant and does not replace `cc99b11` as
the exact gate-qualified product source.

## Superseded initial qualification

Initial source `51a3b95d04f27216c164febf0808a180b6775537`, tree
`8d154a147a08c7d6bc79008f19b74311cd60905a`, passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The clean worktree, HEAD and tree were unchanged from 15:48:17 through 16:00:06 AEST. The retained
241,980-byte log is `/tmp/geosolve-m77-clean-gate.51a3b95.log`, SHA-256
`e28c50101df3b9c447ccf1a392f0e3e5644068e8abc460be389aaf3cff1984ed`. It passes formatting/diff,
warnings-denied workspace Clippy and Rustdoc, all locked all-feature tests, unchanged 270/270 clean
golden authority, every carried native/WASM parity target through M77, demo WASM, benchmark
compilation, M14/M32 workloads, the 150.55-second 256-body sparse crossover, licensing/package
contents and Trunk 0.21.14 release assembly. The only diagnostics are the longstanding non-failing
Cargo advisories for packages declaring both `license` and `license-file`.

Without rebuilding, the exact seven regular gate-output files were byte-compared and frozen at
`/tmp/geosolve-m77-uat.1mDjQv`, directory mode `0555`, file mode `0444`, no symlinks. Its ordered
manifest is recorded in `docs/M77_UAT.md`; aggregate:
`af7c2fbca1a6481c8c055142c9a64578b570fbcb297f687f09cc8ffc85bd1b8b`.

PID `3912158`, command-runner session `12828`, formerly served only that snapshot at the shared
Tailscale endpoint. Proxy-disabled, cache-bypassed identity requests for `/` and every file returned
HTTP 200 with zero redirects, no content encoding, expected media types/lengths and exact bytes;
`/` equalled `index.html` and the fetched aggregate matched. Freeze evidence is
`/tmp/geosolve-m77-freeze-evidence.qbBmc5`; HTTP evidence is
`/tmp/geosolve-m77-http-verify.eu1KMY`. The previous M76 PID `1780608` was retired only after the
new snapshot was ready. M77-F012/F013 supersede those product bytes for UAT; PID `3912158` is now
retired and these bytes are historical evidence only.

Corrected implementation source `f53934f` passes formatting, diff hygiene, warnings-denied
workspace Clippy, locked all-feature workspace tests, focused sketch 11/11, controls 11/11,
coordinator 16/16, native/WASM parity 5/5, demo 131/131 and unchanged golden survey/check/clean
270/270. The clean documentation descendant, complete release gate, no-rebuild immutable freeze and
served-byte verification recorded above are the replacement nomination authority.

## Acceptance and known limits

The approved family inventory, selected-only ownership, exact projection, rational semantics,
properties, preview/cancellation/staleness, one-step history and persistence contracts have direct
native evidence. Weight rails, knot/degree/topology editing, generalized derived-point constraint
targets, automatic trim/branch changes and mobile layout remain deliberate non-goals.

Replacement mechanical nomination is complete. U1-U6 remain genuine human UAT; no item is accepted
by automation alone. GitHub Pages publication is intentionally withheld until explicit approval.

## Closeout evidence

Pending explicit human UAT disposition, accepted-source GitHub Pages publication, hosted-byte
verification and milestone closure. The evidence-recording descendant must retain a clean
worktree and does not replace the exact qualified product source above.
