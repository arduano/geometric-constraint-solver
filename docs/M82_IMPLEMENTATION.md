<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M82 implementation — certified computed all-family Curve Offset

Status: **implementation complete; clean committed-tree release qualification, immutable
Tailscale nomination and human UAT pending**. The implementation and focused/broad development
qualification pass. This is not an acceptance or closure claim. Product scope is owned by
`docs/M82_GOALS.md`; architecture is owned by ADR 0038.

Activation baseline: M81 closeout commit
`e3cbb8f2ae2800181545bb3405704bdcc3ff46a6`.

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
(406/406) and the final demo library (159/159), including exact M82-F001 coverage and live Offset
collector refresh after computed-feature-only changes. The clean-gate rerun remains pending below.

### Phase 4 — strict persistence compatibility

- [x] Add private computed-feature version 2 for Curve Offset intent.
- [x] Continue emitting feature version 1 byte-for-byte for empty and Fillet-only state.
- [x] Round-trip v2 through unchanged workspace v6 and reproduction v1; reject malformed or
  unsupported state atomically and never serialize generated patches, certificates or output IDs.

Strict v2 records only feature intent and allocator authority. Existing v1 fixtures and output
bytes remain unchanged whenever no Curve Offset exists. Workspace and `GEOSOLVE_REPRO_V1`
round-trips regenerate output from accepted sketch plus feature intent.

### Phase 5 — regression, review and release qualification

- [x] Add focused owner regressions for every curve family, analytic exactness, mixed chains,
  faces/holes, spline adjacency, singularity, self-contact, topology barriers, bounded work,
  lifecycle, routing and persistence.
- [x] Add and independently review one justified systemic golden row for the new public Curve
  Offset authoring family.
- [x] Run focused owner suites, golden survey/check/require-clean, native/WASM parity, demo adapter,
  format/diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature workspace tests
  and a pinned Trunk release build during development qualification.
- [ ] Run the complete release gate from the clean nominated commit and retain its exact log/hash.
- [ ] Freeze the gate-produced distribution without rebuilding, byte-verify it over Tailscale and
  bind `docs/M82_UAT.md` to that exact source/tree/artifact.

The reviewed golden inventory increases from 271 to 272 `PASS` rows by exactly
`feature.curve-offset.authoring.general-open-chain`, input fingerprint
`input-86b0e748c84c8899`. The fixture SHA-256 is
`a0d53753e5597dd950024a8108b12f92f325b1896766a5c601220c64970ee123`.
Golden `--survey`, `--check` and `--require-clean` passed; a later post-cleanup `--check` passed in
78.54 seconds without changing authority.

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

## Development qualification record

The following commands actually passed on the implemented tree before clean nomination:

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
```

Results are respectively 2/2, 1/1, 1/1 and 1/1. The complete clean release gate will requalify
the final committed tree before any UAT nomination.

## Compatibility guardrails and remaining boundary

- M80 `ProfileOffset` equations, persistence and exact native routing remain unchanged.
- Computed Curve Offset adds no solver residual, sketch variable or persistent generated topology.
- Curve evaluation remains owned by public sketch APIs; topology is independently certified and
  the web remains equation-free.
- Non-finite, singular, uncertified, self-contacting or topology-changing geometry can never
  publish a success-like result or partial output.
- Computed-on-computed chaining, topology repair, trimming/splitting, loop removal, distance
  reduction and stable generated-edge naming remain explicit non-goals.

The only remaining nomination work is the clean committed-tree release gate, no-rebuild immutable
artifact freeze, exact Tailscale byte verification and the human scorecard in `docs/M82_UAT.md`.
GitHub Pages publication and M82 closure remain blocked on explicit supervising-human acceptance.
