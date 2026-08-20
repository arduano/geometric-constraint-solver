<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 implementation — Core architecture consolidation

Status: **implementation, rotating independent review, final clean qualification and immutable
Tailscale nomination complete; focused human UAT pending**. Product behavior remains the M80
baseline except for reproduced transactional defect M81-F001. GitHub Pages publication and
milestone closure are not claimed.

Nominated product source: `e4eca327fc69c92f95b1722142289302ba4f67bc`

Nominated product tree: `f3ed1bf50b793daae328adf04c0924655dc13d74`

Activation source: `e6df1a26610a2e4ee811fa122867d2b86518e516`

Activation tree: `6135ae4617feee129fd549b2e51d684d8ac8ce19`

Architecture decision: no new ADR. M81 preserves the existing crate graph, public APIs, solver
semantics, independent validators and persistence languages.

## Compatibility freeze and final comparison

The activation evidence was captured outside the worktree at
`/tmp/geosolve-m81-baseline.iPhVsl`:

- `cargo metadata --locked --no-deps --format-version 1`:
  `691591282713d87fb2ae63ff12873577c91daadaa2af0c817c0543b9bfd4d66e`;
- ordered leading crate-root declarations/exports selected with single-threaded `rg` across the
  originally frozen core, sketch, linkage and editor roots:
  `0b46f015459f16f20de98373c452c8cb203aebe4cd4cdc54dbfbc78f4d01e9cd`;
- milestone-neutral golden authoring/scene inventory: 271 rows, all `PASS`; and
- golden `--check` and `--require-clean`: pass without changing authority bytes.

The final comparison was reproduced from an isolated `git archive` extraction of the activation
source and the nominated working tree with these exact selections:

```bash
cargo metadata --locked --no-deps --format-version 1

rg --threads 1 '^pub ' \
  crates/geosolve-linkage/src/lib.rs \
  crates/geosolve-core/src/lib.rs \
  crates/geosolve-constraint-editor/src/lib.rs \
  crates/geosolve-sketch/src/lib.rs

rg --threads 1 --with-filename '^(pub use|pub struct|pub enum)' crates/*/src/lib.rs
```

Activation and nominated bytes compare equal. Metadata remains
`691591282713d87fb2ae63ff12873577c91daadaa2af0c817c0543b9bfd4d66e`; the original four-root
selection remains `0b46f015459f16f20de98373c452c8cb203aebe4cd4cdc54dbfbc78f4d01e9cd` (119
lines, 8,584 bytes); and the supplemental all-nine-library-root selection is
`5cd55480a3d0f8a1d7175ef9359c94cc4dcd14cbf6b5d865abf1697667d1af90` (180 lines,
13,210 bytes). Evidence is `/tmp/geosolve-m81-public-surface.0DYVLo`. These `rg` snapshots prove
the selected declaration/export-leading lines stayed equal; they do not hash multiline type/export
bodies and are not presented as a complete semantic API-diff mechanism. Locked compilation,
package tests and the clean gate supply the complementary type-level evidence. The release copy of
`docs/API_COMPATIBILITY.md` remains the exact nominated product byte and was not edited during this
evidence-only reconciliation.

## Implementation ledger

### C1 — core solver responsibility split

Commit `5755970` (`refactor(core): separate solver responsibilities`) leaves the public solver
DTOs and `Problem` orchestration in `solver.rs` and adds three private modules:

- `solver/hard.rs` owns component iteration, bounds, dense/sparse hard steps, linear algebra,
  damping, backend evidence and hard solve aggregation;
- `solver/priority.rs` owns lexicographic temporary-level planning, preservation certification,
  coupled/component optimization and priority diagnostics; and
- `solver/validation.rs` owns independent returned-row validation, rank/bound mobility,
  conflict/redundancy diagnostics and audit annotation.

No residual evaluator moved into the validator and no public export, solver trait, tolerance,
fallback, trace, work-charge or priority policy changed.

