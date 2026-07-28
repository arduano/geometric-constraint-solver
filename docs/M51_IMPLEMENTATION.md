<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M51 implementation ledger: single-workbench consolidation and hardening

## Status

Complete as of 2026-07-28. Exactly one non-authoritative workbench remains, with direct
Rust/WASM qualification and no cleanup-era compatibility shim or dead fixture. M52
subsequently prepared the disposable candidate; M53 human review later passed.

## Consolidation inventory

### Requirements

- M51 must consolidate the one surviving workbench; remove M50-dead compatibility/dependency/test infrastructure; isolate minimal browser glue from direct-testable transformations; and add direct regressions for purge defects (`PLAN.md:2177-2192`).
- Acceptance requires one workbench, directly testable presentation/persistence/evidence transformations, no dead dependencies, compatibility shims, stale docs or cleanup-only fixtures, and direct native/WASM-only qualification (`ACCEPTANCE.md:875-879`).

### Evidence and source pointers

- M50 records one workbench, no executable E2E/CDP/server/profile/download infrastructure, and a retained `web-sys` DOM/event/storage surface required by that workbench (`docs/M50_IMPLEMENTATION.md:44-82`; `scripts/release-gate.sh:14-45`).
- The source entrypoint has one WASM start path to `workbench::wasm::install`, with no route selection (`crates/geosolve-demo-web/src/lib.rs:3-17`).
- The current workbench has separately named adapter, persistence, scene/presentation, panels, platform, and test-only evidence modules (`crates/geosolve-demo-web/src/workbench/mod.rs:3-14`).
- **Resolved — confirmed compatibility shim:** the inventory found that `persistence.rs:11` exported
  `LEGACY_STORAGE_KEY = "geosolve.workbench.design.v1"`; `wasm::install` read it when the v1
  workspace snapshot is absent and reconstructs a coordinator from design-only JSON
  (`workbench/mod.rs:59,70-89`). This is an executable pre-consolidation storage migration, not
  a domain persistence contract, and had no direct migration regression. The inventory disposition
  was to remove the constant, legacy read/fallback branch and import and retain only `WorkspaceSnapshot` under
  `STORAGE_KEY`. M51 removed the key, branch and now-dead design-only constructor.
- **Resolved — confirmed cleanup-only fixture:** the inventory found that
  `workbench/mod.rs:17-31,801-825` wrapped and
  compared the editor's M40 qualification report/corpus golden. The editor owns that report
  directly; this duplicate adapter is neither startup nor workbench behavior and keeps an M40
  cleanup fixture in demo-web. The inventory disposition was to delete the adapter and its
  duplicate test and retain the editor
  qualification suite as its sole owner. M51 deleted the adapter and duplicate test.
- **Resolved — confirmed stale cleanup-only evidence fixture:** `workbench/evidence.rs` is compiled
  only under `cfg(test)` (`workbench/mod.rs:6`) and contained `M40EvidencePackage` plus
  `serialize_m40_evidence` (`evidence.rs:7-28,299-317`). No runtime/export caller uses it. Its
  SVG/package format was historical M40 capture infrastructure, so the inventory disposition was
  to delete it and its test rather
  than promote it into the survivor. M51 deleted only that M40 package and test.
- **Kept and consolidated — retained direct evidence owner:** the former `typed_host_capture` and
  `typed_host_capture_contains_inputs_attempt_and_accepted_evidence`
  (`evidence.rs:30-114,185-297`) are the M47 direct replacement for preserved UAT point 9
  (`docs/M47_IMPLEMENTATION.md:34-47,111-117`), so they are not cleanup-only. They do mix a
  deterministic envelope with a host-state HTML string and test-supplied location/user-agent/
  capture-time values. Keep the contract but move/recast it as a clearly named pure evidence DTO
  serializer test, with canonical test inputs and no M40 SVG/package wrapper. This is the one
  confirmed M51 test-consolidation change, not a feature addition. It is now the pure
  `serialize_typed_host_evidence` transformation with the deterministic
  `typed_host_evidence_serializer_contains_inputs_attempt_and_accepted_evidence` regression.
- **Resolved — stale copied documentation:** the inventory found that `index.html:10-13` copied licence,
  compatibility, and `docs/M32_SCALE_PERFORMANCE.md` into Trunk output. The M32 performance
  record was historical and unrelated to the one-workbench runtime; the inventory proposed removing that
  copy-file entry as M51 stale-document cleanup. M51 removed the M32 copy while retaining the
  licence and API compatibility release artifacts.
