<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M44 implementation record: host-state workbench integration

Status: complete (2026-07-27). Focused native/WASM/browser M44 qualification passes. The
supervising user explicitly removed the costly legacy full-M14 carry-forward run from M45
preparation; its incomplete runs remain non-passing historical evidence. M45 later closed as
a cleanup investigation without human approval, and the intended review moved to M53. M43
remains complete.

## Requirements

- M44 has six workbench outcomes, now checked in `PLAN.md` after focused
  qualification: construction/profile controls;
  activation/suppression distinct from dimension mode; constrained parameter inputs,
  bindings and output proposals; external-reference status and explicit rebinding;
  concurrent design/attempt/accepted revision display; and browser proof of atomic
  batch updates, stale/missing input, and accepted-state retention (`PLAN.md:1938-1953`).
- The gate requires objective qualification of every host-state workflow before M45 and
  preserves an equation-free, callback-free browser (`PLAN.md:1952-1953`; `ACCEPTANCE.md:765-769`).
- The review originally prepared for M45 covers role conversion, suppression/reactivation,
  a shared parameter, invalid-parameter recovery, stale/missing/valid external recovery,
  and retained unsolved design over accepted geometry. M45 preserved those points but did
  not perform UAT; M53 now owns the post-cleanup review. Revisions, digests, and atomicity
  remain directly automated (`docs/SCENARIOS.md:1339-1345`).
- The editor is the deterministic interaction-policy consumer of public sketch APIs;
  the web layer may only map platform inputs, render returned DTOs, and apply typed
  effects. It owns no equations, host expression system, callbacks, or parallel
  interaction state (`docs/adr/0029-headless-constraint-editor-state-machine.md:21-63`).
- Retained design, attempt, and accepted state are distinct identities. Only accepted
  state supplies solved geometry/audit/profile, and every result carries immutable
  input stamp members, including parameter, external, and activation revision/digest
  evidence (`docs/adr/0025-retained-design-attempt-and-accepted-state.md:22-100`).
- Host formula/configuration/PDM/projection/rebinding policy remains host-side. One
  attempt receives prebuilt immutable parameter and external inputs; no solve,
  validation, diagnostic, profile, or browser path may call a host resolver
  (`docs/adr/0026-immutable-host-inputs-and-external-snapshots.md:19-56,97-111`).

## Historical pre-implementation inventory and seams

The table below records how M44 was originally implemented and qualified. Its E2E and
source-scan slices are not future guidance: cleanup M47 replaced retained M44 claims with
direct presentation/capture tests and deleted the broad fixture and `e2e/m44.mjs`.

### Checkbox and gate evidence/gap table

