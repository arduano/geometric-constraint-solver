<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 implementation ledger — Focused bug fixes and UAT follow-up

Status: **active; M86-F001 is implemented and focused-qualified**. Complete affected-crate/
workspace and clean release qualification, unchanged-golden confirmation, immutable artifact,
Tailscale nomination, human acceptance and Pages publication are not yet claimed.
`docs/M86_GOALS.md` owns the contract.

## Finding ledger

### M86-F001 — Code-owned direct dimension edits reject after nested Intent mutation

Disposition: **repaired; focused qualification passes; clean candidate nomination and human UAT
remain pending**.

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
- [ ] Pass complete affected-crate/workspace tests, unchanged clean golden and proportional clean
  release qualification.
- [ ] Freeze the exact no-rebuild candidate, exact-verify local/Tailscale bytes and complete the
  pending UAT scorecard.
- [ ] Publish and exact-verify Pages only after explicit supervising-user approval; retire retained
  services and close M86 afterward.

## Files and API surface

- `crates/geosolve-demo-web/src/workbench/code_projects.rs` adds the private closed ownership enum,
  authenticated code-project route and focused owner regressions.
- `crates/geosolve-demo-web/src/workbench/mod.rs` asks that route before generic Inspector mutation,
  installs complete returned authority on acceptance, retains current authority on failure and owns
  the thin adapter regression.
- No public crate API, persistence/wire version, code grammar, Intent schema or solver API changes.

## Commands and focused evidence

The following focused commands genuinely pass on the shared uncommitted M86 implementation:

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
coherent. No complete `geosolve-demo-web --lib`, workspace, golden, release, freeze or publication
claim is made here yet.

## Semantic-preservation ledger

The repair remains an optional code-authoring transaction adapter. It changes no solver
equation, dimension residual, Jacobian, hard/soft policy, branch state, persistence format or
ordinary GUI Inspector behavior. Success still requires ordinary Intent materialization, native
solve and independent residual validation; rejection preserves complete prior accepted authority.