### C2a — sketch document query and validation split

Commit `d29a078` (`refactor(sketch): isolate document query validation`) adds private
`document/query.rs` and `document/profile_offset.rs`. Curve-control/conic query translation and
exact Profile Offset operand/path/junction validation moved mechanically out of `document.rs`.
Existing public and crate-visible document paths, error strings, validation order and persistence
DTOs remain unchanged.

### C2b — Profile Offset compiler split

Commit `4f89c85` (`refactor(sketch): isolate profile offset compilation`) adds private
`compiler/profile_offset.rs`. Only grouped Profile Offset source registration, path lowering,
incidence construction and audit-row assembly moved. Candidate and independent validation remain
in `compiler.rs`, so the evaluator/validator trust boundary did not collapse. The normalized
original and extracted blocks are byte-identical at SHA-256
`d88e4ee3114396ed28d0ec4a8ce8338a97339b570fe914204a9004cff3ad1ddb`.

### C3 — equation-free Profile Offset planning split

Commit `8c14ff8` (`refactor(sketch-ops): isolate profile offset planning`) moves operand/path/
junction planning to private `profile_offset.rs`. Commit `0e19b42`
(`refactor(sketch-ops): make offset dependencies explicit`) applies the first independent review
finding by replacing its gate-blocking wildcard import with the exact private dependency list.
Commit `860a9e9` (`style(sketch-ops): normalize profile offset imports`) applies rustfmt's final
deterministic import order. Planning logic, topology provenance, traversal order and typed outcomes
are unchanged.

### C4 — computed-feature mutation transaction and retained history split

Commit `34ba3c3` (`fix(editor): keep rejected feature mutations allocator-neutral`) resolves
M81-F001. Commit `1ccbcfb` (`refactor(editor): isolate retained history publication`) moves
unchanged checkpoint/restore/publication helpers to private `coordinator/history.rs`. Commit
`557841c` (`test(editor): clarify bounded allocator regression`) changes only the focused
assertion wording.

`mutate_features` and `apply_computed_fillet_configuration` now evaluate and checkpoint against a
clone of the live computed-output allocator. Only a successful authenticated feature publication
installs that candidate allocator alongside feature intent and computed snapshot. The successful
publication order remains unchanged.

### C5 — computed-feature source composition split

Commit `d90a1a6` (`refactor(features): isolate source composition`) adds private
`evaluation/composition.rs`. Endpoint-claim conflict attribution, source-interval composition,
discarded Construction validation, combined source-role resolution and revision-local output ID
creation move together; public evaluation DTOs, branch/root resolution, tolerances and work
charging remain in their prior owner and order.

## Finding ledger

### M81-F001 — rejected computed-feature mutation consumed allocator authority

Reproduced through public `RetainedEditorCoordinator::remove_computed_feature` on the M80 baseline.
A valid 130-feature sidecar exhausts bounded computed-feature work; removing one feature returns
`CoordinatorError::ComputedFeatureWorkStopped`. In the pre-repair reproduction the rejection
advanced the computed evaluation high-water from revision 2 to 3 even though the focused fixture's
direct sentinels remained unchanged:

- feature-document identity, which covers the complete persistent feature payload and its feature/
  corner allocator cursors;
- retained design and accepted sketch identities plus exact exported JSON;
- history length/cursor and the complete transcript value;
- computed input, absent computed snapshot and durable computed-evaluation problem state; and
- the complete computed-output allocator high-water.

Owner: retained editor coordinator. The exact focused regression failed at the allocator-neutrality
assertion before the production change and passes after it. No golden expansion is warranted: this
is an isolated publication-authority defect, not a missing authoring family or lifecycle axis.
The regression does not construct or compare an `EditorScene`, the complete retained sketch
session, transient coordinator fields or checkpoint contents; documentation therefore does not
claim those as direct assertions. The repaired ownership flow stages both durable mutation paths,
and the full collateral suites qualify their broader publication behavior.

