<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M45 UI cleanup investigation

## Status and parent disposition

This is the completed M45 source snapshot. `PLAN.md` now resolves its open decisions:
M46 froze direct owners/retirements, M47 deleted the broad M44 fixture/E2E after focused
replacement, M48 deleted M40 browser E2E after direct workbench qualification, M49 extracted
retained legacy semantics, and M50 deleted `#/dev/lab` and all remaining old E2E. No UI
feature-parity requirement survives; native domain behavior is preserved while legacy-only
browser delivery retired. M51-M52 subsequently completed; M53 human review later passed.

## Requirements

- Establish from source whether the desktop workbench has replaced the legacy playground at the normal entry point, while identifying any remaining explicitly routed legacy ownership.
- Preserve the product boundary: the desktop consumer is a non-authoritative WASM adapter over public sketch/editor APIs, and the headless editor—not either browser surface—owns deterministic interaction policy (`START_HERE.md:12-17`; `docs/adr/0029-headless-constraint-editor-state-machine.md:21-52`).
- Do not retire M45 host-semantics coverage without replacement: role/profile, activation versus dimension mode, immutable parameter and external recovery, retained accepted state, and finding capture must remain represented (`docs/M53_UAT.md#preserved-verification-points`; `docs/SCENARIOS.md:1339-1345`).

## Evidence and source pointers

### M45 snapshot: routing, default entry point and runtime ownership

- The one WASM start function reads the URL fragment and dispatches `#/dev/lab` to the playground; every other hash, including empty and `#/sketch`, dispatches to the workbench (`crates/geosolve-demo-web/src/lib.rs:4604-4624`; `crates/geosolve-demo-web/src/workbench/routing.rs:4-15,21-26`). The workbench is therefore the default/root UI, not a visual mock replacing a still-default playground.
- Both DOM roots are shipped in one `index.html`: the workbench root appears first and visible, while `#playground-root` is initially `hidden`; the workbench header exposes the explicit `#/dev/lab` link (`crates/geosolve-demo-web/index.html:16-30,126-458`). On the lab route, startup reverses visibility and calls the playground installer (`src/lib.rs:4614-4622`). Thus legacy remains a supported, separately routable runtime, not dead markup.
- Workbench runtime state is a `RetainedEditorCoordinator` plus presentation-only host/preview/problem state. It restores the workbench snapshot or legacy design key, translates DOM events into editor inputs/effects, and renders accepted scene plus design tree/status (`src/workbench/mod.rs:48-105,155-218,362-428,573-671`). Ordinary workbench persistence is `geosolve.workbench.session.v1`, containing design JSON, optional accepted JSON, and lifecycle high-water revisions (`src/workbench/persistence.rs:8-64`).
- The M44 fixture sidecar deliberately is neither canonical sketch state nor persisted workspace state (`src/workbench/host_state.rs:20-32`); saving is deliberately skipped while it is loaded (`src/workbench/mod.rs:730-744`). It exists to exercise host-state semantics and M45 evidence capture, not to replace normal workbench persistence.
- Playground runtime state is independently owned by `PlaygroundState`, backed by the older `SketchDocumentSession`, optional read-only spatial example state, draft/selection/viewport/branch controls, and its own storage keys (`src/playground.rs:1047-1074,1076-1162,9208-9302`). It restores `geosolve.sketch-playground.accepted.v1` with a backup key, installs its own DOM/pointer/wheel/keyboard/file listeners, and is an operational developer lab.
- The demo-web package still intentionally depends on the editor as well as sketch, linkage, core, and geometry (`crates/geosolve-demo-web/Cargo.toml:16-21`); the editor is a distinct workspace crate over sketch (`crates/geosolve-constraint-editor/Cargo.toml:1-19`; root `Cargo.toml:1-9`). This supports retaining the workbench as the current thin editor consumer rather than moving old browser policy into it.

### Feature/responsibility matrix

| Responsibility | Legacy playground / developer lab | Desktop workbench | Cleanup conclusion |
| --- | --- | --- | --- |
| Normal user entry and ordinary CAD-like editing | Explicit `#/dev/lab` only; broad legacy tool and inspector surface (`index.html:126-452`; `src/lib.rs:4613-4623`). | Default for root/unknown routes; coordinator-backed select, core drawing, constraints, dimensions, history, delete, Problems (`index.html:16-123`; `src/workbench/mod.rs:430-570`). | Workbench has replaced the playground **as default workbench UI**, but not as all-purpose diagnostic UI. Retain workbench. |
| Deterministic interaction policy | Legacy browser state owns selection, drafts, gestures and previews (`src/playground.rs:1047-1074`). | DOM is an adapter to `RetainedEditorCoordinator` effects (`src/workbench/mod.rs:221-310,362-428`). | Do not migrate legacy interaction policy; preserve headless-editor ownership per ADR 0029. |
| Core M40 desktop regression coverage | Not its target. | M40 browser suite starts root workbench and freezes 14 adapter/render/accessibility/persistence coverage groups (`e2e/m40.mjs:19-35,141-150,241-259`). | Retain workbench/editor seam; replace durable claims directly and delete the suite in M48. |
| M41–M45 host semantics and finding package | Legacy capsule input is expressly diagnostic only and defaults to empty host inputs (`src/playground.rs:143-156,209-232`). | Deterministic M44 fixture drives typed roles, batches, snapshots and rebind via coordinator; M44 E2E proves six host groups and captures M45 JSON/SVG/HTML (`src/workbench/host_state.rs:40-185,188-324`; `e2e/m44.mjs:18-25,171-293`). | M45 snapshot only; M47 preserves the semantics directly and deletes the fixture/E2E. |
| Broad alpha scenario catalog and focused UAT cards | Canonical, diagnostic, conic, spatial, construction, NURBS, profile, motion, and performance selectors are still exposed (`index.html:140-211`; `src/playground.rs:626-679`). | No example selector or corresponding broad catalog. | Unique legacy capability; migrate selected cases to focused non-legacy regressions before deleting lab. |
| Advanced interactive authoring controls | Quadratic/cubic/conics, directed branches, fillet and NURBS editing, JSON import/export/download/upload, autosave and scene capsules (`index.html:221-450`; `src/playground.rs:682-743,9208-9302`). | Only seven core editor tools and seven constraint choices; no equivalent advanced controls (`index.html:32-43,69-76`; `src/workbench/mod.rs:788-821`). | Deletion is blocked unless these are intentionally retired with replacement test evidence; do not assume workbench parity. |
| Spatial linkage/assembly diagnostic views | Loads read-only shaft-bearing/block-base spatial examples (`src/playground.rs:1140-1153`; `index.html:161-164`). | No linkage/spatial route or state. | Preserve solver semantics in native linkage tests; retire the old browser view at M50. |
| Legacy alpha E2E oracle | `m14.mjs` explicitly served `/#/dev/lab` and waited for `#playground-root` (`e2e/m14.mjs:110,380-403`); the full post-correction suite was incomplete, not passing evidence (`docs/M53_UAT.md#archived-pre-cleanup-record`). | M40/M44 independently targeted `#workbench-root` at root (`e2e/m40.mjs:141-150`; `e2e/m44.mjs:102-112`). | M14 was deletion-coupled debt at this snapshot. M49 closed its ledger; M50 deleted it. |

