<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 implementation ledger — Optional code/GUI sketch authoring

Status: **M84-F005 collaborative semantic interaction and M84-F006 adversarial authority hardening
are implemented and focused-qualified; the clean replacement gate, immutable nomination and
refreshed UAT remain pending**. Direct-authoring source
`41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, is withdrawn historical evidence with the initial,
F003 and F004 candidates. No F005/F006 replacement is nominated, no UAT row is accepted, no M84
Pages publication is claimed, and accepted M83 remains public authority.

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
- `CodeProject::managed_only(ProjectKey, source)` is its smallest code-only host seam. It accepts
  only artifact-free managed source and the SDK import, validates the complete envelope, and
  publishes no native geometry by parsing alone.
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
- [x] Bootstrap a complete supported accepted GUI dependency closure into truthful, dependency-
  ordered managed declarations; reject unsupported recipes instead of inventing lineage. Omit only
  the exact canonical fresh-workspace document foundation and never hide other bootstrap geometry.
- [x] Represent connected Segment endpoints and direct computed Fillet parents with lexical
  declaration members (`line.end`, `line.span`) rather than native IDs or transport DTOs.
- [x] Represent ordinary Horizontal and Vertical span relations as lexical
  `$.constraint.horizontal`/`$.constraint.vertical` declarations, preserve suppression and lower
  them to the existing native constraint kinds.
- [x] Keep one checked-in managed-v1 line/Horizontal/Vertical/line/Fillet source as both a
  TypeScript compile target and the Rust parser/cold-materialization fixture.

### I2 — custom artifacts and typed SDK

- [x] Compile custom TypeScript only in an explicit caller-owned Node step into canonical,
  data-only, digest/interface/ABI-pinned artifacts.
- [x] Admit only the central executable declaration-family catalog, bounded template DAGs,
  schemas, bindings, lenses and keyed collections. All nested structural values count toward the
  65,536-item bound; individual artifacts remain capped at 16 MiB.
- [x] Generate named rectangle results, mapped Fillet records and Polyline-derived corner
  collections from Rust declaration descriptors. Raw IDs, cross-project references, misspelled
  outputs and point/curve/corner mismatch fail TypeScript compilation.
- [x] Describe direct `computed.filletSet` as an opaque `FilletSetFeature`: its explicit parent
  spans are typed, but it does not falsely expose evaluated child arcs as ordinary native ports.
- [x] Generate `NativeCurveSpanRef` from the central Rust declaration-result catalog only for
  direct line spans, rectangle edges and Polyline segments. Computed Fillet arc outputs remain
  ordinary non-native curve-span references and are not assignable to a direct Fillet parent.
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
- [x] Lower direct managed `computed.filletSet` declarations back to the existing Intent
  `ComputedFeature::FilletSet` with exact persisted contact/branch state, without invoking native
  Fillet authoring heuristics or changing any solver equation.
- [x] Authenticate every direct parent as an Intent-backed native span during Rust lowering and
  reject computed host outputs even if a caller bypasses TypeScript branding. Accept radius only
  as a positive finite model-unit number or branded millimetre literal `mm(...)`.

### I4 — one code/editor transaction

- [x] `SketchCodeSession` owns project/files/artifacts/lock/program/expansion/reconcile state,
  current/accepted semantic overlays, current/accepted editor checkpoints and one outer Undo/Redo
  history.
- [x] Every persisted current, accepted, Undo and Redo nested checkpoint is host-validated before
  construction. Hostile session IDs cannot poison allocator high-water.
- [x] Pointer frames use the existing retained native preview without parsing, expansion,
  serialization or durable-panel rebuild. Terminal publication commits the authenticated newest
  accepted preview once, after complete cold/native parity.
- [x] Managed rectangle corner drags reverse-write `lowerLeft`/`upperRight`; generated Polyline
  point drags become generation-bound explicit overrides; Reset removes only that override.
- [x] M84-F005 adds one bounded persistent semantic interaction overlay to this drag ownership.
  Its key is project plus semantic owner/output/field and never-reused owner generation;
  no intent/native alias enters the persisted wire. M84 exposes only finite Cartesian point drafts
  in this overlay; scalar edits remain managed-source lens edits. Point-seed precedence is typed
  overlay draft > legacy generated override > managed source seed, while Reset removes the complete
  semantic edit bundle and restores the applicable lower tier. Equal duplicate terminal writes
  collapse deterministically; unequal same-tier writes reject atomically. The overlay never alters
  constraint priority or solver equations.
- [x] M84-F005 routes selection, drag and deletion through accepted expansion provenance. A
  uniquely selected referenced consumer detaches only that consumer; no semantic preference or a
  selected producer retains ordinary attached ownership. Consumer detachment may replace that
  Segment's intent/native identity while preserving its code-owner generation and rebinding
  retained code-owned and ordinary GUI dependents. Repeated drags use the detached lens; Undo/Redo
  restores the complete attachment/overlay/editor authority. A rectangle corner writes the two
  coupled canonical seeds atomically. Unknown, stale-generation, non-finite and wrong-type drafts
  fail closed, and multiple matching selected lenses reject without mutation. Semantic deletion
  authenticates the exact session, accepted alias and managed/generated semantic owner before it
  rewrites the exact code-owned dependency closure or suppresses one generated child; a dirty
  managed draft, retained code failure, stale token or GUI-owned selection cannot select that
  semantic route.
- [x] M84-F006 hardens the completed authority boundary after adversarial review: imported session
  identities have an allocator-safe ceiling; persisted managed drafts retain the 4 MiB source
  bound; Reset/Restore use typed canonical tokens; retained failure prunes exact direct/generated
  owners; generated detachment carries exact provenance; direct and generated Segment replacements
  plus generated circle-centre replacements rebind their dependents; transient detachment cancels
  exactly; repeated generated drags survive exact Undo/Redo; and same-tier conflicts compare
  persisted IEEE bits, including `+0.0` versus `-0.0`.

### I5 — workbench, persistence and demonstrations

- [x] Add managed/custom tabs, Apply/Revert, diagnostics, artifact state, source ownership,
  generated-member groups, edit lenses, override badges and Reset-to-code controls.
- [x] Keep Code discoverable even when ordinary all-or-nothing conversion rejects: show escaped
  read-only diagnostic source alongside the Intent IR fallback and withhold Promote.
- [x] Round-trip complete offline code-project authority/history through save/reload and Copy/Load
  repro under the 64 MiB project bound. Missing, tampered, corrupt and oversized inputs reject
  atomically. M84-F005 carries separate current/accepted overlays and deterministically prunes
  attempted owners removed by a retained structural/native failure without advancing the accepted
  overlay or canvas.
- [x] Ship four genuine sessions: adaptive rounded Polyline; typed panel/keyed Fillets; GUI↔code
  braced frame; reusable mounting plate.
- [x] On only the canonical, current-and-accepted empty workspace, show a dedicated Code landing
  with one **Start from code** action and four centrally sourced project cards. Starting installs
  an artifact-free `Authored` project through the shared cold-validated project path, focuses the
  editable source, and never manufactures a GUI scene or promotion entry.
- [x] Persist `CodeProjectOrigin::Authored` separately from bundled/promoted origins; reject a
  conflicting legacy demo identity. Preserve valid and retained-invalid source, accepted native
  checkpoint, whole-source replacement and exact outer Undo/Redo across reload.
- [x] Prove an ordinary GUI reference CurveLength dimension on
  `brace.diagonals.rising` retains its native identity, follows a managed frame rewrite, updates
  its measured value, remains GUI-editable and shares exact outer Undo/Redo.

The semantic-overlay addition intentionally changes the still-unreleased optional persistence
contract. Code sessions now identify as `geosolve-sketch-code-session-v2`; composed workbench
payloads identify as `geosolve-code-workbench-v2`. Prototype-v1 M84 payloads reject rather than
being silently reinterpreted under changed identity-digest semantics. Plain M83 workspace-v8
persistence remains byte/behavior compatible and does not acquire an optional-code dependency.

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

### M84-F003 — transport-shaped Structured Source was not lexical authoring code

Reproduction owner: optional GUI-to-code projection. Draw an aligned rectangle, then a Segment
between two rectangle corners. The ordinary Structured Source route serialized
`IntentProjectedPortReference { declaration, output, kind }` into a TypeScript-shaped object. That
is valid bounded transport IR, but it is not a variable-driven reference coupled to the rectangle's
inferred result type. The bundled Braced Frame began as managed code and therefore did not cover
ordinary GUI conversion.

Repair: retain the original DTO as labelled Intent IR, add direct managed
`$.geometry.line`, dependency-order supported GUI bootstrap declarations, and emit endpoint
expressions such as `frame.corners.lowerLeft`. Direct expansion must alias the exact owning native
point rather than copy its coordinates. Ordinary Code preview/promotion must create one genuine
persisted code project/session. Raw strings, transport DTOs, foreign-project and forged reserved-
project references, wrong kinds and misspelled members fail closed. Only the exact canonical fresh-
workspace document foundation is ignored; other bootstrap geometry makes conversion reject
atomically. Same-cell branch normalization is limited to current code-expansion-owned Segments so
ordinary GUI Segments retain explicit branch authority. Focused Rust, TypeScript, workbench/
persistence and browser tests plus the complete replacement qualification and nomination pass.

### M84-F004 — an ordinary computed Fillet made Code undiscoverable

Reproduction owner: ordinary GUI managed projection plus the workbench Code surface. Draw two
connected Segments and place one Fillet between them. Projection was all-or-nothing, but GUI
bootstrap could neither express the second Segment's endpoint as a lexical reference to the first
nor lower `ComputedFeature::FilletSet`. The resulting conversion error caused presentation to omit
the Code tab entirely, so the user saw neither code nor an explanation. After that direct Fillet
slice was implemented, browser replay of the exact mouse-authored path exposed the remaining
closure gap: ordinary drafting had also created Horizontal and Vertical constraints, and bootstrap
rejected those declarations before it could present the otherwise-supported Fillet source.

Repair contract: add Segment-to-Segment lexical endpoint projection and a distinct direct
`$.computed.filletSet` declaration. Each corner carries exactly two ordered lexical
`NativeCurveSpanRef` parents plus explicit parameter, winding, neighborhood, normal-side,
retained-endpoint and periodic-anchor state; endpoint order, sweep and suppression are also
explicit. The descriptor generator grants the native-span brand only to direct line spans,
rectangle edges and Polyline segments. Computed host Fillet arcs remain unbranded; Rust lowering
also rejects such host outputs as parents so branding cannot be bypassed. Radius accepts only a
positive finite model-unit number or branded `mm(...)`, not forged unit records or another length
unit. Direct lowering reconstructs the existing Intent computed feature and must not rerun Fillet
authoring or change solver behavior. The declaration returns an opaque `FilletSetFeature`.
Ordinary projection remains intentionally all-or-nothing. Code is now always discoverable:
supported scenes offer a read-only managed preview and Promote, while unsupported scenes show an
escaped read-only conversion diagnostic with Intent IR still available and no Promote action.
The complete ordinary path additionally emits existing inferred axis relations as
`$.constraint.horizontal(... { curve: line.span })` and
`$.constraint.vertical(... { curve: line2.span })`, preserves suppression, and lowers them to the
existing Intent Horizontal/Vertical kinds. It does not add or reinterpret a constraint equation.
Bootstrap authenticates `accepted.validation.semantic` against the exact current retained intent
before reading declarations. A retained-failed rebind therefore presents Code as unavailable with
no Promote action instead of combining prior accepted geometry with current unaccepted wiring.

Focused fixture: `packages/geosolve-sketch-code/test/managed/line-fillet.managed.ts` is the same
managed-v1 two-line/two-axis-constraint/one-Fillet source compiled by the TypeScript suite, parsed
by Rust and cold-materialized through the ordinary intent/editor authority. It is the focused
fixture exercised within the complete release-candidate qualification below.

Focused Rust GUI-bootstrap/owner, direct-lowering, descriptor-parity, workbench, TypeScript type-
contract and Code-surface diagnostic coverage passes. Exact source `c2cf160`, tree `94a1786`, also
passes the complete clean release gate. Its immutable no-rebuild snapshot
`/tmp/geosolve-m84-f004-hv-uat.FF5RFBZe` was nominated on retained Tailscale after exact served-byte
verification and baseline 4/4, F003 1/1 and F004 2/2 browser suites passed on both temporary and
retained endpoints. The later direct-authoring amendment withdraws those bytes from current
nomination; this remains historical mechanical evidence, not human UAT acceptance.

### M84-F005 — collaborative semantic interaction authority

Reproduction owner: code-enabled canvas manipulation and deletion after a code project has been
cold-materialized. Earlier direct and generated reverse edits handled isolated demonstrations, but
they did not provide one durable semantic authority for literal points, rectangle-corner coupling,
shared references, generated owners, Reset, deletion and retained failure. Selection could not
truthfully distinguish “move the producer and keep consumers attached” from “detach only this
selected consumer,” and implementation aliases were not a safe ownership language.

Repair: `CodeInteractionOverlay` is a bounded, generation-authenticated, point-only semantic seed
layer. Its key contains project plus direct/generated owner, semantic output and writable field;
it contains no intent/native ID. Expansion applies exact precedence
`semantic overlay > legacy generated override > managed source`. Rectangle roles stage their two
canonical seeds atomically. No semantic preference or producer selection preserves shared-point
attachment, while a uniquely selected consumer stages local detachment. Replacement direct and
generated Segments keep stable code ownership and rebind surviving code/GUI dependents; generated
circle centres use the same provenance-owned replacement path. Repeated detached drags and exact
Undo/Redo remain valid. Equal duplicate writes collapse deterministically and unequal writes to one
same-tier address reject before publication.

Selection and deletion now resolve through accepted expansion provenance. A delete token binds the
exact code-session identity, accepted alias and direct/generated semantic owner. Managed deletion
rewrites the exact code-owned source closure; generated-child deletion is reversible suppression;
dirty source, retained failure, stale authority and GUI-owned selections cannot use this route.
Current and accepted overlays persist separately. A parseable structural attempt which later fails
native publication keeps only its exactly owner-pruned current overlay while the accepted overlay
and canvas remain unchanged. `geosolve-sketch-code-session-v2` and
`geosolve-code-workbench-v2` intentionally make that unreleased wire change explicit; plain M83
workspace-v8 remains unchanged.

Owning coverage is concentrated in `crates/geosolve-sketch-code/src/overlay.rs`,
`crates/geosolve-sketch-code/tests/m84_semantic_overlay.rs`,
`crates/geosolve-sketch-code/tests/m84_native_composition.rs` and the code-workbench tests. It
separately exercises direct literals, all rectangle roles, direct and generated shared references,
generated circle centres, seed precedence, exact conflict handling, Reset, managed deletion,
generated-child suppression, retained failure, persistence and Undo/Redo.

### M84-F006 — adversarial persistence and authority hardening

An independent post-F005 audit found no architecture blocker, but identified defensive gaps which
could weaken an otherwise-valid authority contract under hostile persistence or uncommon generated
ownership. Imported session IDs are now capped before allocator adoption; managed editor drafts
retain the 4 MiB source bound; Reset and Restore use typed canonical tokens; and retained-failure
pruning authenticates both direct and generated owners exactly. Generated-reference provenance,
detachment, direct/generated Segment replacement, generated circle-centre replacement, dependent
rebinding and transient cancellation are explicit. Repeated generated drag has exact Undo/Redo
coverage. Same-tier point conflict compares persisted IEEE bits, so identical writes collapse but
`+0.0` and `-0.0` conflict in either order. These are resource, persistence and transaction-
authority repairs only; no equation, residual, constraint, priority, tolerance or branch policy
changed.

## Focused F005/F006 qualification

The final dirty implementation state passes the proportional pre-release matrix:

- `cargo test --locked -p geosolve-sketch-code` passes, including
  `m84_native_composition` 11/11 and `m84_semantic_overlay` 8/8;
- the separate M84 code-project golden passes 1/1;
- `RUST_MIN_STACK=16777216 cargo test --locked -p geosolve-demo-web --lib` passes 264/264;
- milestone-neutral golden survey/check/require-clean passes unchanged at 271/271;
- `packages/geosolve-intent` passes 40/40 and the normalized Rust package closure contains the
  expected 29 files; and
- focused warnings-denied Clippy, formatting and `git diff --check` pass during the audits.

These focused results establish implemented behavior but do not nominate bytes. The exact clean
committed-source release gate, no-rebuild freeze, temporary/retained served-byte verification and
refreshed browser suites must still be repeated after documentation reconciliation.

## Historical F003 focused evidence observed before its withdrawn nomination

- `cargo test --locked -p geosolve-sketch-code --all-features` — 44 unit and 29 integration tests
  pass, including parser/rewrite, artifact, descriptor, bootstrap, reconciliation, native
  composition, direct-line lexical lowering, override, optional-boundary and TypeScript-
  interoperability owners.
- `cargo test --locked -p geosolve-demo-web --lib` — 243/243 pass, including ordinary Intent IR,
  read-only lexical preview, real promotion/persistence, fresh-foundation handling, dependent
  movement and ordinary-Segment branch-authority owners.
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

These development/focused results are incorporated into, but do not substitute for, the clean
committed-source gate and frozen nomination below.

## Withdrawn clean qualification and frozen nomination

Exact committed product source `79078eca44a5af4de5cccd92bf6fee570c473624`, tree
`05aefb0cbd3972d423f1713df1e58628b24ec216`, passed:

```bash
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run ./scripts/release-gate.sh
```

The gate ran on 2026-08-25 from 21:31:20 to 21:51:56 AEST. Its 6,107-line, 414,758-byte log is
`/tmp/geosolve-m84-release-gate.GOEuXP.log`, SHA-256
`0f50e6bcdf019c71d70497acc301dcdfd194db1142b248bcd469d0f3ed9efda0`. Workspace
warnings-denied Clippy, locked all-feature tests, Rustdoc, the clean 271-row golden, actual WASM,
both TypeScript suites, package closure, performance/benchmark gates, licensing and Trunk 0.21.14
release assembly all passed. The separate workspace-test log
`/tmp/geosolve-m84-workspace-tests.bQDpzA.log` has SHA-256
`72e9c6efd229b7441f00da5b13c4381e01cb1386c5aac87d2d257981a6306689`. A standalone host
`trunk build` was unavailable because Trunk is not on the host `PATH`; the canonical Nix gate
provided Trunk and passed.

Without rebuilding, the exact gate output was copied to `/tmp/geosolve-m84-uat.aHw5ePSW`. The
directory is mode `0555`; its seven regular non-symlink files are mode `0444`. Freeze evidence is
retained at `/tmp/geosolve-m84-freeze-evidence.7loLImo2`. The ordered-manifest aggregate is
`99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
57918914596207c2a3aa27abae8cbce1e321a54d797620e0ea7e66900d940732  geosolve-demo-web-e0d2c05fe4bed1f4.js
15cdf9f3ecabd8c0b42e419009af43065686a2d66a8f6209a48ae57449bc172e  geosolve-demo-web-e0d2c05fe4bed1f4_bg.wasm
2afe27a4143da8521f07945cb0671e3412985b05d3ab035a26255196079776fe  index.html
9c4cc19e4ead8b15276095c81983cd0824f792e580e5929adc75f979264e2952  styles-5c4359127dc3a0bb.css
```

Local and retained-Tailscale verification covered `/` and all seven files: HTTP 200, zero
redirects, exact MIME type/length/hash, no `Location` or `Content-Encoding`, and root equality with
`index.html`. Ledgers `/tmp/geosolve-m84-temp-verify.dowsOmMZ/results.tsv` and
`/tmp/geosolve-m84-final-verify.HlnJrbWo/results.tsv` both have SHA-256
`dd8e6c1350f56cb6e7a483892a188187edc68ddcb63ee8ba9335401432ba8895`. Focused Playwright
passes 4/4 locally and 4/4 over Tailscale. Its bounded-surface check passes at `1440x900` and
`1024x720`; the suite also covers managed-lens/native/history publication and all four genuine
projects. Config/spec hashes are `f0308eb1d706ead212d370963f9b6b6c87fe8488923a02e9efe62d2d1f457ee0` and
`5ef1b00cc17073a789c8f86af2e29a225f5f1d091941b28172e1546ceba181a0`; local/final log hashes
are `54858c5a1f75cc2e286d07360ed8342c7f8a09290a8f3e7ee1c4d295afe91d9e` and
`37f289b62adc02362e8c34a1ea23377c2a3fc5446d43ac262a5b075c1652f4c0`.

Historical `geosolve-m84-uat.service` PID `2426265` served only that immutable directory at
`http://100.94.63.83:8080/`; it was retired after the F003 replacement passed temporary
verification. The old temporary listener is also retired. This withdrawn nomination claims no
human UAT, approval, Pages publication or milestone closure.

