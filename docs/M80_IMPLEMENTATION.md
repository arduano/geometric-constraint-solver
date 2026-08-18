<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 implementation — native topology-preserving Profile Offset

Status: **implementation and broad pre-nomination qualification complete**. Focused owner,
native/WASM, workspace, formatting, Clippy and unchanged golden qualification pass. No clean
release candidate is yet nominated, frozen, human-accepted or published.

Architecture owner: ADR 0037. Product owner: `docs/M80_GOALS.md`.

## Ownership boundary

- `geosolve-sketch` owns persistent/runtime `ProfileOffset`, its scalar and operand validation,
  residual lowering, explicit traversal/junction/branch state, independent equation/domain/topology
  validation, grouped source diagnostics, history and draft-v5 persistence.
- `geosolve-sketch-topology` authenticates a complete eligible bounded face and exact native edge
  provenance. It remains read-only and does not become a solver or mutation owner.
- `geosolve-sketch-ops` owns deterministic same-family target construction and immutable Profile
  Offset proposals over one authenticated accepted topology/input stamp. It mutates no retained
  session until the coordinator consumes its exact-CAS proposal.
- Topology preservation certifies the selected source/target operand paths and their contours; it
  does not freeze or compare unrelated sketch arrangement geometry. A source polyline span creates
  a standalone native `CurveDefinition::Line` target.
- `geosolve-constraint-editor` owns offset authoring, ordered chain collection, shared hover/click
  ownership, proposal-backed preview lifecycle, stale invalidation and one atomic retained commit.
- `geosolve-demo-web` owns only Modify-menu/panel presentation, platform events, rendering and the
  disposable annotation-placement cache. It contains no offset equation, intersection, branch or
  topology-validity policy.
- `geosolve-sketch-features` and ADR 0031 remain unchanged. M80 is not a computed feature.

## Planned implementation slices

### 1. Domain and wire compatibility — implemented

- [x] Add persistent operand, loop/chain, edge-pair, traversal, junction-provenance, branch and
  terminal-policy types with bounded validation.
- [x] Add one driving-only document dimension and positive scalar; forbid scalar sharing and
  reference mode; make deletion/suppression leave target geometry/connectivity.
- [x] Preserve runtime `DimensionKind: Copy` through a `ProfileOffsetId` arena and compile one
  persistent source into multiple ordered residual blocks.
- [x] Freeze v2-v4 dimensions behind private seven-variant DTOs; reject canonical export with typed
  `UnsupportedM80State`; add omitted-when-empty draft-v5 side-section conversion.
- [x] Prove old canonical/draft bytes are unchanged and workspace-v6/repro-v1 restore the new state
  atomically. Workspace-v6 retains compatible annotation placement; reproduction export omits and
  reproduction import ignores that disposable cache so placement is recomputed.

### 2. Equations and independent validation — implemented

- [x] Reuse ADR 0020 line support rows and add three-row equal-center/signed-radius circular
  support.
- [x] Add tangential-anchor rows for both open terminals and every tangent internal junction.
- [x] Keep both source and target arc endpoint angles active. Add explicit `Preference`-priority
  source Start/End retention rows as the deterministic shared-angle gauge, allowing either hard
  target endpoint driver to propagate without weakening or weighting any hard equation.
- [x] Publish structured row audits and grouped source mapping; add analytic/local-AD versus central
  finite-difference coverage at `1e-6`, `1` and `1e6` scales.
- [x] Validate side/direction, traversal alignment, terminal translation, arc endpoint branch,
  miter turn and tangent alignment independently of solver termination.
- [x] Validate edge/loop count, source-target pairing, connectivity provenance, simplicity,
  non-contact, orientation, hole nesting and unchanged topology before acceptance.

### 3. Deterministic construction and authoring — implemented

- [x] Authenticate a whole eligible native face, including holes, from one exact accepted topology
  snapshot; reject partial, external, Construction, computed and unsupported edges.
- [x] Collect one ordered non-branching open chain, preserving explicit traversal and accepting one
  line or one circular arc.
- [x] Construct same-family target seeds deterministically: offset supports plus persisted miter
  intersections, tangent normal translations and open terminal normal translations.
- [x] Trial all target geometry, ordinary junction constraints, scalar and grouped dimension in one
  cloned retained session; publish only one independently valid exact-CAS transaction/history step.
- [x] Keep preview non-selectable and state-neutral; revoke it on scene/topology/history/import/tool
  changes and preserve all IDs/history on rejection, cancellation or exhaustion.

### 4. Presentation and annotation — implemented

- [x] Add Modify → Offset with the persistent bottom-left operand/Distance/Flip/Apply/Cancel panel,
  process-local valid-distance memory and `0.1 * model_scale` fallback.
- [x] Render exact headless face/chain hover, collection, provisional target and local rejection
  status without browser-owned geometry decisions.
- [x] Add one movable grouped Profile Offset annotation with disposable cache placement, selection,
  editing and safe cache-loss recomputation; retain compatible placement in workspace-v6 only,
  while reproduction copy/load deliberately omits and ignores it.
- [x] Keep the tool active after Apply; explicit close/Cancel returns to Select.

### 5. Qualification, UAT and closeout — pre-nomination mechanics complete

- [x] Pass focused owner tests, native/WASM parity, unchanged historical persistence/golden checks,
  formatting, warnings-denied workspace Clippy and locked all-feature workspace tests.
- [ ] Pass the exact clean release gate from committed source, including Rustdoc, demo WASM and
  Trunk, and record its complete log.
- [ ] Copy the exact gate-produced distribution without rebuilding, freeze it read-only, verify
  every file locally, then keep it running and byte-verified on the shared Tailscale endpoint until
  the supervising human accepts or replaces the candidate.
