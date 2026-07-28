<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M48 implementation record: direct workbench qualification and M40 purge

## Status

Complete (2026-07-28). This document preserves the contract/retirement ledger and records
the passing direct qualification and deletion boundary. `PLAN.md` remains authoritative for
milestone order; M49-M52 subsequently completed.

## Requirements

- Retain the native M40 transition corpus and its byte-identical golden oracle.
- Give every durable M40 claim a direct Rust owner: editor interaction/state, workbench
  presentation, persistence, evidence serialization, or a WASM adapter boundary.
- Do not replace browser qualification with another browser-shaped test: no browser, HTTP
  server, CDP, DOM scraping, screenshots, wall-clock/retry assertions, or source-substring
  policy scans.
- Explicitly retire delivery-only keyboard/focus, DOM/layout, reload, download/blob, and
  browser-console claims. Preserve only their underlying semantic contracts at their owners.
- Delete `crates/geosolve-demo-web/e2e/m40.mjs`, `scripts/serve-m40.sh`, and the M40-only
  CDP/server/profile/download machinery only after retained owners pass.

## Evidence and source pointers

- M48 scope and gate: `PLAN.md:2085-2100`; acceptance: `ACCEPTANCE.md:826-830`.
- The frozen M40 browser inventory, its HTTP/CDP/profile helpers, source scans, and all
  fourteen groups are in `crates/geosolve-demo-web/e2e/m40.mjs:21-530`.
- The authoritative native corpus, report and golden checks already exist in
  `crates/geosolve-constraint-editor/src/qualification.rs:2041-2068`:
  `m40_transition_corpus_passes_the_native_oracle`,
  `m40_transition_report_matches_the_canonical_golden_bytes`, and
  `qualification_matrix_is_complete_and_uses_only_frozen_evidence_ids`.
- Existing direct editor evidence includes viewport conversion and construction transitions in
  `geosolve-constraint-editor/src/lib.rs` (notably `viewport_round_trip_and_invalid_inputs_are_explicit`,
  `every_core_draft_has_exact_completion_and_cancellation`,
  `finish_commits_then_clears_the_polyline_preview`, and
  `projected_drag_retains_last_valid_preview_and_requires_matching_pointer`), plus identity,
  history, retention and preview tests in `src/coordinator.rs`.
- Workbench seams are `effect_adapter.rs`, `scene.rs`, `panels.rs`, `persistence.rs`,
  `evidence.rs`, and `routing.rs`. Existing direct tests include effect-adapter preview
  retention, accepted-scene isolation in `scene.rs`, and deterministic typed evidence capture
  in `evidence.rs`.
- The frozen M46 ledger is `docs/M46_DIRECT_TEST_REPLACEMENT.md:61-83,121-169`.

## M40 contract-to-owner matrix