| M44 checkbox / gate | Reuse mandated by existing M41-M43 contract | Presentation gap found at the time | Historical M44 slice |
| --- | --- | --- | --- |
| Construction styling and profile participation | `SketchDocument::geometry_role`/`set_geometry_role` provide the closed `Profile`/`Construction` role and guarantee no geometric/discrete mutation (`document.rs:2351-2380`); M41 keeps construction lowerable but default-profile-ineligible (`docs/M41_IMPLEMENTATION.md:61-70`). | `EditorScene` only supplies accepted curve polylines; `scene::svg_markup` emits one `.wb-curve` class, and `panels::tree_markup` has no role/profile row (`workbench/scene.rs:18-66`; `workbench/panels.rs:70-140`; `styles.css:275-294`). | Extend public scene/panel DTO consumption with role/activity/profile-result metadata; role edit is `DocumentEdit`/session transaction, never an SVG inference. |
| Activation/suppression distinct from dimension mode | `EffectiveActivity::{elements,reason,is_active}` exposes the ordered M41 closure and reason (`document.rs:1367-1428`); coordinator already has independent `Suppress`/`Unsuppress` and `SetDimensionMode` actions plus `set_selected_suppressed`/`set_dimension_mode` (`coordinator.rs:125-138,659-709,890-988`). | Workbench offers dimension creation mode only; it neither renders nor invokes existing suppression/mode-change actions (`index.html:73-79`; `workbench/mod.rs:426-506,605-639`). It also has no effective-activity reason panel. | Add separate selected-source suppress/reactivate and selected-dimension mode controls; render effective reason from session-derived M41 state. Do not make construction a suppression alias. |
| Parameters, bindings, output proposals | Public `parameters`, `parameter_bindings`, `parameter_outputs` declarations and `ParameterBatch`/`DocumentParameterOutputProposal` are available (`document.rs:2670-2680,2765-2873`; `document_session.rs:1355-1369`; `lib.rs:101-131`). `update_parameter_batch` accepts one complete batch and stamps/retries it (`document_session.rs:3531-3596`). | Workbench constructs `RetainedSketchDocumentSession::new` with empty inputs and has no parameter panel, batch control, or proposal rendering (`workbench/mod.rs:104-118`; `index.html:61-80`). Existing `scene.rs` reads raw driving target scalars for dimension labels, so it must not become a parameter catalogue (`scene.rs:85-120`). | Use only document-declared parameter/binding/output rows. A fixture builds a complete immutable `ParameterBatch`, calls the public batch update once, and renders proposal identity/unit/value/provenance from accepted state. |
| External tree/status/rebind | `external_bindings`/`external_binding` and `rebind_external_binding` are public declared-local APIs (`document.rs:2682-2757`); session exposes exact retained set and `update_external_snapshot_set` (`document_session.rs:3123-3127,3599-3668`). Attempt failures expose M41 activity plus structured external error (`document_session.rs:1202-1244`). | Tree covers only point/curve/constraint/dimension and `selection_item` recognizes only those kinds (`panels.rs:70-120`; `workbench/mod.rs:702-713`); no snapshot or rebind UI exists. | Add a non-selectable external-reference tree/panel row per binding, use static typed snapshots, display declared kind/topology plus attempted/accepted stamp/status, and submit an explicit document rebind transaction. |
| Design/attempt/accepted revisions together | `design_identity`, `last_attempt`, `accepted_state`, input stamps, output proposals, and accepted provenance are public (`document_session.rs:1103-1200,1249-1369,3106-3154`). | Existing workbench reduces lifecycle to one status badge (`workbench/mod.rs:547-551,751-758`); persistence has revision counters but no rendered attempt/input stamps (`workbench/persistence.rs:20-50`). | One lifecycle/evidence panel renders current design identity, latest attempt identity+input stamp/failure, and accepted identity+input stamp concurrently; render geometry/audit only from accepted state as current code already does (`workbench/mod.rs:508-543`). |
| Atomic batch/stale/missing/retention browser proof | M42 batch updates are whole-batch operations; M43 snapshot updates publish the new retained set only on acceptance and leave failure evidence inspectable (`document_session.rs:3540-3596,3599-3668`). Existing M40 browser fixture uses fresh local-storage resets and checks accepted-scene retention (`e2e/m40.mjs:145-156,452-487`). | No M41-M43 fixture factory/input switch exists in the workbench. Local-storage `WorkspaceSnapshot` persists only v1 JSON design/accepted documents and revision high-water values, not M41-M43 draft input envelopes (`workbench/persistence.rs:11-64`). | Add an in-memory, deterministic M44 demo fixture selector/state only; it materializes public values before each public call and keeps fixture state outside `WorkspaceSnapshot`/canonical sketch. Add E2E DOM assertions of atomicity and prior accepted evidence. |
| Gate: no browser equations or host callback | Existing M40 E2E already source-checks adapter/scene/panel forbidden policy/equation symbols and drives a fresh desktop profile (`crates/geosolve-demo-web/e2e/m40.mjs:21-44,234-259`). | Extend, rather than weaken, static boundary checks to M44 host-input paths. | Add explicit forbidden host resolver/callback checks and M44 coverage IDs in a new/focused E2E suite. |

### Proposed minimal architecture boundary

`geosolve-sketch` remains the only owner of canonical document declarations, effective
activity, parameter/external validation, lowering, residuals, lifecycle identity, stamps,
and atomic publication. `geosolve-constraint-editor` remains the only owner of deterministic
editing policy and typed effects. `geosolve-demo-web` renders public editor/session DTOs,
turns DOM events into editor inputs, and supplies deterministic *demo fixture values* as
already-materialized immutable `ParameterBatch`/`ExternalSnapshotSet` inputs. A fixture is
not a host callback, resolver, formula evaluator, or canonical sketch field. Arbitrary host
keys, expressions, configuration graphs, projection recipes, and rebind selection policy stay
outside the sketch document in host sidecars (`ADR 0026:19-56`).

### Existing workbench/editor seam inventory

- The workbench is already a thin coordinator consumer: `Workbench` stores a
  `RetainedEditorCoordinator`; pointer events become editor inputs, and effects are applied
  through the coordinator (`workbench/mod.rs:48-53,217-305,358-424`). Keep this path for
  ordinary selection/drafting. M44 host-state buttons may use explicit coordinator methods
  where they already exist, or a narrow coordinator wrapper around public `DocumentEdit`/
  session calls—not browser-owned semantic policy.
