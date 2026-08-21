<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 implementation — certified computed all-family Curve Offset

Status: **M82-F006/F007 replacement implementation is under qualification; human UAT pending**.
The post-F005 source and immutable artifact are withdrawn after exact periodic-NURBS plus fresh
Bezier reproductions exposed blank-scene composition, and after the computed route gained
source-owned inverse-edit proxies. Its listener is retired. No replacement source, tree, frozen
artifact or clean-gate claim exists yet. Product scope is owned by `docs/M82_GOALS.md`;
architecture is owned by ADR 0038.

Activation baseline: M81 closeout commit
`e3cbb8f2ae2800181545bb3405704bdcc3ff46a6`.

Withdrawn post-F005 product source: `d52104595ee11f9e460e98ea5e26200bb34a5d94`.

Withdrawn post-F005 product tree: `0a3bcb066a6a2d5d5d2d99591441035be23d20fe`.

Replacement product source/tree: **not yet nominated**. They will be recorded only after the
expanded golden matrix and complete clean release qualification pass from one unchanged source.

Withdrawn pre-F005 product source: `7fd31c0137f6979f945e5ab4d320e7adb552c03d`.

Withdrawn pre-F005 product tree: `c6b6c89cecde30b2b3a7cf057ec61317a38a5634`.

## Implementation ledger

### Phase 1 — certified parallel-curve kernel

- [x] Add one public sketch-domain curve-offset evaluation boundary over accepted
  `SketchDocument` curve data; no primitive equation is duplicated in features or the browser.
- [x] Preserve exact Line, Circle and CircularArc output and add deterministic adaptive
  endpoint-Hermite cubic patches for every other built-in planar family.
- [x] Certify finite denominators/speed, signed curvature and the `1 - delta*kappa > 1e-8`
  regularity margin over bounded intervals.
- [x] Independently certify endpoint equality/tangent alignment, continuous position/tangent
  error bounds, deterministic subdivision and cooperative work limits.

Primary API: `compute_curve_offset` and caller-controlled
`compute_curve_offset_with_controller`, with typed `CurveOffsetGeometry`,
`CurveOffsetCertificate` and `CurveOffsetError` results. Interval jet bounds remain in
`geosolve-sketch`; exact analytic pieces and certified cubics are returned without adding solver
variables, residuals or persistent generated geometry.

The all-family scale/translation/rotation/reversal matrix passed one exact test in 46.86 seconds.
The complete all-feature `geosolve-sketch` suite passed in 148.65 seconds before the final bounded-
control audit; the two exact M82-F003 owner regressions pass on the corrected tree in 14.97 seconds.

### Phase 2 — topology-aware computed feature

- [x] Add the reviewed acyclic `geosolve-sketch-features -> geosolve-sketch-topology` dependency.
- [x] Add persistent `ComputedFeatureDefinition::CurveOffset` intent for faces and ordered open
  chains, including traversal, side/direction, source adjacency, junction cells and terminal
  policy.
- [x] Compose exact and fitted output with stable feature provenance and revision-local generated
  IDs while preserving Fillet source-replacement and corner-selection behavior.
- [x] Certify source and fitted-output self-contact, inter-contour contact, topology, loop winding,
  hole nesting and complete atomic failure under bounded work.

`geosolve-sketch-topology` remains the source of authenticated face/chain adjacency. Owned
opposite-inward tangent endpoints and intrinsic spline boundaries form local tangent joins;
unowned coordinate-only contacts remain incomplete. General inner/outer junctions use certified
curve/curve intersection and miter construction rather than linearizing a curved carrier. Exact
outer/hole containment is checked before publication. The feature suite passed its complete
56-test package slice during development qualification; M82-F003's exact feature-level exhaustion
regression also passes on the corrected tree.

### Phase 3 — authoring, scene and coordinator lifecycle

- [x] Generalize the existing Offset resolver to all supported native families and intrinsic
  spline-span adjacency while preserving M80 routing for wholly native-eligible input.
