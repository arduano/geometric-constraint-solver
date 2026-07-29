<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M54 stable diagnostics and mobility evidence

Status: complete on 2026-07-29.

## 1. Files and APIs added

- `crates/geosolve-sketch/src/diagnostics.rs` adds the stable
  `SketchDiagnosticSnapshot` family. It publishes accepted/attempt provenance, complete currently
  implemented input identity, solve facts, structural and numerical rank, equality/bidirectional/
  one-sided mobility, persistent component membership, source and bound summaries,
  completeness-aware conflict/redundancy searches, activation/dependency evidence, parameter and
  external-reference states, and typed non-mutating repair suggestions.
- `RetainedSketchDocumentSession::{latest_attempt_diagnostics,accepted_diagnostics}` and
  `SketchAcceptedDocumentState::diagnostics` publish coherent immutable snapshots.
- Structured `SketchParameterInputIssue` evidence and exact `EffectiveActivity` evidence now survive
  every retained-attempt path that reaches the applicable input stage.
- `SketchSolveResult::unstable_core_report` and `SketchSession::unstable_bound_report` are the
  explicit advanced compatibility seams. Their raw fields are no longer public.
- `geosolve-constraint-editor` consumes stable persistent conflict candidates for current-problem
  attribution. `geosolve-demo-web` renders stable rank, mobility, search-completeness and repair
  evidence rather than interpreting `geosolve-core` reports.
- `crates/geosolve-sketch/tests/m54.rs` adds nine focused milestone regressions. Existing sketch
  tests/examples/bench support were mechanically migrated to the explicit unstable accessor where
  low-level core evidence remains intentional.

No old playground, `/#/dev/lab`, browser E2E, CDP or browser-owned solver semantics were added.

## 2. Mathematical behavior implemented

M54 adds no residual equation and changes no convergence or acceptance tolerance. It translates
already independently validated solver evidence into the owning sketch domain without collapsing
distinct claims:

- numerical rank/nullities and singularity remain separate from structural matching rank/nullities
  and classification;
- equality nullity, bounded bidirectional lineality and one-sided feasible motion remain separate;
- coordinate bounds remain inequality evidence and do not become equation rows;
- diagnostic candidates always carry `Complete`, `Truncated`, `Skipped` or unknown status plus
  configured budget and consumed work, so an incomplete empty list never means “none”;
- reduced components are identified by persistent document elements and sources, not runtime
  variables;
- parameter and external-input failures retain their persistent local IDs and exact attempt stamp;
- repair suggestions are read-only typed proposals. The API contains no mutation operation and
  makes no globally-minimal-conflict claim.

The solver still returns success-like state only after the existing independent finite residual,
geometry, domain and branch validation.

## 3. Exact commands run and outcomes

All commands ran from the repository root through `nix-shell shell.nix`.

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown
cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release
```

Outcomes:

- formatting check: pass;
- warnings-denied workspace Clippy: pass;
- complete all-feature workspace suite: pass, including M54 9/9, editor 60/60 and demo-web 31/31;
- all-feature `wasm32-unknown-unknown` check: pass;
- Trunk 0.21.14 optimized release build: pass.

The first direct Trunk invocation inherited `NO_COLOR=1`, which Trunk 0.21.14 rejects before
building. The supported release invocation above unsets that environment variable, matching the
repository release gate, and passed.

## 4. Acceptance criteria passed

- Stable domain DTOs cover solve, source, component, dependency, activation, parameter,
  external-reference and bound evidence with persistent identities and exact provenance.
- Structural/numerical rank, equality/bounded/one-sided mobility and diagnostic completeness are
  independently represented and directly regression-tested.
- Complete redundancy and conflict candidates map to persistent source IDs across runtime
  remapping; truncated/skipped searches remain explicitly incomplete.
- Parameter missing/wrong-kind and external missing/topology-mismatch failures target persistent
  input IDs with typed repair suggestions.
- Snapshot generation does not mutate retained design, attempt or accepted state.
- Production editor/workbench code contains no raw sketch core-report interpretation.

## 5. Known limitations and next blocker

- The raw core-report and runtime/compiler inspection seams remain available only for advanced
  compatibility users and remain intentionally unstable until a future API-freeze milestone.
- M54 does not add prepared jobs, sparse incremental rebuilding, operation/topology companions or
  new constraint actions.
- M55 is the next active gate. It must expose the preserved alpha relation, dimension and explicit
  branch-action matrix through the headless editor and sole workbench before M56 begins.