| Frozen group / static claim | Retained contract and direct authoritative owner | Browser-only retirement |
| --- | --- | --- |
| `browser.wasm-report-parity` | Keep the three native qualification tests above; add a direct `workbench` WASM-adapter test which calls the exported qualification-report/checksum seam and compares exact report bytes and checksum to the editor corpus/golden. | Root dataset delivery and independent JavaScript FNV calculation. |
| `browser.creation-routes` | Editor draft/effect tests own terminal completion, Finish, Enter, double-click semantics, and cancellation; `workbench/effect_adapter.rs` tests consume emitted effects and resolved preview DTOs. | Button, key, pointercancel, SVG draft selectors, and CSS wire styling. |
| `browser.pointer-normalization` | `geosolve-constraint-editor` owns `Viewport` conversion; retain its round-trip/invalid-input test and add direct letterbox/scale cases at the adapter input conversion seam. | Device emulation, `getScreenCTM`, and viewport CSS width. |
| `browser.selection-identity` | Editor selection transitions and `coordinator.rs` persistent-ID reconciliation; add scene/panel view-model tests proving one typed ID is used by canvas and tree DTOs. | Shift/control/meta DOM event wiring and `aria-selected` observation through a page. |
| `browser.constraint-glyph` | Editor constraint action tests own creation; add `scene.rs` glyph DTO tests for persistent, unique IDs and supported constraint kinds. | SVG element counting/class selectors. |
| `browser.dimension-persistence` | `coordinator.rs` dimension routing/replay and `persistence.rs` snapshot codec round trips; add dimension DTO tests preserving ID, driving/reference mode, and rendered domain value. | localStorage and page reload mechanics. |
| `browser.projected-drag` | Existing editor preview/commit/cancel and exact one-checkpoint tests own solving/history; add a direct scene lifecycle projection assertion if absent. | Pointer delivery, rendered point coordinates, and undo button state. |
| `browser.history-delete-reload` | `coordinator.rs` dependency-delete/history/ID restoration plus `persistence.rs` accepted-checkpoint round trip own the semantics. | Keyboard Delete/click wiring, localStorage inspection, and reload. |
| `browser.redundancy-presentation` | `accepted_redundancy_is_a_verbatim_sketch_dto` owns source fidelity; add `panels.rs` DTO/markup test for accepted/design identities, sorted rows, and scope disclaimer. | Dataset reads from rendered DOM. |
| `browser.conflict-retention` | Editor rejected-attempt retention and `scene.rs` accepted-scene isolation own retained geometry/identity; add focused problem-panel DTO test if needed. | DOM lifecycle labels and storage fixture injection. |
| `browser.lifecycle` | Native M40 lifecycle corpus remains authoritative; add direct lifecycle-label/view-model coverage in `panels.rs` or a pure presentation helper. | Reload/fresh page delivery and root datasets. |
| `browser.malformed-storage` | `persistence.rs` directly decodes malformed snapshots and selects a newly solved empty fallback without accepting invalid geometry. | Browser exceptions and localStorage/reload delivery. |
| `browser.accessibility` | Add inspectable semantic-markup tests for `main`, labelled viewport/tree, button/pressed semantics, and polite/assertive live regions. | Focus management and Enter activation through the browser event loop. |
| `browser.evidence-route` | `evidence.rs` owns deterministic evidence-package JSON/SVG/checksum serialization; retain typed capture content tests and add package serialization golden/parity if absent. | Anchor click, blob URL/download, and `#/dev/lab` route transition. |
| M40 forbidden/required source scans | Replace with executable public typed-API use: editor tests construct policy-bearing requests/effects and workbench tests consume only resolved scenes, previews, redundancy, lifecycle, and evidence DTOs. Module ownership and compiler-visible signatures—not text searches—prove the boundary. | All forbidden-symbol, required-call, fixed-literal, and implementation-substring scans. |

## Direct test inventory

1. Preserve `qualification.rs` native corpus, canonical golden, malformed corpus, and matrix
   validation tests unchanged as the oracle.
2. In `geosolve-constraint-editor`, retain/add focused tests for construction effects,
   viewport conversion, selection persistent IDs, projected preview lifecycle, conflict
   retention, redundancy DTO fidelity, dimension replay, and one-history-checkpoint drag.
3. In `geosolve-demo-web/src/workbench`, add pure tests at the named module owners:
   `effect_adapter.rs` (typed effects and normalized input), `scene.rs` (selection/glyph/
   dimension DTOs and accepted-scene isolation), `panels.rs` (lifecycle, redundancy, problem
   and semantic accessibility markup), `persistence.rs` (codec/fallback), and `evidence.rs`
   (deterministic package serialization/checksum).
4. Add a direct WASM-adapter parity/golden test at a test-visible adapter seam. It must invoke
   the Rust/WASM-facing API without serving the application or querying a DOM.

## Retirement and deletion inventory

- Delete `crates/geosolve-demo-web/e2e/m40.mjs`, including its Node imports, HTTP server,
  Chromium/CDP connection, fixed profile, waits/retries, device emulation, DOM helpers,
  browser error collection, source scans, and blob-download interception.
