<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M50 implementation ledger: old E2E and legacy application purge

## Status

Complete as of 2026-07-28. M50 removed the final old M14 E2E and legacy application after M49
established direct owners or reviewed retirement for every retained legacy claim. Parent final
gates and independent read-only verification pass.

## Requirements and fixed boundary

- The acceptance boundary is absence of all old E2E scripts and Chromium/CDP/server/profile/
  download infrastructure, plus absence of `#/dev/lab`, the playground/frozen legacy
  application, hidden DOM, obsolete CSS, legacy persistence glue and legacy-only tests
  (`ACCEPTANCE.md:860-864`).
- The survivor is one non-authoritative desktop workbench in `geosolve-demo-web`, consuming
  public sketch and headless-editor APIs; direct native/WASM tests are the qualification path.
- Do not use browser automation, Chromium/CDP, HTTP serving, DOM scraping, screenshots,
  wall-clock browser timing, or source-substring scans as cleanup qualification
  (`START_HERE.md:57-62`; `M46_DIRECT_TEST_REPLACEMENT.md:172-185`).
- The purge must not reintroduce browser-owned equations or interaction policy, and may not
  delete the workbench or its direct owner tests.

## Deleted runtime/assets/scripts

Post-purge filesystem inventory (read-only):

| Expected deleted artifact | Post-purge evidence |
| --- | --- |
| `crates/geosolve-demo-web/e2e/m14.mjs` and the remaining `e2e/` directory | Neither appears in the demo-web recursive file inventory. |
| `src/playground.rs` and legacy `lib.rs` route branch | Demo-web source inventory contains only `src/lib.rs` and `src/workbench/**`; `lib.rs:3-17` installs only `workbench::wasm`. |
| Legacy hidden DOM and legacy CSS | `index.html:16-91` contains one visible `#workbench-root`; `styles.css:1-262` is workbench-scoped (`.wb-*`/`.workbench`) styling. |
| M40/M45 serving scripts | `scripts/` contains only `release-gate.sh`; `serve-m40.sh` and `serve-m45.sh` are absent. |
| Release-gate Node E2E/environment cleanup | `scripts/release-gate.sh:14-45` has Cargo, package/licence and Trunk-build gates only; no Node, E2E or `GEOSOLVE_E2E` use. |
| CDP/server/profile/download runtime | No demo-web source/assets/script path containing an E2E directory, Node script or legacy route survives. The remaining `Trunk.toml:[serve]` is ordinary build-tool configuration, not an E2E HTTP test server. |

The parent Git audit confirms tracked deletion of `e2e/m14.mjs` and `src/playground.rs`,
modification of the demo-web manifest/assets/startup, lockfile dependency pruning and removal of
the release-gate E2E invocation. The surviving `src/workbench/**` tree is part of the broader
uncommitted M39-M49 stack, so aggregate Git status is not used to attribute that earlier work to
M50. `git diff --check` passes.

## Dependency and survivor audit

- **Single runtime owner:** `src/lib.rs:3-17` has no route parser or playground module; on
  `wasm32` its sole entrypoint obtains `Document` and invokes `workbench::wasm::install`.
  `index.html:16-91` has only `#workbench-root`; no hidden alternate root or `#/dev/lab` link.
- **Direct owners retained:** `workbench/mod.rs` composes public
  `geosolve-constraint-editor`, `geosolve-core` and `geosolve-sketch` DTO/session APIs
  (`:40-59`). `effect_adapter`, `panels`, `persistence`, `scene` and `evidence` retain unit
  tests; `mod.rs:814-825` directly compares the editor M40 report and checksum to its native
  golden. Representative direct workbench tests cover coordinate normalization
  (`effect_adapter.rs:137-189`), construction/inference effect lifecycle (`:191-295`), strict
  snapshot codec/fallback and explicit A4 contact state (`persistence.rs:92-217`), accepted
  scene identity (`scene.rs:398`), presentation/host-state fixtures (`panels.rs:829-1242`) and
  deterministic evidence serialization (`evidence.rs:299-310`).
- **Manifest/lock survivor dependencies:** `geosolve-demo-web/Cargo.toml:16-21` retains only
  public domain/editor crates plus `serde`/`serde_json`; target-specific dependencies are
  `console_error_panic_hook`, pinned `wasm-bindgen = 0.2.121`, and pinned `web-sys = 0.3.98`.
  `Cargo.lock:586-598` confirms exactly that demo-web dependency set. No Node/CDP/HTTP test
  dependency exists in Cargo metadata.
- **Exact `web-sys` surface:** `Document`, `DomRect`, `Element`, `Event`, `EventTarget`,
  `HtmlElement`, `HtmlSelectElement`, `KeyboardEvent`, `MouseEvent`, `PointerEvent`, `Storage`,
  `Window` (`Cargo.toml:26-39`). `workbench/mod.rs:51-57` uses these for the one browser adapter;
  `platform.rs:3-6` is the minimal `Window` lookup. There are no `Blob`, `Url`, `File`,
  `HtmlInputElement`, `Request`, `Response`, `WebSocket`, or browser-process/CDP features.
  `web-sys` correctly remains because the surviving WASM workbench requires DOM/event/storage
  adaptation, not because legacy E2E infrastructure survives.
- **Asset caveat:** `dist/` remains in the filesystem with built WASM/JS/CSS/HTML and copied
  release documents. It is generated output and was not inspected as source; its tracked/ignored
  status and freshness require the parent Git audit.

## Repository-reference audit

### Runtime/source references: absent

