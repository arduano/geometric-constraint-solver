<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M89 implementation ledger

Status: **Superseded by M90's closed typed clean-break contract.** M89-F004/F005 implementation,
provisional dirty-tree mechanical qualification and its immutable F005 nomination completed. The
supervising-human Compass Rose retest, targeted manual preflight and M89-U1 through M89-U8 were not
run and are not retrospectively passed or waived. All M89 snapshots are preserved historical
evidence. ADR 0043 remains the architecture record; this is not standalone M89 clean-source
qualification or human acceptance.

## Files and APIs

- `packages/geosolve-sketch-code/src/managed-v2.ts` owns the closed parser, normalized printer,
  source-site instrumentation, deterministic recorder execution, compiler-envelope construction
  and prepared IR mutation. `scripts/compile-managed-v2-deno.mjs` is the pinned native compiler
  host; browser and Deno fixtures are generated from the same implementation.
- `crates/geosolve-sketch-code/src/managed_v2.rs` admits bounded
  `ManagedSketchIrV2`, `ExecutedSketchArtifactV2` and `CompiledManagedSourceV2` envelopes only after
  independent canonical JSON, digest, source-site, result, generated-member, group, suppression and
  value-consumer validation. It also reconstructs authenticated source declaration closures.
- `crates/geosolve-sketch-code/src/prepared_mutation.rs` publishes the digest-bound
  `PreparedManagedV2MutationRequest`/`Receipt` boundary, exact value compare-and-swap, declaration
  insertion, reorder, suppression and deletion. One shared result-leaf reconstruction owns both
  cold-envelope and prepared-mutation validation. `src/editor_insertion.rs` reverse-projects
  accepted editor deltas into typed source declaration plans, including explicit Profile Offset
  closures, compact exactly proven Polyline/axis declarations, one descriptor-projected compact
  declaration for every other geometry recipe, 31 non-axis standalone constraint variants and
  direct computed Fillet declarations with exact lexical parent and branch/contact state.
- `crates/geosolve-sketch-code/src/expansion.rs`, `composition.rs`, `project.rs` and `session.rs`
  retain Rust expansion/materialization, managed-v1 isolation, accepted authority and persisted
  monotonic declaration-name high-water state. An explicit `representation: "singleCurve"`
  Polyline lowers to one native recipe while the absent marker preserves legacy composite
  behavior. `expansion.rs` admits `$.geometry.recipe(...)` only after reconstructing the exact
  native draft from central Rust Intent descriptors and authenticating its complete inputs,
  definition fields, writable instance values and semantic result paths. It applies the same
  fail-closed descriptor authentication to `$.constraint.recipe(...)` and lowers direct
  `$.computed.filletSet(...)` only from lexical native spans plus complete explicit state.
- `crates/geosolve-sketch-intent/src/schema.rs` and `graph.rs` expose the central projection
  descriptors needed for that authentication; they remain the sole catalog for recipe cardinality,
  semantic paths, writable leaves and result kinds. TypeScript contains no copied geometry schema
  or equation.
- `packages/geosolve-sketch-code/src/index.ts` provides typed data-only
  `$.geometry.recipe(...)`, `$.constraint.recipe(...)` and direct
  `$.computed.filletSet(...)` surfaces, while `src/managed-v2.ts` parses, executes, normalizes and
  mutates their dynamic result trees and source-owned suppression. Rust independently re-derives
  those trees and accepted semantics rather than trusting TypeScript declarations.
- `crates/geosolve-demo-web/src/workbench/code_projects.rs` and `bridge.rs` route source, canvas,
  Explorer lifecycle and compiler-correction actions through one prepared transaction. Direct
  Fillet selection resolves to its complete authenticated declaration; radius edits restore that
  selection after cold rematerialization. The React compiler host returns the complete candidate;
  it never publishes solver or scene authority.
- `crates/geosolve-headless/tests/m87_headless.rs` proves a compiled managed-v2 project reloads,
  inspects and renders deterministically without browser or Deno. The TypeScript and Rust fixture
  corpus covers round-trip, Unicode spans, direct value fan-out, lifecycle, v1 upgrade and Profile
  Offset closure reconstruction.
- `crates/geosolve-sketch-code/tests/compact_geometry_recipe_roundtrip.rs` covers the complete
  25-variant geometry catalog across the direct Segment/Polyline paths and the remaining 23 compact
  recipe paths, including real canvas construction, reverse projection, cold Rust replay, exact
  semantic result paths, finite accepted geometry and independent Hard-residual validation.
- `crates/geosolve-sketch-code/tests/compact_constraint_recipe_roundtrip.rs` audits all 35
  persistent constraint variants, cold-replays the 33 standalone-replayable rows and proves the
  exact two host-external variants remain gated. `tests/direct_fillet_set_lowering.rs` and the
  direct-Fillet cases in `tests/m89_editor_insertion.rs` cover exact lowering, cold replay, branch/
  contact state, keyed Polyline parents, source-owned suppression and malformed-state rejection.
- `crates/geosolve-demo-web/src/workbench/bridge.rs`, the generated managed-v2 Fillet fixtures and
  `frontend/tests/e2e/workbench.spec.ts` cover whole-declaration navigation, radius edit selection,
  suppress/restore, delete/Undo and exact source/scene publication through the real bridge and
  release-WASM boundary.

## Implemented behavior

