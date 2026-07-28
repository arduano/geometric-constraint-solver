<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M42 implementation record: typed host parameters

Status: completed and globally qualified 2026-07-27. The supervising caller approved
completing executable dimensionless bindings in M42 before M43-M44 integration so the next
human UAT would evaluate the complete parameter model rather than an intentionally reduced
one. That review was later relocated to post-cleanup M53. The
implementation preserves M41's retained-design, attempted, and accepted state separation.

## Requirements

- Host-owned expression/configuration systems supply immutable, revisioned, finite typed
  values. GeoSolve must not add a formula parser, configuration evaluator, or host
  dependency graph (`PLAN.md:1877-1888`; `ACCEPTANCE.md:742-746`).
- Persist parameter identities and bindings for length, angle, dimensionless, and
  activation inputs. One parameter may drive multiple targets without becoming a solver
  unknown (`PLAN.md:1880-1883`).
- Accepted geometry and diagnostics must be reproducible from identical design and
  parameter bytes. A rejection must change neither accepted input nor accepted output
  revision (`PLAN.md:1887-1888`).
- Input batches and declared reference-measurement outputs require persistent identity,
  unit, provenance, revision evidence, and stale-commit protection. Ownership cycles are
  rejected atomically (`PLAN.md:1883-1885`).
- Preserve global safety rules: validate finite geometry/residuals independently before
  success-like publication, retain accepted truth on failure, and do not infer discrete
  state from coordinates (`AGENTS.md`; ADR 0025).

## Existing typed scalar/dimension seams

`SketchDocument` already stores typed scalar values and document dimensions. Dimension
lowering resolves a document scalar through `scalar_value(document, target)` and passes
the resulting `f64` directly to core dimension creation; driving/reference mode is mapped
explicitly from `DocumentDimensionMode` (`crates/geosolve-sketch/src/document_lowering.rs:1370-1419`).
This is the fixed-coefficient seam: M42 must substitute a validated batch value before
lowering rather than introduce a runtime scalar variable or residual equation.

Reference dimensions already become `ReferenceDimensionValue` in the compiler/solve
result (`crates/geosolve-sketch/src/compiler.rs:1150,1911,1975`). M38 exposes the closest
output vocabulary: `DocumentMeasurementValue` carries a value, `DocumentScalarUnit`,
optional residual, audit record, and explicit retained-design versus accepted-document
provenance (`crates/geosolve-sketch/src/m38.rs:126-213`). M42 output proposals should use
the same typed-unit and audited-evaluation discipline but must add parameter/output
identity and input/accepted revision stamps rather than overload an M38 measurement.

## Proposed public and persistence contract

Add a small sketch-domain parameter vocabulary, exported from `lib.rs`, with:

- a persistent `DocumentParameterId`, allocated by the existing monotone document
  allocator, represented in `DocumentElementId`, and never reused;
- a closed declared parameter kind and batch value for canonical length, angle,
  dimensionless, or Boolean activation input; numeric kinds reuse the matching
  `DocumentScalarUnit` semantics and do not introduce display-unit conversion;
- persistent binding records from a parameter ID to a typed target: a driving dimension,
  an explicitly declared dimensionless runtime scalar property, or an M41 activation
  element; and
- declared reference-output records which name a reference dimension and an output
  parameter identity, without granting GeoSolve ownership of that parameter.

The retained document owns stable local parameter/binding/output identity and expected
unit/kind, but not an evaluated expression. The host supplies a `ParameterBatch` with a
monotone revision, canonical digest, and canonical ordered entries of `(parameter ID,
typed finite value)`. It must be immutable once accepted as an attempt input. Entries are
complete for all required active bindings, unique, finite, bounded by resource limits,
and exact-unit compatible; unknown, duplicate, missing, non-finite, or incompatible
entries are typed unsolved-design/attempt outcomes, never implicit defaults.

The existing supported v4 JSON codec remains frozen. `to_canonical_json` rejects
non-default M41/M42 state rather than dropping it. The additive, explicitly unstable
draft-v5 DTO round-trips M42 declarations, bindings and outputs canonically; M107X remains
the persistence freeze point.

## Immutable batch canonicalization and digest

Canonicalize batch entries by persistent parameter ID, reject duplicates before digesting,
and digest a versioned binary/canonical serialization containing revision, parameter ID,
unit/kind, and exact finite `f64` representation. Do not hash host formula text, display
units, host configuration keys, or unordered-map iteration. The empty batch has a single
canonical digest/identity.

`SketchAttemptInput::for_document` is constructed before the retained attempt runs
(`crates/geosolve-sketch/src/document_session.rs:1906-1933`). Extend that immutable input
and every accepted/attempt audit capsule with the parameter batch revision/digest,
alongside the existing M41 activation stamp. The batch must be captured once before
lowering/evaluation; no callback or mutable host read may occur after capture.

## Fixed-coefficient lowering/no-artificial-unknown design