Human UAT subsequently opened M84-F003. This snapshot is preserved as historical defect evidence
and claims no current nomination, approval, Pages publication or milestone closure.

## Withdrawn M84-F003 replacement qualification and frozen nomination

Exact committed product source `b9e67bad7f4935b1e0591ea4f149fae478b32675`, tree
`7062806695e1e134c339cfa47903145d321f6350`, had a clean worktree and passed:

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run ./scripts/release-gate.sh
```

The gate ran on 2026-08-26 from 00:24:02 through 00:46:20.940662 AEST, approximately 22m19s,
and exited 0. Its 6,192-line, 414,397-byte log `/tmp/geosolve-m84-f003b-release-gate.log` has
SHA-256 `eb05d3c1e460f5cb7be410dc44d0af0c4b4eaf4fd775423b676c77a84a433f90`.
Workspace warnings-denied Clippy, locked all-feature tests, Rustdoc, the unchanged 271-row golden,
actual WASM, both TypeScript suites, package closure, performance/benchmark gates, licensing and
Trunk release assembly pass. Golden SHA-256 remains
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate M84
code-project ledger remains
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

Without rebuilding, the exact gate output was copied to `/tmp/geosolve-m84-f003-uat.mO67NI`.
Source, copied and frozen manifests are identical. The directory is mode `0555`; exactly seven
regular non-symlink files are mode `0444`. Complete evidence is retained at
`/tmp/geosolve-m84-f003-freeze-evidence.Ue4SCM`. The ordered-manifest aggregate is
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`:

