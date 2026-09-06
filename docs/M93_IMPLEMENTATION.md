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
