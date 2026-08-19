<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M80 implementation — native topology-preserving Profile Offset

Status: **amended implementation and development qualification complete; the clean replacement
gate, frozen nomination, human UAT, GitHub Pages publication and closure remain pending**. The
completed Profile Offset implementation and both prior nomination records remain valid historical
evidence, but the `b83dad2` snapshot is withdrawn from acceptance by this amendment and is no
longer served. Pages stays on accepted M79.

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
- ADR 0031 persistence, equations and default computed Apply remain unchanged. The transient
  `geosolve-sketch-features::ComputedCircularArc` preview DTO now carries its independently
  validated tangent orientations so native publication never infers them from a later seed or
  solved result. M80 itself is not a computed feature.
- Ordinary **Apply computed** remains the unchanged/default ADR 0031 computed-feature path. The
  amended explicit **Apply native profile** action is a headless authoring/coordinator transaction that produces only
  ordinary `geosolve-sketch` geometry and definitions. Offset continues to reject all computed
  output and consumes the resulting native line-arc-line profile through its existing path.
- `geosolve-sketch` owns exact native line-line eligibility, deterministic document construction,
  ordinary tangency/Radius definitions and complete candidate validation. The headless editor owns
  translation of one authenticated current Fillet preview into that request; the retained
  coordinator owns the cloned-session trial, exact publication token and one-step history boundary.

## Implementation slices

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

### 5. Explicit native line-line Fillet publication — implemented

- [x] Keep ordinary **Apply computed** unchanged and default. Add a separate explicit **Apply
  native profile** terminal action only when the exact current authoring preview contains one eligible
  line-line corner; publish a typed disabled reason for every unavailable case.
- [x] Authenticate before allocation two distinct standalone, untrimmed native Profile `Line`
  parents sharing exactly one persistent endpoint, exactly two direct sharp-point curve owners and
  no other point-based dependent. Reject an existing Profile Offset or persisted computed-feature
  claim on either source, plus polyline-owned, dependent/high-valence, line-circle/other-curve,
  multiple/batched and already-published computed-Fillet conversion cases.
- [x] Build one deterministic immutable native proposal that preserves the parent lines and their
  non-corner ends where semantically possible, physically shortens both corner ends, inserts one
  ordinary `CircularArc`, adds two exact endpoint `LineCurveTangency` definitions and adds one
  ordinary driving Radius dimension.
- [x] Authenticate explicit first/second parent and retained-endpoint order, both normal-side
  choices, arc Start/End order and sweep, and both tangent orientations from the held preview. The
  ordinary result materializes those choices into line endpoints, arc/contact mapping, sweep and
  persistent tangent orientations; it retains no Fillet-specific provenance and never reconstructs
  a branch from post-solve coordinates. Canonical line-parent ordering co-permutes the computed
  arc contacts and tangent orientations, so a reverse manual pick cannot attach otherwise-correct
  preview branch data to the wrong canonical parent.
- [x] Trial the whole edit in one cloned retained session and independently require finite regular
  geometry, exact endpoint incidence/tangencies/radius, matching branch state and normalized hard
  residual at most `1e-9`. Publish only one exact transaction/history step; invalid, stale,
  ambiguous or exhausted attempts retain exact scene/document/history/transcript/allocator state.
- [x] Prove exact one-step Undo restores the original shared corner while allocator high-water
  remains monotonic, Redo restores the same native identities/branch state, and the resulting
  ordinary line-arc-line path feeds the existing Profile Offset proposal and validator unchanged.
  Computed Fillet fragments
  remain rejected Offset provenance; create no `FilletSet` and leave the feature sidecar unchanged.
- [x] Add the smallest owner regressions at the sketch/operation/editor/coordinator boundaries plus
  thin native/WASM/presentation parity where authority crosses an adapter. No residual changed or
  was added; existing `LineCurveTangency` and Radius audit/Jacobian coverage remains authoritative.
- [x] Add and review one native-profile Fillet authoring family plus deterministic transforms in
  the stable golden matrix; never bless new or changed rows without row-by-row review.

### 6. Qualification, UAT and closeout — amended development qualification complete

- [x] Pass the pre-amendment focused owner tests, native/WASM parity, unchanged historical
  persistence/golden checks, formatting, warnings-denied workspace Clippy and locked all-feature
  workspace tests.
- [x] Pass the pre-amendment exact clean release gate from committed `b83dad2` source, including
  Rustdoc, demo WASM and Trunk, and record its complete log.
