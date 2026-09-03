<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 implementation: parallel workstreams and integration

Status: **In progress.**

## Integration order

1. Contact-range reconciliation and numerical continuation.
2. Dual-backend semantic golden-oracle parity.
3. TypeScript language service and declaration regeneration.
4. Explorer visibility hierarchy and pinned display controls.
5. Complete code-defined sample conversion.
6. Cross-feature repair, documentation freeze, clean qualification and immutable UAT publication.

The order is deliberate: the document exporter and managed compiler must use the final contact API
before they become oracle or sample authority. Language declarations are regenerated only after
that API is stable. Presentation state then composes over the integrated managed catalog.

## Workstream acceptance

### Contact range

- Model intrinsic `ContactDomain` separately from optional authored `ContactAdmissibleRange`.
- Infer bounded/periodic topology from the curve and expose supporting-line selection explicitly.
- Intersect intrinsic topology, selected locality and authored limits in native lowering.
- Project an excluded accepted seed onto the changed feasible interval, re-solve, and publish only
  finite independently validated geometry; otherwise retain exact prior authority with a specific
  diagnostic.
- Regress native and managed changes from parameter `0.8` to range `[0, 0.5]`, including exact
  terminal `0.5`, active-upper-bound evidence and unrelated-geometry stability.

### Semantic parity oracle

- Export public `SketchDocument` meaning to deterministic typed managed source without equations.
- Compile through pinned TypeScript, cold-materialize through the normal managed path and compare a
  canonical semantic snapshot to the native lifecycle snapshot.
- Cover authoring creation, dimension create/edit/Undo/Redo, computed Fillet and accepted-scene
  authority. Keep ordinary Cargo tests Node-independent; orchestration belongs in the golden script.
- Freeze an explicit, reviewed exclusion file and reject missing, duplicate or stale classifications.

### Language service

- Run TypeScript 5.9.2 in a dedicated worker using generated declarations from the pinned SDK.
- Integrate diagnostics, completion, hover and signature help into CodeMirror while keeping native
  materialization diagnostics separate.
- Bound files, bytes and result counts; discard stale replies and settle pending work harmlessly on
  project changes/disposal.

### Samples

- Deterministically export the 25 native-reference samples and retain the 12 curated code projects.
- Present one 37-entry managed catalog with no native/code split.
- Check source/envelope drift, cold materialization, independent residuals, semantic native parity,
  expected DOF, source/IR/artifact round-trip, edit/Undo and native/WASM scene parity.
- Preserve computed-feature meaning honestly; a missing document-side feature definition must not
  be silently represented as an ordinary curve.

### Explorer visibility

- Retain independent row/group choices plus global Construction filtering.
- Exclude hidden geometry from paint, picking, controls and annotations without touching solve or
  source authority.
- Provide mixed/inherited eye states, group bulk toggle, isolate/restore, keyboard labels and
  persistence across project restore and managed reprojection.

## Qualification

Run focused owner regressions before collateral suites. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

Record exact commands, counts, artifact hashes and any limitations before nomination.
