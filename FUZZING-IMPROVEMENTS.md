# FUZZING-IMPROVEMENTS.md — active implementation plan

**Status:** ACTIVE plan (written 2026-09-22). Supersedes the read-only review framing that
originally lived here; the review findings are preserved verbatim at the bottom as a resolution
history so nothing is lost.

**Progress (2026-09-22):** Phase 0 DONE. Phase 1 DONE (generator, repurposed target 03, shared
`validate_solve`; builds clean, clippy clean, determinism passes, 600 corpus seeds smoke-tested
with 0 panics). Phases 2-3 pending.

**Companion contract:** `FUZZING.md` (program contract, architecture, run loop, corpus layout).
This file records the concrete improvements to make *before* offloading a long campaign to a
dedicated machine, and how each is validated locally first.

**Toolchain:** `cargo-fuzz` is installed (`/run/current-system/sw/bin/cargo-fuzz`), so the deep
mutation gate below runs on this machine. Release profile + instrumentation come from
`fuzz/fuzz.toml`.

---

## Objectives

1. Prove the harness is **false-positive-free under mutation** (the one thing never measured).
2. Give target 03 the **arbitrary-document coverage** `FUZZING.md §7` documents but never implemented.
3. Validate the **oracle** (`SurveyOutcome::check()`) on its rejection paths before trusting it unattended.
4. Leave a **local qualification gate** that must pass before the dedicated-machine handoff.

Confidence today: crash-safety + accept/reject contract **HIGH**; coverage breadth **MODERATE**
(the improvements below target exactly the MODERATE axis).

---

## Resolution history — the six original review items

| # | Original review item | Resolution |
|---|---|---|
| 1 | Perturb the solver configuration | **DONE.** `harness::perturbed_solver_config` derives an idempotent `SolverConfig` from the input; threaded through all three `run_*` paths. Golden witness runs keep the default config. |
| 2 | Calibrate the residual bound | **DONE.** `run_solver`/`validate_accepted` use the solver's own `normalized_residual_tolerance` (adaptive), not a hardcoded `1e-9`. |
| 3 | Harness panics vs. solver defects | **DEFERRED (acceptable).** `MatrixFixture::new` + `coordinator()` still `expect`; fixture geometry is constant and transforms are similarities, so build panics are unreachable in practice. Revisit only if unattended runs are very long. |
| 4 | Target 02 narrow / under-specified | **PARTIALLY.** `branch_flip` now compares base vs `displaced` solve by **nearest world position** (not shared id) and flags a shared point that moved beyond a `4*scale` budget — a genuine branch-flip signal that tolerates the `displaced` point-set reindexing (see `docs/FUZZING-02_BRANCHFLIP_AUDIT.md`). The full `FUZZING.md §4` consistency check is intentionally scoped to the shared-point displacement. |
| 5 | Add a determinism pass | **DONE.** `fuzz/tests/determinism.rs` re-solves each corpus entry N times and asserts identical diagnostics. |
| 6 | Differential / oracle cross-check | **PENDING (nice-to-have).** Targets 01 (authoring coordinator) and 03 (bare session) already solve the same inputs via independent paths. Not in scope for this gate; track as a future enhancement if crash yield justifies it. |

---

## Plan

### Phase 0 — Oracle negative tests (foundational, test-only, no risk) [DONE]
The fuzzer trusts `SurveyOutcome::check()` blindly; nothing currently exercises its rejection
paths. Added a `#[cfg(test)]` module in `crates/geosolve-survey/src/survey.rs` that constructs
`SurveyOutcome` (public struct, all fields public) with **each gate set to its failing value**
and asserts `check()` returns `Err`, plus the complementary `Ok` cases:
- `geometry_finite = false`; `max_coordinate > 1e300`; `hard_validity != Some(Valid)`;
  `hard_residuals_validated = false`; `max_residual = NaN / None / > 1e-9`; `accepted_current = false`;
  `error = Some(..)`.
