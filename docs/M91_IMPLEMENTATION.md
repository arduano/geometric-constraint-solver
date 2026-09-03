<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 implementation: parallel workstreams and integration

Status: **Implementation complete; nomination evidence pending.**

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

Implemented. Intrinsic topology is retained independently from the optional authored range, with
supporting-line selection explicit. Authenticated prior retained/accepted state may seed only the
candidate solve. Feasible edits project to the changed interval and independently validate before
publication; infeasible edits retain exact source/scene/history authority and identify the contact,
requested range and failed feasible interval. Rejection history and later recovery are covered.

### Semantic parity oracle

- Export public `SketchDocument` meaning to deterministic typed managed source without equations.
- Compile through pinned TypeScript, cold-materialize through the normal managed path and compare a
  canonical semantic snapshot to the native lifecycle snapshot.
- Cover authoring creation, dimension create/edit/Undo/Redo, computed Fillet and accepted-scene
  authority. Keep ordinary Cargo tests Node-independent; orchestration belongs in the golden script.
- Freeze an explicit, reviewed exclusion file and reject missing, duplicate or stale classifications.

Implemented. The golden runner exports native semantic manifests, compiles source through the
pinned managed batch, validates the managed result and adds code-backend parity to applicable
authoring, lifecycle, computed-Fillet and accepted-scene cases. The exporter preserves current
Segment, Midpoint Line and Polyline branch directions; only authenticated historical contact-schema
migration normalizes the former legacy representation. Spline source is explicit across open/
periodic control B-spline and open/periodic control NURBS variants, and Parametric C2 retains exact
`firstRate`/`secondRate` rather than a reduced ratio.

The reviewed exclusion ledger is exactly:

- `constraint.external-line-collinear.*` — host snapshot/binding required;
- `constraint.external-point-coincident.*` — host snapshot/binding required;
- `dimension.profile-offset.*` — flattened document cannot reproduce the aggregate source closure;
- `spline.noncanonical-knot-topology.*` — typed recipe cannot preserve knot-inserted control
  structure.

Computed Fillet is always a parity requirement and is never excluded.

### Language service

- Run TypeScript 5.9.2 in a dedicated worker using generated declarations from the pinned SDK.
- Integrate diagnostics, completion, hover and signature help into CodeMirror while keeping native
  materialization diagnostics separate.
- Bound files, bytes and result counts; discard stale replies and settle pending work harmlessly on
  project changes/disposal.

Implemented with TypeScript `5.9.2` in a dedicated Worker. Generated declaration bytes are checked
for drift. Diagnostics, completion, hover and signature help use bounded files, UTF-8 byte budgets
and result counts; stale replies, worker synchronization failures, project changes and disposal
settle without publishing or mutating accepted sketch authority.

### Samples

- Deterministically export the 25 native-reference samples and retain the 12 curated code projects.
- Present one 37-entry managed catalog with no native/code split.
- Check source/envelope drift, cold materialization, independent residuals, semantic native parity,
  expected DOF, source/IR/artifact round-trip, edit/Undo and native/WASM scene parity.
- Preserve computed-feature meaning honestly; a missing document-side feature definition must not
  be silently represented as an ordinary curve.

Implemented as one 37-entry code-authoritative catalog: 25 deterministic native-document exports
plus 12 reviewed curated projects. Native constructors remain internal reference fixtures. All
entries have checked source/compiler envelopes, cold finite materialization, residual/DOF checks,
representative edit plus exact Undo, source/IR/artifact inverse mutation and release-WASM
frame/source parity. Scotch Yoke, Scissor Jack and Five-stage Scissor Tower have deterministic
managed mechanism drags. Computed Fillets retain computed ownership.

### Explorer visibility

- Retain independent row/group choices plus global Construction filtering.
- Exclude hidden geometry from paint, picking, controls and annotations without touching solve or
  source authority.
- Provide mixed/inherited eye states, group bulk toggle, isolate/restore, keyboard labels and
  persistence across project restore and managed reprojection.

Implemented. Stable semantic visibility keys survive managed reprojection and workspace restore.
Row/group choices, isolate/restore and the Construction filter compose into effective paint,
pick/control/annotation visibility while source, IR/artifact, solve participation, suppression and
history remain unchanged.

