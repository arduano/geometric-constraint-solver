<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 implementation ledger — Focused bug fixes and UAT follow-up

Status: **active and unaccepted; M86-F001 is implemented and human-approved, and M86-F002 is
implemented with replacement qualification pending**. Post-F002 human acceptance and
post-approval Pages publication remain pending. `docs/M86_GOALS.md` owns the contract.

## Finding ledger

### M86-F001 — Code-owned direct dimension edits reject after nested Intent mutation

Disposition: **repaired, clean-qualified, immutably nominated and accepted by the supervising
user's scoped “Looks good” assessment**.

Reproduction baseline is M85 closeout head `4b69a57`. Open PC Water Manifold, select
`code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`,
and change its Inspector value from `16` to `8`. Authenticated expansion provenance resolves the
alias to managed declaration `topScrewRail3Length` and exact source `target: mm(16)`. The displayed
failure says changed leaves lack semantic GUI-draft provenance; source, annotation and accepted
native target remain `16`, and no outer code-history entry is created.

The generic Inspector dispatcher edits the nested Intent scalar. Code-project checkpoint
publication then classifies the delegated editor delta and rejects because the only semantic GUI
draft provenance currently modeled there is for point-placement overlays. Existing managed scalar
lenses already know how to rewrite a declaration's `target`; the missing boundary is an
authenticated Inspector-to-managed-source route.

First regression owner: `geosolve-demo-web`'s optional code-project workbench composition. The
crossed browser adapter receives one thin dispatch test. No core/sketch residual, Jacobian or broad
golden change is warranted.

### M86-F002 — Computed Fillet radius surface hides its persistent native corner

Disposition: **repaired and proportionally qualified; clean replacement nomination and human UAT
remain pending**.

Reproduction baseline is F001 documentation head `bcc5ae4`. In Select mode, use two joined native
line/polyline spans with a computed Fillet, expose the selected Fillet radius affordance and sample
their retained shared endpoint where the radius rail also hits. `EditorScene::hit_test` returns the
persistent point, but both `pointer_move` and `pointer_down` return the computed `FeatureCorner`, so
the original corner cannot be selected or dragged.

The headless Select resolver unconditionally asked the blended Fillet resolver first. That resolver
correctly treats grip, spoke, continuation rail and arc as one radius surface for M75 painted-item
parity, but could not express the narrower source-corner exception. First regression owner is
`geosolve-constraint-editor`; no browser-only or solver-layer correction is warranted.

## Implemented design

### I1 — Exact owner regression

- [x] Load the checked-in PC Water Manifold through the ordinary code-project constructor.
- [x] Authenticate the exact alias and assert it resolves to `topScrewRail3Length`, direct
  `dimension.curveLength`, target leaf and source `target: mm(16)`.
- [x] Drive one Inspector-equivalent managed edit to `8`; assert only that source token changes,
  native target is `8`, accepted geometry is finite and independently Hard-valid, the stable alias
  is immediately reselected and exactly one outer code-history entry is added.
- [x] Assert Undo/Redo restore exact source, target and accepted checkpoint. Redo additionally proves
  the stable semantic alias remains present; durable UI selection across the history step is not
  asserted.
- [x] Add an accepted direct-diameter `5 -> 8` fixture with exact Undo, plus manifold diameter
  retained-invalid `5 -> 0`, byte-exact accepted-authority retention and exact Undo. No golden row
  is added.

Owning tests:

- `m86_f001_manifold_dimension_inspector_rewrites_managed_target_atomically`
- `m86_f001_direct_diameter_inspector_rewrites_managed_target_and_exact_undo`

### I2 — Managed Inspector route

- [x] Add private `CodeInspectorEditRoute::{NotClaimed, Claimed}` and
  `CodeProjectWorkbench::apply_managed_dimension_inspector_edit`, accepting the exact authenticated
  Inspector identity, selected alias, semantic target leaf and finite scalar.
- [x] Authenticate the current selected Inspector projection, materialized session identity,
  accepted alias-to-managed-declaration provenance, direct non-patch builder family and exact
  `Target[0]/Value` leaf. Only `dimension.curveLength` and `dimension.diameter` are claimed; opaque
  alias text is never decoded.
- [x] Require the managed `target` scalar and accepted Intent literal to agree, then reuse the
  existing `apply_scalar_lens(..., "target", value)` path. That ordinary path rewrites source and
  parses, expands, materializes, solves and independently validates before publication; native
  authority is never edited directly.
- [x] On accepted rematerialization, look up and immediately reselect the stable semantic alias.
  The adapter installs the complete returned `WorkbenchDocumentAuthority`, including revision
  high-water metadata. Save-time checkpoint publication observes the already-published checkpoint
  and adds no duplicate outer history row.
- [x] Reject dirty, retained-failed, pending semantic-point gesture, stale, foreign, unsupported,
  wrong-leaf, wrong-unit and non-finite work before nested Intent mutation.