- Complementary `Ok`: accepted + all-good; and **rejected-but-bad is `Ok`** (the "rejected is a
  valid outcome" rule, `survey.rs:82`).

Verify: `cargo test -p geosolve-survey` — 13 oracle tests pass.

### Phase 1 — Target 03 random-document generator (biggest coverage gain) [DONE]
`FUZZING.md §7` documents target 03 as "random-document generator → retained-session solve," but
the implementation decoded the 51-byte `FuzzInput` and re-ran the matrix fixture via
`run_core_solver` — same inputs as 01/02, so **zero** document-space coverage. No
`SketchDocument::random` or reusable generator existed in the workspace.

Implemented:
- `fuzz/src/random_doc.rs` — an **internal** generator (no generic traits, per AGENTS.md).
  `FuzzDocument::decode` reads a variable-length record into a fixed 512-byte buffer (left-padded,
  so short inputs read as zeros — **total**, never panics). `FuzzDocument::build` synthesizes an
  arbitrary `SketchDocument`: `with_id(scale, fixed_id)` + finite `add_point`s + positive-domain
  `add_scalar` radii + `add_curve` (Line/Circle/QuadraticBezier) referencing already-added ids,
  gated on finiteness + magnitude caps (`GEOMETRY_MAGNITUDE_CAP = 1e6`, `MIN_SCALE = 1e-3`, from
  `FuzzInput::normalize`). Curve references are clamped into range; unsatisfiable curves and
  empty radius sets are skipped, so a malformed document is a bad input, never a crash. `decode`
  is **total**; `build` is **defensive**.
- `harness::run_random_core_solver(data)` decodes → builds → bare
  `RetainedSketchDocumentSession::new` solve → validates with the shared `validate_solve(config,
  solve, label)` (label `"core-solver"`). Malformed documents are skipped, not crashes.
- `harness::validate_solve` is the single accept-or-reject contract: a success-like solve must be
  finite, hard-valid, independently residual-validated, and below the solver's own
  `normalized_residual_tolerance`; any violation is a defect. `run_core_solver` and
  `validate_accepted` now delegate to it (exact prior message formats preserved).
- **Repurposed** `fuzz/fuzz_targets/03_core_solver.rs` to call `run_random_core_solver`.
  Arbitrary documents strictly subsume the matrix fixture, so no coverage is lost and
  `FUZZING.md §7` is finally met.

Verify:
- `cargo build` (all four targets): Finished, no errors.
- `cargo clippy --lib`: 0 warnings in `fuzz` (the only remaining warnings are pre-existing
  manifest-license notes and `geosolve-sketch`'s deprecated `fetch_update`).
- `cargo test --test determinism`: `core_solve_is_deterministic` ok (15.9 s).
- Smoke run of target 03's binary against 600 corpus seeds: 0 panics.

### Phase 2 — Branch-flip budget calibration (de-risks target 02 false positives) [DONE]
The `4*scale` / "8× margin" threshold was an unverified judgment. `fuzz/tests/branch_calib.rs`
now *measures* it: for every family × 10 scales (MIN_SCALE=1e-3 … cap=1e6) × 24 perturbation
options it solves the base (`displaced=false`) and `displaced=true` pair under the same config
(matching target 02's single-config setup), then records the max nearest-position displacement of
a shared point, normalized by scale (scale-invariant because the fixture scales uniformly; common-mode
translation/rotation cancel in the difference).

Result (6960 accepted base+`displaced` pairs, 0 failures):
- **max displacement/scale = 0.4000** (EqualCurvature, scale=0.01, option=0; displacement 0.004).
- **0** displacements exceed the current `4*scale` budget.

Interpretation: the budget sits ~10× above the worst observed stable displacement (not merely the
assumed 8× over the ~0.5·scale perturbation). Translation/rotation are common-mode and cancel in the
base-minus-perturbed difference; flags (reverse_spans/swap_operands) affect solver span handling, not
the geometric displacement, so the sweep covers target 02's full observable space. The current
`PERTURBATION_BUDGET = 4.0` is therefore empirically safe; it is retained, and the calibration test
acts as a permanent regression lock (`assert!(4.0 >= 2.0 * max_ratio)` prints the observed max ratio).

### Phase 3 — Local qualification gate (deep on all 4) [DONE]
Gate bar: **0 crashes**, or every crash resolved via `$geosolve-harden-defect`, and no unexplained
false positives.**

Sequence:
1. Phase 0 `cargo test -p geosolve-survey` passes.
2. Phase 1 builds; short smoke run on target 03 confirms no false positives on novel inputs.
3. Phase 2 calibration passes.
4. Deep mutation on **all four** targets: `cargo fuzz run <target> --runs <N> --timeout 30` for a
   bounded per-target budget. libFuzzer accumulates coverage across invocations from a persistent
   corpus dir, so a target exceeding the shell timeout runs in sequential windows and the tally is
   cumulative. Record per target: crash count, false-positive count, run rate.
5. Any real crash → `$geosolve-harden-defect` (reproduce → classify → fix → regression scenario,
   per AGENTS.md). Suspected false positive → the audit workflow in
   `docs/FUZZING-02_BRANCHFLIP_AUDIT.md`.
6. Gate passes only if the bar holds for all four targets.

**RESULTS (2026-09-22) — GATE PASSES (clean on all four targets):**

| Target | Budget | Wall-clock | Rate | Crashes | FAIL/CRASH/panic | Coverage | Notes |
|---|---|---|---|---|---|---|---|
| 01 authoring_survey | 25000 | 1296s (~21.6min) | ~21.6 exec/s | 0 | 0 | cov 18710, corp 411 | `survey()` does full constraint resolution → slow; steady cov growth, no crashes |
| 02 fixture_perturbation | 40000 | 1465s (~24min) | ~27 exec/s | 0 | 0 | cov 6758 | base+displaced solves + validation + branch comparison per run (~2× a bare solve) |
| 03 core_solver | 100000 | 372s (~6.2min) | ~268 exec/s | 0 | 0 | cov 6851 | reduced-budget smoke run (target 03 already covered by Phase 1) |
| 04 fillet | 30000 | 572s (~9.5min) | ~53 exec/s | 0 | 0 | cov 7714 | 30000 expected `[fillet] ... no contact candidate (WrongOperandKind)` warnings — tool correctly rejecting non-corner selections, one per run, non-fatal |

Run recipe (detached; survives the 120s tool timeout via `setsid`):
```
cd fuzz
# export PATH/CC/CXX/LLVM_CONFIG (nightly-2026-08-27, clang 22.1.8, llvm-config 21.1.8)
setsid cargo fuzz run <target> -- -runs=<N> -timeout=30 -detect_leaks=1 </dev/null >log 2>&1 &
disown
```
Flag syntax: **single-dash** libFuzzer flags (`-runs=`, `-timeout=`, `-detect_leaks=1`) after the single `--`; this build ignores double-dash. `setsid` (not `nohup`) is required for the child to outlive the tool timeout. Launch command must not `pkill -f` its own pattern string (self-kill) — kill by PID or in a separate command.

No crash artifacts were produced on any target. All four targets are cleared for the deep-mutation campaign on the dedicated machine.

### Dedicated-machine prep (handoff, after the gate passes)
- Confirm the build is `cargo fuzz run` (release + instrumentation), **not** the debug
  `cargo build --bin` binaries used for golden validation.
- Package: seed corpus (5597/family) + mutated corpus + crash artifacts + this plan + the audit log.
- Document the run command, wall-clock targets, and the "clean run" definition (0 crashes / all
  crashes resolved).

---

## Decisions recorded (2026-09-22)

1. **Target 03:** repurpose the existing target 03 (honors `FUZZING.md §7`, subsumes matrix-fixture
   coverage).
2. **Local budget:** deep mutation on all four targets before sending.
3. **Gate bar:** 0 crashes OR every crash resolved via `$geosolve-harden-defect`, no unexplained
   false positives.

---

## (Preserved) Original review findings — 2026-09-22, commit `52a8289`

> **Status:** reviewed 2026-09-22 (four fuzz targets crash-free, corpus clean).
> Read-only against the current tree. Ordered by value; each lists evidence, risk, and change.

**Crash-safety + accept/reject contract: HIGH.** Residual validation is genuinely independent
(`geosolve-core/src/solver.rs:1118` rebuilds acceptance at every returned state; the harness adds
a second finite-and-`<=1e-9` gate). Codec is total (`FuzzInput::decode` zero-pads; `normalize()`
clamps scale to `[1e-3, 1e6]`, rotation mod 2π, contact → [0,1], option_index `% 24`). Build
failures don't masquerade as defects (`run_core_solver`/`run_fillet` return `Ok(())` on session
failure). Only `MatrixFixture::new` uses `expect`/`panic`, but its geometry is constant and
transforms are similarities. Hard-vs-soft respected; finite-difference Jacobian tests + audit
descriptor exist at the solver level.

**Coverage breadth before a large campaign: MODERATE.** The fuzzer mainly explores 29 families ×
transform × variant bits. Solver config, base geometry, and input nondeterminism are not exercised.

### 1. Perturb the solver configuration (high value)
Every target used `SolverConfig::default()`. Convergence/branch paths gated on iteration limits,
stabilizer behavior, tolerance sensitivity, and branch hints were never reached. — Resolved by
item #1 above.

### 2. Calibrate the residual bound (do not assume)
The harness hardcoded `<= 1e-9` while the solver admits `normalized_residual_tolerance`. — Resolved
by item #2 above (now adaptive).

### 3. Harness panics vs. solver defects (low risk, worth a guard)
`MatrixFixture::new`, `coordinator()`, and the editor `add_*` use `expect`/`unwrap`. Currently
acceptable because the fixture is fixed and transforms are similarities.
Change (defer unless running very long runs): make fixture/session/coordinator construction
fallible and map build failures to a non-fatal skip (`Ok`) rather than a panic. — Deferred.

### 4. Target `02_fixture_perturbation` is narrow / under-specified
It only flipped `displaced` and re-ran the core solver, largely redundant with `03_core_solver`,
and did not implement the branch-stability comparison documented in `FUZZING.md §4`. — Resolved by
item #4 above (branch_flip implemented).

### 5. Add a determinism pass (cheap insurance)
The harness ran each input once, so a large campaign wouldn't catch input nondeterminism. — Resolved
by item #5 above (`determinism.rs`).

### 6. Differential / oracle cross-check (nice-to-have)
`survey()` (target 01) and `run_core_solver` (target 03) are independent solve paths over the same
input. Compare acceptance between them; disagreement is a defect. — Pending (item #6).

### Campaign configuration notes
- Single-dash flags for this libFuzzer build: `-runs=300000 -timeout=30` (double-dash `--runs=…`
  is silently ignored — `FUZZING.md §7` uses the wrong form).
- Keep `--fresh` for the final qualified gate. Reuse only authenticated unchanged-input evidence
  through the runner per `docs/RELEASE_QUALIFICATION.md`; do not rerun the full gate merely because
  an independent harness stage failed.
- Inputs are 51 bytes; keep `max_len` ≈ 64 and `--detect_leaks=1`.