- [x] Route mixed/general operands to computed `CurveOffset`, retain computed-Fillet exclusion and
  accept M80 native-published Fillet topology normally.
- [x] Reuse the Offset panel, preview and distance-drag lifecycle with complete computed preview,
  typed failures and exact current-scene/CAS authentication.
- [x] Extend tree, Problems, scene composition, picking, selection, highlighting, Undo/Redo,
  replay, suppression/delete and source-edit reevaluation. Offset output selects its feature;
  Fillet output continues to select its corner.

Route-specific preview authority now boxes the large native/computed payloads and Apply publishes
only the exact held proposal. Inconsistent preview authority returns `OffsetPreviewMismatch`
instead of panicking. Computed preview construction is cold with respect to existing feature
continuation state. The live browser collector also refreshes when feature identity changes even
if the accepted sketch identity does not.

Development qualification passed the final M82 coordinator slice (16/16), the editor library
(406/406) and the final demo library (160/160), including exact M82-F001 coverage, live Offset
collector refresh after computed-feature-only changes and the M82-F005 red/green browser-copy
regression. Those counts describe the now-withdrawn post-F005 tree, not the F006/F007 replacement.

### Phase 4 — strict persistence compatibility

- [x] Add private computed-feature version 2 for Curve Offset intent.
- [x] Continue emitting feature version 1 byte-for-byte for empty and Fillet-only state.
- [x] Round-trip v2 through unchanged workspace v6 and reproduction v1; reject malformed or
  unsupported state atomically and never serialize generated patches, certificates or output IDs.

Strict v2 records only feature intent and allocator authority. Existing v1 fixtures and output
bytes remain unchanged whenever no Curve Offset exists. Workspace and `GEOSOLVE_REPRO_V1`
round-trips regenerate output from accepted sketch plus feature intent.

### Phase 5 — withdrawn post-F005 regression, review and release qualification

- [x] Add focused owner regressions for every curve family, analytic exactness, mixed chains,
  faces/holes, spline adjacency, singularity, self-contact, topology barriers, bounded work,
  lifecycle, routing and persistence.
- [x] Add and independently review one justified systemic golden row for the new public Curve
  Offset authoring family.
- [x] Run focused owner suites, golden survey/check/require-clean, native/WASM parity, demo adapter,
  format/diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature workspace tests
  and a pinned Trunk release build during development qualification.
- [x] Run the complete release gate from the corrected clean commit and retain its exact log/hash.
- [x] Freeze the replacement gate-produced distribution without rebuilding, byte-verify it over Tailscale and
  bind `docs/M82_UAT.md` to that exact source/tree/artifact.

The withdrawn post-F005 golden inventory increased from 271 to 272 `PASS` rows by exactly
`feature.curve-offset.authoring.general-open-chain`, input fingerprint
`input-86b0e748c84c8899`. The fixture SHA-256 is
`a0d53753e5597dd950024a8108b12f92f325b1896766a5c601220c64970ee123`.
Golden `--survey`, `--check` and `--require-clean` passed; a later post-cleanup `--check` passed in
78.54 seconds. M82-F006/F007 prove that one generic row was not sufficient product authority.

### Phase 6 — M82-F006/F007 replacement hardening

- [x] Preserve the supplied periodic-NURBS reproduction payload as an exact fixture and reproduce
  the same blank-scene root with fresh quadratic and cubic Bezier Offset previews.
- [x] Authenticate staged Curve Offset feature identity during Fillet-affordance composition and
  retain the complete native accepted scene if computed presentation composition fails.
- [x] Add transient feature-selected inverse proxies that retain ordinary source curve-control
  IDs and feed pointer motion through prepared solving, independent validation, exact CAS,
  history, replay and computed reevaluation.
- [x] Keep generated patches revision-local, non-persistent and unavailable as direct constraint
  or operation operands; scalar/trim controls stay on the native source cage.