- Delete `scripts/serve-m40.sh`; it exists solely to release-serve the M40 workbench.
- Remove M40-only browser-E2E invocation/documentation references that become dead with those
  files, but do not use repository-wide text scanning as M48 acceptance evidence.
- Review `index.html` and `styles.css` only for selectors/markers exclusively supporting the
  removed M40 harness; retain product workbench semantics and shared/legacy styling until their
  assigned cleanup milestone.

## Shared artifacts that must remain

- `crates/geosolve-constraint-editor/tests/m40_transition_corpus.json` and
  `m40_qualification_report.golden.json`, plus the public qualification APIs.
- `scripts/serve-m45.sh`: archived M45 manual inspection; M50 owns its deletion.
- `crates/geosolve-demo-web/e2e/m14.mjs`, legacy playground route/CSS/persistence, and the
  `scripts/release-gate.sh` M14 invocation: M49-M50 own their ledger/extraction/purge. The
  release gate itself labels this as historical cleanup debt (`release-gate.sh:6-7`).
- Generic workbench capability and future single-workbench consolidation work belong to M49-M51,
  not an opportunistic M48 cleanup.

## Implementation order

1. Parent confirms each matrix row has an exact existing test or adds the smallest direct test
   at its authoritative module; keep the native golden unchanged.
2. Implement pure editor and workbench owner tests, including typed API boundary usage and the
   adapter parity seam; run focused native and WASM checks.
3. Record each browser-only assertion as retired in review, then delete M40 E2E/server/source
   scan/download machinery and M40-only selectors.
4. Parent integrates deletion and documentation changes after both test areas pass; do not mix
   M14/M45/legacy removal into this slice.

At most two independent implementation children may work concurrently, with disjoint scope:

| Child | Exclusive file scope | Deliverable |
| --- | --- | --- |
| Editor qualification child | `crates/geosolve-constraint-editor/src/lib.rs`, `src/coordinator.rs`, `src/qualification.rs`, and editor tests | Editor contracts, native corpus preservation, and typed editor boundary tests. |
| Workbench qualification child | `crates/geosolve-demo-web/src/workbench/mod.rs`, `crates/geosolve-demo-web/src/workbench/{effect_adapter,scene,panels,persistence,evidence}.rs`, and their direct tests | Presentation/persistence/evidence/private WASM-adapter tests only. |

The parent owns integration, `e2e/m40.mjs`/`serve-m40.sh` deletion, shared HTML/CSS review,
commands, and documentation. Children must not edit each other's files or delete shared
infrastructure.

## Focused acceptance commands

Run from the workspace root; these are native/WASM commands only and intentionally do not
serve a site, launch a browser, run Node, scrape a DOM, or perform source-policy scans.

```bash
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'git diff --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
```

## Completion record

### 1. Files and API behavior

- Kept the native M40 corpus, canonical report and public editor qualification API unchanged.
- Added only private/pure workbench seams: client-coordinate normalization and construction
  effect dispatch in `effect_adapter.rs`; inspectable scene, panel and persistence
  transformations; and a test-only qualification adapter/evidence serializer. No M40 UAT API
  or runtime qualification dataset was promoted into the product.
- The production `WorkspaceSnapshot` codec is directly available to native tests and uses the
  same `serde_json` encode/decode path as WASM. `serde` and `serde_json` are therefore ordinary
  demo-web dependencies rather than target-only test substitutes.
- Deleted `crates/geosolve-demo-web/e2e/m40.mjs` and `scripts/serve-m40.sh`. Removed the M40
  qualification datasets, workbench `data-e2e-ready`, `data-evidence-checksum`,
  `capture-finding` action and its browser download/blob route. Retained M14/legacy selectors,
  platform dependencies, `e2e/m14.mjs` and `scripts/serve-m45.sh` for M49-M50.

### 2. Mathematical and semantic behavior