1. Parsing owns lexical structure and attached comments; deterministic execution owns the complete
   declaration/result graph, source-site authentication and value-consumer provenance. Managed-v2
   has neither `p.editLens` nor a terminal `$.outputs(...)`.
2. A structured edit is prepared against exact accepted source/IR/artifact/session digests. Rust
   admits the returned candidate only after exact semantic-delta checks, cold expansion,
   materialization, solving and independent validation. Source, IR, artifact, scene and one outer
   history row then publish together; preview, cancellation, stale input and rejection publish none.
3. Every enabled geometry, every standalone-replayable constraint, dimension and operation
   authoring recipe becomes managed source under `Canvas additions`. The geometry contract is
   complete and concise across all 25 variants: Segment uses `$.geometry.line(...)`; exact native
   Polyline uses `$.geometry.polyline(...)`; the other 23 variants use lossless
   `$.geometry.recipe(...)`. Of 35 persistent constraint variants, Horizontal/Vertical retain
   concise SDK builders and the other 31 standalone variants use
   `$.constraint.recipe(...)`. `ExternalPointCoincident` and `ExternalLineCollinear` remain gated
   because managed source lacks immutable host snapshot/binding authority. Names use a persisted
   monotonic high-water allocator and are never reused after deletion or Undo. There is no GUI-
   owned duplicate or dishonest fallback.
4. Explorer order is real source/IR order. Independent roots can move; a move that places a
   consumer before its dependency refuses without mutation. Top-level and generated-member
   suppression are explicit source state, and delete/reload/Undo/Redo reproduce the same authority.
5. One gesture owns one **user-facing declaration closure**, not necessarily one lexical
   declaration. Ordinary recipes have one root. Profile Offset retains an explicit aggregate helper
   plus its root; ownership is reconstructed only from exact authenticated IR semantics. Private
   helpers stay selectable, editable and source-navigable but cannot independently move, suppress
   or delete. Root reorder/delete is atomic over the closure, while a shared aggregate remains an
   independent root.
6. A checked-in or untouched managed-v1 project remains byte-preserved. Its first source-aware
   structured edit upgrades only the active copy; a persisted source/GUI hybrid rejects rather than
   silently dropping geometry. Compiler failure retains accepted paint and authority plus the
   candidate source draft and positioned diagnostic for correction.
7. A canvas-authored computed Fillet becomes one direct `$.computed.filletSet(...)` declaration.
   Its lexical native-span parents, exact parameter/winding/neighborhood/normal-side/retained-
   endpoint/periodic-anchor/endpoint-order/sweep state, radius, display name and source-owned
   suppression reconstruct the same computed owner on cold replay. Selection owns the whole
   authenticated declaration; radius editing restores selection; suppress/restore and delete/Undo
   reproduce exact source and accepted authority. F004/F005 add no Offset behavior; the Profile
   Offset closure above is unchanged.

## Mathematical behavior

M89 adds no residual equation, geometry family, tolerance, rank rule or inferred branch. TypeScript
records equation-free data only. Rust remains sole owner of Intent expansion, native
materialization, solving, explicit branch/domain checks and independent residual validation. A
success-like candidate still requires finite accepted geometry and normalized Hard residual at
most `1e-9`; unsupported, stale, invalid or non-finite candidates retain the exact previous
accepted authority.

## Mechanical evidence

The following commands passed through the pre-F002 implementation nomination. The proportional
post-F002 reruns and immutable compact-source candidate are recorded in the F002 section below:

- `cargo fmt --all -- --check` and `git diff --check`.
- In `packages/geosolve-sketch-code`, `npm run test:mutation` passed `16/16`,
  `npm run test:runtime` passed `25/25`, and `npm run test:deno` passed `2/2` browser/pinned-Deno
  parity tests.
- In `packages/geosolve-intent`, `npm test` passed `40/40`.
- `cargo test --locked -p geosolve-sketch-code --all-features` passed all `104` unit tests plus all
  integration and doc tests.
- `cargo test --locked -p geosolve-headless --test m87_headless` passed `11/11`, including
  byte-deterministic managed-v2 inspect/report/logical-scene/SVG/PNG reconstruction.
- `cargo test --locked -p geosolve-demo-web --lib --all-features` passed `357/357`.
- `RUST_MIN_STACK=16777216 cargo test --locked --workspace --all-features` passed the complete
  locked all-feature workspace suite.
- `RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib \
  managed_v2_profile_offset_closure_is_nested_and_mutates_as_one_source_block` passed the focused
  workbench closure lifecycle after the final helper extraction.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` passed. A later
  WASM-only test-boundary correction only cfg-gates native `geosolve_headless` assertions; final
  post-correction warning qualification is owned by the release gate below.
- In the pinned Nix toolchain, the locked demo-web actual-WASM tests passed `2/2`, then
  `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown`
  passed. The target emitted its expected dead-code warnings; there was no compile error.
- In `crates/geosolve-demo-web/frontend`, `npm test` passed `54/54`. Manifest, licence,
  production-build and distribution validation passed through `npm run check:manifest`,
  `npm run check:licenses`, `npm run test:build`, `npm run build` and `npm run validate:dist`. The
  build used optimized release WASM; `npm run test:e2e` against the optimized port-`4189` server
  passed `13/13`.
- `./scripts/golden-authoring-scene-oracle.sh --survey`, `--check` and `--require-clean` all passed.
  All `271` reviewed authoring/scene rows remain clean and the checked-in fixture is unchanged.

`NO_COLOR=true nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh'`
completed successfully with exit `0`. This is provisional **dirty-tree** qualification of the
exact working-tree candidate, not clean-source qualification. The gate re-passed formatting and
diff hygiene, warnings-denied all-target/all-feature Clippy, the complete locked native/all-feature
workspace suite, the unchanged clean `271`-row golden, actual WASM and the WASM check, Rustdoc,
benchmark compilation and performance owners, package/licence checks, both TypeScript packages,
frontend `54/54`, optimized release-WASM/Vite assembly and validation of the nine-file distribution.
The performance leg includes the M83 interaction owner and the 256-moving-body sparse crossover,
which completed in `132.36 s`.

