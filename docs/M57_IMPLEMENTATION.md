# M57 implementation report

M57 is complete as of 2026-07-29. It carries the existing component-local `SolveSession`
transaction model through the retained persistent-document lifecycle. Compatible attempts retain
runtime/core identities and caches; incompatible topology or source shape takes an explicit full
rebuild. No residual equation, tolerance, branch rule, schema, host-history policy, `unsafe` code
or native solver dependency changed.

## 1. Files and APIs added

`geosolve-sketch` adds:

- `SketchDocument::dependent_closure`, the canonical reverse persistent dependency graph;
- persistent-ID indexes inside `DocumentRuntimeMap` plus `runtime_contact`;
- retained/scratch topology compatibility checks for document and compiled runtime mappings;
- `SketchSession::{execution_summary,production_scale_assessment}`;
- `SketchSessionExecutionKind`, `SketchSessionExecutionSummary`,
  `SketchProductionScaleAssessment` and `SketchRankAuthority`; and
- `SketchAcceptedDocumentState::{analyze_visual_profiles_cached,
  visual_profile_cache_entries}`.

The internal `SketchSession::apply_compatible_candidate` compares scratch and retained shape
variables, rebuilds only the supplied source closure into one core `SessionPatch`, retains exact
variable/source/residual/bound IDs, and uses the existing complete-candidate validator before
commit.

`crates/geosolve-sketch/tests/m57.rs` adds ten direct incremental/scale regressions.
`docs/SCENARIOS.md` records M57-C1.

## 2. Mathematical behavior implemented

The optimized path does not introduce a second equation implementation. A retained attempt still
resolves activation, parameter and immutable external inputs, validates the document, and lowers a
scratch runtime. The scratch compile is a compatibility oracle:

- point, circle, arc, conic, NURBS and latent variable mappings must match;
- source/core-source/residual identities, residual incidence/category/dimension and bound
  coordinate incidence must match; and
- document point/curve/source/contact mappings plus parameter target/runtime identities must
  match.

When compatible, changed finite shape values and affected persistent sources become one
revision-checked `SessionPatch`. A source is affected when its transitive persistent dependency
closure contains a changed document element, or when its exact parameter/external input provenance
changes. Core derives dirty components from those replacements. Unchanged components may reuse
zero nonlinear iterations, but every returned component still receives fresh hard-row,
Jacobian/derivative, bound, audit and rank evaluation. Sketch finalization then independently
reconstructs geometry, normalizes latent state, validates branch/domain rules and projects through
persistent mappings before atomic publication.

Parameter and external-snapshot updates with unchanged request shape enter this retained path
directly. A point edit preserves the existing temporary-drag attempt semantics, then uses the
retained runtime for accepted publication. Same-shape activation revisions submit an empty
equation patch and freshly validate every reused component. Any mapping/source-shape mismatch
takes `FullRebuild`; this includes a contact rebind whose persistent source identity remains
stable but whose residual now touches a different curve span. It is never labeled incremental.

Sparse hard steps remain available. Numerical rank is not claimed as sparse-authoritative:
`SketchProductionScaleAssessment` reports `BoundedDenseSvd`, with support limited to connected
components having no more than 256 active rows and 256 active tangent coordinates. This is the
safe-Rust controlled dense-kernel limit.

## 3. Exact commands run and outcomes

Focused qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-sketch --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-sketch --test m57 --test m34_lifecycle --test m42 --test m43 --test m56'
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-core -p geosolve-sketch --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-sketch --all-features'
nix-shell shell.nix --run '/usr/bin/time -f "elapsed=%e max_rss_kib=%M" cargo test --locked --release -p geosolve-sketch --test m57'
```

The M57 suite passes 10/10. Targeted warnings-denied Clippy, M34/M42/M43/M56 regressions and the
complete all-feature sketch package pass. The release M57 corpus passes in 0.09 seconds of test
execution; a warm complete command observes 0.21 seconds elapsed and 66,496 KiB maximum RSS on the
development host. These observed values describe the fixture/host and are not mathematical
tolerances.

Final milestone qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

The complete formatting, warnings-denied Clippy, locked all-feature workspace tests,
all-feature WASM check and Trunk release build pass. Cargo's pre-existing duplicate
`license`/`license-file` warnings remain non-fatal.

## 4. Acceptance criteria passed

- Incremental local-geometry and parameter results match fresh rebuilds on accepted branch-bearing
  document state and rank.
- Parameter and immutable external-reference changes dirty their source component and reuse an
  unrelated component.
- A same-shape activation revision reuses every component; a topology addition and a contact
  rebind that changes residual incidence report a full rebuild.
- Two- and 16-component workloads retain runtime/topology identities and freshly validate all
  returned hard/rank evidence.
- Persistent point/curve/source/contact lookups use retained indexes.
- Profile analysis is cached only by options inside one accepted revision and invalidated by the
  next accepted state.
- Deterministic component-work exhaustion publishes no input, lifecycle, geometry or cache state.
- Rank authority is honestly bounded dense SVD at 256 active rows/tangent coordinates per
  connected component; sparse execution does not imply sparse rank certification.

## 5. Known limitations and next blocker

Scratch document validation/lowering/compatibility compilation remains linear in document size;
M57 retains numerical topology and component caches rather than introducing a general partial
document compiler. Point edits keep the established temporary-drag solve before retained accepted
publication. Source activation that changes equation shape, object creation/deletion and request
shape changes intentionally rebuild. Host/application history remains outside
`RetainedSketchDocumentSession`; the older accepted-only command session retains its directly
indexed history contract.

M58 is next: a separate equation-free sketch-operations companion must emit deterministic public
transactions for split/break/trim/extend, mirror, chamfer, grouped macros/patterns and
multi-interval visible topology.