## Cross-workstream repairs

- M90's exact supplied workspace and capsule retain their historical hashes and restore through
  authenticated contact-schema migration. The migration-only normalization prevents one-ULP legacy
  branch-direction differences from rejecting mixed bootstrap reconstruction; current source remains
  lossless.
- Existing-point and curve-control terminals remain solver-instance overlay edits with no managed
  compilation. Generated structural edits retain the M90 source transaction and Undo/Redo boundary.
- The GUI construction adapter now names its recipe mapping explicitly and documents/tests the two
  intentional B-spline-to-NURBS collector mappings.
- The stable authoring/scene golden remains 271 data rows (`272` lines including its header) at
  SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

## M91-F001 — release bundle exceeds the accepted size envelope

Status: pending until the focused repair lands and is integrated.

The first integrated optimized release build exposed a release-bundle defect, not a solver or
semantic-parity defect. Its WASM is `27,296,927` bytes against the strict `< 20 MiB` limit and its
distribution is `36,085,444` bytes against the strict `< 30 MiB` limit. All 37 compiler envelopes,
about `10.96 MB` of raw JSON, are embedded through `include_str!` in
`geosolve-sketch-code::demos`.

The bounded repair contract is pure Rust: deterministically zlib-compress only
`assets/{samples,demos}/*.compiled.json` at build time via the existing `miniz_oxide`; lazily
inflate each envelope once while enforcing the existing managed-wire ceiling before allocation,
exact declared output length, complete compressed-input consumption, successful zlib checksum/
terminal status and UTF-8 validity. Public `compiled_source: &'static str`, authenticated JSON bytes,
catalog ordering and deterministic semantics must remain unchanged. Focused regressions compare all
37 reconstructed envelopes byte-for-byte with test-only raw assets and reject oversized input or
declared output, short/long lengths, truncation, corruption, trailing bytes and invalid UTF-8.

No final evidence below may be filled until F001 is committed, integrated and all gates run from the
same clean source.

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

### Final evidence template

- source commit/tree: `@M91_FINAL_COMMIT@`, `@M91_FINAL_TREE@`;
- `cargo fmt --all -- --check`: `@M91_FMT_RESULT@`;
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  `@M91_CLIPPY_RESULT@`;
- `cargo test --locked --workspace --all-features`: `@M91_WORKSPACE_TEST_RESULT@`;
- golden `--survey`: `@M91_GOLDEN_SURVEY_RESULT@`;
- golden `--check`: `@M91_GOLDEN_CHECK_RESULT@`;
- golden `--require-clean`: `@M91_GOLDEN_CLEAN_RESULT@`;
- full release command/result: `@M91_RELEASE_COMMAND@`, `@M91_RELEASE_RESULT@`;
- release log/bytes/SHA-256: `@M91_RELEASE_LOG@`, `@M91_RELEASE_LOG_BYTES@`,
  `@M91_RELEASE_LOG_SHA@`;
- package/frontend/declaration/sample checks: `@M91_PACKAGE_RESULTS@`,
  `@M91_FRONTEND_RESULTS@`, `@M91_DECLARATION_DRIFT_RESULT@`, `@M91_SAMPLE_RESULTS@`;
- dual-backend parity and reviewed exclusions: `@M91_PARITY_RESULT@`,
  `@M91_EXCLUSION_RESULT@`;
- release WASM artifact/bytes/SHA-256: `@M91_WASM_ARTIFACT@`, `@M91_WASM_BYTES@`,
  `@M91_WASM_SHA@`;
- distribution file count/bytes/result: `@M91_DIST_FILE_COUNT@`, `@M91_DIST_BYTES@`,
  `@M91_DIST_RESULT@`;
- immutable snapshot/manifest/aggregate: `@M91_SNAPSHOT@`, `@M91_MANIFEST@`,
  `@M91_SNAPSHOT_SHA@`;
- staging/live ledger and browser checks: `@M91_HTTP_LEDGER_SHA@`,
  `@M91_BROWSER_RESULT@`;
- Tailscale service/PID/invocation/URL: `@M91_SERVICE@`, `@M91_SERVICE_PID@`,
  `@M91_SERVICE_INVOCATION@`, `@M91_UAT_URL@`.