- [x] Expand the broad golden authoring/scene matrix into separately reviewed rows for every
  built-in geometry family, mixed chains, faces, holed faces and proxy/source regeneration. Every
  computed row inventories all eligible two-dimensional proxies, projects one proxy edit through
  an ordinary Coincident follower, requires independent X/Y source motion, regenerates fresh
  Current output and restores the complete state through Undo/Redo. The reviewed result is exactly
  289/289 `PASS` with SHA-256
  `cec9ad971e8e445f6a0e040a534d6790880e1a31903addea0d6d08ae1a5ad5f7`.
- [ ] Run focused F006/F007 owner and adapter tests, golden survey/check/require-clean, formatting,
  Clippy/Rustdoc, locked all-feature workspace tests, WASM/Trunk and the complete clean release
  gate from one unchanged replacement source.
- [ ] Freeze and byte-verify that replacement without rebuilding, restore a replacement Tailscale
  listener, bind the UAT scorecard to exact authority, complete human UAT and publish Pages.

## Finding ledger

### M82-F001 — a new Curve Offset preview could inherit unrelated Fillet continuation state

Reproduced at the retained coordinator preview boundary. A computed Fillet can be Current while a
separate general curve is collected for Offset. The provisional feature document differs from the
durable document, so continuing evaluation from the Fillet snapshot is not valid authority for the
new Offset preview. The pre-repair path passed that unrelated snapshot into continuation-capable
evaluation.

The repair cold-evaluates the provisional feature document. Exact regression
`m82_f001_computed_offset_preview_cold_evaluates_beside_an_unrelated_fillet` proves the computed
Offset preview succeeds, both Current features are present in the complete provisional snapshot,
and durable feature identity, history and transcript remain unchanged. The focused command passes
one test with 15 filtered out.

### M82-F002 — owned tangent endpoints were rejected as non-local intersections

Reproduced at public sketch profile analysis. A Line/general-curve endpoint with exact persistent
ownership and opposite-inward tangents was classified like an unowned coincident tangency, making
a legitimate continuous selected chain incomplete.

The repair treats only the owned opposite-inward tangent endpoint as a local join. Unowned
coordinate-only tangency continues to fail closed. Exact regression
`crates/geosolve-sketch/tests/m82_owned_tangent_endpoint.rs` binds both positive and negative
controls; the separate containment regressions remain in
`crates/geosolve-sketch/tests/m82_profile_containment.rs`.

### M82-F003 — adaptive curve fitting escaped the caller's cooperative work envelope

Reproduced at the public sketch offset kernel and computed-feature execution boundary during final
diff review. The initial implementation checked the outer feature controller but ran adaptive
curve fitting through an unlimited local controller, so cancellation or `profile_subdivisions`
exhaustion after fitting began could not stop the recursive work at the declared checkpoints.

The repair adds a caller-controlled kernel entry point, charges one span before construction and
one unit before every actual subdivision, and returns no patch chain when stopped. Exact kernel
regressions `m82_f003_control_stops_before_curve_fitting_starts` and
`m82_f003_subdivision_exhaustion_stops_before_recursive_child_work` prove zero-work cancellation,
zero-work exhaustion, child-before-recursion stopping and completed accounting. Feature regression
`m82_curve_offset_work_exhaustion_publishes_nothing_and_never_reuses_output_ids` proves the stop
survives the owning feature boundary without publication or allocator reuse.

### M82-F004 — feature-only mutation left the active Offset collector stale

Reproduced in the workbench lifecycle with Offset active while a computed Fillet excluded its
native source spans. Suppressing that Fillet changes computed-feature document identity but not
the accepted `PreparedSketchInput`; the prior refresh condition therefore retained obsolete
exclusions and continued rejecting now-eligible source geometry.

The repair authenticates the collector against both accepted sketch input and computed-feature
identity after successful workspace actions. It preserves the existing `offset-apply` exception,
whose coordinator publication already reactivates the collector. Exact adapter regression
`m82_f004_computed_feature_only_change_reactivates_the_live_offset_collector` proves the sketch
input stays equal, feature identity changes, the refresh is required and the formerly excluded
span becomes collectible after reactivation.

### M82-F005 — Offset help text still claimed native-only curve support