- [x] Copy that exact pre-amendment gate-produced distribution without rebuilding, freeze it
  read-only and verify every file locally. It remained running and byte-verified through its
  pre-amendment nomination, then retired when the scope amendment withdrew it from acceptance.
- [x] Record exact pre-amendment source/tree/log/manifest/server evidence in this file and
  `docs/M80_UAT.md`; retain it as superseded evidence rather than silently overwriting it.
- [x] Pass all focused amended owner/collateral tests, formatting, warnings-denied workspace
  Clippy, locked all-feature workspace tests, relevant native/WASM/golden/persistence parity and
  the reviewed 271-row clean oracle from the development worktree.
- [ ] Pass warnings-denied Rustdoc, Trunk and the exact complete clean release gate from committed
  replacement source.
- [ ] Copy the exact amended gate-produced distribution without rebuilding, freeze and byte-verify
  it on a temporary Tailscale port, and replace `:8080` only after that temporary verification
  passes. Record the new source/tree/log/manifest/server authority here and in `docs/M80_UAT.md`.
- [ ] Receive explicit amended-candidate UAT/scoped-close direction, publish the approved
  descendant through GitHub Pages, verify hosted bytes and only then mark M80 complete across all
  milestone records.

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
| Native Fillet | One exact eligible line-line corner; forward/reverse manual line-pick order canonicalizes parents while keeping arc contacts/tangent orientations aligned; ordinary arc/tangencies/Radius; explicit branches; one-step Undo/Redo; invalid/stale/exhausted atomicity; unchanged Offset consumption |

## Scope amendment — explicit native Fillet publication

This is an approved product-scope amendment, not a reproduced defect and therefore receives no
finding ID. The computed Fillet remains unchanged and default. The explicit **Apply native
profile** action materializes exactly one eligible standalone line-line corner into persistent
line-arc-line Profile topology under the implemented slice above. Existing computed output remains unavailable to
Offset; the materialized ordinary topology reaches Offset through the already implemented native
path.

The amendment supersedes the `b83dad2` frozen candidate before human acceptance. Its implementation,
clean gate, immutable snapshot and historical served-byte ledgers remain truthful pre-amendment
evidence. The snapshot is no longer served. No UAT row, Pages publication or milestone closure may
use those bytes.

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

### M80-F008 — one self-adjacent span masqueraded as an open chain

Owners: `geosolve-constraint-editor` chain collection and `geosolve-sketch-ops` operation planning.
The multi-span closure check did not cover one bounded span whose Start and End were explicitly
joined by ordinary endpoint continuity. Such a span is already topologically closed and therefore
cannot be an open-chain operand, regardless of its non-periodic curve family. Both owners now
reject that self-adjacency as `WouldCloseChain`/`ProfileOffsetClosedChain` before proposal or
allocation. Focused editor and operation regressions use a regular near-full circular arc with an
authenticated G0 Start/End join and prove exact state neutrality.

### M80-F009 — delayed terminal effects could consume a newer distance drag

Owner: retained Offset gesture/effect authentication in `geosolve-constraint-editor`. Consecutive
distance drags over one unchanged provisional candidate share the same prepared input and proposed
commit, and may request the same distance. Those proposal fields therefore could not distinguish a
delayed preview, release or restore effect from the current capture. Offset distance gestures now
receive monotonic epochs carried through every Preview/Finish/Restore effect and authenticated by
the coordinator before any state or held patch changes. The regression
`m80_f009_stale_terminal_effect_cannot_consume_a_new_distance_gesture` replays all three effects
from drag one during drag two, proves exact state/payload/pointer retention, then applies drag two's
own restore and proves exact pointer-down rollback. No solver equation, residual, branch or
persistence representation changed.

### M80-F010 — native publication used retained seeds instead of the accepted preview

Owner: retained sketch publication and exact feature-preview authority across
`geosolve-sketch` and `geosolve-constraint-editor`. An independently reproduced corner whose
retained line coordinates pointed opposite its current accepted solution could advertise native
availability, then reject Apply at `contact.tangent_orientation`; availability had staged only a
structural edit while Apply reconstructed and solved different work. Native Fillet preparation now
preserves every pre-existing retained point bit, gives only newly allocated contact points a
branch-valid retained seed, and solves the identical edit from the authenticated accepted document.
The coordinator retains the complete non-cloneable `PreparedSketchPatch`, computed-scene parity,
expected identities and history checkpoint beside the exact preview token. Availability reads that
held result and Apply only consumes it through compare-and-swap; it never reconstructs or solves a
second candidate. The exact accepted-versus-retained regression, stale/cached-unavailable tests,
Undo/Redo checks and ordinary Offset chain/face tests pass with independently validated normalized
hard residual at most `1e-9`.

