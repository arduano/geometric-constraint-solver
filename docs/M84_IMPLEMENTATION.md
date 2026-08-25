<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 implementation ledger — Optional code/GUI sketch authoring

Status: **implementation complete; focused qualification passes and final clean release
qualification is pending**. No frozen M84 candidate, human UAT result or M84 Pages publication is
claimed. Accepted M83 remains public authority.

## Baseline and authority

- Product baseline: accepted M83 qualified source `ee18dbd`, approval descendant `2006c86` and
  exact Pages run `32817232564`.
- Controlling architecture: ADR 0041 and `docs/M84_GOALS.md`.
- Numerical authority remains the existing Rust materializer/solver plus independent validation.
- M84 adds structural authoring only: no primitive, relation, dimension, residual, priority,
  branch rule or JavaScript solve path changed.

## Files and public seams

- `crates/geosolve-sketch-code/` is the optional pure-Rust code-project layer. It owns bounded
  managed-v1 parsing, authenticated edits, artifact admission, declaration-family execution,
  typed semantic expansion, keyed reconciliation, overrides, composite history, bootstrap and the
  four bundled projects.
- `packages/geosolve-sketch-code/` is the optional TypeScript authoring/build package. It owns
  project-branded `FeatureRef`/`OutputRef`, descriptor-generated result types, `definePatch`,
  `p.each`, `p.mapRecord`, caller-side artifact compilation and positive/negative type fixtures.
- `geosolve-sketch-intent::IntentSession::{delegated_checkpoint,
  to_delegated_checkpoint_json,from_delegated_checkpoint_json}` is the neutral history-free seam
  used by a composite host. Unordered atomic patches apply retained rebinds before validating
  deletions, so member replacement is independent of patch-array order.
- `geosolve-demo-web::workbench::code_projects` composes the optional module through public intent,
  coordinator, ownership and scene APIs. Persistence carries either the unchanged plain
  workspace-v8 authority or a bounded authenticated code-project envelope.
- `scripts/verify-geosolve-sketch-code-package.sh` packages the real normalized Rust crate, checks
  all eight crate-owned runtime assets, extracts it and performs a locked offline build with local
  patches for GeoSolve crates that are not yet on crates.io. `scripts/release-gate.sh` runs this
  verifier and both TypeScript packages.

No core, geometry, sketch, linkage, intent or constraint-editor manifest depends on
`geosolve-sketch-code`; only the optional demo composition does.

## Implemented slices

### I1 — bounded managed source

- [x] Parse the exact `"use geosolve managed-v1";` lossless subset in pure Rust under the 4 MiB
  source bound.
- [x] Preserve comments, formatting and all unowned bytes while rewriting authenticated direct,
  aggregate, organization, declared-lens and override value spans.
- [x] Keep unsupported/syntax-invalid text as a non-authoritative draft; retain valid failed code
  intent above the last complete accepted scene.
- [x] Bootstrap a supported accepted GUI rectangle into truthful managed declarations; reject
  unsupported recipes instead of inventing lineage.

### I2 — custom artifacts and typed SDK

- [x] Compile custom TypeScript only in an explicit caller-owned Node step into canonical,
  data-only, digest/interface/ABI-pinned artifacts.
- [x] Admit only the central executable declaration-family catalog, bounded template DAGs,
  schemas, bindings, lenses and keyed collections. All nested structural values count toward the
  65,536-item bound; individual artifacts remain capped at 16 MiB.
- [x] Generate named rectangle results, mapped Fillet records and Polyline-derived corner
  collections from Rust declaration descriptors. Raw IDs, cross-project references, misspelled
  outputs and point/curve/corner mismatch fail TypeScript compilation.
- [x] Keep `patches/*.patch.ts` byte-identical through every GUI edit and never execute them in
  Rust, WASM, browser runtime or load.

### I3 — keyed reconciliation and native composition

- [x] Reconcile by invocation/template/member-key/output path. Reorder is non-semantic; insertion
  advances high-water; removal tombstones; reused retired keys receive a new generation.
- [x] Preserve unchanged logical nodes, ports, reservations, native geometry, computed Fillet
  ownership, overrides and ordinary outside GUI dependents across warm Apply, reload, Undo/Redo and
  generated-point edits.
- [x] Reject generated-output deletion when an outside dependent would dangle; never silently
  cascade or retarget it.
- [x] Lower direct Polyline/rectangle declarations and composed Fillets through the ordinary M83
  graph, cold materializer, native solver, computed-feature evaluator and independent validation.

### I4 — one code/editor transaction

- [x] `SketchCodeSession` owns project/files/artifacts/lock/program/expansion/reconcile state,
  overrides, current/accepted editor checkpoints and one outer Undo/Redo history.
- [x] Every persisted current, accepted, Undo and Redo nested checkpoint is host-validated before
  construction. Hostile session IDs cannot poison allocator high-water.
- [x] Pointer frames use the existing retained native preview without parsing, expansion,
  serialization or durable-panel rebuild. Terminal publication commits the authenticated newest
  accepted preview once, after complete cold/native parity.
- [x] Managed rectangle corner drags reverse-write `lowerLeft`/`upperRight`; generated Polyline
  point drags become generation-bound explicit overrides; Reset removes only that override.