- **Keep — reviewed platform configuration:** `Trunk.toml:3-4` sets only
  `[serve] no_autoreload = true`. It is not E2E/server infrastructure (M50 explicitly reviewed
  it as ordinary Trunk configuration). It is unused by the release build and is not needed by the
  direct qualification path, but final implementing-parent and independent review retained it as
  optional local-development configuration rather than qualification infrastructure.
- **Keep — reviewed no change:** `Cargo.toml:16-39` dependencies are all live: editor/core/sketch
  types are imported by the workbench; `serde`/`serde_json` implement `WorkspaceSnapshot`; and
  the target-only panic hook, bindgen and exact listed `web-sys` DOM/event/storage features are
  used by the single WASM adapter (`lib.rs:8-16`; `workbench/mod.rs:51-57,70-76,159-363,696-733`).
  No dead Cargo dependency was found in the permitted manifests.
- **Keep — reviewed no change:** M50's absence boundary still holds: there is no route parser or
  second application (`lib.rs:3-17`), no E2E directory/script, and `release-gate.sh:14-45` runs
  direct Cargo checks/tests plus the release Trunk build only. Browser E2E, HTTP server, profile,
  download, and cleanup-only browser qualification infrastructure do not remain.

### Decisions / inferred constraints

- Preserve the desktop-only, non-authoritative workbench and its public sketch/editor boundary; do not restore browser automation, HTTP serving, or legacy routes (`START_HERE.md:56-61`; `ARCHITECTURE.md:199-229`).
- Findings below distinguish a confirmed, executable cleanup gap from reviewed-no-change infrastructure. Historical M45-M50 ledgers are evidence, not stale current instructions (`docs/M50_IMPLEMENTATION.md:84-132`).
- The design-only storage fallback is a compatibility shim despite its key being named `LEGACY`;
  deleting it changes only obsolete browser-local restore behavior, not sketch persistence or
  accepted-state semantics.
- No new feature behavior was warranted: M51 removed the legacy migration/M40 wrappers,
  made retained evidence pure and directly owned, and removed the stale copied M32 document.
  The ordinary Trunk developer setting remains and is not part of automated qualification.

### Reviewed question

- `Trunk.toml`'s disabled autoreload setting remains ordinary optional local-development
  configuration. Implementing-parent review and independent verification found no executable
  evidence that it is browser qualification or server-launch infrastructure.

### Out of scope

- Domain equations, sketch/editor implementation changes, generated `dist/` output, browser E2E, serving, mobile/layout behavior, and historical-ledger rewriting.

### Consolidation conclusion and smallest scope

| Area queried | Result after review | Disposition |
| --- | --- | --- |
| Routing | Exactly one startup path and one root; `href="#/"` is a brand anchor, not a routed legacy application (`lib.rs:3-17`; `index.html:16-18`). | Reviewed no change |
| Browser E2E/server/profile/download | No executable infrastructure remains; release gate has no browser invocation. Trunk's `[serve]` setting is development configuration, not qualification infrastructure. | Absent; do not recreate |
| Cleanup-only fixtures | The duplicate demo M40 report adapter and M40 JSON/SVG evidence package are deleted; the editor directly owns its corpus/golden report. | Resolved |
| Compatibility shim | The design-only `LEGACY_STORAGE_KEY`, migration branch and now-dead constructor are deleted. | Resolved |
| Dependencies/platform glue | All Cargo dependencies/features are live. The one `[serve] no_autoreload` setting is optional local-development configuration, not a dead dependency or qualification path. | Reviewed keep |
| Stale docs | The copied M32 performance record is removed; licence/API compatibility outputs remain release artifacts. | Resolved |
| Missing direct regression | No retained domain/editor/presentation contract is unowned. The retained typed-host evidence serializer has one deterministic pure-owner regression. | Resolved |

**Implemented M51 scope:** removed the design-only storage migration, duplicate M40 report
adapter/test and M40 JSON/SVG evidence wrapper/test; retained and renamed the pure typed-host
evidence serializer with one deterministic regression; removed the stale M32 distribution copy;
and left workbench tools, editor transitions, domain persistence, HTML/CSS behavior, release
dependencies and ordinary Trunk developer configuration unchanged.

## Module and test ownership map