## Withdrawn initial UAT candidate

M89-F001 withdrew this exact candidate after the Compass Rose Polyline flow failed to publish any
code. Retain the values below as historical reproduction evidence only. They are not current UAT
authority; old PID `825513` was retired only after the replacement passed temporary frozen-byte
and browser proof, while the snapshot remains preserved.

- Frozen path: `/tmp/geosolve-m89-uat.TIPyWl`; exactly nine regular files, zero symlinks,
  two directories at `0555` and files at `0444`. The final distribution was built at
  `2026-09-01 21:08:46 AEST`; freeze and manifest capture completed at
  `2026-09-01T21:10:00+10:00`.
- External manifest: `/tmp/geosolve-m89-uat.TIPyWl.sha256`; relative-path-sorted manifest aggregate
  `5b842c6541261a9386c6a060475350712326a467137f166b4c02d8782635ff6a`.
- Optimized release-WASM SHA-256:
  `04a8b6504db1e8ce68f9c8cec864d293d95e07394dff302bc877782b4ef4ce04`.
- Evidence directory: `/tmp/geosolve-m89-freeze-evidence.h3dbAC`. The pre/post-copy distribution, frozen and
  live-Tailscale manifests are byte-identical. The HTTP ledger SHA-256 is
  `a80f45fab8452c5ae6d605490633069a1494d5c222c66eb86a62f19d30066077`; root, explicit index and
  every distribution file return HTTP `200` with exact bytes and expected media types. Independent
  strict HTTP audit (zero redirects, no `Location` or `Content-Encoding`, root equal to frozen
  index) is `live-http-strict.tsv`, SHA-256
  `280f931df5793c39203ee266c9b87c30cf5040a40a5166711581cfd2a300a25f`.
- Tailscale endpoint: `http://100.94.63.83:18089/`; unit
  `geosolve-m89-react-uat-current.service`, PID `825513`, started
  `2026-09-01 21:10:15 AEST`, invocation `9d5bdefb7e674b5792d3c95d6d1b77b1`, historically served
  only the frozen path.
- Final real optimized Playwright passed `12/12` before freezing via
  `GEOSOLVE_E2E_PORT=4189 GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome \
  npm run test:e2e` inside `nix-shell`. An earlier launcher-only attempt omitted the browser path
  and reported `12` failures at about `1 ms` because the default executable lacked `libglib`; no
  test body ran, so it was environment setup rather than product evidence. The subsequent plain
  `npm run build && npm run validate:dist -- ../dist ./` passed and restored the normal nine-file
  product distribution, which was frozen without rebuilding.
- Build/tool identity: Rust/Cargo `1.95`, Node `24.19`, npm `11.17`, Chrome `151`, Cargo
  `--release`, wasm-bindgen `0.2.121` and wasm-opt `131 -Oz`.
- Source basis is HEAD `71a51ee534f034e2328a07e0d80f9a9ee5e0fc62` plus the intentional dirty working
  tree (`103` status paths at nomination). This is never a clean-source claim.

The M88 candidate, snapshot, unit and endpoint remain untouched: PID `1021511` still serves port
`18088` with aggregate `700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`.
Human UAT has not accepted any row in `docs/M89_UAT.md`; no public deployment or service retirement
is inferred.

## M89-F001 repair and qualification

### Exact reproduction and root cause

On the withdrawn candidate, open Compass Rose, choose Sketch → Polyline, click a right angle and
press Finish. The deterministic optimized-browser regression clicks normalized canvas positions
`(0.35, 0.40)`, `(0.60, 0.40)`, `(0.60, 0.65)`. Expected: normalize the active managed-v1 copy to
v2, add one Polyline declaration plus inferred horizontal and vertical constraint declarations to
`sketch.ts`, accept two new spans and publish one outer history row. Observed: source remained v1
and native materialization retained ``semantic reference `core.markers` cannot be resolved``.

The owning defect was in `geosolve-sketch-code` invocation publication. Compass Core declares two
collection outputs, `markers` and `ring`. Their collection members were lowered, but the
multi-output invocation path created an opaque root without publishing each declared top-level
semantic path. Normalized source still referenced `core.markers`, so the first source-aware legacy
upgrade failed before the canvas addition could publish.

### Production repair layers

1. `expansion.rs` now resolves every artifact-declared output from the lowered path/root-member
   maps, checks its `FeatureKind`, and publishes the exact named path. Compass v2 therefore exposes
   both four-member `core.markers` and `core.ring` collections.
2. `session.rs` represents the transactional failure honestly: accepted managed-v1 authority may
   coexist with a current managed-v2 attempt only while that upgrade failure is retained. A clean
   current-v2/accepted-v1 mismatch and the inverse current-v1/accepted-v2 mismatch both reject.