No executable Rust, shell, HTML/CSS, TOML or JS/MJS source reference to `e2e/m14.mjs`,
`#/dev/lab`, `serve-m45.sh` or `GEOSOLVE_E2E` remains. The recursive demo-web inventory has no
`e2e/`, playground source, or legacy route asset. `release-gate.sh` has no Node/browser/CDP/
server/profile/download invocation. The only source-tree `serve` match is `Trunk.toml:[serve]`,
which configures Trunk's ordinary development behavior and is not an old test server.

### Truthful historical/cleanup evidence (retain)

- Frozen/implemented cleanup ledgers: `docs/M46_DIRECT_TEST_REPLACEMENT.md` (including its
  frozen deletion gates and historical M14 command), `docs/M47_IMPLEMENTATION.md`,
  `docs/M48_IMPLEMENTATION.md`, `docs/M49_IMPLEMENTATION.md`,
  `docs/M45_{CLEANUP_PLAN,TEST_FIXTURE_CLEANUP_INVESTIGATION,UI_CLEANUP_INVESTIGATION}.md`, and
  `docs/M46_REBASE_INVENTORY.md`. These explicitly describe snapshot state, ownership or a
  completed/assigned cleanup slice; their old path references are evidence, not commands.
- Completed milestone records and ADR history: the M13/M14 and M39--M44 completion sections in
  `PLAN.md`/`ACCEPTANCE.md`, `docs/M14_PERFORMANCE.md`, `docs/M29_SCALE_PERFORMANCE.md`,
  `docs/M32_SCALE_PERFORMANCE.md`, `docs/M40_HEADLESS_QUALIFICATION.md`,
  `docs/M44_IMPLEMENTATION.md`, the archived M45 section in `docs/M53_UAT.md`,
  `docs/adr/0010-disposable-sketch-playground.md`,
  `docs/adr/0020-associative-linear-constructions.md`, and historical
  `CHANGELOG.md` entries. Chromium/CDP/profile/download terminology there records past gates,
  runs or retired scope and must not be treated as a current qualification path.
- `ACCEPTANCE.md:860-864` is a truthful current acceptance requirement: it requires absence
  without providing an obsolete command. Historical mentions in completed records remain evidence.

### Current-state records after the completed cleanup sequence

`PLAN.md`, `ACCEPTANCE.md`, `START_HERE.md`, `ARCHITECTURE.md`, `README.md` and `CHANGELOG.md`
now record M50-M53 complete and M54 active. `docs/SCENARIOS.md`, `docs/M14_PERFORMANCE.md`,
`docs/M29_SCALE_PERFORMANCE.md` and `docs/M32_SCALE_PERFORMANCE.md` use historical or
direct-successor wording. `docs/M40_UAT.md` remains an archived pre-cleanup candidate record. The
historical M45 candidate evidence is archived inside `docs/M53_UAT.md`, whose distinct current
sections retain the completed M53 procedure and scorecard; old browser/serving details remain historical
evidence, not current instructions.

### Exhaustive token disposition

- **`e2e/m14.mjs`:** no executable match; remaining text is historical/cleanup evidence in
  M45-M50 ledgers and earlier milestone records, plus the completed M50 gate in `PLAN.md`.
  No current document directs the reader to run the deleted script.
- **`#/dev/lab`:** no runtime source match. Remaining text is explicit snapshot/cleanup evidence,
  historical completion evidence or the M50 absence requirement. Current status documents
  record the route's deletion; `ACCEPTANCE.md` correctly requires absence.
- **`playground` (many historical matches):** source/assets contain no runtime implementation;
  remaining mentions fall into (a) immutable M13/M14/ADR/completion history, (b) M45--M49
  evidence ledgers, or (c) current cleanup statements. Parent must correct only unlabelled
  current cleanup/completion statements. Current route and architecture prose now describe one
  workbench; historical milestone/ADR records remain intact.
- **`serve-m45.sh`:** all remaining text is historical assignment, investigation or archived
  UAT evidence. No script path or current serving instruction survives.
- **`GEOSOLVE_E2E`:** no environment use survives; remaining text is deletion-history evidence
  or this audit record.
- **Chromium/CDP/server/profile/download infrastructure:** no executable implementation match
  survives. Remaining mentions are historical outcome/retirement evidence in M14/M29/M32/M40/
  M44/M45--M49 records, plus explicit current prohibitions/absence requirements. The actionable
  archived UAT documents explicitly identify their browser procedures as historical and
  superseded; no current qualification instruction survives.

## Validation plan

Run from the workspace root; none serves the app or launches a browser:

```bash
nix-shell shell.nix --run 'git status --short'
nix-shell shell.nix --run 'git diff --name-status'
nix-shell shell.nix --run 'git diff --check'
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && trunk build --release'
```

For parent-only independent deletion verification, use a reviewed repository-text search that
excludes historical records or classifies them as above; do not make a raw substring scan itself
the behavioral qualification gate.

## Open cautions

- Generated `crates/geosolve-demo-web/dist/` assets are present in the filesystem and require
  review only as artifacts; this ledger must not modify generated output.
- Historical cleanup records necessarily mention deleted infrastructure. Those records are
  evidence, not current execution instructions unless wording still directs users to run it.
- M50's `PLAN.md` gate intentionally requires no live executable/current instruction; historical
  ledgers retain deleted names as classified durable evidence.

## Completion record

M50 is complete. Formatting/diff, focused editor and demo-web tests, the complete locked
all-feature workspace suite, warnings-denied workspace Clippy, the all-feature WASM check and
release Trunk build pass. Independent read-only verification passed after correction of a stale
reference table. No browser automation or serving was run. M51-M52 subsequently completed.
