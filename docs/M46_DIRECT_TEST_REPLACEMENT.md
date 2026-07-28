<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M46 direct-test replacement plan

## Status

M46 is complete as the ownership-freeze milestone. This document is the frozen deletion
ledger and historical implementation map whose M47-M51 slices are now complete; `PLAN.md`
owns milestone order and current gates. Imperative wording below records the checkpoint
criteria, not unfinished work.

## Requirements

- Delete all three legacy demo-web CDP E2E scripts (`e2e/m14.mjs`, `e2e/m40.mjs`,
  and `e2e/m44.mjs`) only after every still-relevant assertion has a direct owner.
- A direct test must exercise the authoritative Rust boundary for its claim: sketch or
  linkage domain semantics, headless-editor state/effects, workbench presentation
  formatting/dispatch/storage, persistence codec, or WASM export/adapter. It must not
  use Chromium/CDP, a served `dist`, DOM scraping, or browser timing as a substitute
  for a domain assertion.
- Preserve the strict domain contracts: independently validated accepted state,
  explicit branch state, transactional retention, and no browser-owned equations or
  interaction policy. The workbench remains a thin WASM consumer of the editor and
  public domain APIs.
- Classify every M14, M40, and M44 frozen coverage group/static check as replaced or
  explicitly retired; name proposed direct test files and fixtures without implementing
  them. Legacy-only product/UI scope is a retirement decision, not a deletion blocker.

## Evidence and source pointers

- `e2e/m14.mjs` is a large legacy-lab suite: it serves `dist`, starts Chromium via a
  custom CDP client/profile, targets `/#/dev/lab`, and combines pointer/touch/UI,
  DOM, download/upload, storage, timing, alpha, advanced sketch, profile, and spatial
  assertions. Its historical full post-correction run is explicitly incomplete, not
  passing evidence (`docs/M44_IMPLEMENTATION.md:158-182`; `docs/M45_UI_CLEANUP_INVESTIGATION.md:31-33`).
- `e2e/m40.mjs` freezes 14 root-workbench browser groups. Its source checks thin-adapter
  boundaries and compares a release-WASM M40 qualification report byte-for-byte with
  the native golden; the remaining groups mostly observe browser delivery/rendering
  of editor-owned policy (`e2e/m40.mjs:21-44,241-268`).
- `e2e/m44.mjs` freezes six temporary-host-fixture groups and a host-boundary source
  scan (`e2e/m44.mjs:18-32,171-293`). M41-M43 semantics already have durable sketch
  tests; the fixture is deliberately in-memory, non-canonical, and scheduled for
  replacement (`docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md:18-38,63-93`).
- Existing investigation classifies all 92 legacy inline consumer tests and the M14
  groups, including unresolved advanced-authoring/spatial/profile/performance/file
  scope decisions (`docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md:174-295`). This
  plan must turn that inventory into deletion gates rather than infer parity from the
  root workbench being default.

## Decisions / inferred constraints

- “Direct” means ordinary Rust unit/integration tests at the layer that owns the
  behavior, plus deterministic WASM-callable Rust tests when an exported adapter
  contract itself must be checked. It excludes browser process, HTTP server, CDP,
  fresh browser profile, DOM query, CSS layout, and wall-clock assertions.
- Do not move M14’s broad legacy authoring, mobile, spatial, capsule, profile, file, or
  performance claims into M40/M44 merely to delete a script. Under the supervising-user
  decision, assertions tied only to the legacy lab or deprecated browser delivery are
  explicitly retired; durable underlying semantics must instead have a named direct owner.

## Replacement matrix

### M40 frozen browser groups