### I3 — Adapter and failure retention

- [x] Route only authenticated code-owned direct dimension controls through I2 before the generic
  Inspector mutation path; preserve the existing generic dispatcher for GUI-owned targets and
  every unrelated Inspector field.
- [x] Add thin adapter regression
  `m86_f001_projectional_browser_inspector_routes_managed_dimension_to_source`. It proves exact
  browser dispatch, complete authority metadata installation, idempotent save/no duplicate outer
  history, and byte-exact accepted-authority retention after a zero target.
- [x] Prove failed rematerialization retains prior accepted native authority while retaining failed
  managed source and one exact outer Undo row. While a retained code failure is active, a further
  Inspector rewrite rejects until source correction or Undo. No partial accepted editor or
  selection publication occurs.
- [x] Keep existing GUI-owned reference-dimension and generic Inspector Undo/Redo tests passing;
  GUI-owned declarations return `NotClaimed` from the optional code route.

### I4 — Qualification and release

- [x] Run focused owner, diameter, adapter and GUI-owned collateral tests.
- [x] Pass format, diff hygiene, warnings-denied affected-crate Clippy and the relevant WASM target
  check.
- [x] Pass complete affected-crate/workspace tests, unchanged 271-case clean golden and the clean
  Nix release gate.
- [x] Freeze the exact no-rebuild candidate and exact-verify local/Tailscale bytes.
- [x] Record the supervising user's scoped “Looks good” assessment as acceptance of M86-U1 through
  M86-U5 without claiming a separately logged row-by-row replay.
- [ ] Publish and exact-verify Pages only after explicit supervising-user approval; retire retained
  services and close M86 afterward.

### I5 — Source-specific Fillet corner precedence

- [x] Add exact native/WASM hover/down parity regression
  `m86_f002_fillet_source_corner_remains_selectable_through_its_radius_surface`. It independently
  proves that both ordinary native point picking and the computed radius surface contain the same
  sample before requiring Point hover, Point selection and an ordinary Point gesture.
- [x] Reuse the feature-authoring owner's exact line/polyline span-endpoint query. In Select mode
  only, resolve a point-sized hit as a Fillet source corner when it is an endpoint of both current
  parents. Active explicit Coincident equivalence is accepted; solved coordinate proximity is not.
- [x] Keep `EditorScene::resolve_fillet_hit_with_policy` unchanged for active Fillet authoring,
  painted-radius reconciliation and public Fillet-aware picking. The exception is applied only by
  the shared Select hover/click resolver after an actual radius-surface hit.
- [x] Add explicit Coincident, coordinate-only non-topology and post-Apply persistent-feature rows.
  Keep the pre-existing unrelated-point-over-arc row, native-authoring contact row and radius
  affordance/gesture owners passing unchanged.
- [x] Pass focused native and WASM parity, all 429 editor unit tests plus every editor integration
  and doc test, warnings-denied editor Clippy, formatting, diff hygiene and unchanged 271-case
  golden authority.
- [ ] Pass the complete clean release gate, freeze and exact-verify a replacement local/Tailscale
  candidate, then complete the focused F002 human recheck.

## Files and API surface

- `crates/geosolve-demo-web/src/workbench/code_projects.rs` adds the private closed ownership enum,
  authenticated code-project route and focused owner regressions.
- `crates/geosolve-demo-web/src/workbench/mod.rs` asks that route before generic Inspector mutation,
  installs complete returned authority on acceptance, retains current authority on failure and owns
  the thin adapter regression.
- `crates/geosolve-constraint-editor/src/feature_authoring.rs` exposes its existing private
  line/polyline span-endpoint query to the parent module.
- `crates/geosolve-constraint-editor/src/lib.rs` adds the private source-corner point resolver and
  the Select-only precedence exception.
- `crates/geosolve-constraint-editor/tests/m75_hover_pointer_parity.rs` adds four focused native/WASM
  F002 rows covering shared identity, Coincident equivalence, coordinate-only overlap and an applied
  persistent Fillet.
- No public crate API, persistence/wire version, code grammar, Intent schema, solver API, residual or
  Jacobian changes.

## Commands and focused evidence

The following focused commands genuinely pass on committed M86 product source `9050424`:

```bash
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::m86_f001_manifold_dimension_inspector_rewrites_managed_target_atomically \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::m86_f001_direct_diameter_inspector_rewrites_managed_target_and_exact_undo \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::m86_f001_projectional_browser_inspector_routes_managed_dimension_to_source \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::code_projects::tests::braced_frame_gui_reference_dimension_survives_overlay_drag_and_history \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::projectional_browser_inspector_commits_accepted_edits_once_and_undo_redo_restores_them \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib --no-run
cargo fmt --all -- --check
git diff --check
cargo clippy --locked -p geosolve-demo-web --all-targets --all-features -- -D warnings
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
```