- The coordinator currently has no M41 role/activation, M42 batch, or M43 snapshot/rebind
  action vocabulary. It *does* accept a revision-checked arbitrary `DocumentEdit` through
  `apply_edit` (`coordinator.rs:713-736`) and has atomic `transact` for a typed document
  mutation (`coordinator.rs:739-764`). Thus role/rebind declaration edits can be wrapped
  without moving their validation from `SketchDocument`; batch/snapshot updates need narrow
  coordinator methods because they mutate session inputs rather than a `DocumentEdit`.
- `RetainedSketchDocumentSession::new_with_inputs` is the exact fresh-fixture constructor,
  while `update_parameter_batch` and `update_external_snapshot_set` are the exact runtime
  public-input seams (`document_session.rs:2850-2873,3531-3668`). No host closure is part of
  those APIs. This is the minimal path for browser examples.
- Existing M40 E2E is a desktop Chromium/CDP test, uses a frozen coverage-ID list, static
  source-boundary checks, and fresh/reload helpers (`e2e/m40.mjs:21-44,141-220,241-259`).
  M44 should add a separate `e2e/m44.mjs` rather than alter M40’s frozen qualification list.

## Automated acceptance plan

| Case | Focused E2E fixture and exact assertions |
| --- | --- |
| Construction/profile participation | Start M41 fixture with a constrained curve. Toggle `Profile → Construction` through one role transaction; assert `.wb-curve[data-role=construction]`, activity `active`, unchanged accepted constraint evidence, and default-profile result excludes the curve. Toggle back and assert retained explicit geometry/discrete state and profile inclusion. |
| Suppression versus dimension mode | Fixture has one driving dimension/source. Suppress then reactivate it; assert source requested/effective state and `UserSuppressed` reason change while `data-dimension-mode` stays `driving`. Independently invoke existing `set_dimension_mode` to `reference`; assert no suppression/reason change. Include host-inactive and unavailable-dependency reason rendering from fixture inputs. |
| Parameters/bindings/proposals | Fixture declares one length parameter bound to two dimensions plus one declared reference output. Assert the parameter panel lists exactly declarations/bindings (never arbitrary `DesignScalarId`); submit one valid two-entry batch and assert batch revision/digest, matching accepted input stamp, two target provenance rows, and accepted proposal identity/unit/value. |
| External status/rebind | Fixture declares a point/line binding. Assert valid binding kind/topology, set revision/digest and accepted stamp; submit a well-formed missing/stale set and assert `UnavailableExternalReference`, newer attempt, unchanged accepted scene/stamp; submit topology change and assert it remains unavailable until explicit rebind, then a fresh valid snapshot publishes. |
| Three lifecycle revisions | Assert (a) a valid state where design/attempt/accepted all identify the current solve, (b) an invalid parameter or missing external retained attempt whose design/attempt advance while accepted is older, and (c) a successful recovery where accepted advances. Each assertion compares data attributes for all three identities and verifies SVG/audit provenance name the accepted identity, never attempt geometry. |
| Atomic batches/stale/missing recovery | Submit a two-entry `ParameterBatch` in one action; assert no intermediate DOM batch/accepted state appears. Submit stale parameter revision and stale external revision, then a complete but missing active binding. For each, assert typed attempt/status, prior accepted revision/scene/audit/accepted input stamp/proposals byte-for-byte unchanged. A newer complete valid input alone advances accepted evidence. |
| Boundary regression | New `m44.mjs` source checks `workbench/{mod,panels,scene}.rs` and fixture adapter for no residual/Jacobian/solver geometry helpers and no `Fn`, resolver, callback, formula, PDM, projection, or host-key path. It must also assert only `ParameterBatch`, `ExternalSnapshotSet`, public session APIs, and typed document edits enter the demo. Retain M40 checks unchanged. |

### Proposed disjoint implementation ownership

1. **`geosolve-constraint-editor/src/coordinator.rs` and native tests only:** add narrow
   revision-checked wrappers for M41 role/suppression/activity views, M42 complete batch
   update, and M43 snapshot update/rebind where an editor policy action is actually needed.
   Do not change equations, lowerers, or canonical M41-M43 semantics.
2. **`crates/geosolve-demo-web/src/workbench/host_state.rs` (new) plus `mod.rs` only:** own
   deterministic M44 fixture construction and a presentation DTO assembled from public
   session/coordinator values. It owns no persistence and no host identifiers. `mod.rs`
   routes buttons to typed coordinator/session methods and rerenders.
3. **`workbench/panels.rs`, `scene.rs`, `index.html`, `styles.css` only:** render tree,
   inspector, lifecycle/input-stamp/proposal/status rows and role classes. No mutation or
   validation logic in markup/CSS/JS. Preserve accepted-only SVG geometry.