3. `code_projects.rs` exercises the real active-copy preparation/compiler receipt/cold native
   materialization/publication boundary and requires finite accepted points/scalars plus validated
   Hard residuals at most `1e-9`.
4. The frontend pending-mutation resolver reports Rust's retained
   `workbench-interaction` diagnostic before classifying a still-present pending ticket as recursive
   compiler work. A repaired resolution atomically returns settled source/scene authority.

### Regressions and historical F001 checkpoint evidence

- `m89_f001_compass_v2_multi_collection_outputs_materialize_cold` authenticates both four-member
  collection outputs and independently accepted cold geometry.
- `m89_f001_compass_rose_managed_v1_canvas_segment_upgrade_publishes_atomically` minimizes the
  browser Polyline to one Segment while retaining the exact Compass project, active-copy upgrade,
  one revision publication, finite geometry and independent residual validation.
- `m89_f001_retained_v1_to_v2_failure_preserves_accepted_version_authority`,
  `clean_current_v2_with_accepted_v1_version_mismatch_is_rejected` and
  `retained_current_v1_with_accepted_v2_version_mismatch_is_rejected` freeze the one allowed
  cross-version failure state and both forbidden broadenings.
- Frontend `surfaces Rust's retained native diagnostic instead of masking it as recursion` freezes
  the diagnostic boundary.
- Optimized Playwright `Compass Rose Polyline Finish upgrades and publishes inferred constraints
  into sketch.ts` reproduces the exact three-click Finish flow and requires managed-v2 source,
  one geometry plus two constraint rows/source targets, two added accepted curves, no draft,
  accepted revision `r2`, one Undo route and no console/network/runtime error.

Historical F001 checkpoint qualification passes the complete `geosolve-sketch-code` crate, demo-web
`357/357`, frontend `54/54`, warnings-denied Clippy, the locked all-feature WASM check, unchanged
clean golden survey/check/require-clean and optimized release-WASM Playwright `13/13`.

## M89-F002 compact canvas-authored Polyline source

### Exact reproduction and root cause

The F001 replacement correctly inserted code, but the result was not a usable authoring surface.
Exact captures are retained at `/tmp/geosolve-m89-f002-source-evidence.x5hjHn`:

- one inferred axis: `compass-polyline-one-axis.sketch.ts`, `4,238` bytes, `172` lines, SHA-256
  `fa76f529e5bce663e54d22409498ed1cb382dc5e682ae13017a9bddede5f4925`;
- two inferred axes: `compass-polyline-two-axis.sketch.ts`, `4,943` bytes, `199` lines, SHA-256
  `43cd56d5c617e98be4ded3022dcc062b7497f444b1c454465143d8e75616f133`.

`editor_insertion.rs` had an ergonomic Segment branch only. The accepted Polyline and inferred
Horizontal/Vertical nodes therefore fell through to transport-level `$.intent.recipe(...)`
serialization. Geometry and atomic publication were correct; the reverse-projected authoring
language was needlessly verbose.

### Production repair layers

1. `editor_insertion.rs` recognizes only the complete accepted single-native-Polyline shape. It
   emits one `$.geometry.polyline` declaration with deterministic `v0`, `v1`, ... keys, `closed`,
   `role` and explicit `representation: "singleCurve"`. Input-bound vertices preserve their lexical
   point reference. Exact Horizontal/Vertical consumers use
   `geometryN.segments.byKey.vN`. At F002 anything outside that proof retained the generic fallback;
   F003 replaces the geometry fallback with compact `$.geometry.recipe(...)` while preserving the
   narrow direct-Polyline proof.
2. `expansion.rs` treats `singleCurve` as an opt-in lowering to one native
   `GeometryRecipeKind::Polyline`, with keyed point/span/corner results, writable seeds,
   provenance, role and closed state. A missing representation retains the existing composite
   direct-Polyline behavior.
3. `managed-v2.ts` and `managed_v2.rs` distinguish every vertex from only real open spans and only
   interior open filletable corners. `prepared_mutation.rs` now shares the same exact direct-result
   reconstruction as cold envelope validation, preventing the prepared path from inventing absent
   open span/corner keys.
4. Canvas insertion already authenticates old GUI and staged source declaration labels to one
   persistent node. Terminal parity now consumes that prepared projection only for this exact
   publication seam, preferring the expansion alias; ordinary code drags still require exact
   expansion aliases.

### Regressions and historical F002 checkpoint evidence

- `canvas_polyline_and_inferred_axes_project_to_compact_semantic_declarations`,
  `compact_polyline_preserves_an_input_bound_vertex_as_a_lexical_point_reference` and
  the historical pre-F003 generic-fallback case freeze the direct reverse projection boundary.
  F003 renames the collateral to
  `polyline_without_the_exact_single_curve_source_contract_uses_the_compact_geometry_fallback` and
  keeps it compact without broadening the direct one-native-curve proof.
- `direct_polyline_single_curve.rs` proves one native Polyline node, exact keyed results and axis
  references, materialized role/geometry, writable-seed provenance, legacy composite behavior and
  invalid-form rejection.
- `prepared_mutation::tests::open_polyline_insertion_result_uses_exact_keyed_subsets` freezes the
  open vertex/span/corner sets.
- The managed-v2 runtime executes the three compact declarations with no `$.intent.recipe` at
  most `30` lines and `900` bytes.
- The optimized browser regression requires one direct Polyline and two direct axis declarations,
  no generic recipe, complete Compass source at most `75` lines and `2,500` bytes, exact source
  navigation for all three declarations, two added accepted spans, revision `r2`, one Undo route
  and no runtime error.

