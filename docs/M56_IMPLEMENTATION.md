# M56 implementation report

M56 is complete as of 2026-07-29. It adds host-scheduled prepared sketch work with immutable
complete-input capture, scratch execution, non-mutating candidate patches and exact-input
compare-and-swap publication. It introduces no scheduler, lock around solver state, schema change,
`unsafe` code or browser-owned concurrency policy.

## 1. Files and APIs added

`geosolve-core::OperationOutcome::map` transforms only completed values and preserves cancellation
or work-exhaustion reports exactly.

`geosolve-sketch` adds:

- `PreparedSketchInput`, covering current design, latest attempt, accepted/high-water, request,
  policy, activation, parameter and external-snapshot identity;
- `PreparedSketchSnapshot`, `PreparedSketchOperation`, `PreparedSketchOperationKind` and
  `PreparedSketchJob`;
- `PreparedSketchPatch` and `PreparedSketchCommit`;
- `RetainedSketchDocumentSession::{prepared_snapshot,commit_prepared_patch}`; and
- `RetainedSketchDocumentSession::update_parameter_batch_controlled`, completing cooperative
  control for every prepared operation family.

`crates/geosolve-sketch/tests/m56.rs` adds four direct concurrency/input-stamp regressions.
`docs/SCENARIOS.md` records the deterministic M56-C1 worker-ordering and cancellation schedule.

## 2. Mathematical behavior implemented

M56 adds no residual, Jacobian, rank policy, solver tolerance or geometric branch rule. Prepared
execution invokes the existing controlled retained-session paths on a captured clone, so every
completed candidate still uses the ordinary independent residual/domain/branch validation before
it can contain a newly accepted state.

The CAS base stamp is recomputed from the owning session and compares:

- retained design identity;
- latest attempt identity;
- accepted identity and accepted revision high-water;
- candidate/publication solve requests and accepted solver policy;
- effective activation revision and digest;
- parameter revision and digest; and
- external snapshot-set revision and digest.

Cancellation or work exhaustion produces no patch. Completed work remains inert until
`commit_prepared_patch`; any mismatch returns typed `StalePreparedPatch` and leaves the live session
unchanged. A matching patch replaces the complete coherent session in one owner-thread assignment.

The safe ownership contract is deliberately asymmetric. Session-bearing snapshots, jobs and
patches are movable `Send` values but are not promised `Sync`, because core numerical caches use
safe single-owner interior mutability. Immutable prepared stamps, operation DTOs and commit
metadata are `Send + Sync`. Native hosts may move a job to one worker and return its patch to the
session owner. Single-threaded WASM performs the same prepare/execute/commit calls synchronously.

## 3. Exact commands run and outcomes

Focused qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo test --locked -p geosolve-sketch --test m56'
nix-shell shell.nix --run 'cargo fmt --all && cargo clippy --locked -p geosolve-core -p geosolve-sketch --all-targets --all-features -- -D warnings && cargo test --locked -p geosolve-core --all-features && cargo test --locked -p geosolve-sketch --test m56'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
```

The M56 suite passes 4/4. Targeted warnings-denied Clippy passes; the core unit suite passes with
31 tests and one manual measurement ignored; the complete core/sketch package suites and
all-feature WASM check pass.

Final milestone qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

The complete formatting, warnings-denied Clippy, locked all-feature workspace tests,
all-feature WASM check and Trunk 0.21.14 release build pass. Cargo's pre-existing duplicate
`license`/`license-file` warnings remain non-fatal.

## 4. Acceptance criteria passed

- Prepared snapshots expose immutable design/accepted views and a complete exact-input stamp.
- Typed edit, reattempt, parameter and external-snapshot operations execute only on scratch state.
- A native `std::thread` worker returns a patch while the owner remains unchanged until CAS commit.
- Two completed out-of-order patches from one base cannot overwrite each other.
- Pre-cancelled parameter work returns no patch and advances no input or lifecycle revision.
- Non-default parameter/external revisions and same-design reattempts all invalidate older stamps.
- Session-bearing values satisfy the documented single-owner `Send` contract; immutable DTOs
  satisfy `Send + Sync`; the same API compiles for single-threaded WASM.
- No `unsafe`, FFI, solver equation, undocumented priority behavior or false-success path was
  introduced.

## 5. Known limitations and next blocker

GeoSolve does not own a worker pool, event loop, shared-session mutex or task queue. Hosts must
retain exclusive ownership of the live session and serialize CAS commits. Prepared work currently
covers retained typed edits, reattempts and complete parameter/external input replacement; generic
closure transactions and UI preview sessions remain owner-thread workflows.

M57 is the next blocker: persistent runtime mappings, dependency-closure rebuilds, indexed/history
storage, profile caches, workload envelopes, sparse-rank evaluation and fresh validation must
provide production-scale behavior while agreeing with full rebuilds on geometry, diagnostics,
branch state and acceptance.