Found during the final browser-copy audit after the first immutable nomination. The active Offset
panel still said “Exact lines, circles and circular arcs only,” directly contradicting M82's
general-family computed route and making the new capability undiscoverable in the demo.

The repair states the routing boundary truthfully: Line/Circle/CircularArc remain native, other
regular built-in curves use certified computed output, and singular, self-intersecting or
topology-changing offsets remain unavailable. Exact static-adapter regression
`m82_f005_offset_help_describes_native_and_computed_curve_routes` failed before the copy change and
passes afterward. The pre-F005 snapshot is withdrawn and its listener retired; no mathematical,
topology, persistence or coordinator behavior changed.

### M82-F006 — provisional general-curve Offset could blank the complete accepted scene

Reproduced through the ordinary workspace decoder/workbench composition path from the user's exact
periodic-NURBS reproduction and independently with fresh quadratic and cubic Bezier curves. The
durable text fixture is
`crates/geosolve-demo-web/tests/fixtures/m82_f006_periodic_nurbs_repro.txt`; after removing its one
storage newline, the exact encoded payload is 1,542 bytes with SHA-256
`33f0caeea427f6048067a6abf51411ee53a428c3d44c12c2e59c22a516360e02`; decoded workspace length
8,193 bytes, SHA-256
`929a43c8f51900f1f6bf34fb1696cdcef88e842c26cf80cee3f58f8da88640af`; envelope checksum
`e0db72996122baa0`. The persisted payload does not retain the transient Offset click identity, so
the adapter regression deliberately selects a deterministic representative periodic-NURBS span
rather than claiming to recover the exact click.

The curve-offset evaluator and tessellation were finite and valid. The actual root was scene
authority: `computed_feature_document_for_input` did not recognize a staged
`OffsetAuthoringPreviewPublication::ComputedCurve`, so Fillet-affordance composition rejected the
provisional feature as `StaleComputedFeatureCandidate`; `compose_editor_scene` then discarded the
entire result through `.ok()?`. The repair authenticates the staged Curve Offset feature document
and makes computed presentation failure fall back to the complete native accepted scene. A final
pre-nomination audit found that annotation, retained-session, proxy-control or interaction-origin
enrichment could still discard a successfully composed Current scene through later `.ok()?`
operators. That whole Current-composition/enrichment leg is now transactional: any failure retries
the independent native path, withholds computed controls and cancels an in-flight proxy gesture
without durable mutation. It does not publish invalid computed output or turn a typed failure into
Current.

Focused regressions are
`m82_f006_bezier_offset_preview_is_not_rejected_by_fillet_affordance_composition` and
`workbench::tests::m82_f006_exact_periodic_nurbs_offset_preview_never_blanks_the_accepted_scene`,
plus the downstream boundary regression
`workbench::tests::m82_f006_post_current_proxy_enrichment_failure_returns_complete_native_scene`.
The latter failed at the crossed adapter before the transactional fallback expansion, then passed
with exact source/feature/history/transcript neutrality. Full replacement qualification remains
pending.

### M82-F007 — computed Offset needed constraint-aware inverse direct manipulation

The post-F005 product exposed general computed geometry only as read-only generated output. That
kept revision-local patches out of solver state, but it did not meet the product expectation that
dragging an Offset curve control should move the owning constrained source and regenerate the
parallel.

The replacement keeps the one-way computed architecture and adds transient inverse-edit proxies.
Feature selection paints eligible two-dimensional source point and rational-middle controls
relative to computed output while retaining the ordinary `DocumentCurveControlId`. A pointer
request is inverse-translated to that source control, then uses the existing prepared solve,
source constraints, independent residual validation, exact CAS, history, replay, Undo/Redo and
computed reevaluation. Successful publication revokes old generated IDs and publishes fresh
Current output. Generated geometry remains revision-local, non-persistent and unavailable as a
direct constraint or operation operand; scalar rails and trim/orientation controls remain on the
native source cage.