Focused evidence after the repair includes `cargo fmt --all -- --check`, `git diff --check`, the
complete all-feature `geosolve-sketch-code` suite (`104` unit tests plus every integration/doc
test), full demo-web library `357/357`, warnings-denied sketch-code and demo-web Clippy,
`packages/geosolve-sketch-code` runtime `25/25`, mutation `16/16`, pinned-Deno parity `2/2` plus
type/managed checks, frontend `54/54`, the locked all-feature WASM check, exact prepared-mutation
regression `1/1` and focused demo-web terminal-parity regression `1/1`. A fresh optimized
release-WASM build and nine-file distribution validated. The isolated compact fixture is `24`
lines/`722` bytes. The exact no-build Compass browser case passed on both frozen staging and live
bytes; its normalized generated source is `64` lines/`1,841` bytes (`65`/`1,842` including the
CodeMirror trailing blank), below the `75`-line/`2,500`-byte gate.

## M89-F003 compact source for the complete geometry family

### Systemic finding and ownership

F002 fixed the reported Polyline, but the same authoring path still projected every other
non-Segment geometry through transport-level `$.intent.recipe(...)`. A Quadratic or Cubic Bezier,
circle, arc, ellipse, conic or NURBS therefore remained structurally correct but needlessly large
and unsuitable as ordinary agent- or human-authored source. Classify M89-F003 as a systemic
`geosolve-sketch-code` reverse-projection and managed-execution defect, not a solver or geometry-
equation defect.

The complete 25-variant source contract is now:

- Segment → direct `$.geometry.line(...)`;
- exact native Polyline → direct `$.geometry.polyline(...)`;
- the remaining 23 → compact lossless `$.geometry.recipe(...)`: Sketch Point, Midpoint Line,
  Two-Point Aligned Rectangle, Three-Point Corner Rectangle, Center Rectangle, Three-Point Center
  Rectangle, Center–Radius Circle, Two-Point Diameter Circle, Three-Point Circle, Center Arc,
  Three-Point Arc, Tangent Arc, Center-Axes Ellipse, Axis-Endpoints Ellipse, Center-Axes Elliptical
  Arc, Axis-Endpoints Elliptical Arc, Quadratic Bezier, Cubic Bezier, Rational Quadratic Conic,
  Parabola, Hyperbola, Open-Control NURBS and Periodic-Control NURBS.

### Production correction layers

1. Reverse projection emits one compact declaration containing only the recipe kind, optional
   dynamic-child count/name, canonical slash-path input references, non-default definition fields,
   writable authored values and the exact semantic result path/kind list. It preserves explicit
   Segment construction role, omits the default Segment profile role and retains keyed native
   Polyline ownership after reopen.
2. Rust derives the accepted input slots, definition fields, writable selectors and complete
   result tree from the central `IntentNodeKind::Geometry` descriptors. Expansion rejects unknown,
   missing, repeated, malformed, non-canonical or descriptor-inconsistent paths and kinds before
   native publication. The source can therefore stay compact without moving a geometry equation,
   branch rule or schema authority into TypeScript.
3. Managed-v2 parsing, deterministic execution, normalization, insertion mutation and public types
   understand `geometry.recipe` and expose its recipe-dependent dynamic result tree. Prepared
   point edits authenticate the exact writable `values` pair against Rust's reconstructed result
   authority; nested and overlapping outputs such as `contacts/0` and
   `contacts/0/parameter` remain distinct.
4. Tangent Arc reverse projection recovers writable contact parameters that may originate from
   materializer defaults from independently accepted native ownership, so cold replay is lossless.
   Source-owner refinement ignores a logical multi-output patch root that has no native graph node
   while continuing to authenticate each concrete generated output; this preserves the Compass
   Rose integration path.

### Focused regression and size evidence

- `compact_geometry_recipe_roundtrip.rs` applies real canvas construction, reverse projection,
  source parsing and cold Rust rematerialization to 22 independent non-Segment/Polyline/Tangent
  variants, then covers Tangent Arc separately with its lexical source span, contact/branch state
  and cold replay. A third regression preserves direct Segment construction role; existing direct
  Polyline regressions retain its keyed one-native-curve contract. Together these cover all 25
  variants with exact semantic result paths, finite accepted geometry and independently validated
  normalized Hard residual at most `1e-9`.
- Every generated single compact geometry declaration is at most `1,024` bytes. TypeScript's
  complex insertion batch records exact declaration sizes of `387` bytes for Cubic Bezier, `921`
  for Tangent Arc and `872` for Periodic NURBS; the complete normalized three-shape sketch is
  `3,087` bytes. None contains `$.intent.recipe` or `geosolve-intent-recipe-v1`.
- `cargo test --locked -p geosolve-sketch-code --test compact_geometry_recipe_roundtrip` passed
  `3/3`; `cargo test -p geosolve-sketch-code --all-features --test m89_editor_insertion` passed
  `13/13`; and `cargo test --locked -p geosolve-sketch-code --all-features` passed `109` unit tests
  plus every integration and doc test.
- `cargo clippy --locked -p geosolve-sketch-code --all-targets --all-features -- -D warnings`,
  `git diff --check`, the `packages/geosolve-sketch-code` suite (`runtime 27/27`, mutation `18/18`,
  pinned-Deno `2/2` plus type/managed checks), and frontend `npm test` (`54/54`) passed.