| Frozen group | Direct authoritative replacement / proposed owner | Deletion disposition |
| --- | --- | --- |
| `browser.wasm-report-parity` | Keep `geosolve-constraint-editor::qualification` corpus, completeness and byte-identical golden tests. M48 adds `workbench::tests::m40_qualification_adapter_matches_native_golden`, which directly calls a test-visible WASM-adapter transformation without loading a page. | Replaced by the named editor and adapter tests; the DOM data-attribute delivery retires with M40 E2E. |
| `browser.creation-routes` | Editor interaction-state/effect tests for terminal completion, finish/Enter/double-click and cancel; workbench `effect_adapter` tests only for dispatch-to-render DTO conversion. | Replace semantic paths; retire draft SVG selectors. |
| `browser.pointer-normalization` | Workbench viewport-coordinate conversion unit tests with letterbox and scale inputs. | Replaced; no device-emulation assertion remains. |
| `browser.selection-identity` | Editor selection transition tests plus workbench tree/scene view-model identity tests. | Replaced; modifier/DOM wiring retires. |
| `browser.constraint-glyph` | Editor constraint creation tests and workbench glyph DTO/markup unit tests with persistent IDs. | Replaced. |
| `browser.dimension-persistence` | Sketch persistence/history tests and workbench persistence codec plus dimension DTO rendering tests. | Replaced; reload/localStorage mechanics retire. |
| `browser.projected-drag` | Existing editor preview/commit/cancel tests; add a direct one-history-checkpoint regression if not already covered. | Replaced. |
| `browser.history-delete-reload` | Sketch dependency-delete/history tests, persistence codec round trips, and workbench identity rendering tests. | Replaced; browser reload is retired. |
| `browser.redundancy-presentation` | Editor accepted-redundancy DTO tests and sorted workbench panel rendering tests. | Replaced. |
| `browser.conflict-retention` | Sketch/editor rejected-attempt retention tests and accepted-scene identity test (`workbench/scene.rs`). | Replaced. |
| `browser.lifecycle` | M40 native lifecycle corpus plus direct lifecycle-label/view-model tests. | Replaced. |
| `browser.malformed-storage` | Workbench persistence decoder/fallback unit tests. | Replaced; absence of browser exceptions is not a direct semantic contract. |
| `browser.accessibility` | Retain semantic markup unit tests where renderer output is inspectable as Rust text/tree. Retire browser keyboard/focus delivery with the browser harness. | Replaced for direct semantic markup; delivery-only assertions retire. |
| `browser.evidence-route` | `workbench/evidence.rs` deterministic package/checksum tests and routing tests. Download/blob and hidden-DOM route mechanics retire with the lab; retain an explicit route parser test only while the lab ships. | Replaced after capture contract and route disposition are complete. |

The source scans in M40 and M44 are not a durable behavioral test technique. Replace their
intent with reviewable module ownership plus compiler-visible APIs: `workbench` accepts
resolved editor/domain DTOs, while `geosolve-constraint-editor` owns interaction policy.
No replacement should duplicate a forbidden-symbol substring scan.

### M44 frozen browser groups

| Frozen group | Direct authoritative replacement / proposed owner | Deletion disposition |
| --- | --- | --- |
| `m44.construction-profile` | Keep M41 sketch construction/activity tests; add focused workbench role/profile DTO regression. | Replace before removing fixture. |
| `m44.suppression-dimension-mode` | Keep M41/M42 semantics; add focused activity-reason and dimension-mode presentation regression. | Replace before removing fixture. |
| `m44.parameters-bindings-proposals` | Keep M42 typed batch/binding tests; add a small accepted-batch/proposal stamp fixture in `workbench/host_state.rs` or successor module. | Replace before removing fixture. |
| `m44.identities-retention` | Keep M42 invalid/stale/recovery tests; add direct accepted-versus-attempt presentation identity regression. | Replace before removing fixture. |
| `m44.external-rebind-retention` | Keep M43 snapshot/rebind tests; add focused retained external-evidence presentation regression. | Replace before removing fixture. |
| `m44.host-boundary` | M47 adds `workbench::evidence::tests::typed_host_capture_contains_inputs_attempt_and_accepted_evidence`, using a small test/UAT-only capture harness and testing checksum/content directly. | Replaced by the named test-only harness; it is not a stable product adapter, and the broad fixture must not survive. |

These six replacements implement the five focused fixture groups selected in
`docs/M45_CLEANUP_PLAN.md:81-87`: role/profile/activity, parameter/binding/proposal,
external/rebind, lifecycle/retention, and capture.

### M14 frozen groups

