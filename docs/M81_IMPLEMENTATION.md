<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M81 implementation — Core architecture consolidation

Status: **implementation and rotating independent review complete; final clean qualification and
focused UAT pending**. Product behavior remains the M80 baseline except for reproduced
transactional defect M81-F001. No candidate, clean release gate, frozen UAT artifact or human
acceptance is claimed yet.

Activation source: `e6df1a26610a2e4ee811fa122867d2b86518e516`

Activation tree: `6135ae4617feee129fd549b2e51d684d8ac8ce19`

Architecture decision: no new ADR. M81 preserves the existing crate graph, public APIs, solver
semantics, independent validators and persistence languages.

## Compatibility freeze

The activation freeze is stored outside the worktree at `/tmp/geosolve-m81-baseline.iPhVsl`:

- `cargo metadata --locked --no-deps --format-version 1`:
  `691591282713d87fb2ae63ff12873577c91daadaa2af0c817c0543b9bfd4d66e`;
- ordered crate-root declarations selected with single-threaded `rg`:
  `0b46f015459f16f20de98373c452c8cb203aebe4cd4cdc54dbfbc78f4d01e9cd`;
- milestone-neutral golden authoring/scene inventory: 271 rows, all `PASS`; and
- golden `--check` and `--require-clean`: pass without changing authority bytes.

The metadata and root declaration files were compared byte-for-byte again after the first core,
sketch and operations cuts and remained identical. Final committed-tree comparison remains part
of qualification.

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

### C2 — sketch document query and validation split

Commit `d29a078` (`refactor(sketch): isolate document query validation`) adds private
`document/query.rs` and `document/profile_offset.rs`. Curve-control/conic query translation and
exact Profile Offset operand/path/junction validation moved mechanically out of `document.rs`.
Existing public and crate-visible document paths, error strings, validation order and persistence
DTOs remain unchanged.

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
M81-F001. Commits `1ccbcfb` (`refactor(editor): isolate retained history publication`) and
`557841c` (`test(editor): clarify bounded allocator regression`) move unchanged checkpoint/
restore/publication helpers to private `coordinator/history.rs` and clarify the focused fixture.

`mutate_features` and `apply_computed_fillet_configuration` now evaluate and checkpoint against a
clone of the live computed-output allocator. Only a successful authenticated feature publication
installs that candidate allocator alongside feature intent and computed snapshot. The successful
publication order remains unchanged.

### C5 — Profile Offset compiler split

Commit `4f89c85` (`refactor(sketch): isolate profile offset compilation`) adds private
`compiler/profile_offset.rs`. Only grouped Profile Offset source registration, path lowering,
incidence construction and audit-row assembly moved. Candidate and independent validation remain
in `compiler.rs`, so the evaluator/validator trust boundary did not collapse. The normalized
original and extracted blocks are byte-identical at SHA-256
`d88e4ee3114396ed28d0ec4a8ce8338a97339b570fe914204a9004cff3ad1ddb`.

### C6 — computed-feature source composition split

Commit `d90a1a6` (`refactor(features): isolate source composition`) adds private
`evaluation/composition.rs`. Endpoint-claim conflict attribution, source-interval composition,
discarded Construction validation, combined source-role resolution and revision-local output ID
creation move together; public evaluation DTOs, branch/root resolution, tolerances and work
charging remain in their prior owner and order.

## Finding ledger

### M81-F001 — rejected computed-feature mutation consumed allocator authority

Reproduced through public `RetainedEditorCoordinator::remove_computed_feature` on the M80 baseline.
A valid 130-feature sidecar exhausts bounded computed-feature work; removing one feature returns
`CoordinatorError::ComputedFeatureWorkStopped`. Before the repair, the rejection advanced the
computed evaluation high-water from revision 2 to 3 even though all of these remained unchanged:

- feature document identity;
- retained design and accepted sketch identities/JSON;
- history length/cursor and transcript;
- computed input, snapshot and problem state; and
- persistent feature/sketch allocator authority.

Owner: retained editor coordinator. The exact focused regression failed at the allocator-neutrality
assertion before the production change and passes after it. No golden expansion is warranted: this
is an isolated publication-authority defect, not a missing authoring family or lifecycle axis.

## Rotating audit ledger

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

## Final qualification still required

- Re-run focused subsystem suites after the final committed tree.
- Prove canonical persistence/workspace/reproduction compatibility and exact 271-row golden
  survey/check/clean output.
- Recompare locked metadata and ordered public declarations byte-for-byte.
- Run format, diff hygiene, warnings-denied workspace Clippy/Rustdoc, locked all-feature workspace
  tests, native/WASM parity, demo adapter checks and the complete clean release gate.
- Freeze the exact no-rebuild output, serve it over Tailscale and byte-verify every release file.

## Known limitations and deferred work

M81 deliberately does not introduce generic solver/domain/compiler traits, a generic coordinator
transaction manager, a new crate, public facade, supported draft-v5 language or browser refactor.
Files that remain large after the accepted mechanical seams are documented technical debt, not a
reason to mix unrelated behavior into this milestone.
