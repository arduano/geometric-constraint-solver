<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M93 implementation plan: release qualification

Status: **in progress; runner, native/golden adapters, browser preparation and input-reuse policy implemented; integrated qualification and measured acceptance outstanding.**
[M93_GOALS.md](M93_GOALS.md) owns scope, the proposed reuse matrix and measurable acceptance targets.

## Implemented checkpoint — 2026-09-06

`scripts/release_gate.py` and `release_policy.json` own the stage inventory, dependency planning,
private HMAC-authenticated receipts, per-stage timing/resource logs, safe resume, source checks,
isolated scratch and bounded worker admission. `release_gate_native.py` compiles/discovers Cargo
executables and runtime environments once, then validates exact completed libtest inventories.
The optimized test profile uses level 1 with debug assertions and overflow checks explicitly on.
The original sequential gate and golden driver remain available as references.

`golden_oracle.py` preserves the 271 cases and existing 30/60-second limits while running direct
executables and Deno, with deterministic reduction and retained observations. Browser preparation
builds optimized WASM once and separate harness/production assets; all 37 browser rows share the
prepared harness server. Exact production transport and bounded actual-WASM readiness are separate.
[RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md) documents the runnable interfaces and policy.

Focused checks passed: 20 scheduler/policy regressions, 16 native-adapter regressions, 13 golden
runner regressions, 11 artifact/transport tests, existing build contracts and 97 frontend unit tests.
Browser discovery preserves 20 workbench, 16 sample and one language row. An integrated preflight
passed inventory, metadata/format, managed package generation/tests and frontend checks in roughly
55 seconds. Fresh transport/readiness against unchanged M92 bytes passed in about five seconds.
These are development results, not full M93 qualification. Native profile preparation, integrated
fresh serial/parallel parity, representative reuse and latency measurements remain in progress.

Current reuse is conservative at owning crate/test-binary and artifact boundaries; per-sample
independence is not assumed when the compiled catalog changes. Measure this path and add a reviewed
smaller dependency boundary only if the small-change target requires it. No target is waived or
claimed achieved by runner availability.

The first integrated attempt, `20260906T105515-90e33de0`, retained successful preflight and
preparation receipts but failed qualification: the default-feature build replaced the workspace
CLI at Cargo's shared binary path, and Chromium could not bind its socket under the long evidence
path. Both are harness failures, with no observed solver defect. Profile-specific captured CLI
dependencies and private short runtime paths repair these failures. A real Chromium launch smoke
and actual prepared CLI override probe pass. An input audit also added embedded Markdown/fixture
dependencies and separated native, optimized-WASM and browser preparation identities. Integrated
qualification of this repaired runner and the timing targets remain pending.

## Catalog reuse checkpoint — 2026-09-06

The first repaired full attempt passed 219 stages, including all ordinary/ignored native sample
work and 37 browser rows, before a strict-Clippy style error in the CLI test helper stopped it.
That run took 1,339.393 seconds (22m19s); browser execution was 672.4 seconds. The focused correction
and targeted continuation retain the earlier successes. The remaining golden (271 unchanged rows),
37 WASM interaction tests, three optimized-WASM lifecycle tests, documentation, packaging,
licences, benchmarks, performance and exact artifact transport have now passed. These accumulated
targeted results are not yet a complete fresh result on the final M93 implementation.

Browser cost required a smaller reviewed input boundary for the under-ten-minute sample target.
The independent catalog contract replaces scattered count/order constants; full browser workflows
have stable sample keys. Exact open-state witnesses gate authenticated reuse of unchanged workflows,
with fresh shared catalog and non-sample checks. The program boundary is explicitly pinned and
fails closed after unreviewed code/input changes. Signed invocation/build provenance and an
independent 22-case non-sample inventory protect each browser aggregate. All 16 real prefixes pass
twice with identical complete witnesses (one successful batch 53.4 seconds); a full Whitworth
control passes in 13.8 seconds with the same witness. Two catalog harness failures were repaired
and retained; the corrected catalog check passes in 11.1 seconds.

The dedicated Bondtech regressions preserve the original fixture outside public catalog discovery,
so an explicit retirement can retain every geometry/history assertion. This supports a real prune
benchmark without silently skipping missing samples. Current product remains the accepted16-entry
catalog. Performance inputs follow their actual sketch/editor/linkage owners. The two-worker gate
can admit two memory-marked native/browser stages; exclusive builds and performance measurements
remain serialized. Final serial/parallel and edit/prune timing acceptance remains outstanding.

## Measured build and cache correction — 2026-09-06

Clean `0e5d7a6` passed all 239 stages in **2,432.441 seconds (40m32s)**, with every stage
executed and no browser workflow reused. This is the first complete M93 fresh-results baseline,
using warm existing compiler caches. It passes the 45-minute target once; cold/reference parity
and repetitions remain outstanding. Its exact source, signed receipt and timing summary are in
`target/m93/baseline-fresh-confirmation.json`.