- The last repair was prompted by the complete demo-web run exposing one Compass Rose logical-root
  integration regression. Exact post-repair test
  `workbench::code_projects::tests::m89_f001_compass_rose_managed_v1_canvas_segment_upgrade_publishes_atomically`
  passes. The fresh complete demo-web rerun passes `357/357`; formatting, affected warnings-denied
  Clippy, the locked WASM check and optimized nine-file frontend distribution validation pass.
  Release-WASM Playwright passes `14/14`, including real Cubic Bezier insertion, Center–Radius
  Circle insertion/radius source editing and the Compass Polyline regression. The unchanged normal
  product passes `13/13` before freezing and again against both frozen staging and live endpoints.

### Historical F003 nomination identity

One unchanged optimized output passed every remaining gate:

- frozen snapshot `/tmp/geosolve-m89-f003-uat.uF6Yjsc3`; nine regular files, two directories, zero
  symlinks/other entries, directory/file modes `0555`/`0444`, ordered-manifest aggregate
  `c37d832a6dc076982e3fda8f2ffbcc8b2f26ed99ad6ffd0da0b7ba4f96d705ed`;
- optimized release-WASM `assets/geosolve_demo_web_bg-BJyeIjFR.wasm`, SHA-256
  `99eeaa5668276282201cb1971b042d3d0904a13b7a75d40e2357ce08a34c748a`;
- freeze/HTTP/browser evidence `/tmp/geosolve-m89-f003-freeze-evidence.5TcRPlem`; strict staging
  and live ten-route HTTP ledgers are byte-identical at SHA-256
  `c5efab20409773cc7f43cecdd9c6fca5b961402611d073f6bf0e8f40cca0ce1e`;
- Tailscale endpoint `http://100.94.63.83:18089/`, unit
  `geosolve-m89-react-uat-current.service`, PID `3637680`, invocation
  `73bdb82cf2a64436ab6f65fe76b08ccf`.

That service identity describes the historical F003 checkpoint; F005 has replaced it at the shared
endpoint. The F003 snapshot remains preserved as rollback evidence and is no longer current. The
preliminary F003 snapshot `/tmp/geosolve-m89-f003-uat.W5GIfQi7`, F002 and every earlier snapshot
also remain preserved but withdrawn.

## M89-F004 compact source for the persistent constraint catalog

### Systemic finding and ownership

F003 completed geometry projection, but non-axis canvas constraints such as Parallel still fell
through to transport-level `$.intent.recipe(...)`. Accepted constraint semantics, solver behavior
and atomic publication were correct; the reverse-projected source was not an appropriate managed
authoring surface. Classify M89-F004 as a systemic `geosolve-sketch-code` reverse-projection and
managed-execution defect, not a constraint-equation defect.

The persistent catalog boundary is exact. Of 35 `ConstraintKind` variants, 33 cold-replay from
standalone managed source. Horizontal and Vertical retain their direct SDK builders; the other 31
use one descriptor-authenticated `$.constraint.recipe(...)`. `ExternalPointCoincident` and
`ExternalLineCollinear` require immutable host-external snapshot/binding authority that
`sketch.ts` cannot yet express. Their wire schemas are audited, but projection remains fail-closed
and they are not counted as standalone replayable.

All 25 canvas geometry variants and all 33 standalone persistent constraint kinds reverse-project to compact, cold-replayable managed source. The two host-external schema variants remain compile-visible and schema-authenticated, but require separately supplied host snapshots.
Those variants are `ExternalPointCoincident` and `ExternalLineCollinear`. F004/F005 implement no
standalone host-snapshot execution path, so without that host authority both remain gated and
neither is counted among the 33 standalone replay rows.

### Production correction layers

1. Reverse projection emits canonical constraint kind, input references, definition fields,
   writable values and exact result descriptors rather than a native transport draft.
2. Managed-v2 parsing, deterministic execution, normalization, insertion mutation and public
   TypeScript types understand `constraint.recipe` as equation-free data.
3. Rust re-derives every accepted input slot, field, writable selector and result descriptor from
   the central constraint descriptors. Unknown, missing, repeated, malformed, non-canonical or
   descriptor-inconsistent content rejects before native publication.
4. The two external-reference wire schemas remain covered by the catalog audit while reverse
   projection refuses their missing host authority. No external binding is inferred from local
   coordinates or initial state.

### Focused regression and evidence

`compact_constraint_recipe_roundtrip.rs` reviews all 35 persistent variants in one bounded matrix.
It cold-replays the 33 standalone rows, compares their exact semantic signatures, checks every
accepted scalar/coordinate is finite and independently requires normalized Hard residual at most
`1e-9`. It separately proves the two host-external rows are gated. The matrix passes `1/1`.

TypeScript runtime `28/28`, mutation `19/19` and pinned-Deno parity `2/2` pass with the compact
constraint surface. The frontend suite passes `55/55`. These focused checks are included in the
completed final provisional dirty-tree qualification and immutable F005 nomination below.

## M89-F005 direct computed-Fillet source and lifecycle

### Systemic finding and ownership

Canvas-computed Fillet was still source-backed through a generic transport recipe, so the managed
declaration did not expose the stable public Fillet operation or its complete explicit branch/
contact contract. Classify M89-F005 at `geosolve-sketch-code` reverse projection, direct lowering
and workbench source-ownership composition. It changes no Fillet equation, tolerance or inferred
branch.

### Production correction layers