| Concern | Runtime/module owner | Direct test / build owner | Status |
| --- | --- | --- | --- |
| Routing/startup | `geosolve-demo-web/src/lib.rs::wasm::start` → `workbench::wasm::install`; one `#workbench-root` in `index.html:16` | Directly exercised by WASM compilation; no route-specific fixture is needed because there is no router | Reviewed no change; one workbench |
| DOM event translation | `workbench/mod.rs::{install_clicks,install_canvas,install_keyboard,install_pointer_listener,pointer_input,selection_item}` | `workbench/effect_adapter.rs::tests::{client_normalization_rejects_non_positive_extents,client_normalization_accounts_for_letterboxing_and_css_size_only}` for the pure coordinate boundary | Keep adapter thin; browser listener wiring has no browser test by design |
| Pure action/effect transformation | Headless transitions: `geosolve-constraint-editor::{ConstraintEditor,RetainedEditorCoordinator}`; `RetainedEditorCoordinator::apply_editor_effect` applies revision-checked commits (`coordinator.rs:1122-1164`); demo adapter: `workbench/effect_adapter.rs::{dispatch_construction_effect,dispatch_inference_effect}` | Editor native corpus/golden tests (`qualification.rs:2041-2068`); demo tests `failed_construction_commit_retains_preview_across_terminal_clear`, `successful_construction_commit_clears_preview_on_terminal_clear`, `inference_dispatch_stages_commits_and_clears_typed_preview_state`, `m49_editor_cancel_and_invalid_completion_only_clear_or_retain_staged_preview` | Keep; direct native owner |
| Scene/presentation DTO rendering | `workbench/scene.rs::{svg_markup,viewport}` and `panels.rs::{tree_markup,host_state_markup,accepted_report_markup,accepted_redundancy_markup}` | `scene.rs` accepted identity/arc tests; `panels.rs` tree/lifecycle, M47 host-state, external-rebind, and accepted-diagnostic tests | Keep; direct native owner |
| Persistence | `workbench/persistence.rs::WorkspaceSnapshot` | `checkpoint_codec_round_trips_design_accepted_and_revisions`, `m49_checkpoint_codec_round_trips_accepted_a4_contact_state`, malformed/version/unknown-field codec test | Consolidated; only the workspace snapshot remains |
| Evidence/diagnostics | Runtime diagnostics: `panels.rs::{diagnostic_evidence_markup,accepted_report_markup}`; retained test-only pure serializer: `evidence.rs::serialize_typed_host_evidence` | Panels diagnostic tests plus `typed_host_evidence_serializer_contains_inputs_attempt_and_accepted_evidence` | Consolidated; no M40 package wrapper remains |
| Direct native tests | `#[cfg(test)]` workbench modules plus `geosolve-constraint-editor` native suite | `cargo test --locked -p geosolve-demo-web --all-features`; `cargo test --locked -p geosolve-constraint-editor` | Sole automated behavior qualification |
| WASM compile | Demo-web public WASM consumer | `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown` | Confirmed gate |
| Release Trunk build | `index.html` Trunk inputs and demo-web crate | `cd crates/geosolve-demo-web && trunk build --release` via `scripts/release-gate.sh:44-45` | Confirmed gate; no serving |

## Implementation outcome

- Startup restores only `geosolve.workbench.session.v1` as a complete design/accepted/revision
  `WorkspaceSnapshot`; absent storage starts empty and malformed storage keeps the existing
  user-visible fallback-to-empty notice.
- The design-only browser migration, duplicate M40 qualification adapter and M40 JSON/SVG package
  are absent. Their authoritative editor or reviewed-retirement owners are unchanged.
- Typed host evidence remains a pure deterministic serializer test over public coordinator/domain
  evidence. No browser location, clock or user-agent lookup was added.
- The distribution keeps licence and API compatibility artifacts but no longer copies the
  historical M32 performance record.
- No Cargo dependency or `web-sys` feature was removed: review confirmed every remaining item is
  used by the one workbench. No tool, equation, editor transition or domain API changed.

## Validation

The following commands passed on 2026-07-28 without browser automation or serving:

- `git diff --check`;
- `nix-shell shell.nix --run 'cargo fmt --all -- --check'`;
- `nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'` — 58 tests;
- `nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'` — 19 tests;
- `nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'`;
- `nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'`;
- `nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'`;
- `nix-shell shell.nix --run 'cd crates/geosolve-demo-web && trunk build --release'`.

The first workspace-test invocation reached the final sketch suites but exceeded the 120-second
shell timeout. The exact command was rerun with a larger tool timeout and passed completely.
Independent read-only verification found no functional loss or unowned survivor contract and
requested only that this inventory and the milestone status be updated from pre-implementation
wording.

## Acceptance disposition

M51 is complete. Exactly one non-authoritative workbench remains; persistence, evidence,
presentation and effects have direct owners; automated qualification is native/WASM/Trunk build
only; and no cleanup-era compatibility shim or dead test fixture remains in the survivor.