The first real single-sample candidate changed only the manifold's upper-outlet source radius and
compiled envelope. It exceeded ten minutes during preparation: workspace 361.3s (including 170s
compilation), headless 29.3s, WASM 149.7s and browser preparation 72.2s. Full browser execution then
took 820.4s because one generated Vitest timing-cache file changed the installed-dependency digest.
Authenticated before/after contexts prove that program inputs, policy, tools and environment were
otherwise equal: `target/m93/browser-leaf-cache-invalidation.json`. This is a missed C5 target,
not a passing proportional-qualification result.

The follow-up retains line-number debug information, enables incremental release/bench compilation
with the original codegen-unit count, and raises bounded Cargo compilation to four jobs. It stops
writing Vitest timing-cache output into installed dependencies. Optimized-WASM lifecycle compilation
and execution are separated so the unchanged three cases can overlap other independent tests.
Fresh runtime qualification and new complete edit/prune measurements are required before claiming
these corrections meet the target. All earlier failures and measurements remain evidence.

## Pipeline correction and measured limits — 2026-09-06

Source `c3f86d0` passed all **240/240** stages in **4,550.336 seconds (75m50s)**,
with every stage freshly run. This was the first build of the changed test/release profiles:
Nix/npm/download caches were warm, while the new profile artifacts were cold. It is not a
warm-build C6 pass. The independent evidence audit authenticated 9,856 files, 2,629 native selected
executions (including 25 explicit ignored cases), all browser/sample coverage, 37 ordinary WASM
cases, three release-WASM lifecycle cases and the unchanged 271-case golden. Native binaries
occupy 14.47 GB, down from 50.56 GB with full debug information. Production bytes changed and
received fresh transport/browser qualification; the accepted M92 service remains untouched.

The real manifold radius candidate `08f2e7e`, changing 3 to 3.1 with its compiled envelope,
qualified in **900.420 seconds (15m00s)**, or **904.101 seconds including generation**. Exactly
15 unchanged browser workflows reused authenticated baseline evidence; the manifold and shared
checks ran fresh. This improves the earlier 38m29s result but **fails the ten-minute C5 target**.
The candidate and receipt indexes were restored. Preparation consumed 392.5 seconds before any
semantic suite could start, motivating overlap of prepared execution with later compilation.

At the same source tree, three documentation runs took **1.771 / 1.729 / 1.716 seconds**.
Stale-count failures took **6.318 / 5.622 / 5.197 seconds**, and generated-order failures
**5.297 / 5.052 / 5.174 seconds**. Every failure stopped at inventory preflight before compilers
or semantic tests. Evidence is retained under `target/m93/compile-optimized-*` and
`target/m93/benchmark-numeric-1-c3f86d0`.

The next runner revision removes the all-preparations barrier. Each exact deferred test group
expands after its own preparation; missing, failed or interrupted groups cannot disappear from
qualification. Private executable copies and immutable Nix runtime closure checks protect native
execution from Cargo replacement. A separate reviewed program digest disables native/build
overlap after an unaudited program change. One Cargo writer, three total stages and at most two
memory-marked stages bound admission; performance remains exclusive. Plan and execution share
the same contracts. The unchanged 204 existing native binaries passed a read-only loader audit;
this is runtime eligibility evidence, not a test execution or a pipeline timing result.

Focused Python qualification passes **133 gate/policy/native/runtime tests** and **13 golden
runner tests**; the independent comparison helper passes **21 fixtures**. The 19 scheduler tests
include real child overlap, build/performance exclusion, interruption, deferred discovery,
resume, targeted selection, read-only plan behavior and native cache/evidence isolation. Main and
auxiliary executable replacement and loader mutation cannot produce passing native receipts.
The two property suites that replay and persist tracked regression seeds retain the build lock.
Integrated qualification, final edit/prune repetitions and serial/cold parity remain outstanding.
No timing target is waived or inferred from the design.

## Warm pipeline measurement and planning correction — 2026-09-06

Clean `7de0a25` passed **240/240 stages**, all freshly executed, in **1,392.971 seconds**
(23m13s gate report; **1,395.136 seconds / 23m15s end to end**). Existing compilation caches
were warm. The independent audit authenticated 9,872 evidence files, 2,629 native selected
executions, all 16 fresh sample workflows, 37 ordinary WASM cases, three release-WASM lifecycle
cases and the unchanged 271-case golden. Evidence lives in `target/m93/pipeline-qualification`.
The three-worker pipeline meets the warm comprehensive target at this checkpoint.

The real numeric candidate `c591c16` qualified in **637.353 seconds**, or **640.937 seconds
(10m41s) including generation**. Exactly 15 unchanged browser workflows and 172 independent
stages reused authenticated baseline evidence. This **misses C5 by 41 seconds**; qualification
success does not waive the latency target. Source and signed indexes were restored. Three
documentation runs took **1.363 / 1.334 / 1.354 seconds**; stale-count failures took
**4.542 / 4.573 / 4.692 seconds**, and generated-order failures **4.836 / 4.688 / 4.684 seconds**.
Every induced failure stopped during inventory preflight before compilation or semantic tests.