Exact focused command and result:

```bash
cargo test --locked -p geosolve-constraint-editor --lib \
  coordinator::tests::m81_f001_rejected_computed_feature_mutation_is_allocator_neutral \
  -- --exact
```

Result: one passed, zero failed, 403 filtered out.

## C6 rotating audit ledger

### Audit A — core/linkage and operations

An independent read-only review compared moved definitions, orchestration and normalized Profile
Offset planning with the activation source. It found no semantic drift in solver iteration,
charging, priority order, diagnostics, source ordering, fallback selection, traces or returned-row
validation. A second reviewer accounted for all 230 moved core production items: 211 were token-
identical after removing required `pub(super)` visibility, and the other 19 differed only by
rustfmt trailing commas. All 16 relocated unit-test bodies were byte-identical. The first review
did find the wildcard-import Clippy failure corrected by `0e19b42`; final rustfmt ordering is
recorded by `860a9e9`.

Evidence at that checkpoint:

- core tests: 211 pass, one intentional ignore;
- core warnings-denied Clippy: pass;
- linkage collateral: 174 pass, one intentional ignore;
- sketch operations: 40 pass;
- corrected sketch-ops warnings-denied Clippy: pass; and
- metadata, root declarations and diff hygiene: pass.

### Audit B — sketch document/compiler/persistence

The document split passed the full locked all-feature `geosolve-sketch` suite, package check,
format check and warnings-denied all-target Clippy. Focused M11, M19, M77 curve-control and M80
Offset coverage passed 71/71. No wire DTO, canonical encoding, public path, validation order or
error-text change was found. A rotating follow-up independently reviewed `d29a078` and `0e19b42`,
then proved the Profile Offset compiler block token-identical before extraction. The focused
Profile Offset/native-Fillet matrix passes 30/30; full all-feature sketch tests, package check,
format and warnings-denied Clippy pass.

### Audit C — editor/coordinator

The allocator concern was not treated as a refactor assumption: it was reproduced through the
public owner, assigned M81-F001, frozen with an exact regression and only then repaired. The full
editor library passed 404/404; every package integration/doc test and warnings-denied all-target
Clippy passed after the repair and private history extraction. A fresh cross-subsystem reviewer
confirmed the defect first entered the durable mutation path before M81, both durable paths now
stage the same allocator/checkpoint authority, preview never-reuse policy is intentionally
unchanged, and the extracted history block is token-identical after visibility normalization. No
additional finding was warranted.

### Audit D — computed features

The source-composition cut was compared against the committed parent after normalizing only module
visibility and a trailing blank line. It is token-identical. All-feature feature check, 46 unit
tests plus integration/doc coverage, assigned-file format/diff hygiene and warnings-denied all-
target Clippy pass. A fresh editor-side reviewer additionally confirmed private visibility,
deterministic conflict/interval ordering, exact discarded-fragment validation, unchanged output-ID
allocation, error order and work charging. No finding was opened.

## Final committed-tree qualification

Fresh committed-tree package qualification passed before nomination: core reported 211 passing
tests with one intentional ignore; linkage 174 with one intentional ignore; sketch operations 40;
sketch features 46 unit tests plus its integration coverage; the full all-feature sketch suite;
editor 404 library tests plus all package integration/doc tests; and demo 154/154. The focused
M81-F001 command above passed 1/1.

The exact golden commands all passed without changing authority bytes:

```bash
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
```

The inventory remains exactly 271 `PASS` rows. M81 adds no geometry/authoring scenario and no
golden row; its only new case is the focused coordinator transaction regression.

