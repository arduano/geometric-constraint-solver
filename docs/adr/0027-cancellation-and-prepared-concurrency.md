# ADR 0027: Cancellation, work exhaustion and prepared concurrency

Status: accepted

## Context

Sketch solving, diagnostics and profile analysis can contain long bounded searches or
nonlinear and linear-algebra kernels. An interactive CAD host must be able to stop
obsolete work, place deterministic resource limits on untrusted input and run work on
host-managed native workers. The same public contract must also compile and remain
truthful for a single-threaded `wasm32-unknown-unknown` consumer.

The existing session lifecycle already builds candidates away from accepted state and
commits only after independent validation. It does not yet define cancellation,
work-exhaustion outcomes, immutable prepared work or stale-result publication. Adding
an internal async runtime or thread pool would impose scheduling and platform policy
on hosts, while allowing a worker to mutate a shared session would make rollback and
accepted-state identity timing-dependent.

## Decision

### Milestone ownership

M33 accepts this contract only. It adds no cancellation, job, scheduling or commit
capability and must not present any part of this ADR as implemented behavior.

M35 implements cooperative cancellation, deterministic work limits, operation
outcomes and checkpoints for synchronous sketch solve, diagnostic and profile paths.
M101X implements immutable accepted snapshots, prepared jobs, candidate patches and
complete-input-stamp compare-and-swap commit. No implementation claim is made before
the corresponding milestone gate passes.

### Synchronous single-writer sessions

A mutable sketch session has one logical writer. Every session mutation and commit is
a synchronous call requiring exclusive access. The library does not start threads,
install a global executor, expose an async runtime, dispatch a hidden task or own a
thread pool. Host applications decide whether a synchronous operation runs on their
UI thread, a native worker, a Web Worker or another host-owned scheduler.

Concurrent execution never means concurrent session mutation. Work executes from an
immutable snapshot and produces an immutable result or candidate patch. Only the
session owner may attempt the short synchronous commit step. Session caches and
accepted state are not shared mutable working storage for a prepared job.

### Cancellation and deterministic work controls

Cancellation is cooperative and monotonic for one operation. The public mechanism is
a library-owned read-only token paired with a cancellation handle, not an arbitrary
host callback invoked from residual evaluation. Once requested, a token cannot be
reset or reused to make cancelled work live again. A pre-cancelled token stops at the
first checkpoint without beginning expensive work.

Every potentially expensive operation also accepts typed deterministic work limits.
Limits count algorithmic work rather than elapsed time. Applicable counters include:

- document validation, dependency and lowering items;
- nonlinear iterations, rejected trials, component linearizations and bounded
  factorization calls;
- rank and diagnostic candidates or deletion trials;
- profile candidate pairs, subdivisions, root trials, fragments, integrations,
  containment tests and faces.

Each result reports configured limits, consumed work and the exact counter or typed
reason that stopped the operation. Counters are overflow-checked. A host deadline may
request cancellation through the same token, but wall-clock time is never an input to
convergence, rank, hard validity, topology completeness or deterministic work
exhaustion.

`Cancelled` and `WorkExhausted` are operation-control outcomes. They are distinct from
invalid input, invalid geometry, evaluation failure, numerical rejection, nonlinear
termination, hard validity and successful completion. Neither may be translated to
`Converged`, `HardValidity::Valid` or `Complete`. Finite attempted geometry,
diagnostics and consumed-work evidence may remain inspectable, but they are explicitly
non-authoritative.

A state-producing operation that exhausts work before complete independent validation
does not commit. A read-only bounded analysis may return its existing typed
`Truncated` or `Skipped` evidence and deterministic provisional payload, but no such
payload becomes an accepted state or a production-complete topology result.

### Checkpoints and cancellation latency

M35 places checkpoints at deterministic safe boundaries, including:

- before validation/lowering and between bounded document or dependency batches;
- before and after each dirty component, nonlinear iteration and trial batch;
- before and after each non-interruptible factorization or rank kernel;
- between diagnostic candidates and deletion/rank trials;
- between profile candidates, subdivision batches, integration batches and
  containment/face work;
- before independent final validation, after it, and immediately before direct
  session commit.

Residual and Jacobian evaluation remains behavior-pure and does not call host code.
The implementation need not poll inside a third-party factorization or another
non-interruptible kernel. Such kernels have documented input bounds, and M35 measures
the maximum observed cancellation latency at their boundaries. M102X may improve those
bounds but cannot weaken outcome or commit semantics.