| Legacy group(s) | Direct owner and disposition |
| --- | --- |
| `layoutPrioritySuite` | Retire: legacy page layout only. |
| `scenarioSuite`, `scaleWorkflow`, `stressExampleSuite`, `reportedRegressionSuite` | Keep solved-state, rank, scale and branch semantics in `crates/geosolve-sketch/tests/m14.rs`; retire scenario-selector, pointer-flow and SVG claims. |
| `historySuite`, `recoverySuite`, `branchHistoryRecoverySuite` | Keep transaction/retention/branch semantics in `crates/geosolve-sketch/tests/m13.rs` and `m14.rs`; add any missing codec claim to proposed `src/workbench/persistence.rs` tests. Retire browser storage retry/reload delivery. |
| `creationSuite` | Keep ordinary transaction completion/cancel in editor effect tests and sketch document tests; retire legacy drawing UI flow. |
| `conicCreationSuite`, `mobileConicSuite` | Move any exact conic document/branch assertion to proposed `crates/geosolve-sketch/tests/m46_advanced_geometry.rs`; retire conic controls and mobile interaction. |
| `newDomainExampleSuite` | Keep planar semantic claims in proposed `crates/geosolve-sketch/tests/m49_advanced_geometry.rs` and linkage-only semantics in proposed `crates/geosolve-linkage/tests/m49_legacy_consumer.rs`; retire the legacy spatial demo consumer. |
| `m28VisibleTrimSuite` | Add/retain sketch interval, explode and dependency-delete regressions in proposed `crates/geosolve-sketch/tests/m46_visible_topology.rs`; retire browser render checks. |
| `m30DesktopSuite`, `m30MobileSmokeSuite` | Move any explicit branch/transaction semantic to `m14.rs`; retire legacy desktop/mobile interaction. |
| `m31DesktopSuite`, `m31MobileSmokeSuite` | Move certified profile/sampling semantics to proposed `crates/geosolve-sketch/tests/m46_profile.rs`; retire profile presentation and mobile UI. |
| `m32DesktopSuite` | Keep non-timing diagnostic/profile semantics in `m14.rs` or proposed `m46_profile.rs`; retire desktop presentation flow. |
| `m32BrowserPerformanceSuite`, `renderBudgets` | Retire browser timing, rendering-budget and retry assertions. Any future native performance envelope needs a separate direct benchmark contract, not a replacement browser gate. |
| `fileSuite` | Keep canonical document persistence semantics in `m14.rs` and proposed workbench persistence-codec tests; retire import/download/file-picker delivery. |

The M14 grouping above follows the exhaustive inventory in
`docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md:253-284`. It deliberately does not claim
that native M14 tests replace browser authoring or performance behavior.

### Static forbidden-symbol and boundary checks

| Legacy check | Direct owner | Disposition |
| --- | --- | --- |
| M40 adapter/scene/panels forbidden-symbol scans and required-call scans | `geosolve-constraint-editor` public request/effect DTO tests; demo-web workbench unit tests that consume those DTOs; module-level API review. | Retire substring scans. A text scan cannot prove policy ownership. |
| M40 fixed viewport literals and resolved-preview scan | `workbench/routing.rs` and a proposed viewport conversion unit-test module; editor preview lifecycle tests. | Replace behavior, retire implementation-text assertions. |
| M44 adapter/panels/scene and host forbidden-symbol scans | M41--M43 domain tests plus focused host-input/capture harness tests using only typed `ParameterBatch` and `ExternalSnapshotSet` APIs. | Retire substring scans. |
| M44 required typed-seam scan | Proposed focused host harness tests that construct and replace typed batches/snapshots, set roles, and rebind externals through public APIs. | Replace the scan with executable API use. |

## Deletion gates

- **`e2e/m44.mjs` and broad host fixture:** before deletion, the six M44 rows must have
  named, passing direct owners: construction/profile/activity; suppression versus dimension
  mode; parameter/binding/proposal stamps; retained attempt/accepted identity; external
  rebind retention; and deterministic typed evidence capture. Delete the broad `HostState`
  composition and fixture-only controls with the script; retain only small focused fixtures.
- **`e2e/m40.mjs`:** before deletion, the M40 corpus/golden remains native and every durable
  group above has a passing editor, workbench, persistence, or direct WASM-export test.
  Retire browser-only keyboard/focus delivery, DOM details, and download/blob delivery;
  keep their durable semantic markup, identity, lifecycle, and evidence-package claims in
  named direct owners. Remove the M40 source substring scans rather than preserving them.
- **`e2e/m14.mjs`:** before deletion, migrate the named M14 domain, transaction, branch,
  history, persistence, sampling, interval/delete, and diagnostic claims to sketch/editor/
  persistence tests. Retire every claim tied only to the legacy lab, deprecated demo flow,
  mobile/responsive UI, CSS/layout, browser timing/performance, downloads/files/blobs, or
  browser-specific storage/pointer delivery. Advanced capability absence is not a blocker
  once its legacy-only assertion is explicitly retired.
- **CDP/browser infrastructure:** delete `crates/geosolve-demo-web/e2e/`, embedded Node HTTP
  servers, Chromium/CDP launch and wait code, fixed `/tmp/geosolve-*-browser-profile` use,
  browser environment variables, DOM scraping, timing/retry helpers, and download/blob
  interception when all three scripts are gone.