```text
bc99bec852a174e58de5027da25cffd31a5e21580fff4f4cba80e700a3d5f252  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
8c06f0303535f09aa7bec53703136f5e29625c0aa193abff945bc867ab13af99  geosolve-demo-web-c14103084aedc965.js
178016828122190de25998de41a9a1991028a0180da0ff665a843fc170c9b85f  geosolve-demo-web-c14103084aedc965_bg.wasm
d8faa1ccc37a0758aaf8ba94d45cfd12661ba42bc4f80b35d754ae59897311fe  index.html
5b30ea9a86e4be495437705c2b2a9cb30800068ff5a27bcd61a408e13d7e1700  styles-4a93ed51c1144512.css
```

Temporary `:18084` and retained `:8080` verification cover `/` plus all seven files: HTTP 200,
zero redirects, exact MIME type/length/hash, no `Location` or `Content-Encoding`, and root equality
with `index.html`. Both result ledgers have SHA-256
`438d747641522dd567e5790c56663f9bcb596147d48b843e1fe3367830822d0c`. Existing focused browser
checks pass 4/4 and the dedicated F003 GUI draw → Intent IR → lexical preview → promotion → managed
rectangle edit → dependent-line movement → reload flow passes 1/1 on both endpoints.

Temporary listener PID `3728373` is retired. The worktree listener PID `2872083` is retired. Old
withdrawn retained PID `2426265` was retired only after the replacement passed temporary byte and
browser verification. `geosolve-m84-uat.service`, PID `3736900`, served only the immutable F003
replacement snapshot at `http://100.94.63.83:8080/`. M84-F004 withdraws that snapshot from current
UAT even if the endpoint remains reachable. This historical nomination claims no human UAT row,
approval, Pages publication or milestone closure.

