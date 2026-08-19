<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 implementation — native topology-preserving Profile Offset

Status: **post-nomination amendments implemented and development-qualified**. The first candidate
is withdrawn by `M80-F006` and `M80-F007`; replacement clean release qualification, immutable
Tailscale nomination and human UAT remain pending. GitHub Pages stays on accepted M79.

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
- [x] Define non-branching over the selected span set: allow a selected continuous path through a
  high-valence junction, never auto-absorb unselected incident geometry, and retain typed selected-
  branch, disconnected and closed-path rejection.
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
- [x] Add direct authoring-only shared-distance dragging over the provisional target/grouped
  presentation with exact preview authentication, the common threshold and pointer capture,
  absolute frozen-rail sampling, last-valid invalid/recovery behavior and cancel-to-origin. Pointer
  release remains history-neutral; Apply consumes the exact held patch later.
- [x] Add one movable grouped Profile Offset annotation with disposable cache placement, selection,
  editing and safe cache-loss recomputation; retain compatible placement in workspace-v6 only,
  while reproduction copy/load deliberately omits and ignores it.
- [x] Keep the tool active after Apply; explicit close/Cancel returns to Select.

### 5. Qualification, UAT and closeout — replacement nomination pending

- [x] Pass focused owner tests, native/WASM parity, unchanged historical persistence/golden checks,
  formatting, warnings-denied workspace Clippy and locked all-feature workspace tests.
- [ ] Re-pass the exact clean release gate from committed replacement source, including Rustdoc,
  demo WASM and Trunk, and record its complete log.
- [ ] Copy the exact replacement gate-produced distribution without rebuilding, freeze it
  read-only, verify
  every file locally, then keep it running and byte-verified on the shared Tailscale endpoint until
  the supervising human accepts or replaces the candidate.
- [ ] Record exact replacement source/tree/log/manifest/server evidence in this file and
  `docs/M80_UAT.md`.
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

### M80-F004 — consumed point-edit guidance made fresh topology capture look stale

Owners: retained accepted-state authentication in `geosolve-sketch-topology`, with a thin
`geosolve-constraint-editor` lifecycle regression. After a successful ordinary
`SetPointPosition`, the accepted publication remains current even though its consumed one-shot
point-edit guidance is intentionally absent from the next prepared attempt. Topology capture
incorrectly compared those two attempt payloads and rejected Modify → Offset with
`AcceptedInputMismatch`. Capture and consumption now authenticate
`accepted_state_for_current_input()` while retaining the exact current `prepared_input()` as the
topology/proposal CAS stamp. `successful_point_edit_remains_current_for_fresh_offset_operand_capture`
freezes the owner boundary, and
`m80_f004_profile_offset_after_point_edit_keeps_preview_and_apply_current` proves activation,
selection, preview and Apply through the ordinary coordinator path.

### M80-F005 — Construction supports could remain in a persistent Profile Offset

Owner: central `geosolve-sketch` document validation. Profile Offset operand validation checked
curve family and topology but did not require every retained source and target support to remain
`GeometryRole::Profile`. Direct creation, a later role mutation and draft-v5 restoration could
therefore admit an association containing Construction geometry. Central path validation now
requires Profile role on both sides. Five focused regressions cover Construction source and target
creation, atomic associated source and target role-change rejection with identical document/draft
bytes, and invalid draft-v5 restoration. The pre-fix focused run reproduced all five failures; the
same run passes after the repair. No residual, Jacobian, priority or branch behavior changed.

The existing operations stale-plan fixture now deletes only its newly created Profile Offset
dimension before changing the source role. This deliberately detaches the native target geometry,
so the fixture can still prove that an old prepared edit rejects after a source-role change without
asking an active association to violate the new central Profile-only invariant.

### M80-F006 — global junction degree vetoed a valid selected path

Owners: `geosolve-constraint-editor` chain collection and `geosolve-sketch-ops` operation planning.
At a T-junction, selecting two connected arms reproduced a `BranchingJoin` solely because an
unselected third arm raised the topology endpoint's global degree. Both owners now measure degree
inside the proposed selected span set. The topology index still publishes the truthful global
`Branched { adjacent: 2 }`; the selected pair previews and applies, while adding the third selected
arm remains `BranchingJoin`, an isolated arm remains disconnected and a selected loop remains
closed. Focused editor and operations regressions preserve exact two-edge publication, finite
independent hard validation and unchanged unselected geometry.

### M80-F007 — provisional distance had no direct authoring gesture