From a clean `e4eca32` worktree, this exact command completed with exit 0:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate ran from 2026-08-20 11:42:50 to 11:58:14 AEST. Its 271,230-byte, 3,516-line log is
`/tmp/geosolve-m81-clean-gate.e4eca32.nix.log`, SHA-256
`43abb1e262293d607e6e37d636b90979d9be7c0807020c0d7bbc49800716797e`. It passed formatting and
diff hygiene, warnings-denied locked all-feature workspace Clippy, locked all-feature workspace
tests, exact 271-row golden `--require-clean`, M70/M71/M74/M75/M76/M77/M79 native/WASM parity,
the demo WASM check, warnings-denied Rustdoc, benchmark compilation, M14 and M32 performance
budgets, the ignored 256-moving-body sparse crossover in 147.70 seconds, licence/package checks
and Trunk 0.21.14 release assembly.

## Immutable candidate and served-byte evidence

Without rebuilding, the gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m81-uat.QqItRd` and byte-compared before and after freezing. The directory is `0555`;
all seven entries are regular non-symlink files at `0444`. The C-locale ordered `sha256sum *`
manifest aggregate is
`df24deb988a31a373b3f973432081078c15e157382134f62c99aaabe96b8e49e`:

```text
7acf06ec28c181468f26a92f6978af0f4b9d4f3205e076e602c517f00923d07f  API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  LICENSE
61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803  THIRD_PARTY_LICENSES.md
93f79eda2bf49cc53495dbafe2b7aabb5f7f9cc3852c9a48af2758694718e953  geosolve-demo-web-5512436a65f3f954.js
599deb5b6e7241b76963318e921ecf0e58e39e8ee1e70c74d31ebcaeef10f8a1  geosolve-demo-web-5512436a65f3f954_bg.wasm
f4a69f88a58b2a12b17c972ca98c34d8f69d4c7192f6276aa89df62480adc4a2  index.html
957c7809eab90b61a2a72266af8f8660390b8c04fcce7b6c9e06398582097bbf  styles-a41d7984178d1121.css
```

| File | Bytes |
| --- | ---: |
| `API_COMPATIBILITY.md` | 28,079 |
| `LICENSE` | 35,148 |
| `THIRD_PARTY_LICENSES.md` | 3,120 |
| `geosolve-demo-web-5512436a65f3f954.js` | 33,750 |
| `geosolve-demo-web-5512436a65f3f954_bg.wasm` | 7,749,099 |
| `index.html` | 31,033 |
| `styles-a41d7984178d1121.css` | 38,291 |

Temporary service `geosolve-m81-temp-uat.service`, PID `2842248`, first served only that snapshot
at `100.94.63.83:18080`. Proxy-disabled, cache-bypassed identity requests for `/` and all seven
files returned HTTP 200 with zero redirects, no `Location` or `Content-Encoding`, exact expected
media type and length, and snapshot-identical bytes; `/` equals `index.html`. Evidence is
`/tmp/geosolve-m81-temp-verify.baolJt/results.tsv`, SHA-256
`7e981a47c3d02957c55e81eddb747e749e21f32d42464cdff4f6b1065e94a855`.

Only after that ledger passed, `geosolve-m81-uat.service`, PID `2850776`, began serving the same
immutable directory at `http://100.94.63.83:8080/`. The same eight checks passed independently;
final evidence is `/tmp/geosolve-m81-final-verify.Z4aCP5/results.tsv`, with the same result-ledger
SHA-256 because every asserted path/status/type/length/body hash is identical. The temporary
listener is retired and the retained `:8080` service remains live for focused human UAT.

The documentation changes recording this evidence are descendants of `e4eca32`; they do not
replace the nominated product source/tree or rebuild its artifact. Human acceptance and the normal
GitHub Pages closeout remain pending.

## Known limitations and deferred work

M81 deliberately does not introduce generic solver/domain/compiler traits, a generic coordinator
transaction manager, a new crate, public facade, supported draft-v5 language or browser refactor.
Files that remain large after the accepted mechanical seams are documented technical debt, not a
reason to mix unrelated behavior into this milestone.