### M80-F011 — reverse manual line pick misaligned canonical parents and preview branches

Owner: `geosolve-sketch-features` computed-Fillet evaluation. The native proposal canonicalized
the two line parents, but a reverse manual pick left the computed arc contacts and tangent
orientations in the pre-canonical order. The resulting preview could therefore pair valid branch
metadata with the wrong parent. Evaluation now swaps both contacts and both tangent orientations
whenever it swaps the parents; it does not reorder the computed arc itself or infer a new branch.
`reverse_line_pick_order_keeps_canonical_parents_and_preview_contacts_aligned` freezes the owner
contract. The editor regression
`grouped_adjacent_authoring_is_canonical_and_keeps_corner_branches_on_radius_edit` now requires
native requests from both forward and reverse manual line-pick orders. No residual equation,
Jacobian, priority or persistence format changed.

### M80-F012 — radius rollback advertised a native patch whose computed reservation was stale

Owner: retained feature-preview authority in `geosolve-constraint-editor`. Restoring the exact
pointer-down Fillet preview after a radius sample correctly retained its original single-owner
native sketch patch, but the discarded sample had advanced the live computed-evaluation allocator.
Native availability only saw a prepared cache entry, while Apply additionally authenticated the
older allocator stamp and rejected the visibly restored candidate as
`NativeFilletPreviewMismatch`.

Rollback now keeps the exact same non-cloneable sketch patch and renews only its computed-scene
parity, allocator reservation and history checkpoint against the current monotonic evaluation
high-water. It neither re-solves nor reconstructs the native edit. The regression
`native_fillet_preparation_tracks_radius_refresh_and_exact_origin_restore` proves the patch's heap
identity is unchanged, the discarded evaluation revision is not reused, availability and Apply
agree, and the restored native corner publishes atomically. The exact-held-preview regression also
compares the rendered centre, contacts, radius, Start/End angles and sweep bit-for-bit with the
accepted staged native arc. No residual equation, Jacobian, branch rule or persistence format
changed.

### M80-F013 — rejected preview replacement consumed the live computed revision

Owner: retained feature-preview preparation in `geosolve-constraint-editor`. A valid eligible
native preview was held while a replacement grouped Fillet preview was attempted with crossing
shared-span trims. Computed evaluation reserved the next allocator revision before rejecting the
replacement, but the coordinator retained the old visible preview and native patch. Availability
therefore still reported that patch prepared while Apply rejected its now-stale computed stamp.

Preview preparation and refresh now evaluate against a cloned allocator and publish its new
high-water only after the replacement completes and is authenticated as Current. The exact
regression
`rejected_feature_preview_replacement_keeps_last_valid_native_apply_authoritative` proves the
rejected replacement consumes no live revision, the old visible preview/cache remain exact,
availability and Apply agree, and the native edit still publishes atomically. No residual,
branch, persistence or durable allocator rule changed. The existing preview-token exhaustion
regression now also requires that a successfully evaluated but unpublishable candidate cannot
advance the live computed allocator.

### M80-F014 — dependent and high-valence corners leaked document internals into disabled copy

Owners: native eligibility in `geosolve-sketch`, retained error presentation in
`geosolve-constraint-editor`, and the thin demo presentation adapter. Trial removal of a sharp
point with another point-based dependent or a third incident line correctly returned
`ObjectInUse`, but stringification exposed the persistent ID and generic document-error framing as
the action's disabled reason.

The domain now maps `ObjectInUse` for the authenticated sharp corner to the stable reason
`shared corner must be owned only by the two selected source lines`. The coordinator removes only
the native-Fillet `InvalidField` wrapper, preserving every other typed error. Exact sketch,
coordinator and demo regressions require that sentence and prove the failed attempt is mutation-
and allocation-neutral. No eligibility case was broadened.

### M80-F015 — native preparation completed an uncontrolled validation before bounded work began

Owners: native document preparation in `geosolve-sketch` and retained staging in
`geosolve-constraint-editor`. The coordinator previously performed synchronous preparation,
including a complete trial mutation and document validation, before installing the bounded
prepared job. The later zero-budget test could therefore stop only after all meaningful
preparation work had already run.

