<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M93 goals: fast, proportional release qualification

Status: **next milestone; scope approved by the supervising caller on 2026-09-06;
implementation and qualification not started.** M92 is closed with its accepted 16-sample catalog.
Further sample expansion is future work and does not block this milestone.

## Outcome and measured problem

Make a small change cheap to qualify, make a comprehensive run substantially faster, and retain
successful evidence when an unrelated stage fails. Optimize execution and qualification policy
together, while preserving the existing mathematical and release contracts.

The M92 pruning run is the motivating workload. Its first gate spent 72 min 52 sec before a late
TypeScript generator assertion rejected the new 16-entry inventory. After that single assertion
was corrected, the replacement gate reran everything for 152 min 24 sec. The successful run
included 2,603 native/documentation tests, the unchanged 271-case golden, 37 WASM interaction tests,
three optimized-WASM lifecycle tests, all sample history/design-intent checks, packaging,
performance, 20 workbench browser rows, 16 sample workflows and 97 frontend tests. A separate
survey/check pair ran alongside the first attempt. Exact receipts and source identities remain in
[M92's qualification ledger](M92_VISUAL_AUDIT.md#pruned-catalog-qualification-and-immutable-nomination).
These are historical elapsed times on the qualification host, not a standardized cold-cache benchmark.

The current `scripts/release-gate.sh` serializes most stages and puts cheap package/frontend
validation after expensive native and oracle work. The oracle repeatedly starts Cargo and managed
compiler subprocesses per case. Three separate Playwright invocations rebuild their fixture server,
and the final frontend check builds again. M93 must measure the actual costs and resource conflicts
before choosing concurrency; merely adding background shell jobs is insufficient.

## Qualification policy to implement

A reviewed, machine-readable inventory must identify every stage/case, its owning source and
transitive inputs, prerequisites, resource needs and evidence requirements. Produce a readable
execution plan that says **run**, **reuse**, or **not applicable**, with a reason for each decision.
The same inventory must drive execution and coverage reconciliation; do not maintain independent
lists that can silently lose cases.

| Change | Required fresh work | Evidence eligible for reuse |
|---|---|---|
| Prose-only documentation or acceptance sign-off | Diff, links/status/receipt consistency, confirmation that no build/test input changed | Qualified product, solver and browser evidence; no rebuild or server replacement just for prose |
| Remove a catalog entry | Registry/order/count/source-generation and packaging checks; retired lookup/menu/Recent checks; new artifact and affected integration checks | Unchanged survivor intent/history/trajectory cases and unrelated solver golden cases, only with verified input equivalence |
| Edit one sample | Changed sample's compile/materialization, independent intent/residual/mobility, two-edit/history and relevant drag/browser checks; shared registry/build checks | Other samples whose complete inputs are unchanged |
| Frontend presentation | Frontend/static/build checks and affected real-browser workflows; exact artifact/transport verification for a new nominee | Rust mathematical evidence when no adapter/domain/build input changes |
| Domain/solver/bridge/serialization/compiler behavior | Owning regression and all impacted downstream native, golden, WASM, sample and browser coverage | Only evidence outside a demonstrated dependency closure; broad shared changes normally require comprehensive qualification |
| Fix a test, generator or runner | Corrected stage plus stages consuming changed outputs; validate inventory/selection semantics | Successful independent stages from the failed attempt, when inputs and qualification policy remain equivalent |
| Toolchain, lockfile, feature/build configuration, golden inventory, policy or unknown input | Conservatively expand to the comprehensive gate; validate the planner itself for policy changes | No reuse based on an unreviewed assumption or changed qualification semantics |

A file extension or a passing Git ancestor is not an equivalence proof. Markdown can be included
in builds or consumed by tests. Input closure includes source, tests, fixtures, generated assets,
build scripts and configuration, manifests/lockfiles, golden bytes and exclusions, target/features,
relevant environment, compiler/SDK/browser versions, runner and policy versions. A missing mapping,
missing input, ambiguous change, incomplete receipt or unverifiable result forces fresh work.

Per-stage receipts must authenticate those inputs, exact selected inventory, command, environment,
exit/completion state and output/log hashes. Reused evidence stays attributed to its original run
and is explicitly linked into the new candidate's qualification manifest. Never present reused
work as freshly executed. Failed, timed-out, interrupted, flaky or incomplete results cannot become
passing evidence. A failed overall attempt may contribute independently completed successful
stages, but remains a failed attempt. A later fix must rerun every stage affected by that fix.
Treat result reuse separately from ordinary build caches: cached compilation never proves a test ran.

Define the trusted receipt store and its threat boundary. Validate schema and content integrity;
exclude arbitrary external/untrusted receipts from release decisions. Do not silently import M92
logs into an input-complete cache: either prove the required identities or establish a new baseline.

## Execution changes

- Run cheap inventory, generation, metadata, static and contract checks before expensive tests.
  In particular, the M92 stale sample count must fail during preflight.
- Build each required native/WASM profile and each managed compiler/declaration set once per
  relevant input identity. Invoke already-built test executables when safe, preserving Cargo's
  test environment and exact inventory. Avoid repeated package installation and compiler startup.
- Run independent suites and oracle cases with bounded CPU/memory concurrency and deterministic
  result aggregation. Isolate scratch paths, generated outputs, browser profiles, ports, fixture
  state and writable package trees. Serialize shared builds/installs and measured performance runs
  where resource interference would invalidate results. Timeouts must distinguish queued time
  from execution time and preserve process-tree cancellation and failure attribution.
- Evaluate the golden once per input identity. Derive comparison and require-clean dispositions
  from that exact captured result; retain explicit survey/discovery and reviewed-update workflows.
  Do not rerun the entire corpus merely to obtain three differently named receipts.
- Reuse built production assets across browser suites. Keep compiler-parity harness assets separate
  from production output. Qualify the exact immutable nominee without an unnoticed final rebuild.
  After a byte-identical server move, run fresh transport checks and a bounded readiness smoke;
  repeat full semantic browser suites only when their actual execution inputs changed.
- Support resume after interruption/failure, named targeted work, an explained candidate plan and
  a comprehensive **fresh** run that bypasses result reuse. The default must not unexpectedly turn
  a documentation edit into hours of work. Report duration, critical path, work avoided, reused
  evidence and remaining work without flooding the user with unchanged polling messages.

The planned policy is not permission to skip checks today. Implement it with its invalidation
regressions, then update `AGENTS.md`, the defect-hardening skill/reference, release documentation
and any affected CI instructions together so agents and automation use one authoritative policy.
Pages is already publication-only; do not reintroduce a second complete integration gate there.

## Acceptance and speed targets

Use the same named host/toolchains and fixed input corpus, record CPU/RAM/concurrency and explicit
cache state, and report both end-to-end wall time and CPU/resource cost. Separate cold dependency/
build caches, warm build caches with fresh results, and valid result reuse. Report failures and
retries, not just the fastest successful run.

1. **M93-C1 — inventory and timing:** every current release obligation maps to the new stage/case
   inventory; reports distinguish prerequisite, build, execution and wait time. Preserve the exact
   271-case golden, reviewed exclusions, sample witnesses and release/browser inventories.
2. **M93-C2 — fail-fast:** replay M92's stale-count failure and a representative generated-manifest
   drift. Both must be detected before expensive native/oracle/sample tests; warm preflight target
   is **under 2 minutes**.
3. **M93-C3 — sound selection and reuse:** fixture diffs covering every policy row prove required
   invalidation. Include transitive dependency changes, fixture/golden edits, toolchain/flag drift,
   unknown files, removed/renamed tests, corrupt/stale/missing receipts and a failed predecessor.
   No required case is omitted or counted twice, and preserved evidence identifies its original run.
4. **M93-C4 — deterministic concurrency and recovery:** serial and parallel fresh runs agree on
   inventory, classifications, semantic results and golden bytes. Induced failures, timeouts and
   interruption retain complete attributable evidence; safe resume runs only missing/invalid work.
5. **M93-C5 — proportional latency:** on warm prepared tools, documentation/sign-off is **under
   1 minute**; catalog pruning and a representative single-sample change are **under 10 minutes**
   with valid unaffected evidence. A count-assertion-only repair must not rerun completed unrelated
   native/golden/WASM stages. Report qualification latency separately from deployment latency.
6. **M93-C6 — comprehensive latency:** target **45 minutes or less** for a warm-build-cache,
   comprehensive fresh-results gate on the recorded host, versus M92's roughly 152 minutes. Measure
   a cold run too. Use repeated representative runs (at least three for fast paths and two full
   fresh runs, one of them serial/reference as needed for parity); show each result and cache state.
   Missing a target remains an explicit blocker/tradeoff for caller review, not a reason to weaken
   checks or quietly redefine the workload.
7. **M93-C7 — immutable release:** any new nominee receives fresh artifact inventory/size/hash,
   HTTP byte/MIME/base-path and targeted runtime verification. Moving identical bytes does not
   rerun every domain test. A changed artifact cannot borrow an old artifact's transport receipt.
   One qualification manifest proves complete coverage via executed and authenticated reused work.

No residual equation, finite validation, Hard tolerance, branch choice, rank/DOF contract,
transaction/history invariant, test assertion, golden disposition or bundle ceiling may be weakened
for speed. Profile expensive fixtures, batching and test execution profiles before changing them;
any behavioral solver defect follows the owning-layer defect-hardening workflow. New samples and
solver-feature work remain outside M93. Closure requires measured evidence and supervising-user
acceptance of the optimized workflow; this plan makes no speedup or implementation claim.