Build an immutable resolved-binding view from document intent plus the canonical batch.
For each driving dimension bound to a parameter, use its typed batch value as the target
passed to existing lowering. A parameter shared by N dimensions therefore supplies N
fixed coefficients/targets, not an N+1th solver coordinate and not an equality residual.
Unbound document scalars retain current behavior. Reference dimensions remain
non-driving and cannot be parameter-driven as a solver target.

An activation binding resolves before an attempt into an immutable M41 activity overlay:
`false` contributes `HostConfigurationInactive`, while `true` contributes no inactivity
and cannot override an explicit M41 host-inactive or unavailable-reference reason.
Validate binding target type and unit before lower/closure construction. The lowered
audit/provenance view must identify the persistent parameter and batch revision/digest
that supplied each bound target. The separate M41 activation and M42 parameter stamps
are both retained; neither digest is silently folded into the other.

## Output proposals/provenance

After an independently validated accepted solve, evaluate declared reference outputs and
return proposals, not host mutations. Each proposal includes output parameter ID, source
dimension/measurement identity, finite typed value/unit, accepted document revision,
accepted revision, parameter-batch revision/digest, and explicit `AcceptedDocument`
provenance. Failed/unsolved attempts produce no accepted proposal and cannot replace the
last accepted output set.

The host alone decides whether/how to commit a proposal into its expression system. The
document's declared input-parameter and output-parameter ownership sets must be disjoint;
overlap is the complete locally observable cycle and rejects atomically. GeoSolve stores
no parameter-to-parameter formula edges and therefore must not invent a transitive host
graph or attempt to discover host-side formula/configuration cycles.

## Lifecycle and stale-commit semantics

M34/M41 already publish an immutable attempt after a pre-commit checkpoint and retain an
independent accepted state (`document_session.rs:1870-1945`; ADR 0025). M42 must require
an exact parameter input stamp at attempt creation and compare it at publication/commit.
A newer host batch, changed design, changed activation stamp, or changed declared binding
set makes a candidate stale: it remains inspectable as an attempt but cannot update
accepted geometry, accepted parameter-input identity, or output proposals.

Parameter batch acceptance is atomic with the solve attempt. Invalid batch structure,
missing input, unit mismatch, ownership cycle, cancellation, solve failure, or stale
commit leaves accepted geometry and its input/output stamps untouched. Attempt revisions
remain never-reused; parameter batch revision is host-owned evidence and is not a
replacement for design, attempt, or accepted revision.

## Implementation slices with exact file scopes

1. **Domain DTOs and persistence:** edit only
   `crates/geosolve-sketch/src/document.rs` and `lib.rs`; add IDs, closed units, binding
   and output declarations, validation/canonicalization, `DocumentElementId` integration,
   draft-v5 DTO fields, and v4
   rejection characterization. Do not change core or web crates.
2. **Batch and resolved input:** edit `document_session.rs` plus its public exports; add
   immutable canonical batch/stamp, resolved binding validation, and propagation through
   `SketchAttemptInput`, attempts, and accepted state.
3. **Lowering/audit:** edit `document_lowering.rs`, `compiler.rs`, and only the necessary
   sketch-domain audit types; thread resolved fixed values into dimensions and preserve
   parameter provenance. No solver-variable or residual-family change is permitted.
4. **Outputs and atomic publication:** edit `document_session.rs` and `m38.rs` only if a
   reusable typed measurement evaluator is needed; publish stamped output proposals after
   independent validation and enforce local ownership-cycle/stale checks.
5. **Qualification:** add focused `crates/geosolve-sketch/tests/m42.rs`; update no UI.
   Run the full workspace and required WASM gates only after focused coverage passes.

## Qualification matrix

- Same document plus byte-identical canonical batch yields identical accepted geometry,
  audit, input stamp, and proposals.
- One length/angle parameter drives multiple compatible dimensions and one dimensionless
  parameter drives multiple explicitly declared scalar properties while the host
  parameter itself adds no solver unknown.
- Missing, duplicate, unknown, non-finite, wrong-unit, oversized, and stale batches are
  rejected atomically; old accepted geometry/input/proposals remain visible.
- Reference outputs have persistent identity, expected units, accepted provenance and
  exact accepted/input stamps; a failed attempt emits no newly accepted proposal.
- Any overlap between local input and output parameter ownership rejects deterministically;
  unrelated host formula/configuration graphs are neither parsed nor evaluated.
- Activation bindings feed the M41 closure with explicit revision/digest evidence and do
  not alter retained user activation intent.
- Draft-v5 round trip is canonical; v1-v4 golden bytes/readers/writers remain unchanged,
  and supported v4 export rejects all non-default M42 state.
- Retained design, last attempt, and accepted state remain distinct across rejected,
  stale, and successful parameter updates; attempt identity is never reused.

## Decisions / inferred constraints

- Reuse document scalar units and dimension target semantics; do not create a parallel
  generic expression/value system.
- Parameter values are immutable attempt inputs and fixed numerical coefficients, never
  core variables, hidden fixed geometry, or soft constraints.
- Persistent parameter identity is local to the sketch document. Arbitrary host IDs and
  formula text remain host data.
