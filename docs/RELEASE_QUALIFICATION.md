<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Proportional release qualification

M93 implements one local gate with explicit stage inputs, immutable preparation outputs,
bounded execution and authenticated reuse. It preserves the sequential reference and all
existing solver, golden, native, WASM, package, browser and performance assertions.
Milestone timing and full integrated qualification remain recorded separately in
[M93_IMPLEMENTATION.md](M93_IMPLEMENTATION.md); runner availability is not milestone acceptance.

## Commands

Run in the repository Nix shell so Rust, Deno, Node, wasm-bindgen and wasm-opt match the
pinned environment. The default is three stage workers and two libtest threads per native
stage. Up to two memory-marked stages can overlap within that total; Playwright retains one slot
for its large sample cases. One build lock serializes Cargo writers independently of memory
admission. Protected tests start after their own preparation and can overlap later builds.
Package installation and generation finish during preflight, before this pipeline starts.
Measured performance remains fully exclusive. Cargo compilation uses four jobs by default, with an explicit `CARGO_BUILD_JOBS`
override recorded in the manifest. The recorded host has 64 GiB RAM; integrated qualification records the actual overlap
and resource costs. `--jobs 1` also reduces the memory-stage limit to one.

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

`--preflight` runs the cheap inventory, metadata/format, managed and frontend tier. Integrated
nomination then runs strict Clippy before preparations and semantic tests; a cold Clippy build is
reported separately from the under-two-minute cheap failure target.

`--plan` explains execution/reuse without executing tests. On an unprepared source it lists
preparation obligations; Cargo metadata, JSON compiler artifacts and libtest discovery expand
the exact native case inventory after preparation. The plan uses the execution scheduler's
contracts; it expands a group only from authenticated current preparation outputs. Each deferred
group remains an explicit obligation, and failed or unexpanded discovery prevents completion.
`--prepare` performs preparation only.
Targeted and preparation reports do not establish a complete release. `--fresh` bypasses passing
test receipts, retaining normal compiler caches. `--reference` invokes the historical sequential
gate and original Cargo/npm oracle. `--test-opt-level 0` selects the unoptimized test profile;
the default level 1 applies to native and non-release WASM tests and retains debug assertions and overflow checks. Profile differences invalidate
evidence and are recorded, not silently compared as identical builds. Tests retain source-line
backtraces with `line-tables-only` debug information. Release and benchmark compilation enables
Cargo incremental artifacts and explicitly retains the original 16 codegen units and level-3
optimization. Test assertions and overflow checks remain enabled; release semantics remain unchanged.
Profile overrides are authenticated inputs and force new qualification. These settings are being
measured against M93's proportional-latency target; configuration alone is not a speedup claim.

The optimized-WASM lifecycle tests have a separate preparation: Cargo compiles and discovers
exactly the existing three cases, captures its runtime/package environment, and preserves the test
WASM in an immutable preparation directory. The pinned runner executes those same cases as a bounded
memory stage alongside independent suites. Build and execution receipts remain separate.

Native workspace and default-feature headless preparations have separate identities and captured
CLI binaries. Every native test executable is also copied into its preparation directory, using
a verified reflink or byte copy, never a hard link. Original Cargo paths remain provenance.
Overlap requires immutable Nix loader/library resolution, unchanged runtime environment, and the
explicitly reviewed `native_build_overlap` program boundary. Unknown runtime inputs or changed
program inputs retain the build lock. The two property suites that preserve source-tree
regression seeds always retain the lock. Ordinary native children use private Deno caches and
evidence folders and clear inherited golden-harness controls. Copies and runtime libraries are hash-checked before use;
executables are checked again after execution. A targeted native selector prepares only its required profile. Rust input closure
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
- Sample/catalog changes use the reviewed boundaries below. Native catalog consumers, artifact
  preparation and packaging remain fresh when their complete inputs change. Do not manually bypass
  broader stages to meet a latency target.

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

The immutable catalog contract at `crates/geosolve-sketch-code/assets/bundled-sample-catalog.json`
is independent expected data. Build/generator, native and frontend checks compare actual discovered
entries against its exact order, keys, titles, categories and retired identities. Removing a sample
requires updating that contract, contiguous manifest ordinals and the generated frontend projection;
it does not require editing count constants in program/test source.

Catalog-only equivalence is explicitly pinned in `scripts/release_equivalence.json`. It hashes the
reviewed program input map, including adapters, tests, build scripts, compiler and runner sources.
Only enumerated catalog data files are excluded; global/unknown inputs, executable files and
symlinks remain. An unresolved Rust include or a changed program map disables this reuse. A newly
passing baseline does not approve a changed dependency boundary. Update its digest only after an
explicit input audit; the runner never updates it automatically.

The golden adapters construct their own exported fixtures and `CodeProject::managed` projects;
they do not select bundled samples. With the reviewed program boundary intact, their single
271-case observation can survive a catalog-only change. Its original executable hashes and run
remain provenance, while the changed catalog/build/package obligations execute independently.

Browser coverage has stable sample-key identities and an independently reviewed non-sample
inventory. Each changed artifact executes the catalog/Recent/retired-origin checks and every
sample's actual UI-open prefix. The prefix hashes the complete persisted wire (including history,
branches and allocators), exact source, fitted geometry and authoritative SVG. A surviving sample's
full selection/grouping/two-edit/Undo/Redo/reload workflow can reuse a signed completed leaf only
when its program, complete selected data and entire prefix witness match. Only manifest ordinal
belongs exclusively to the fresh catalog check. Shared workbench and language tests execute afresh
whenever the outer browser stage runs. `--fresh` bypasses all leaf reuse.

Frontend unit tests use `--no-cache` during preflight. Vitest's generated timing results otherwise
change the `node_modules` identity and needlessly invalidate every browser leaf. This prevents the
output from being created; the gate still hashes the complete installed dependency tree.

`preflight.frontend` owns installation, license/SDK checks and unit tests. Its reviewed
`frontend_static` boundary may exclude enumerated sample-folder data, while retaining the catalog
contract and generated UI metadata. `preflight.catalog` always follows it and owns manifest checks
and the complete build-contract/discovery command. Discovery imports sample sources and witnesses,
so these checks retain sample inputs. A numeric sample edit can reuse unaffected static results;
catalog edits still invalidate the UI metadata consumers. Missing or stale audit pins disable the
exclusion. Each runner shares input scans only across identical boundaries, exclusions and review
contracts within its frozen source snapshot; a new plan starts with an empty scan cache.

The parent binds browser execution to its actual prepared artifact and invocation; source and
consumed artifact hashes must remain unchanged before leaves are retained. Failed batches can
donate complete independent single-attempt rows; skipped, retried or incomplete rows cannot pass.
Coverage distinguishes fresh catalog/prefix checks, fresh full rows and reused original workflows.
It never claims that an older workflow ran on newly built bytes. A witness mismatch runs the full
row; fields must not be dropped merely to get a reuse match.

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