The follow-up caches repeated source-boundary scans within one frozen runner and computes each
owning crate's dependency/include closure once per native inventory expansion. Artifact hashes
and runtime validation remain required. It separates reusable frontend licenses, SDK checks and
unit tests from sample-sensitive manifest checks and browser-test discovery. Catalog metadata
remains an input to both stages. The narrower static boundary requires an explicit program audit;
missing or changed review evidence restores the full input set. Qualification and new measurements
of this follow-up remain pending, along with final serial/cold parity and user acceptance.

Independent review found no input-cache lifecycle defect and approved the current static consumer
boundary. **146 focused release tests pass in 28.598 seconds**, including six cache-isolation
fixtures and seven frontend command/input fixtures. The comparator's 21 fixtures and benchmark
helper's nine exact-coverage fixtures also pass. These validate runner behavior, not the pending
latency measurements.

## Implementation order

1. **Instrument and inventory.** Decompose `scripts/release-gate.sh` into one reviewed obligation
   inventory without deleting coverage. Add structured per-stage timing/results and input capture.
   Use M92's retained logs to locate costs; run a controlled baseline only where those logs cannot
   answer the comparison. Record actual host/cache conditions, test discovery, dependency/build
   locks, compiler startup and high-memory cases. Keep the existing sequential runner as the
   reference until parity is demonstrated.
2. **Fail early and prepare once.** Move cheap package/sample counts, generator drift, metadata,
   static and build-contract checks ahead of slow suites. Install/build shared dependencies once.
   Separate production build, compiler-harness build, browser-server lifecycle and test execution;
   freeze and qualify the same production bytes. Measure this change before increasing concurrency.
3. **Bound parallel execution.** Build a dependency graph and resource-aware worker pool. Split
   oracle and independent sample cases with isolated writable state and ordered result reduction.
   Start measurements with two expensive execution slots and at most one memory-heavy workload;
   account for each test binary's internal workers before increasing concurrency.
   Remove repeated Cargo/compiler startup where profiles show it dominates. Avoid concurrent
   `npm ci`/generation against the same package tree, Cargo lock contention, port/output collisions,
   and performance measurements under competing load. Preserve bounded child-process cleanup.
4. **Authenticate reuse and resume.** Version the input/receipt format and conservative change
   planner. Separate build-cache hits, reusable test results and fresh execution. Add explanatory
   plan output and a comprehensive fresh mode. Develop selection/invalidation tests before turning
   reuse on for release decisions; unknown or unprovable input closures run fresh. Corrected harness
   failures reuse only unaffected successful stage receipts, with failed original attempts retained.
5. **Reconcile policy and qualify.** Replace blanket rerun instructions in `AGENTS.md`, the
   defect-hardening reference and release/nomination documentation with the tested proportional
   policy. Preserve the full fresh escape hatch and independent correctness requirements. Replay
   documentation-only, four-sample pruning, one-sample edit, count-assertion repair, shared-core
   change, compiler/lockfile drift, corruption and interruption workloads. Compare serial/fresh,
   parallel/fresh and selective/reused results, then record latency and resource receipts against
   C1–C7 before requesting milestone acceptance.

Each step is independently reviewable. Do not run another multi-hour gate after every planning
or instrumentation edit; use focused runner/selection tests while developing, and perform the
comprehensive parity and timing runs at the integrated acceptance boundary. The final implemented
policy must make this distinction explicit rather than relying on agent discretion alone.

## Parallel ownership opportunities

- Stage inventory/timing and cheap preflight can proceed alongside dependency/reuse-policy design.
- Native/oracle batching and frontend build/server consolidation have separate owners after the
  shared stage/receipt contracts are agreed.
- Independent review attacks invalidation, failure recovery and resource isolation while the
  integrator implements selection and scheduling.
- One integrator owns the shared inventory, qualification status aggregation and policy changes.

## Required evidence

Keep raw stage receipts, exact inventories, source/input/toolchain identities, logs, per-case
classification comparisons and artifact manifests. Benchmark the complete same workload; do not
claim a full-gate speedup from a reused-results run. M92 receipts remain historical comparisons,
not retroactively rewritten optimized results. A release qualification must list every obligation
as executed, authenticated reuse or explicitly inapplicable, with an auditable reason.

The motivating baseline is clean `b153a28d9e44dde934b325835913986b0e316ee7`, tree
`f106276ebecdec1c9d2fc4c7c9937f32d2549e42`, gate interval
`2026-09-05T11:46:19.121966+00:00` to `2026-09-05T14:18:43.460675+00:00`.
Its log SHA-256 is `1c4dec05cb07d66a2b829c4efb163c3bffc382e0393efc5e74c8f237007882ba`.
The failed predecessor, successful stage results, compiler-count correction, visual captures and
immutable nomination receipts are all linked from [M92's audit](M92_VISUAL_AUDIT.md).

No existing CLI flags, tests or APIs are claimed for the planned scheduler, selection or reuse
features. Choose their smallest practical interfaces during implementation and document exact
commands only after they exist and have run.