- `DocumentParameterId` is a first-class `DocumentElementId` variant using the existing
  monotone allocator so canonical graph iteration, lookup and import high-water checks
  remain complete. Individual binding/output records use deterministic parameter/target
  identity pairs rather than allocating unnecessary second-order IDs.
- Numeric parameter bytes are already in canonical model units. Accepted input kinds are
  exactly length, angle and dimensionless; curvature and generic curve-parameter scalar
  units are not M42 host-input kinds. Activation is a Boolean kind, not a numeric unit.
- M42 output proposals are deliberately limited to declared reference dimensions. The
  broader M38 derived-measurement catalog remains available but is not parameter output
  ownership in this milestone.
- Output proposals are one-way, post-validation data. GeoSolve cannot commit host values.
- Staleness must compare every captured input stamp, including M41 activation, not merely
  parameter revision.

### Dimensionless input binding decision

- **Decision: add one narrow, explicitly declared dimensionless scalar target in M42.** The
  target carries a complete `DocumentScalarPropertyRef` whose unit and branch are both
  dimensionless. It is never inferred by enumerating internal `DesignScalar` values, so the
  M44 consumer exposes only host bindings deliberately declared by document intent.
- Binding validation reuses the M36 scalar-property validator and additionally requires
  `DocumentParameterKind::Dimensionless`, `DocumentScalarUnit::Dimensionless`, and
  `DocumentScalarBranch::Dimensionless`. Length, angle, curvature, curve-parameter, contact,
  trim, and branch-bearing scalar properties are not accepted through this target. Unit
  reinterpretation is forbidden.
- Lowering adds one ordinary hard fixed-scalar row for each active dimensionless binding by
  reusing the existing M36/runtime fixed-scalar residual family and derivative/audit
  semantics. The batch value is the row's immutable target coefficient. The host parameter
  itself adds no solver coordinate, and one parameter may supply several independently
  identified scalar targets.
- A dimensionless binding's persistent identity remains its parameter/typed-target pair;
  runtime provenance maps that pair to the generated fixed-scalar source and exact batch
  revision/digest. No generic public scalar mutation API, formula evaluator, or automatic
  exposure of latent solver scalars is introduced.
- Qualification compares bound and equivalent fixed-scalar baselines: the parameterized
  form adds no parameter unknown, uses the existing one-row topology per declared target,
  has finite-difference Jacobian/audit coverage inherited from and cross-checked against
  M36, rejects incompatible properties atomically, and reproduces accepted geometry and
  evidence from identical design/batch bytes.

## Open questions

- None blocking implementation. M101X owns prepared asynchronous CAS jobs; M42 provides synchronous stale-input checks
  without precommitting M101X's public job API.

## Completion evidence

- `crates/geosolve-sketch/tests/m42.rs`: 16 focused lifecycle, validation,
  deterministic-persistence, activation, provenance, fixed-scalar audit and operation
  control regressions passed.
- `crates/geosolve-sketch/tests/m36.rs`: 13 fixed/equal scalar semantic, audit and
  finite-difference regressions passed; M42 reuses this residual family unchanged.
- Independent verification found and prompted correction of driving-dimension batch
  domain validation; the atomic rejection regression is retained in M42.
- Exact final commands passed:
  - `cargo test --locked -p geosolve-sketch --all-features --test m42`;
  - `cargo test --locked -p geosolve-sketch --all-features --test m36`;
  - `cargo test --locked --workspace --all-features`;
  - `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`;
  - `cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown`;
  - `nix-shell ../../shell.nix --run 'trunk build --release'` from
    `crates/geosolve-demo-web`;
  - `cargo fmt --all -- --check`; and
  - `git diff --check`.
- The recurring Cargo `license`/`license-file` warnings predate M42 and did not fail any
  gate.

## Out of scope

- Formula syntax/parsing, display-unit formatting/conversion policy, host configuration
  graphs, PDM, and host output commits (ADR 0026).
- M43 external snapshots and M44 editor/browser workflows.
- New constraint primitives, residual equations, solver variables, weighted-priority
  semantics, or core solver changes.
- M100X stable diagnostics and M101X concurrency/prepared-job APIs beyond the input-stamp
  evidence needed to avoid incompatible future designs.

## Evidence and source pointers

- `PLAN.md:1873-1888` and `ACCEPTANCE.md:742-746` are the M42 authority.
- `docs/adr/0025-retained-design-attempt-and-accepted-state.md` defines revision and
  immutable-input evidence; `docs/adr/0026-immutable-host-inputs-and-external-snapshots.md`
  allocates host formula/configuration ownership.
- `document_lowering.rs:1370-1419` maps typed document dimension modes and reads scalar
  targets at the fixed-coefficient lowering seam.
- `compiler.rs:1150,1911,1975` identifies existing reference-dimension result values.
- `m38.rs:126-213` supplies existing typed measurement, unit, audit, and retained versus
  accepted provenance vocabulary.
- `document_session.rs:1870-1945` establishes retained attempt publication and the
  `SketchAttemptInput::for_document` capture seam.
- `document.rs:6982-7098` demonstrates M41 activation validation, supported-v4 rejection,
  and unsupported draft-v5 encoding precedent.
