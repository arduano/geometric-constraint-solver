<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Proportional release qualification

M93 implements one local gate with explicit stage inputs, immutable preparation outputs,
bounded execution and authenticated reuse. It preserves the sequential reference and all
existing solver, golden, native, WASM, package, browser and performance assertions.
Milestone timing and full integrated qualification remain recorded separately in
[M93_IMPLEMENTATION.md](M93_IMPLEMENTATION.md); runner availability is not milestone acceptance.

## Commands

Run in the repository Nix shell so Rust, Deno, Node, wasm-bindgen and wasm-opt match the
pinned environment. The default is two execution workers, two libtest threads, at most one
memory-heavy stage and exclusive builds/installations/performance work.

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --plan'
nix-shell shell.nix --run './scripts/release-gate.sh --preflight'
nix-shell shell.nix --run './scripts/release-gate.sh'
nix-shell shell.nix --run './scripts/release-gate.sh --fresh'
nix-shell shell.nix --run './scripts/release-gate.sh --fresh --jobs 1'
nix-shell shell.nix --run './scripts/release-gate.sh --stage "workspace.geosolve-core::*"'
nix-shell shell.nix --run './scripts/release-gate.sh --resume RUN_ID'
./scripts/release-gate.sh --docs-only --since BASE_COMMIT
```

`--plan` explains execution/reuse without executing tests. On an unprepared source it lists
preparation obligations; Cargo metadata, JSON compiler artifacts and libtest discovery expand
the exact native case inventory after preparation. `--prepare` performs that preparation only.
Targeted and preparation reports do not establish a complete release. `--fresh` bypasses passing
test receipts, retaining normal compiler caches. `--reference` invokes the historical sequential
gate and original Cargo/npm oracle. `--test-opt-level 0` selects the unoptimized test profile;
the default level 1 applies to native and non-release WASM tests and retains debug assertions and overflow checks. Profile differences invalidate
evidence and are recorded, not silently compared as identical builds.

Native workspace and default-feature headless preparations have separate identities and captured
CLI binaries. A targeted native selector prepares only its required profile. Rust input closure
includes literal embedded files, including Markdown consumed by tests; unrelated frontend CSS
does not invalidate native or optimized-WASM preparation. Runtime temporary files use private
short paths under `/tmp`; durable evidence remains in the run directory.

Clean source is required for product qualification. `GEOSOLVE_ALLOW_DIRTY=1` provides provisional
development evidence only. The runner locks its checkout against a second gate; callers must not
run separate Cargo/npm/generation writers against that checkout during qualification. Use an
isolated checkout for unrelated development. The runner checks source identity again on exit.

## Rerun policy

- During implementation, run focused owner/runner tests after changes. Use preflight before
  expensive qualification. Do not rerun a comprehensive gate after every edit or harness repair.
- Before milestone nomination, run the integrated gate once against clean candidate source. Its
  manifest must account for every obligation as freshly passed or authenticated reused success.
- Reuse requires the exact stage contract, selected cases, transitive owning source inputs,
  fixtures, generated inputs, toolchains, effective environment and consumed artifact hashes.
  Execution ordering alone does not create an input dependency. Unknown files, missing mappings,
  toolchain/lockfile/configuration/policy changes conservatively invalidate evidence.
- An independently completed successful stage may survive a failed overall run. The failed attempt
  remains failed; repaired and downstream affected stages run again. Failed, interrupted, timed-out,
  incomplete or tampered results never supply passing evidence. Build caches are not test receipts.
- Prose-only documentation/sign-off runs diff/link/input checks and preserves the existing
  qualified product identity. It does not qualify a newly packaged README or nominate rebuilt
  bytes. A changed document embedded by a build/test requires affected qualification. The default
  gate recognizes a prose-only diff from `HEAD^`; use `--since` for an explicit baseline.
- Sample/catalog changes currently invalidate every consumer in the demonstrated source/artifact
  closure. Do not manually bypass broader stages to meet the latency target. Finer per-sample
  independence requires explicit inventory/dependency proof and invalidation tests.

The private `target/release-gate` store contains a mode-0600 HMAC key, signed stage results and
qualification manifests. The trusted boundary is this OS user and checkout; it does not defend
against that user replacing the key and evidence. External logs, imported unsigned receipts and
historical M92 records are not automatic reuse candidates. Each reused result links its original
run; source, commands, exit states, selected cases, wall/CPU/RSS measurements and log/artifact
hashes remain available. Deleting the store forces fresh execution without affecting product data.

## Golden and browser evidence

The golden runner evaluates all 271 cases with private scratch directories, fixed existing
30/60-second execution limits and deterministic aggregation. Evaluate once and derive check/clean
dispositions from the captured observation:

```bash
./scripts/golden-authoring-scene-oracle.sh --survey --output-dir NEW_DIRECTORY
./scripts/golden-authoring-scene-oracle.sh --check --from-observation NEW_DIRECTORY
./scripts/golden-authoring-scene-oracle.sh --require-clean --from-observation NEW_DIRECTORY
```

The latter commands validate a retained observation; they are not independent fresh executions.
Golden updates still require explicit row-by-row review. The release gate executes require-clean
once, preserving that complete observation and its exact comparison/clean verdict.

An authenticated WASM preparation is separate from browser bundling, so presentation changes can
reuse the exact optimized package. Browser preparation builds separate compiler-harness and production
distributions. All browser suites share the prepared harness server. The production manifest
identifies the exact output to nominate; do not rebuild after qualifying it. A new artifact or
endpoint requires fresh HTTP byte/MIME/base-path checks and bounded actual-WASM readiness:

```bash
cd crates/geosolve-demo-web/frontend
npm run verify:artifact -- --manifest PRODUCTION_MANIFEST --url ENDPOINT/ --receipt NEW_RECEIPT
```

Moving identical bytes does not require unrelated domain suites again. The command also accepts
`--directory MOVED_COPY` to authenticate a moved artifact. Pages remains publication-only; this
local qualification policy does not add another complete gate to deployment.