1. Reverse projection emits one direct `$.computed.filletSet(...)` declaration with lexical native-
   span parents, exact parameter/winding/neighborhood/normal-side/retained-endpoint/periodic-anchor/
   endpoint-order/sweep state, radius, display name and explicit source-owned suppression.
2. Managed execution and Rust lowering reconstruct the same computed owner, generated children and
   explicit branch/contact state on cold replay. Same-gesture Polyline parents use deterministic
   keyed span paths. Computed-host arcs, non-lexical parents, wrong kinds and malformed state fail
   closed before publication.
3. Explorer/canvas selection resolves to the complete authenticated declaration and reports
   `Modifiable in source`. Radius edits publish one source/IR/artifact/scene/history transaction and
   restore that selection after cold rematerialization.
4. Suppress/restore and delete/Undo rewrite the source-owned declaration lifecycle exactly;
   persistence and reload retain ownership and accepted scene authority.

### Focused regression and evidence

The direct Fillet lowering target passes `7/7`, including the checked TypeScript compile fixture,
exact explicit-state materialization, scalar-bound radius, suppression, earlier-patch native-span
resolution and invalid-parent/state rejection. `m89_editor_insertion` passes `19/19`, covering
ordinary and non-default periodic branch replay, keyed Polyline parent paths and explicit
suppression. Focused bridge tests pass whole-declaration navigation, radius fan-out/atomic
publication and selection restoration. Compact geometry collateral now passes `4/4`; TypeScript
runtime `28/28`, mutation `19/19`, pinned-Deno parity `2/2` and frontend `55/55` pass. The focused
optimized release-WASM Fillet author/edit/suppress/restore/delete/Undo case passes `1/1`.

All accepted focused cases retain finite geometry and independently validated normalized Hard
residual at most `1e-9`. F004/F005 add no Offset work: the previously implemented Profile Offset
closure remains unchanged and no broader Offset claim is introduced.

### Final provisional dirty-tree qualification and immutable F005 nomination

Exact gate

```bash
nix-shell shell.nix --run 'GEOSOLVE_ALLOW_DIRTY=1 ./scripts/release-gate.sh'
```

passed in the pinned shell with `wasm-bindgen-test-runner 0.2.121`. The ambient attempt first built
successfully through the complete workspace and golden checks, then its first WASM parity leg
reported `HARNESS_ERROR` because `wasm-bindgen-test-runner` was absent after reboot. The pinned
success classifies that result as an environment/harness fault, not a product defect.

Final focused and aggregate counts are:

- sketch-code unit tests `114`;
- compact geometry `4/4`, persistent-constraint matrix `1/1`, direct Fillet `7/7` and
  `m89_editor_insertion` `19/19`;
- demo-web `358/358`;
- TypeScript runtime `28/28`, mutation `19/19` and pinned-Deno parity `2/2`;
- frontend `55/55`.

These append to rather than replace the historical F003 `109`, `3/3`, `13/13`, `357/357` and
related counts above. The exact final no-rebuild distribution is:

- snapshot `/tmp/geosolve-m89-f005-uat.hzNuDxF0`;
- external manifest `/tmp/geosolve-m89-f005-uat.hzNuDxF0.sha256` and relative-path-sorted manifest
  aggregate `fb488ad2bf29e8897cf9811c002b748693e5d211bae4bb54c83ed060db5db668`;
- evidence directory `/tmp/geosolve-m89-f005-freeze-evidence.VAoQDl8n`;
- nine regular files, including three JavaScript files, one CSS and one WASM, two directories and
  zero symlinks/other entries;
- `assets/geosolve_demo_web_bg-52ybei8k.wasm`, `16,333,537` bytes, SHA-256
  `3e6f515ff1e5de0f668c13e86c02d280c0dc085bbd89b314bf9ece6c82aae575`.

Strict staging and live ten-route HTTP ledgers are byte-identical at SHA-256
`4de184eb1eb237f98997b70702567a2b110b40d96df5d0d9653f024ac5f23e8d`. The focused Fillet
author/edit/suppress/restore/delete/Undo browser case passes `1/1` on staging and live. The full
normal frozen product passes `15/15` on both; that normal product intentionally excludes the
compiler-parity-only harness test.

Current endpoint `http://100.94.63.83:18089/` is owned by
`geosolve-m89-react-uat-current.service`, PID `1007459`, invocation
`a3fea6dddb59438795e52c6a8fab136f`, started `Wed 2026-09-02 20:21:15 AEST` with
`WorkingDirectory=/tmp/geosolve-m89-f005-uat.hzNuDxF0`. Retired staging PID `995137` had invocation
`efe4de2264774f68bf5852284fa69887`.

This is final **provisional dirty-tree** mechanical qualification and immutable UAT nomination,
not clean-source qualification, supervising-human acceptance or milestone closure. The mandatory
Compass Rose retest, targeted manual F004/F005 preflight, M89-U1 through M89-U8 and explicit
closeout remain pending/not run. F003 remains historical rollback evidence only.

## Superseded M89-F002 UAT candidate

After the reopened-Polyline ownership, TypeScript contract and declaration-catalog audit repairs,
the validated nine-file distribution was frozen without rebuilding at
`/tmp/geosolve-m89-f002-uat.bSM6Nn59`. It has two directories, zero symlinks/other entries,
directories at `0555` and files at `0444`. External manifest
`/tmp/geosolve-m89-f002-uat.bSM6Nn59.sha256` has relative-path-sorted aggregate
`0b9a4ed357a452edd79953679e5bbb68c0cb5b1261a8ba5478a1dbaa597ab433`; optimized release-WASM
`assets/geosolve_demo_web_bg-DXbVrwxa.wasm` has SHA-256
`77e5739cb81565d5a0bc400507e1a35f2f63ba22c1d821442b16baedcb24b72a`.