- [ ] Record exact source/tree/log/manifest/server evidence in this file and `docs/M80_UAT.md`.
- [ ] Receive explicit UAT/scoped-close direction, publish the approved descendant through GitHub
  Pages, verify hosted bytes and only then mark M80 complete across all milestone records.

## Required focused matrix

| Area | Required evidence |
| --- | --- |
| Closed linear | Rectangle and non-axis polygon, outward/inward, source and target edits |
| Closed circular | Circle outward/inward, radius collapse rejection, direction flip |
| Mixed loop | Line/circular-arc miters and tangent joins with traversal/sweep retention |
| Holes | Outer expansion plus hole shrink and inverse; contact/hole-loss barriers |
| Open one-edge | Exact translated line; exact circular arc without antipodal root |
| Open multi-edge | Manual order, both sides, line/arc, miter/tangent, terminal anchors |
| Lifecycle | Delete/suppress association, Undo/Redo, reload, workspace/repro, cache loss |
| Atomicity | Unsupported curves/provenance, stale preview, cancellation, exhaustion, invalid solve |
| Numerics | Finite residuals/Jacobians, scale, rank/DOF, source mapping and audit order |

## Finding ledger

### M80-F001 — computed discarded fragment impersonated a native Offset operand

Owner: presentation-independent Offset target resolution in `geosolve-constraint-editor`.
During the required unsupported-provenance matrix, a computed-Fillet discarded occurrence with a
native source span ID reproduced as a selectable open-chain operand because resolution checked the
semantic span but not the painted occurrence origin. The focused regression
`m80_f001_computed_fillet_fragment_cannot_masquerade_as_a_native_offset_operand` freezes that
failure. Resolution now requires `SceneCurveOrigin::Native`; the computed fragment receives no
hover/click authority and no operand or retained state is created.

### M80-F002 — circular-arc endpoint activation created a shared-angle gauge

Owner: `geosolve-sketch` Profile Offset lowering and explicit solver priority semantics. The first
arc implementation activated only the target endpoint angles. Activating both sides, as required
for bidirectional editing, then exposed an unconstrained common-angle gauge: one or both hard
target endpoint drivers could make the otherwise valid association fail or leave the source unable
to follow. Both source and target angles now remain active, while two structured
`ResidualCategory::Preference` rows retain the source Start/End angles as the deterministic gauge.
They never replace, weight or weaken a hard equation. The regressions
`target_arc_endpoint_constraints_drive_the_free_source_arc_through_profile_offset` and
`one_target_arc_endpoint_driver_propagates_without_a_shared_angle_gauge` cover one and both hard
target drivers, including source-order independence.

### M80-F003 — supporting/interior contacts impersonated endpoint ownership

Owners: `geosolve-sketch-topology` adjacency authentication and `geosolve-sketch` persistent
junction validation. A contact located at the same endpoint coordinate could be accepted even when
it described an unbounded supporting line or an interior neighborhood rather than the exact native
endpoint. Authentication now requires the exact bounded `[0, 1]` domain, winding zero, matching
Start/End neighborhood and bit-exact endpoint scalar, on both topology discovery and document
validation. `supporting_line_contacts_at_endpoint_coordinates_do_not_own_offset_adjacency` and
`supporting_line_contact_cannot_authenticate_a_profile_offset_endpoint_junction` freeze both
boundaries.

### Pre-nomination interaction audit refinements

The final editor/web audit also froze four cross-adapter behaviors without broadening mathematical
scope: a negative Distance entered before operand selection carries transient direction intent;
unsupported and dynamically invalid candidates retain typed unavailable hover/click reasons;
ordered chains render traversal arrows plus Start/End terminals; and tree/keyboard activation uses
the same Offset semantic pick as the pointer without a duplicate canvas click. Focused authoring,
scene and workbench regressions own those presentation-independent contracts.

## Pre-nomination evidence

The implementation has passed the following focused and broad development qualification. These
runs do not replace the required clean-candidate release gate:

```text
cargo test --locked -p geosolve-sketch --test m80_offset
  # 16 passed
cargo test --locked -p geosolve-sketch-topology --test m80_offset_operands
  # 15 passed
cargo test --locked -p geosolve-sketch-ops --test m80_profile_offset
  # 16 passed
cargo test --locked -p geosolve-constraint-editor --lib profile_offset_
  # 7 passed
cargo test --locked -p geosolve-constraint-editor --lib offset_authoring::tests
  # 10 passed
cargo test --locked -p geosolve-constraint-editor --lib
  # 381 passed
cargo test --locked -p geosolve-demo-web --lib
  # 150 passed on the ordinary default test stack
cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity
  # 6 passed natively, including Profile Offset annotation movement/cache-loss parity
nix-shell shell.nix --run 'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity --target wasm32-unknown-unknown'
  # 6 passed, including Profile Offset annotation movement/cache-loss parity
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
./scripts/golden-authoring-scene-oracle.sh --survey
  # all 270 rows PASS
./scripts/golden-authoring-scene-oracle.sh --check
  # exact reviewed oracle match
./scripts/golden-authoring-scene-oracle.sh --require-clean
  # exact reviewed oracle match; no known defects
```

The final source/tree, clean gate log, immutable distribution, manifest and server evidence are
recorded only after exact clean nomination. Historical M1-M79 evidence remains unchanged.

## Known limitations

The exclusions in `docs/M80_GOALS.md` are deliberate scope, not open implementation defects. Any
new defect found while implementing or testing the solver/headless association receives an M80
finding ID, owning-layer regression and replacement-candidate record before closure.