The pre-nomination F007 authority audit also closes two interaction-specific gaps. Constructor-
owned computed geometry, source parameters, feature identity and evaluation input are sealed, so
a caller-mutated presentation DTO cannot manufacture a source solve or retain candidate release
authority. Analytic computed edges carry explicit traversal-correct source-parameter endpoints;
reverse Line, CircularArc and Circle correspondence therefore descends in native parameter space.
Connector-only miter edges remain visible and selectable but deliberately carry no inverse-proxy
correspondence because they do not represent a one-to-one source interval.

Owning coordinator regressions
`computed_curve_offset_proxy_drag_moves_the_source_and_commits_one_replayable_edit` and
`computed_curve_offset_proxy_drag_uses_the_normal_constrained_source_solve` bind source-ID
retention, inverse mapping, hard-residual validation, constraint projection, regenerated Current
output, history/replay and Undo/Redo. The 14 computed-route golden family/topology rows also require
the complete eligible two-dimensional proxy inventory, one constrained source/follower edit with
independent X/Y motion, fresh output, and complete Undo/Redo restoration. Their focused and
collateral replacement qualification is still in progress.

## Withdrawn post-F005 development qualification record

The following commands actually passed on the now-withdrawn post-F005 tree before its clean
nomination. They are historical evidence and do not qualify the F006/F007 replacement:

```bash
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo doc --locked --workspace --all-features --no-deps
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
env NO_COLOR=true nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && trunk build --release'
```

The canonical workspace test completed in 432.99 seconds. Its 2,836-line log is
`/tmp/geosolve-m82-workspace-tests.canonical.log`, SHA-256
`d134d494a866e39a85304781f35d576cc6a14ff0f0162adcf258ec68193a5c08`.
Locked workspace WASM checking and the pinned Trunk release build also passed; Trunk completed in
136.43 seconds.

The diagnostic command `cargo test --locked --workspace --all-targets --all-features` ran all
ordinary tests without failure and then entered Criterion benchmark execution. It was intentionally
interrupted after 1,628.86 seconds during
`representative_decomposition_solve_diagnostics/cad_like/10000`. This is a harness-scope finding,
not a product failure: `--all-targets` executes benchmark binaries, while the canonical workspace
test passed and the release gate owns bounded benchmark compilation/performance checks. The
1,182-line diagnostic log is `/tmp/geosolve-m82-workspace-tests.dev.log`, SHA-256
`df2946074f4a878948a84e47deb915cff307a450a992665cbeee35bf63f2187b`.

After M82-F003, these focused commands also pass:

```bash
cargo test --locked -p geosolve-sketch m82_f003 -- --nocapture
cargo test --locked -p geosolve-sketch-features --lib \
  tests::m82_curve_offset_work_exhaustion_publishes_nothing_and_never_reuses_output_ids -- --exact
cargo test --locked -p geosolve-constraint-editor \
  --test m82_curve_offset_coordinator \
  m82_f001_computed_offset_preview_cold_evaluates_beside_an_unrelated_fillet -- --exact
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::m82_f004_computed_feature_only_change_reactivates_the_live_offset_collector \
  -- --exact
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::m82_f005_offset_help_describes_native_and_computed_curve_routes -- --exact
```

Results are respectively 2/2, 1/1, 1/1, 1/1 and 1/1.

## Withdrawn post-F005 committed-tree qualification

From clean source `d52104595ee11f9e460e98ea5e26200bb34a5d94`, tree
`0a3bcb066a6a2d5d5d2d99591441035be23d20fe`, this exact command completed with exit 0:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate ran from 2026-08-20 23:17:08 to 23:28:33 AEST. Its 269,138-byte, 3,530-line log is
`/tmp/geosolve-m82-clean-gate.d521045.nix.log`, SHA-256
`b66c277a00861854865440911e769aa5f9e94dbd55114e723b43b3bf46743472`. It passed
formatting/diff hygiene, warnings-denied locked all-feature workspace Clippy, locked all-feature
workspace tests, exact 272-row golden `--require-clean`, M70/M71/M74/M75/M76/M77/M79
native/WASM parity, the demo WASM check, warnings-denied Rustdoc, benchmark compilation, M14 and
M32 performance budgets, the ignored 256-moving-body sparse crossover in 126.49 seconds,
licence/package checks and Trunk 0.21.14 release assembly.