Evidence directory `/tmp/geosolve-m89-f002-freeze-evidence.Xs977S3o` retains equal pre-copy,
frozen and post-copy manifests, strict HTTP ledgers, service identities and no-build browser
results. Optimized release-WASM Playwright passed `13/13`; the frozen normal product passed
`12/12` without rebuilding. Temporary `http://100.94.63.83:18189/` and final
`http://100.94.63.83:18089/` both passed the exact optimized release-WASM Compass regression `1/1`.
Their strict ten-route HTTP ledgers are byte-identical at SHA-256
`2a9a939d073f63df095ebc01542eb95443f4e3b86d7df2980c0b586a571b2ba1`.

Unit `geosolve-m89-react-uat-current.service`, PID `3162678`, started
`2026-09-02 11:34:56 AEST`, invocation `90508752f8de4314a85e089ab53e3d8d`, historically served only
the frozen F002 snapshot on port `18089` and was replaced only after exact F003 staging proof. The
pre-audit `/tmp/geosolve-m89-f002-uat.Taocj5bI` snapshot and the F001 snapshots remain preserved but
withdrawn. M88 stayed byte- and service-identical: PID `1021511`,
invocation `2cd4379455f44cde99828658b44601ea`, snapshot
`/tmp/geosolve-m88-react-uat.KGhA7s`, endpoint `http://100.94.63.83:18088/`. This is proportional
dirty-tree historical F002 qualification, not F003 UAT authority, a clean-source full release gate
or human acceptance.

## Withdrawn M89-F001 replacement UAT candidate

The successful gate's final distribution was frozen without rebuilding at
`/tmp/geosolve-m89-f001-replacement-uat.IwvrBh8x`. It has exactly nine regular files, two
directories, zero symlinks/other entries, directories at `0555` and files at `0444`. External
manifest `/tmp/geosolve-m89-f001-replacement-uat.IwvrBh8x.sha256` has relative-path-sorted aggregate
`7d40c31c0eba5aae0d1b6e7febf16a0b5fa44a541c82a9ddbbb0125cd8707424`; the optimized release-WASM
`assets/geosolve_demo_web_bg-BZTEjICo.wasm` has SHA-256
`a2b2fd4ec4852e3d16b10ab0fa41eb8e4d64b52db430447bdbf28bb8d82f3241`.

Evidence directory `/tmp/geosolve-m89-f001-replacement-freeze-evidence.yhuZYse1` retains equal
pre-copy, frozen and post-copy manifests, service identities, strict HTTP ledgers and no-build
browser results. Every named file plus root and explicit index returned HTTP `200`, identity
encoding, zero redirects, exact size/type/hash, no `Location` or `Content-Encoding`, and root equal
to frozen `index.html`. Temporary `http://100.94.63.83:18189/` and final
`http://100.94.63.83:18089/` ledgers are identical at SHA-256
`62ae4d760bb40fb9031ef53a8ec9ee6c8f239572242ed6ee730282e1a0f50e0b`. The exact existing Compass
Polyline regression passed `1/1` against each frozen endpoint without a WASM/Vite rebuild.

Former unit PID `1589113`, started `2026-09-02 00:38:00 AEST`, invocation
`d05d85152a4547f7b32e316b6a3d4b9a`, served only that withdrawn replacement snapshot until the
separately qualified F002 snapshot passed staging proof. Withdrawn PID `825513` was retired after
the earlier staging proof; both historical snapshots remain preserved. M88 stayed byte- and
service-identical throughout: PID `1021511`, invocation
`2cd4379455f44cde99828658b44601ea`, snapshot `/tmp/geosolve-m88-react-uat.KGhA7s`, endpoint
`http://100.94.63.83:18088/`. This is provisional dirty-tree nomination, not clean-source or human
UAT acceptance.

## Known limitations and next boundary

- Managed `sketch.ts` is deliberately a closed normalized subset. Liberal `.patch.ts` remains an
  executed extension and is not reversible from managed IR.
- Editing raw managed source natively requires the pinned Deno compiler sidecar; an already-compiled
  project remains pure-Rust, browser-free and Deno-free for inspect/solve/render.
- Managed-v1 compatibility and the checked-in legacy sample bytes intentionally remain. M90 is the
  planned clean-break normalization/removal milestone only; no M90 migration or deletion is part of
  M89.
- `ExternalPointCoincident` and `ExternalLineCollinear` remain deliberately host-authority-gated;
  managed source cannot cold-replay them until it can authenticate an immutable external snapshot/
  binding. Their wire schemas are audited, but no false standalone support is claimed.
- F004/F005 add no Offset work. The existing Profile Offset closure remains implemented and
  unchanged; no wider Offset family is implied.
- Final provisional dirty-tree mechanical qualification and the immutable F005 nomination are
  complete, but clean-source qualification, the Compass retest, targeted manual F004/F005
  preflight, M89-U1 through M89-U8 and explicit milestone closeout remain absent/pending.
  Automated qualification does not pre-accept any human row. F003 is historical rollback evidence;
  no public Pages deployment, M88 service mutation or legacy-sample rewrite is inferred.