`prepare_native_line_fillet_geometry_controlled` now owns the trial under one
`OperationController`: it checkpoints before validation, defers redundant mutation-time full
validation, performs final validation through that same controller and returns a typed incomplete
outcome without patch or allocator publication. The unlimited convenience API preserves its
public behavior. The coordinator uses the controlled boundary and maps incomplete preparation to
`NativeFilletWorkStopped`. `controlled_native_fillet_preparation_exhaustion_is_state_neutral`
proves exhaustion during preparation as well as during later prepared execution. No solver row or
validation predicate changed.

### Pre-nomination interaction audit refinements

The final editor/web audit also froze four cross-adapter behaviors without broadening mathematical
scope: a negative Distance entered before operand selection carries transient direction intent;
unsupported and dynamically invalid candidates retain typed unavailable hover/click reasons;
ordered chains render traversal arrows plus Start/End terminals; and tree/keyboard activation uses
the same Offset semantic pick as the pointer without a duplicate canvas click. Focused authoring,
scene and workbench regressions own those presentation-independent contracts.

## Pre-amendment development evidence

The implementation has passed the following focused and broad development qualification. These
runs remain evidence for the unchanged Profile Offset implementation, but they do not qualify the
new Apply native profile path or replace the required amended clean-candidate release gate:

```text
cargo test --locked -p geosolve-sketch --test m80_offset
  # 21 passed
cargo test --locked -p geosolve-sketch-topology --test m80_offset_operands
  # 16 passed
cargo test --locked -p geosolve-sketch-ops --test m80_profile_offset
  # 18 passed
cargo test --locked -p geosolve-constraint-editor --lib offset_authoring::tests
  # 12 passed
cargo test --locked -p geosolve-constraint-editor --lib m80_f007
  # 7 passed
cargo test --locked -p geosolve-constraint-editor --lib m80_f009
  # 1 passed
cargo test --locked -p geosolve-constraint-editor --lib
  # 392 passed
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

## Amended development qualification evidence

The final amended development worktree, including `M80-F010`-`M80-F015`, passed the following
commands. These are development evidence only; warnings-denied Rustdoc, Trunk and the complete
clean committed-source release gate remain pending until the replacement commits below are
created:

```text
cargo test --locked -p geosolve-sketch --test m80_native_fillet
  # 7 passed
cargo test --locked -p geosolve-sketch-topology --test m80_native_fillet
  # 1 passed
cargo test --locked -p geosolve-sketch-ops --test m80_native_fillet
  # 2 passed
cargo test --locked -p geosolve-constraint-editor --test m80_native_fillet_authority
  # 1 passed
cargo test --locked -p geosolve-constraint-editor --lib native_fillet
  # 9 passed
cargo test --locked -p geosolve-constraint-editor --lib feature_authoring::tests::
  # 16 passed, including forward/reverse authenticated native requests before and after Radius edit
cargo test --locked -p geosolve-sketch-features --lib
  # 46 passed
cargo test --locked -p geosolve-constraint-editor --lib
  # 403 passed
cargo test --locked -p geosolve-demo-web --lib
  # 154 passed, including three native-action/presentation contracts
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
./scripts/golden-authoring-scene-oracle.sh --survey
  # all 271 rows PASS
./scripts/golden-authoring-scene-oracle.sh --check
  # exact reviewed oracle match
./scripts/golden-authoring-scene-oracle.sh --require-clean
  # exact reviewed oracle match; no known defects