If cancellation is observed at any checkpoint, all scratch work is discarded and the
operation returns `Cancelled`. The accepted document, accepted geometry, branch
state, input revisions, history and accepted audit remain bitwise unchanged. A final
checkpoint occurs after successful independent validation and before the commit
linearization point. The commit itself is a small non-interruptible single-writer
critical section: cancellation observed before it prevents commit; a request racing
after that linearization point cannot retroactively cancel an already committed
operation. Consequently, an operation that returns `Cancelled` has never committed.

### Immutable prepared jobs

M101X separates preparation, execution and commit:

1. the session owner captures an immutable accepted/input snapshot and prepares a
   job;
2. any host-selected worker synchronously executes that immutable job and returns an
   outcome, attempted diagnostics and, when independently valid, a candidate patch;
3. the session owner may synchronously offer that patch for compare-and-swap commit.

A prepared job owns or immutably shares every value needed by the operation. It does
not borrow mutable session state, read a live host parameter store, call a projection
service or consult a changing external-reference provider. Later host or session
changes do not alter the job already prepared.

Every snapshot, job, result and candidate patch repeats the complete input stamp from
ADR 0025:

- document identity and exact design revision;
- exact parameter-batch revision and canonical payload digest;
- exact external-snapshot-set revision and canonical payload digest;
- exact effective-activation revision and canonical payload digest; and
- exact solver-policy revision and canonical payload digest.

An absent input is a stamped value, not an omitted comparison. Digests cover the
canonical typed content rather than host object addresses or runtime IDs. A job may
not drop a stamp member merely because the current document has no dependency of that
kind.

Prepared work additionally records the exact parent accepted-state identity, a
session-local incarnation that changes when runtime mappings are replaced, and
immutable operation-request evidence. The request evidence identifies the job kind,
typed request and scope, analysis policy and deterministic work limits. Those
operation-only controls are not solver-policy input and cannot weaken the acceptance
policy in the complete input stamp.

### Complete-stamp compare-and-swap

Executing a prepared job never mutates a session and never commits automatically. A
candidate patch records its complete input stamp and successful independent-validation
evidence. Commit succeeds only when:

- the candidate is finite and independently valid for its stamped operation;
- cancellation has not been observed at the final pre-commit checkpoint;
- every field of the current session's complete input stamp equals the candidate's
  base stamp;
- the current parent accepted-state identity equals the candidate's parent; and
- the session incarnation still matches the prepared runtime mappings.

Any mismatch returns a typed stale outcome and changes no session field. There is no
field subset comparison, last-writer-wins publication, implicit rebase, patch merge or
special case for numerically equal geometry. The host must prepare new work from the
new snapshot. If two candidates share one base accepted revision, the first successful
commit advances that revision and the second is stale even when both solved identical
input bytes.

Cancelled, exhausted and stale results may be logged or displayed as attempted
evidence with their original stamps. They cannot overwrite accepted state, be relabeled
as current or supply a current production profile. A profile or diagnostic result is
current only while its complete input stamp, accepted-state identity and operation
request evidence still match the host's selected query.

### Native and WASM ownership contract

At M101X, immutable accepted snapshots, prepared jobs, cancellation handles/tokens and
job results are `Send + Sync` on native targets when published as worker-shareable
types; compile-time assertions protect that surface. Mutable sessions have no `Sync`
mutation contract and remain owned by the single writer. This uses safe Rust only and
does not require callers to share references through `unsafe` code.

The same synchronous APIs and outcomes compile for
`wasm32-unknown-unknown`; no thread is required. A single-threaded browser event loop
cannot deliver a new cancellation request while blocked inside one synchronous call,
so such a consumer relies on pre-cancellation and deterministic work limits, or moves
the operation to a host-managed Web Worker. The library does not claim event-loop
preemption or hidden yielding. This platform liveness difference does not change
status, validation, stale-result or no-commit semantics.

For identical immutable input and policy, completed uncancelled execution is
deterministic regardless of the host worker that ran it. Scheduling order and
cancellation timing may change which non-authoritative checkpoint stops work, but
cannot change accepted publication: only one exact complete-stamp CAS can commit.

## Consequences

- Hosts retain control over threads, async integration, deadlines and worker lifetime.
- Cancellation and finite work bounds do not become solver equations or convergence
  heuristics.
- Prepared work can be inspected after cancellation or staleness without risking a
  late write into newer accepted state.
- Complete-stamp CAS plus exact parent/session guards is intentionally stricter than
  merging compatible edits; automatic rebasing remains host policy.
- Single-threaded WASM remains supported without pretending that synchronous code can
  be interrupted by the same blocked event loop.
- M35 must instrument all documented expensive paths before claiming cancellation;
  M101X must prove ownership markers and stale-commit behavior before claiming prepared
  concurrency.