M82-F006/F007 subsequently withdrew this source from product and UAT authority. The record remains
valid only as historical post-F005 evidence and must not be cited for replacement qualification.

## Withdrawn post-F005 immutable candidate and served-byte evidence

Without rebuilding, the replacement gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m82-uat.iOg5Do` and byte-compared before and after freezing. The directory is `0555`;
all seven entries are regular non-symlink files at `0444`. Its C-locale ordered `sha256sum *`
manifest aggregate is
`3e6d15dc04fd190c904559dc540936c4f31921d0e8bb257266dff40a2ed8327e`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
097763e8b1ced1036bed0e5ecd274d5397f0ba22bea5eab5686be4e05172b2f1  geosolve-demo-web-7d599d5013bda062.js
42785f524cb60d34c251a9ec7ae860b0039717ae25f77d75f9c8f919adc5431f  geosolve-demo-web-7d599d5013bda062_bg.wasm
3d9e3c0587be4edaf8cd690d91cdb8705a1eb5ba9a506d3a9a12fe59d47a7c0c  index.html
cd7ea776b4f36425bd0eb23589d9540c5847ba1cad2f3d7aecb6bf49e288685a  styles-4cca540fadc7a849.css
```

| File | Bytes |
| --- | ---: |
| `API_COMPATIBILITY.md` | 28,079 |
| `LICENSE` | 35,148 |
| `THIRD_PARTY_LICENSES.md` | 3,120 |
| `geosolve-demo-web-7d599d5013bda062.js` | 33,750 |
| `geosolve-demo-web-7d599d5013bda062_bg.wasm` | 7,955,136 |
| `index.html` | 31,979 |
| `styles-4cca540fadc7a849.css` | 38,736 |

Temporary service `geosolve-m82-temp-uat.service`, PID `1268826`, first served only that snapshot
at `100.94.63.83:18080`. Proxy-disabled, cache-bypassed identity requests for `/` and all seven
files returned HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected
media type and length, and snapshot-identical bytes; `/` equals `index.html`. Evidence is
`/tmp/geosolve-m82-temp-verify.76WBWb/results.tsv`, SHA-256
`35fa0bb1109d96e97f7107f81ac76292ccd6fbb5cbc10da418d836bb05e6a3dd`.

Only after that ledger passed, the temporary listener was retired and
`geosolve-m82-uat.service`, PID `1272147`, began serving the same immutable directory at
`http://100.94.63.83:8080/`. The same eight checks passed independently; final evidence is
`/tmp/geosolve-m82-final-verify.wU6t8i/results.tsv`, with the same ledger hash because every
asserted path/status/type/length/body hash is identical. M82-F006/F007 subsequently withdrew these
bytes before human UAT; PID `1272147` and the post-F005 listener are retired. The snapshot and
verification records remain historical evidence only.

This evidence-only documentation was a descendant of the withdrawn product source. It did not
replace that source/tree or rebuild its artifact.

## Withdrawn pre-F005 committed-tree qualification

From clean source `7fd31c0137f6979f945e5ab4d320e7adb552c03d`, tree
`c6b6c89cecde30b2b3a7cf057ec61317a38a5634`, this exact command completed with exit 0:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate ran from 2026-08-20 22:35:55 to 22:57:08 AEST. Its 285,006-byte, 3,889-line log is
`/tmp/geosolve-m82-clean-gate.7fd31c0.nix.log`, SHA-256
`7f4ef08a66851c1a117bf091af6bfb49a83abf33face7807e451e9b75ae064cf`. It passed
formatting/diff hygiene, warnings-denied locked all-feature workspace Clippy, locked all-feature
workspace tests, exact 272-row golden `--require-clean`, M70/M71/M74/M75/M76/M77/M79
native/WASM parity, the demo WASM check, warnings-denied Rustdoc, benchmark compilation, M14 and
M32 performance budgets, the ignored 256-moving-body sparse crossover in 116.35 seconds,
licence/package checks and Trunk 0.21.14 release assembly.