The exact manifold owner test covers accepted curve length `16 -> 8`, independently validated
finite geometry, one outer row, exact Undo, executed Redo and retained-invalid diameter Undo. The
small accepted-diameter fixture separately covers `5 -> 8`. The adapter and two existing collateral
tests prove browser routing, authority replacement, GUI-owned fallback and generic history remain
coherent. The complete qualification and freeze evidence below supersede the earlier focused-only
checkpoint; Pages publication is intentionally not claimed before human approval.

The following F002 commands genuinely pass on the implementation checkpoint represented by this
ledger:

```bash
cargo test --locked -p geosolve-constraint-editor --test m75_hover_pointer_parity -- --nocapture
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-constraint-editor --test m75_hover_pointer_parity \
   --target wasm32-unknown-unknown'
cargo test --locked -p geosolve-constraint-editor --lib \
  tests::fillet_affordances_validate_actions_and_expose_only_radius_canvas_handles \
  -- --exact --nocapture
cargo test --locked -p geosolve-demo-web --lib \
  workbench::tests::native_authoring_hit_remains_available_at_a_computed_fillet_contact \
  -- --exact --nocapture
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-constraint-editor --all-features
./scripts/golden-authoring-scene-oracle.sh --require-clean
cargo fmt --all -- --check
git diff --check
```

The native and WASM parity target passes 15/15 including four F002 rows. The full editor run passes
429 unit tests, every integration target and doc tests with only the three release-only performance
sentinels ignored. The complete clean replacement gate and freeze remain the next authority.

## Clean qualification and immutable nomination

Exact product source `90504245e19858f986d5f506f6e42d237e9665b5`, tree
`65e092540dab82618d1129229b566a2e791aa40c`, was clean when the complete Nix gate ran from
2026-08-28 12:04:44 through 12:24:38 AEST. The gate exits `0`; its 6,534-line, 436,494-byte log is
`/tmp/geosolve-m86-nix-gate.CSQkgk/release-gate.log`, SHA-256
`4b81a1d12df503ef220b10645f3f4266891edc1eeaf87f3647071e2a77c33584`. It passes warnings-
denied workspace Clippy, all-feature workspace tests and doc tests, the unchanged 271-case golden,
native/WASM parity, both TypeScript packages, warnings-denied Rustdoc, benchmark compilation,
release performance sentinels, cargo-deny licences, package verification and final Trunk release
assembly.

An earlier direct-PATH clean-gate attempt passed native workspace tests and the golden check, then
stopped with status `101` before executing its first WASM test because
`wasm-bindgen-test-runner` was absent from that shell's PATH. This is recorded as a harness error,
not product evidence, at `/tmp/geosolve-m86-gate.sMkCi9/release-gate.log` (SHA-256
`13c924c7286e248269802145d8f175dae838dec9baa7d7e5e30bf0f2d6382195`). The complete clean Nix
gate above supplies the actual WASM and release authority.

Without rebuilding, that gate's seven-file `dist` was copied and frozen read-only at
`/tmp/geosolve-m86-uat.vdEFAxsF`. It contains exactly seven regular files, zero symlinks or other
non-regular entries at directory/file modes `0555`/`0444`; source, copied, frozen and post-serve
manifests are identical. The ordered-manifest aggregate is
`1f872c6b51317ff810b48ab8965e1e0a0f6cb45feb01f5cbfe654eafbedd5882`; complete evidence is
`/tmp/geosolve-m86-freeze-evidence.6fU7WpCl`.

Local service PID `3879694`, invocation `c00b3e911ce54733be0b4b6a47756de1`, serves only that
snapshot at `http://127.0.0.1:18101/`. Retained Tailscale service PID `3879933`, invocation
`e56fde25abab4481a9bd7c825d4279b6`, serves the same snapshot at
`http://100.94.63.83:8080/`. Both complete eight-path HTTP ledgers are byte-identical at SHA-256
`b5ad691e14198791fa801281724fc3186161b2d2aae7358affea402e9a5f0acd`; every path returns 200,
zero redirects, exact MIME/length/body, no `Location` or `Content-Encoding`, and `/` equals
`index.html`. Both services remain active for UAT. Accepted M85 Pages remains public authority;
M86 publication and service retirement wait for explicit approval.

## Semantic-preservation ledger

F001 remains an optional code-authoring transaction adapter. F002 remains a private headless
Select-priority rule over already accepted scene semantics. Neither changes a solver equation,
dimension residual, Jacobian, hard/soft policy, computed branch state, persistence format or
ordinary GUI Inspector behavior. F001 success still requires ordinary Intent materialization,
native solve and independent residual validation; F002 derives only exact persistent endpoint and
active Coincident incidence and never treats solved coordinate proximity as topology.