- **Serving and release scripts:** delete `scripts/serve-m40.sh` with M40. Delete or replace
  `scripts/serve-m45.sh` when temporary fixture UAT instructions are removed. Remove the
  `node e2e/m14.mjs` release-gate invocation and its `GEOSOLVE_E2E_*` environment cleanup
  in the M14 purge slice.
- **Legacy route/UI:** delete `#/dev/lab`, `src/playground.rs`, legacy branches in `src/lib.rs`,
  hidden playground DOM/CSS, old persistence/capsule glue, and legacy-only inline tests only
  after their class-A durable assertions have direct owners. Class-B/D/E assertions that are
  explicitly legacy-only are retired with this removal; no legacy route smoke remains.

## Cleanup sequence

| Milestone | Pre-cleanup work | Purge in this slice | Exit evidence |
| --- | --- | --- | --- |
| **M46 — direct-test ownership freeze** | Freeze this matrix and name missing direct regression targets without changing behavior. | None. | Each retained M14/M40/M44 claim has a direct owner or reviewed retirement. |
| **M47 — focused host replacement** | Replace the broad M44 composition with five small fixtures: role/profile/activity, parameters/proposals, external rebind, lifecycle/retention, and typed capture. | Broad host fixture, fixture-only controls, `e2e/m44.mjs`, and M44 CDP/server/profile code. | All six M44 contracts pass directly; capture records typed inputs and accepted/attempted evidence. |
| **M48 — direct workbench qualification** | Add direct editor/workbench tests for M40 creation effects, coordinate conversion, selection identity, glyph/dimension DTOs, lifecycle, redundancy, persistence fallback, semantic accessibility markup, and evidence package. Keep native M40 corpus/golden. | `e2e/m40.mjs`, `scripts/serve-m40.sh`, M40 CDP/server/profile code, forbidden-symbol scans, and M40-only download interception. | M40 durable contracts pass without a browser. |
| **M49 — legacy semantic extraction** | Move every retained M14/legacy-inline class-A claim to sketch, linkage, editor, persistence, or focused presentation tests; record explicit retirement of legacy-only UI/timing/download/mobile/layout/demo claims. | None until the migration checklist is complete. | Direct tests cover all retained semantic claims; retirement list is reviewed. |
| **M50 — old E2E and legacy application purge** | Verify no retained owner imports or invokes playground behavior; update docs/UAT instructions and release validation. | `e2e/m14.mjs`, release-gate E2E invocation, remaining CDP/profile/server/download infrastructure, `serve-m45.sh`, legacy route/UI/CSS/tests and obsolete fixture/capsule glue. | Repository search finds no E2E, CDP, legacy-route, or fixture-marker references; direct validation passes. |
| **M51 — post-purge consolidation** | Add regressions for any discovered direct-test blind spot and retain only public WASM adapter contracts. | Remaining dead documentation/scripts and compatibility shims. | Stable direct suite is the only automated qualification path. |

## Direct validation commands

Run these exact commands from the workspace root. They use the project shell but do not
serve `dist`, launch Chromium, invoke CDP, or run a Node E2E script.

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'git diff --check'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m13 --test m14 --test m41 --test m42 --test m43'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
```

M46 verification on 2026-07-27 passed the complete locked workspace test suite,
warnings-denied workspace Clippy and the locked WASM check after a behavior-preserving
match-guard rewrite cleared the linkage `collapsible_match` lint. Formatting and diff
checks remain mandatory after the final ownership-ledger review.

## Finding-capture decision

During cleanup, finding capture is deterministic test/UAT infrastructure over public
domain and audit APIs, not a stable product API. It records typed inputs and
accepted/attempted evidence. M61 may later decide whether any minimized
adapter belongs in the supported release surface.

## Ownership-freeze conclusion

The matrices above are final M46 dispositions, not conditional product-scope questions.
Every retained M14/M40/M44 assertion has a named existing or proposed direct Rust owner;
every browser-only delivery, layout, mobile, timing, file-picker/download, DOM or deprecated
legacy-product assertion has a reviewed retirement. Proposed tests are implemented only in
their assigned M47-M49 slice. Finding capture is explicitly test/UAT-only infrastructure.
No old E2E, broad fixture or playground deletion is authorized by this ownership freeze.

## Out of scope

- Implementing replacement tests, deleting scripts/UI/infrastructure, changing domain
  behavior, or running browser tests.