- The editor's independently checked corpus/golden remains the authority for construction,
  selection, projected-drag, history, conflict-retention, lifecycle and dimension behavior.
- Direct workbench tests prove letterboxed finite coordinate conversion, terminal construction
  effect success/failure behavior, shared persistent identity across scene/tree output, unique
  glyph IDs, dimension driving/reference values, accepted-only scene isolation, lifecycle and
  problem semantics, accepted redundancy provenance/source order/scope disclosure, exact
  persistence round trips and malformed fallback, semantic accessibility markup, deterministic
  finding JSON/SVG/checksum serialization and exact M40 report/checksum parity.
- Browser focus/keyboard event delivery, DOM/layout observation, localStorage reload delivery,
  anchor/blob download delivery, browser timing and source-substring policy scans are explicitly
  retired. They were not recreated as browser-shaped or source-reading tests.

### 3. Commands and outcomes

Run through `nix-shell` on 2026-07-28:

- `cargo test --locked -p geosolve-constraint-editor` — passed, 53 tests.
- `cargo test --locked -p geosolve-demo-web --all-features` — passed, 111 tests.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked --workspace --all-features` — passed.
- `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown`
  — passed.
- `trunk build --release` from `crates/geosolve-demo-web` — passed with Trunk 0.21.14.

No browser, Node E2E, HTTP server, CDP session, DOM scrape or screenshot was run.

### 4. Acceptance criteria

All M48 acceptance rows pass: every retained contract has a direct editor/workbench owner;
delivery-only assertions have an explicit retirement; the M40 E2E and serving artifacts are
absent; and no M40 source-policy scan or browser qualification path remains.

### 5. Historical handoff (resolved by M49-M50)

M48 intentionally did not remove `e2e/m14.mjs`, the 92 legacy inline tests, the legacy
playground, shared platform/download dependencies, `scripts/serve-m45.sh`, or the historical
release-gate invocation. M49 finished the zero-unowned-assertion ledger before M50 deleted
those artifacts. Human approval remains deferred to M53.

## Resolved integration decisions

- Keep the qualification adapter private and test-only in `geosolve-demo-web::workbench`. The
  small pure helper in `workbench/mod.rs` calls `run_m40_qualification()` once and returns the
  exact canonical report bytes, corpus checksum, and pass flag to a native direct test. Production
  `wasm::install` no longer publishes or consumes M40 qualification data; this keeps parity
  compiler-visible without retaining M40/UAT plumbing as a product API or runtime dataset.
- Extract client-rectangle letterbox normalization into a pure numeric helper used by the WASM
  pointer adapter. Direct tests own zero/negative extents, letterboxed coordinates, alternate CSS
  sizes, and device-scale-independent client coordinates; browser `getScreenCTM` and emulation are
  retired.
- `scene.rs` and `panels.rs` already return inspectable markup. Direct tests will cover the
  retained dynamic semantics: typed persistent identity shared by scene/tree output, `treeitem`
  and `aria-selected`, unique glyph IDs, dimension mode/value, labelled redundancy/problem
  sections, and lifecycle labels. Static shell tag choice, browser focus, Enter activation, and
  live-region delivery remain browser-only claims and are explicitly retired rather than tested by
  reading `index.html` as source text.
- The only product attributes dedicated to the M40 harness are
  `data-m40-qualification-report`, `data-m40-corpus-checksum`,
  `data-m40-qualification-passed`, and the workbench `data-e2e-ready` marker. Remove them after
  the private direct qualification seam passes. `data-evidence-checksum` belongs to the M40
  browser download observation and is removed with that route. Shared workbench classes, IDs,
  `data-wb-*`, `data-editor-*`, accessibility attributes, and all legacy-playground selectors stay
  for M49-M51.

## Out of scope

- M14 semantic extraction, legacy playground deletion, M45 manual-fixture removal, release-gate
  M14 cleanup, and all M49-M51 consolidation work.
- New solver equations, browser automation replacement, generic framework work, or changes to
  the M40 corpus/golden semantics.