```

Focused `M80-F013` and user-facing `M80-F014` tests also passed individually. The nine retained
native-Fillet filter tests, separately named F013 regression and accepted-versus-retained
integration test total eleven retained/coordinator authority regressions. The stable golden bytes
remain unchanged at 271 rows; these isolated findings did not add a systemic golden dimension.

## Withdrawn pre-amendment replacement nomination evidence

Exact product source `b83dad2b18cdfbb241fc012337ac5dbfa7234a9a`, tree
`440d66ef07b7df963164e69ebed4b75509f56bd6`, ran the complete clean gate inside the repository's
pinned `shell.nix` from 2026-08-19 13:38:33 through 13:50:00 AEST. The 262,051-byte, 3,416-line log
`/tmp/geosolve-m80-clean-gate.b83dad2.nix.log` has SHA-256
`3e44403e3f2038467aa0c06193030feb6c099cd53634b315a8308ca111113fa0`. It passes formatting,
warnings-denied workspace Clippy and Rustdoc, locked all-feature workspace tests, unchanged
270/270 golden `--require-clean` authority, all seven native/WASM parity suites, demo WASM,
benchmark compilation, M14/M32 budgets, the 120.97-second 256-body sparse crossover,
licence/package checks and the release Trunk assembly.

An earlier direct-shell invocation reached the matching 270-row oracle and then stopped before its
first WASM test because `wasm-bindgen-test-runner` was absent from that ambient `PATH`; its retained
log is `/tmp/geosolve-m80-clean-gate.b83dad2.log`. The WASM binary never executed, the source tree
remained clean and unchanged, and the pinned-Nix gate above is the sole passing release claim.

Without rebuilding, the exact gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m80-uat.hggNdd`, byte-compared before and after freezing the directory `0555` and all
seven regular non-symlink files `0444`. Freeze evidence is
`/tmp/geosolve-m80-freeze-evidence.1S2bfA`; the C-locale ordered `sha256sum *` manifest aggregate is
`d8d740fb852e793925ce4e54e8777a225b68ea5cfa39b2f36060bd3566938e37`:

```text
12861ad65e947547f3ac9b3566717cd228bb7c7177c7138b6340b6005b624d88  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
7edcf614931091f70fcb718e7a5653168f47cf4fa8702f240281ff13bfcea4aa  geosolve-demo-web-ffbd27c03aa5ac90.js
23dc7f48d992f73d6714cefd22757f2cfb4240190751e78c56b7515cfeced2af  geosolve-demo-web-ffbd27c03aa5ac90_bg.wasm
438bd2732d52b23388be0fba1b7eaa061f1819d363e1fb911cf14197c6d711ea  index.html
696181924c67ef61038f01c91c5c091eb7686160a24e10b833f22f9b13eabd38  styles-d9987f1e32f4927.css
```

Temporary service PID `3831926` first served only that snapshot on `100.94.63.83:18080`. Root plus
all seven assets passed proxy-disabled, cache-bypassed identity requests before former PID
`1946736` was retired. Final service `geosolve-m80-uat.service`, PID `3837538`, served the same
snapshot at the pre-amendment nomination checkpoint on `http://100.94.63.83:8080/`; the temporary
service was retained until the final ledger passed and is now stopped. Temporary evidence
`/tmp/geosolve-m80-temp-verify.mGxuDM/results.tsv`
has SHA-256 `1628a6c2e87ab519351598371712ace67584616d1573f510c80fbe48f3cd9bea`;
final historical evidence `/tmp/geosolve-m80-final-verify.M0ThFH/results.tsv` has SHA-256
`03e93b8fcd4d53231d7e3bafd95c6b4315ba2a12515bb8a4f763a91abc8c0b28`. Every request returned
HTTP 200 from `100.94.63.83` with zero redirects, no `Location` or `Content-Encoding`, exact media
type and length, snapshot-identical bytes and the same manifest aggregate; `/` equals
`index.html`. GitHub Pages remains on accepted M79. The native-Fillet scope amendment withdraws
this candidate from acceptance even though its recorded gate and bytes remain valid. The recorded
PID `3837538` has exited and no `geosolve-m80-uat.service` exists; the snapshot is not currently
served and may not receive final human UAT acceptance.

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
`1946736` served the unchanged snapshot at `http://100.94.63.83:8080/` until the replacement passed
temporary verification. Both temporary and final eight-request ledgers have SHA-256
`af8eb2f377450feaa7a12baef23f8d06ff034c739421bd80cdeaf4e9ad7c88fa`; evidence lives at
`/tmp/geosolve-m80-temp-verify.f6vjdf` and `/tmp/geosolve-m80-final-verify.crUf79`. Every response
is HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected media type and
length, snapshot-identical body, root equality and the same fetched aggregate. The temporary unit
was retired only after final verification.

This was mechanical nomination, not human acceptance. `M80-F006` through `M80-F009` withdrew this
snapshot before UAT acceptance. It remains historical and is no longer served; GitHub Pages
remains on accepted M79.

## Known limitations

The exclusions in `docs/M80_GOALS.md` are deliberate scope, not open implementation defects. Any
new defect found while implementing or testing the solver/headless association receives an M80
finding ID, owning-layer regression and replacement-candidate record before closure. Native
Fillet publication is implemented within its deliberate boundary: M80 excludes polyline-owned,
line-circle/other-curve, batch, dependent/high-valence and already-published computed-Fillet
conversion cases.