Owner: presentation-independent Offset interaction in `geosolve-constraint-editor`, with a thin
`geosolve-demo-web` platform adapter. At the nominated source, provisional curves deliberately had
no ordinary selection identity and Offset pointer-down/move only ran base operand pick/hover.
Consequently the visible target could never acquire a distance gesture. The replacement adds an
authoring-only exact-preview owner with a frozen source/target rail, absolute distance samples, the
shared three-pixel threshold, one captured pointer, last-valid rejection/recovery, current-rerender
continuity and cancel-to-origin. Pointer-up finalizes only the candidate value; Apply remains the
single retained transaction and stays unavailable while the gesture is captured. Seven focused
gesture regressions cover threshold/release/Apply,
invalid recovery and exact rollback, foreign/stale input, unavailable-rail cleanup, finite signed
line/miter/arc/circle rails, full-circle annotation/display-side agreement and stale rendered-hover
revocation; all pass together with the collateral suites below. Hover authenticates the rendered
candidate input before resolving or publishing a grab target, so a superseded frame clears both
collector and editor hover exactly where the same press rejects.

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
  # 21 passed
cargo test --locked -p geosolve-sketch-topology --test m80_offset_operands
  # 16 passed
cargo test --locked -p geosolve-sketch-ops --test m80_profile_offset
  # 17 passed
cargo test --locked -p geosolve-constraint-editor --lib offset_authoring::tests
  # 11 passed
cargo test --locked -p geosolve-constraint-editor --lib m80_f007
  # 7 passed
cargo test --locked -p geosolve-constraint-editor --lib
  # 390 passed
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

Historical M1-M79 evidence remains unchanged.

## Withdrawn first nomination evidence

Exact source `949c3dbde769cb7de41a9fd97ba0a40094bea14a`, tree
`23a6f8df89e16bb1ae3a74ee0bd4d90d2cd9245a`, ran the required clean command with
`GEOSOLVE_ALLOW_DIRTY` removed from the environment from 2026-08-19 02:58:27 through 03:11:36
AEST. The complete gate passed formatting, warnings-denied Clippy and Rustdoc, locked all-feature
workspace tests, the focused native/WASM matrices, unchanged 270/270 golden check/clean authority,
benchmarks, M14/M32 budgets, the 113.70-second 256-body sparse crossover, licence/package checks
and Trunk 0.21.14. The retained 263,727-byte, 3,427-line log is
`/tmp/geosolve-m80-clean-gate.949c3db.log`, SHA-256
`389f590c52fba4bc436c4910056e2610d34d6d0fbf1b10dd4b960d985bd8c962`.

The gate-produced `crates/geosolve-demo-web/dist` was copied without rebuilding to
`/tmp/geosolve-m80-uat.Nnxsu7`. Source and copy matched per file before the directory was frozen
`0555` and all seven regular non-symlink files `0444`. Freeze evidence is retained at
`/tmp/geosolve-m80-freeze-evidence.GWHVEQ`; the C-locale ordered manifest has aggregate SHA-256
`18677a4488848e56d463a90ffe2e2653e34fe6931767d25b63d3dc47b69084d9`:

```text
12861ad65e947547f3ac9b3566717cd228bb7c7177c7138b6340b6005b624d88  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
2511e6d4fe9333fc3a1614a107338c67ef0d372346b9ac6d81163cfcb543cd7f  geosolve-demo-web-29072a7472852a87.js
19d0d3959e6c497d0a61a063d949e9dec23146900800ae41addee862b50933cf  geosolve-demo-web-29072a7472852a87_bg.wasm
645e3e393ebee1bd7d30f2e90587561fdd0d92f7f17962d11394e267773ada28  index.html
88eba0838350e15dada19778c71f53d740558a7ae24d574c1f8661d63b55a59a  styles-a142ac484ea610ba.css
```

M80 first ran on temporary Tailscale port `18080` under PID `1940172`. Root and every file passed
proxy-disabled, cache-bypassed identity requests before M79 PID `40049` was retired. Final PID
`1946736` now serves the unchanged snapshot at `http://100.94.63.83:8080/`. Both temporary and
final eight-request ledgers have SHA-256
`af8eb2f377450feaa7a12baef23f8d06ff034c739421bd80cdeaf4e9ad7c88fa`; evidence lives at
`/tmp/geosolve-m80-temp-verify.f6vjdf` and `/tmp/geosolve-m80-final-verify.crUf79`. Every response
is HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected media type and
length, snapshot-identical body, root equality and the same fetched aggregate. The temporary unit
was retired only after final verification.

This was mechanical nomination, not human acceptance. `M80-F006` and `M80-F007` withdrew this
snapshot before UAT acceptance. It remains historical while the replacement is qualified; GitHub
Pages remains on accepted M79.

## Known limitations

The exclusions in `docs/M80_GOALS.md` are deliberate scope, not open implementation defects. Any
new defect found while implementing or testing the solver/headless association receives an M80
finding ID, owning-layer regression and replacement-candidate record before closure.