4. **`crates/geosolve-demo-web/e2e/m44.mjs` only:** own the matrix above and static boundary
   scan. Reuse M40’s server/CDP helper pattern but create M44-specific coverage IDs.
5. **Focused native checks:** coordinator wrappers need native revision/action/replay tests;
   workbench pure markup/fixture formatting needs Rust unit tests where feasible. M41-M43
   existing `geosolve-sketch` tests remain their semantic oracle; M44 adds no residual/Jacobian
   work and therefore no equation relocation or browser finite-difference test.

## Open questions

None blocking. Parent-owned scope decisions are:

- “Profile participation” is the existing canonical M41 `GeometryRole` toggle and truthful
  default-profile eligibility/result display. M44 adds no second profile-scope language.
- `RetainedEditorCoordinator` receives narrow typed wrappers for role/rebind document
  transactions and immutable parameter/snapshot updates. Document edits participate in editor
  checkpoints/replay where representable; host input replacement remains an in-memory host
  action and does not become canonical sketch history or browser-owned policy.
- M44 stale-input automation exercises the existing synchronous revision rejection and typed
  retained attempts only. Captured asynchronous jobs and compare-and-swap publication remain
  exclusively M101X; M44 adds no synthetic CAS state.

## Qualification evidence and deferred carry-forward gate

Implemented ownership matches the approved slices:

- `geosolve-constraint-editor/src/coordinator.rs` owns narrow typed role/rebind and
  immutable complete input-replacement wrappers, with 26 focused tests. Representable
  rebind edits participate in checkpoints/replay; host input replacement does not become
  canonical history.
- `workbench/host_state.rs` owns deterministic in-memory fixture values and presentation
  evidence assembled from public session/coordinator APIs. The web suite has 103 passing
  tests. Fixture sidecars remain outside canonical sketch/workspace persistence.
- `workbench/{mod.rs,panels.rs,scene.rs}`, `index.html` and `styles.css` expose the six
  planned workflows while keeping geometry, profile, audit and proposals accepted-only.
- `e2e/m44.mjs` passes all six frozen coverage groups and statically rejects browser
  equation, formula-resolver and host-callback ownership. The preserved M40 suite passes
  all 14 frozen groups.

The command set qualified at the M44 checkpoint was the following. It is historical
evidence, not a runnable current gate; M48-M50 removed the browser scripts.

```bash
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
(cd crates/geosolve-demo-web && nix-shell ../../shell.nix --run "trunk build --release")
node crates/geosolve-demo-web/e2e/m40.mjs
node crates/geosolve-demo-web/e2e/m44.mjs
```

## Historical deferred-gate record

M44 was deliberately not marked complete after a combined M40/M44/M14 run timed out during
M14 without its final pass line. After exact test-profile cleanup, standalone M14 runs
failed at `tower/burst-drag: 130ms renders=1 budget=100ms`, at
`CDP request timed out: Input.dispatchMouseEvent`, and again at
`tower/burst-drag: 118ms renders=1 budget=100ms`. The reproducible timing failure was
not caused by the unchanged pointer coalescing/render path. Five isolated candidate runs
measured `234`, `61`, `70`, `49` and `48 ms`, while clean detached `HEAD` measured `66`,
`38`, `43`, `41` and `38 ms`. The difference was unconditional M41 dependency-closure
traversal in `SketchDocument::compute_effective_activity_with_input_overlays` even when
the document had no inactivity reason. The implementation now preserves exactly the same
activity entries while skipping closure traversal when the reason map is empty.

After that correction, five isolated tower runs measured `62`, `42`, `37`, `39` and
`44 ms`. `cargo fmt --all -- --check`, the locked all-feature M41 test, the release native
M14 tower test and release Trunk build passed. A full post-correction M14 browser run was
stopped by the supervising user after desktop layout and did not print the final pass
line, so it is not acceptance evidence. The 100 ms budget, historical desktop/mobile
assertions and correctness thresholds were unchanged at that checkpoint. The only
`m14.mjs` source diff then was the required default-route adjustment to `/#/dev/lab`
after the workbench became `/`. During M45 preparation, a release WASM rebuild, the focused 103-test web suite and
all six fresh-profile M44 browser groups passed again, including exact host-input finding
capture. The copied shell initially lacked a `chromium` command; an explicit Chrome run
was stopped after desktop layout and still did not produce the M14 final-pass line. The
supervising user then explicitly authorized avoiding the costly legacy suite and
fast-tracking to M45. M44 is complete on focused evidence, not on a retroactive M14 pass.
Human UAT remains owned by the supervising user. Its historical M45 preparation and active M53
scorecard are consolidated in `docs/M53_UAT.md`.
