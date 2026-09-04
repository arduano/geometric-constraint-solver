<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M91 implementation: parallel workstreams and integration

Status: **Complete and publicly closed on 2026-09-04 after explicit supervising-user approval.**

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

Status: resolved by `d2170c46775b412785b3a2f288c170aba9ac8155`.

The first integrated optimized release build exposed a release-bundle defect, not a solver or
semantic-parity defect. Its WASM is `27,296,927` bytes against the strict `< 20 MiB` limit and its
distribution is `36,085,444` bytes against the strict `< 30 MiB` limit. All 37 compiler envelopes,
about `10.96 MB` of raw JSON, are embedded through `include_str!` in
`geosolve-sketch-code::demos`.

The bounded repair is pure Rust: deterministically zlib-compress only
`assets/{samples,demos}/*.compiled.json` at build time via the existing `miniz_oxide`; lazily
inflate each envelope once while enforcing the existing managed-wire ceiling before allocation,
exact declared output length, complete compressed-input consumption, successful zlib checksum/
terminal status and UTF-8 validity. Public `compiled_source: &'static str`, authenticated JSON bytes,
catalog ordering and deterministic semantics remain unchanged. Focused regressions compare all 37
reconstructed envelopes byte-for-byte with test-only raw assets and reject oversized input or
declared output, short/long lengths, truncation, corruption, trailing bytes and invalid UTF-8.

Those regressions and the complete release gate pass. The final release WASM is `18,368,160` bytes
and the ten-file distribution is `27,158,025` bytes, both below their strict ceilings. The historical
pre-repair sizes remain recorded above for defect provenance.

## M91-F002 — package verifier omitted one unpublished direct dependency

Status: resolved by `6d0155151133ba2540fd1dc4b2b071f141b86064`.

The first post-F001 clean gate failed at package verification with exit `101`: the packaged
`geosolve-sketch-code` crate directly depends on unpublished `geosolve-sketch-features`, but the
verification harness patched only three of its four direct local dependencies. The package itself,
solver behavior and semantic parity were not implicated.

The repair adds `geosolve-sketch-features` to the isolated package patch set and compares that set
against direct local dependencies extracted from the manifest, failing closed on future drift. A
focused `bash -n scripts/verify-geosolve-sketch-code-package.sh` and full package verification pass;
the final clean gate below is the qualification authority.

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

The nominated source passed the complete clean gate. The `--survey` and `--check` runs from the
pre-nomination audit also passed; the final gate's `--require-clean` reran the same 271-case inventory,
required every row to pass and matched the unchanged reviewed bytes.

### Final evidence

- source commit/tree: `6d0155151133ba2540fd1dc4b2b071f141b86064`,
  `972ad507c2cdfad2c9cd664e49feaf79ae381c81`;
- `cargo fmt --all -- --check`: passed;
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo test --locked --workspace --all-features`: passed all workspace tests and doctests;
- golden `--survey`: 271/271 `PASS`; golden `--check`: matched; golden `--require-clean`:
  matched with no non-pass row. The 272-line fixture (header plus 271 rows) remains SHA-256
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`;
- full release command:
  `env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true nix-shell shell.nix --run 'TMPDIR=/home/arduano/t ./scripts/release-gate.sh'`;
  result: exit `0` from the clean nominated source;
- release log: `/home/arduano/m91-gate.t8TTq0Bj/release-gate.log`, `706,478` bytes,
  SHA-256 `cc4f4580a0637cfddad5d96d7510d4a5f4dc707d99010b300f1a43451a7cc8cd`;
- package checks: all workspace package contents and the isolated `geosolve-sketch-code` package
  verifier passed; frontend: language-service Chromium 1/1, Vitest 94/94, manifest/licence/build
  contracts, production build and `validate:dist` passed;
- declaration drift: TypeScript `5.9.2` declarations matched SHA-256
  `32e84bbbfad1d9e3d704ab8a78c7df5b686e0315132018ff287abae55f3e9093`;
- samples: all 37 source-authoritative entries passed native-reference, cold-materialization,
  edit/Undo and source/IR/artifact checks; release-WASM transition/sample parity passed 2/2;
- dual-backend parity: every applicable row passed. The reviewed ledger contains exactly
  `constraint.external-line-collinear.*`, `constraint.external-point-coincident.*`,
  `dimension.profile-offset.*` and `spline.noncanonical-knot-topology.*`; exclusions are not passes
  and computed Fillet was exercised rather than excluded;
- release WASM: `/tmp/geosolve-m91-uat.17Q5LnSg/assets/geosolve_demo_web_bg-tvc8MGYX.wasm`,
  `18,368,160` bytes, SHA-256
  `6832d1b6fd984076a47440ccac82ece0dfd205a9e93346dfb3cbd6783240e961`;
- distribution: 10 files, 4 JavaScript, 1 CSS and 1 WASM, `27,158,025` total bytes;
  `validate:dist` and the stricter freeze inventory passed, with no `compiler-parity.html`;
- immutable snapshot/manifest: `/tmp/geosolve-m91-uat.17Q5LnSg`,
  `/tmp/geosolve-m91-uat.17Q5LnSg.sha256`, manifest SHA-256
  `b1e95b608b465a545791e55cc762052704f2d7139b8e4c3a9f8a68b0411a009b`;
- staging/live HTTP ledger SHA-256:
  `35531210b63479565e4350b44593ebe62d228e756e67378f829c99399e86bab4`;
  frozen Chromium qualification passed 20/20 on both endpoints;
- Tailscale service/PID/invocation/URL: `geosolve-m91-uat-18091.service`, `2142854`,
  `bf93a3f5dab84809a24fdc2db6f23f4f`, `http://100.94.63.83:18091/`.

The first frozen staging-browser attempt selected Playwright's unwrapped bundled binary and exited
before any test body because its host `libglib` was unavailable. The retained harness-error log is
`/tmp/geosolve-m91-freeze-evidence.xQrNE9UJ/staging-browser-harness-error.log`; rerunning with the
same system-Chrome wrapper used by the clean gate passed 20/20 on staging and live. This is harness
evidence, not an M91 defect. The protected M90 unit, process identity, snapshot manifest, inventory
and served root remained byte-identical before and after publication and were never restarted.
At nomination, no public push or GitHub Pages deployment was made and all 14 human UAT rows remained
unrun. On 2026-09-04 the supervising user's explicit blanket/composite approval accepted M91-U1
through M91-U14 as Pass at milestone level without claiming a separately logged row-by-row replay.
Pages publication and accepted-service retirement are complete. GitHub Actions run `33878060784`
(build `101039695274`, deploy `101042386558`) published artifact `9938976843` as deployment
`6265455733` from descendant `177227941af97f24307fe4229797bbd84857e458`. Downloaded
`artifact.tar` SHA-256 is `7dd107d94b22c1f364fbcd824980170cf9ce33f80b6cbefd6118967101b3783d`;
the exact ten-file aggregate is
`a8133280c286771ece4a2069880f417ea05f72980fbfa034cb774cb7a2156bad`. Eleven hosted paths and
public Chromium 20/20 pass. Both accepted M90/M91 listeners were then stopped. M91 is closed.