## Withdrawn pre-F005 immutable candidate and served-byte evidence

Without rebuilding, the gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m82-uat.I58j21` and byte-compared before and after freezing. The directory is
`0555`; all seven entries are regular non-symlink files at `0444`. Its C-locale ordered
`sha256sum *` manifest aggregate is
`cb07c77de43544be251f97321bba8f978a018078a7b332d3752b39b55dff1a8e`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
097763e8b1ced1036bed0e5ecd274d5397f0ba22bea5eab5686be4e05172b2f1  geosolve-demo-web-7d599d5013bda062.js
42785f524cb60d34c251a9ec7ae860b0039717ae25f77d75f9c8f919adc5431f  geosolve-demo-web-7d599d5013bda062_bg.wasm
8c8608a6520ac907375b562c5f3e7936f278912198d809c14df59fdc38db8072  index.html
cd7ea776b4f36425bd0eb23589d9540c5847ba1cad2f3d7aecb6bf49e288685a  styles-4cca540fadc7a849.css
```

| File | Bytes |
| --- | ---: |
| `API_COMPATIBILITY.md` | 28,079 |
| `LICENSE` | 35,148 |
| `THIRD_PARTY_LICENSES.md` | 3,120 |
| `geosolve-demo-web-7d599d5013bda062.js` | 33,750 |
| `geosolve-demo-web-7d599d5013bda062_bg.wasm` | 7,955,136 |
| `index.html` | 31,900 |
| `styles-4cca540fadc7a849.css` | 38,736 |

Temporary service `geosolve-m82-temp-uat.service`, PID `1184609`, first served only that snapshot
at `100.94.63.83:18080`. Proxy-disabled, cache-bypassed identity requests for `/` and all seven
files returned HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected
media type and length, and snapshot-identical bytes; `/` equals `index.html`. Evidence is
`/tmp/geosolve-m82-temp-verify.QnWhyi/results.tsv`, SHA-256
`1355605506f1a656e8ec883e57bc989727f8c24838172a82d46540d3b94748a6`.

Only after that ledger passed, `geosolve-m82-uat.service`, PID `1188633`, began serving the same
immutable directory at `http://100.94.63.83:8080/`. The same eight checks passed independently;
final evidence is `/tmp/geosolve-m82-final-verify.Jlu7xZ/results.tsv`, with the same ledger hash
because every asserted path/status/type/length/body hash is identical. M82-F005 subsequently
withdrew these bytes before human UAT; both pre-F005 service runs are retired. The snapshot and
verification records remain historical evidence only.

This evidence-only documentation is a descendant of the nominated product source. It does not
replace that source/tree or rebuild its artifact.

## Compatibility guardrails and remaining boundary

- M80 `ProfileOffset` equations, persistence and exact native routing remain unchanged.
- Computed Curve Offset adds no solver residual, sketch variable or persistent generated topology.
- Computed Offset proxy gestures reuse ordinary source control IDs and the existing prepared
  constrained source transaction; they do not make generated geometry a solver variable.
- Curve evaluation remains owned by public sketch APIs; topology is independently certified and
  the web remains equation-free.
- Non-finite, singular, uncertified, self-contacting or topology-changing geometry can never
  publish a success-like result or partial output.
- Failure in computed scene/affordance composition cannot suppress the finite complete native
  accepted scene.
- Computed-on-computed chaining, topology repair, trimming/splitting, loop removal, distance
  reduction and stable generated-edge naming remain explicit non-goals.

The immediate remaining work is focused/collateral and complete clean replacement qualification,
no-rebuild freeze and exact Tailscale verification.
Only then can `docs/M82_UAT.md` become executable against replacement authority. Human approval,
exact GitHub Pages publication and closeout follow. M82 remains active with UAT pending; there is
currently no nominated source, artifact or live M82 listener.