## Post-F004 direct code-authoring amendment

This is an intentional M84 scope extension, not M84-F005. A fresh workspace's Code surface now
offers one direct authored entry and the four existing genuine examples. The starter is a complete
artifact-free rectangle-plus-diagonal `sketch.ts`; its line endpoints are lexical
`frame.corners.lowerLeft`/`upperRight` values. `Authored` persists as its own origin and the shared
installer mutates live authority/sample identity only after every fallible candidate conversion
succeeds. Source focus likewise occurs only after successful creation.

Fresh classification is fail-closed: the graph must contain exactly the canonical document
foundation, current and accepted semantic identities must match, and the accepted native evidence
must independently validate zero points, curves, constraints, computed features and residual
failure. Nonempty, unsupported and retained-failed scenes continue through Preview/Unavailable.

Focused commands run from the amended worktree and pass:

```bash
cargo test --locked -p geosolve-sketch-code --lib managed_only_project -- --nocapture
# 2 passed
cargo test --locked -p geosolve-demo-web --lib \
  fresh_code_surface_offers_one_authored_entry_and_every_genuine_sample -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_starter_applies_persists_and_retains_invalid_code_atomically -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_project_accepts_a_complete_source_replacement_with_exact_history -- --nocapture
# 1 passed
cargo test --locked -p geosolve-demo-web --lib \
  authored_origin_rejects_a_conflicting_legacy_demo_identity -- --nocapture
# 1 passed
```