### I5 — workbench, persistence and demonstrations

- [x] Add managed/custom tabs, Apply/Revert, diagnostics, artifact state, source ownership,
  generated-member groups, edit lenses, override badges and Reset-to-code controls.
- [x] Round-trip complete offline code-project authority/history through save/reload and Copy/Load
  repro under the 64 MiB project bound. Missing, tampered, corrupt and oversized inputs reject
  atomically.
- [x] Ship four genuine sessions: adaptive rounded Polyline; typed panel/keyed Fillets; GUI↔code
  braced frame; reusable mounting plate.
- [x] Prove an ordinary GUI reference CurveLength dimension on
  `brace.diagonals.rising` retains its native identity, follows a managed frame rewrite, updates
  its measured value, remains GUI-editable and shares exact outer Undo/Redo.

## Findings

### M84-F001 — generated terminal checkpoint carried stale branch metadata

Reproduction owner: retained code-workbench terminal publication. A generated-point drag could
produce complete valid accepted native geometry, but its reconstructed cold checkpoint retained
schema-derived Segment branch metadata from before the warm override. Strict rehydration then
rejected the valid release and the point snapped back.

Repair: after complete accepted-native parity, terminal publication installs the already staged
warm checkpoint as one outer action. The final explicit branch authority, evaluated computed
geometry, ownership and allocator state must match; only recomputable non-branch Fillet pick seeds
and evaluation revision stamps may differ. Focused generated-point, override/reset, rejected-final-
sample and Undo/Redo regressions pass.

### M84-F002 — aggregate reverse edit used the wrong authentication class

Reproduction owner: managed rectangle drag publication. Planning found the value-owned span for
`lowerLeft`/`upperRight`, but generic application authenticated it only as a top-level declaration
span and returned `managed rewrite does not target an authenticated owned span`.

Repair: `ManagedEditPlan` retains its semantic value target and applies through
`rewrite_managed_value`. Exact stale-source compare-and-swap remains mandatory. Sequential edits
reparse and reauthenticate fresh CST spans; comments, custom source and unrelated managed bytes
remain exact.

## Focused evidence observed before final nomination

- `cargo test --locked -p geosolve-sketch-code --all-features` — 44 unit and 21 integration tests
  pass, including parser/rewrite, artifact, descriptor, bootstrap, reconciliation, native
  composition, override, optional-boundary and TypeScript-interoperability owners.
- `cargo test --locked -p geosolve-demo-web --lib workbench` — 230 workbench tests pass, with six
  non-workbench tests filtered; the 26-test code-project subset and focused braced-frame reference
  dimension pass.
- `cargo clippy --locked -p geosolve-sketch-code --all-targets --all-features -- -D warnings` and
  the corresponding `geosolve-demo-web` focused command pass.
- `(cd packages/geosolve-sketch-code && npm ci --ignore-scripts && npm test)` passes its build,
  artifact fixtures, six runtime tests, type failures and managed-source compilation.
- `./scripts/verify-geosolve-sketch-code-package.sh` passes over the real 28-file normalized crate;
  all runtime assets ship and the extracted crate checks locked/offline.
- `./scripts/golden-authoring-scene-oracle.sh --require-clean` passes unchanged 271/271 authority,
  SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
- The separate M84 ledger passes with SHA-256
  `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.
- All-feature WASM check and the shell-provided actual-WASM suite pass. `cargo fmt --all -- --check`,
  `git diff --check`, focused warnings-denied Rustdoc and shell syntax checks pass.

These are development/focused results, not a substitute for the pending clean committed-source
release gate.

## Known bounds and truthful limitations

- Managed-v1 is intentionally a closed projectional subset. Arbitrary custom code is caller build
  input and has no browser runtime or general AST round-trip promise.
- Only supported accepted GUI recipes bootstrap to managed code; the bundled Braced Frame starts as
  a genuine managed code project rather than interactively invoking that bootstrap path. Its
  reverse GUI edits and ordinary generated-geometry dependents are nevertheless directly tested.
- Integration test sources which inspect workspace TypeScript/manifests are intentionally not part
  of the published Rust archive. Runtime library code and all eight required assets are
  self-contained and extraction-built.
- General ejection, arbitrary formulas/new constraints, arbitrary TypeScript execution, general
  topological naming, 3D/B-rep behavior and the deferred Offset redesign remain out of scope.
- Human discoverability, presentation feel and repeated real-browser drag responsiveness remain
  owned by `docs/M84_UAT.md`. Pages must remain M83 until that scorecard is explicitly approved.

## Remaining release sequence

1. Commit the implemented source and this evidence ledger with a clean worktree.
2. Run the complete locked workspace Clippy/Rustdoc/tests, actual WASM, both TypeScript packages,
   unchanged golden, Trunk build and clean Nix release gate.
3. Freeze the gate-produced seven-file `dist` without rebuilding, make it immutable and record its
   SHA-256 manifest/aggregate.
4. Verify the exact bytes locally, through `100.94.63.83:8080`, and with focused two-size browser
   checks; then bind them in `docs/M84_UAT.md`.
5. Stop for M84-U1 through M84-U10 and explicit approval. Do not publish M84 to GitHub Pages yet.