## Decisions / inferred constraints

### Explicit answer

**At the M45 snapshot, no: the legacy demo UI was not already fully replaced.** The
workbench had replaced it as the default/root CAD-like entry point, while
`#/dev/lab` retained a separate runtime and regression ownership. That evidence
justified ordered replacement, not indefinite retention; M49 closed semantic
ownership and M50 deleted the legacy runtime.

### Relevant component disposition

| File/component | Classification | Source-backed reason / prerequisite |
| --- | --- | --- |
| `src/workbench/**`, workbench DOM/CSS | **Retain and directly test** | They are the default routed desktop adapter (`src/lib.rs:4613-4624`). M47/M48 replace and delete the historical M44/M40 E2E qualification paths. |
| `src/workbench/host_state.rs` and M44 fixture buttons/markup | **Delete after replacement tests** | The archived M45 record says the deterministic fixture is temporary and scheduled for deletion/replacement, while preserving ten verification points (`docs/M53_UAT.md#preserved-verification-points`). Replace fixture-specific controls with durable focused host-state regressions/evidence first. |
| `src/playground.rs`, playground DOM in `index.html`, playground CSS, playground storage | **M49 extract; M50 delete** | These implement the explicit developer lab and its advanced/spatial/persistence capabilities (`src/lib.rs:4614-4622`; `src/playground.rs:1047-1074,9251-9302`). M49 preserves only direct semantic owners; no UI parity is required. |
| `e2e/m14.mjs` | **Delete after replacement tests** | It was coupled to `#/dev/lab` (`e2e/m14.mjs:380-403`) and is incomplete historical evidence, not a current M53 gate (`docs/M53_UAT.md#archived-pre-cleanup-record`). Inventory its assertions, retain only still-required behavior as native/editor/workbench-focused tests, then retire the suite. |
| Legacy frozen visual-fixture code and hidden `#scenario`/`#viewport` DOM in `src/lib.rs`/`index.html` | **Migrate then delete** | It contains its own live sketch/linkage demo states and UI controls (`src/lib.rs:114-197,418-603,1009-1149`; `index.html:460-479`) and is compiled for WASM/tests. It is not safe to infer deadness solely because the workbench is default; first identify its native tests and any remaining developer-lab/fixture callers. |
| Obsolete legacy-only CSS rules | **Delete after replacement tests** | `styles.css` explicitly keeps M39 workbench styles alongside legacy/frozen fixture and M13 playground styles (`styles.css:37-48,1889-1893`). Remove selectors only together with the DOM/render paths they style; otherwise M14/lab behavior silently degrades. |
| Any listed legacy component | **Delete only at its PLAN gate** | The M45 snapshot established no immediate safe deletion. M47-M50 now provide the required direct-replacement and explicit-retirement gates. |

### Parent-approved ordered cleanup slices

1. M46 freezes the assertion ledger; do not remove either runtime before ownership closes.
2. M47 converts the ten preserved points to focused direct regressions, keeps deterministic
   test/UAT capture over public APIs, and removes `HostState`/M44 E2E coupling.
3. M48 replaces durable M40 adapter/presentation/persistence claims directly and removes
   its browser suite and source scans.
4. M49 moves retained advanced/profile/spatial semantics to their native owners and records
   explicit retirement for legacy-only authoring, mobile, file, layout and timing delivery.
5. M50 deletes M14 E2E, playground/frozen runtime, hidden DOM/CSS and obsolete dependencies.

## Resolved questions

- Retained M14 claims are exactly those assigned a direct owner by the M46 ledger; all
  others require reviewed retirement before M50.
- Advanced sketch and spatial **semantics** stay in native domain tests. Their old browser
  controls/views retire; no replacement diagnostic route or UI parity is required.
- Finding capture is deterministic cleanup/UAT infrastructure over public APIs, not a
  stable product API. M62 may revisit release-surface ownership.

## Out of scope

- Changing routing, UI, tests, CSS, Rust code, persistence schemas, or milestone/status documents.
- Deciding product scope for advanced sketch/spatial developer-lab features; this investigation identifies the decision point only.
- Treating incomplete M14 execution as a passing or failing M45 criterion; its preserved thresholds and assertions require a parent-owned replacement/retirement decision.