The owner tests independently check finite accepted geometry, Hard residual validation, exact
native corner-ID aliasing after a rectangle move, artifact-free authority, valid and retained-
invalid persistence, whole-source replacement and Undo/Redo. The separate four-demo M84 ledger is
unchanged.

## Direct code-authoring qualification and immutable replacement nomination

Historical committed source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, passed this clean command:

```bash
env -u GEOSOLVE_ALLOW_DIRTY -u NO_COLOR \
  nix-shell shell.nix --run './scripts/release-gate.sh' 2>&1 | \
  tee /tmp/geosolve-m84-authored-release-gate.log
```

It ran from 2026-08-26 14:55:50.676940562 through 15:12:22.475576111 AEST, exited 0 in
991.798635549 seconds, and its log was independently checked to end with Trunk 0.21.14
`INFO ✅ success` rather than trusting the tee pipeline alone. The 6,146-line,
415,754-byte log `/tmp/geosolve-m84-authored-release-gate.log` has SHA-256
`34bf408f6a565dec5705745f916eb628397a549b4a7002d865269e8d167e179f`. The gate passed formatting,
diff hygiene, warnings-denied Clippy and Rustdoc, locked all-feature workspace tests/doctests,
actual WASM, both TypeScript packages, Rust package extraction/closure, licensing, performance and
benchmarks, and the final Trunk release build. The 271-row golden remains byte-identical at SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; the separate four-demo M84
ledger remains `73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.

Without rebuilding, the exact seven-file gate output was copied to
`/tmp/geosolve-m84-authored-uat.ZYQQyBQQ`. Source, copied and frozen manifests are identical. The
directory is `0555`, every regular non-symlink file is `0444`, and the ordered-manifest aggregate
is `6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`. Complete manifests,
modes, clean-worktree records, gate metadata, HTTP ledgers, browser scripts/logs and screenshots are
under `/tmp/geosolve-m84-authored-freeze-evidence.ngNf7jxZ`.

The same frozen directory was first served on temporary Tailscale port `18086`. `/` plus all seven
files returned HTTP 200, zero redirects, exact MIME/length/hash, no `Location` or
`Content-Encoding`, and `/` equalled `index.html`. The temporary and final `:8080` ledgers are
byte-identical with SHA-256
`1450e4c6d8585ba17dee56feafaf96869c45f764f3400280a0dc37581f9b4eee`.

Sequential Playwright suites against the exact frozen bytes pass on both endpoints: direct authored
lifecycle 3/3, baseline M84 4/4, F003 1/1 and F004 2/2. The authored suite covers `1440x900` and
`1024x720`, zero horizontal/vertical landing overflow, all four cards, Start/edit/Apply, finite
accepted geometry, retained-invalid canvas, Undo, reload/repro and nested card routing. The landing
CSS was corrected during provisional review from 106/342 px vertical overflow to zero at those
resolutions, then the clean gate and exact frozen runs requalified the correction.

Only after temporary byte and browser verification passed was historical F004 PID `3316682`
retired. The temporary PID `4027499` is also retired. M84-F005 withdraws this otherwise qualified
direct-authoring record because it predates the collaborative overlay and semantic-authority
contract. Its snapshot/service evidence is historical only, not a current candidate. GitHub Pages
deliberately remains on accepted M83; a new F005/F006 clean qualification, freeze and UAT
nomination are still required.

## Known bounds and truthful limitations

- Managed-v1 is intentionally a closed projectional subset. Arbitrary custom code is caller build
  input and has no browser runtime or general AST round-trip promise.
- The supported accepted GUI closure now includes lexical rectangle/Segment dependencies,
  Segment-to-Segment endpoints and direct computed FilletSets. Other unsupported declarations
  still reject the complete all-or-nothing promotion and expose a read-only diagnostic rather than
  disappearing. The bundled Braced Frame remains a genuine managed code project.
- Integration test sources which inspect workspace TypeScript/manifests are intentionally not part
  of the published Rust archive. Runtime library code and all eight required assets are
  self-contained and extraction-built.
- General ejection, arbitrary formulas/new constraints, arbitrary TypeScript execution, general
  topological naming, 3D/B-rep behavior and the deferred Offset redesign remain out of scope.
- Human discoverability, presentation feel and repeated real-browser drag/deletion responsiveness
  remain owned by `docs/M84_UAT.md`. Pages must remain M83 until refreshed U1-U14 are explicitly
  approved.

## Remaining release sequence

1. Commit the implemented F005/F006 code and reconciled documentation, run the exact clean release
   gate, freeze its no-rebuild output, exact-verify temporary and retained Tailscale service bytes,
   and run the refreshed baseline/F003/F004/F005 browser suites. **Pending.**
2. Complete refreshed M84-U1 through M84-U14 against only that replacement and record explicit supervising-
   user approval or open another numbered finding.
3. Only after approval, publish the accepted descendant to GitHub Pages, exact-verify its separately
   built hosted artifact, retire `geosolve-m84-uat.service` and close M84.
