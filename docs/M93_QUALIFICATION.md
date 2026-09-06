<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M93 qualification and measured limits

Status: **M93 remains open. Product/runner qualification is complete for the source below; C5 edit/prune latency fails and has not been waived. Supervising-user acceptance has not been granted.** M92 stays closed with its accepted 16 samples.

## Qualified source

Source `08510fcf554d91829c2069a068450bbd4747a6c7` passed fresh parallel gate `20260906T203028-063974c6` and cold-Cargo-build serial gate `20260906T222221-6165fbdf`. The independent comparison passed every recorded source, tool, contract, inventory, result, runtime, witness and golden equality check. Only the comparator’s documented scheduling and build-output path differences are normalized.

The runner uses explicit inputs, conservative invalidation, authenticated receipts, safe resume, deferred complete inventories, one Cargo writer, bounded native/browser execution and exclusive performance admission. Private executable captures and immutable loader/library checks protect overlap. Golden observations are consolidated; browser suites share prepared assets; sample-workflow reuse requires fresh matching opening-state witnesses. TypeScript build-info files live outside installed dependencies.

No solver equation, residual tolerance, finite-state validation, branch/DOF behavior, history assertion, golden disposition or bundle ceiling was weakened. This ledger records qualification; it does not publish or replace the accepted M92 service.

## Host and measurements

Host: `main-pc`, AMD Ryzen 9 3900X, 12 cores / 24 logical CPUs, 64 GiB installed RAM (62.70 GiB reported by Linux). Parallel limits: 3 stages, 2 memory-marked stages, one Cargo writer, 4 compilation jobs and 2 native threads per stage. Serial used one stage with the same compiler/native limits and profiles.

| Workload | Gate process seconds | Preparation + gate seconds | Qualification | Timing target |
|---|---:|---:|---|---|
| Warm build caches, all results fresh | 1309.782 | — | PASS | ≤2700s PASS |
| Cold Cargo build cache, serial fresh | 6444.167 | — | PASS + parity | Cold measurement |
| numeric-3.1 | 752.425 | 756.847 | PASS | <600s MISS |
| numeric-3.2 | 736.607 | 741.510 | PASS | <600s MISS |
| numeric-3.25 | 774.012 | 779.446 | PASS | <600s MISS |
| prune-1 | 925.437 | 941.424 | PASS | <600s MISS |
| prune-2 | 758.670 | 768.546 | PASS | <600s MISS |
| prune-3 | 739.334 | 748.195 | PASS | <600s MISS |

Preparation includes generation, candidate commit and baseline-index setup. The combined figure omits subsequent result checking, evidence archival and final checkout/index restoration, so it is not the full helper wall time. Deployment latency is outside these measurements.

The full-run host snapshot recorded load averages [9.20068359375, 12.45556640625, 23.365234375] on 24 logical CPUs. Earlier checkpoint runs recorded unrelated OCCT triage and SLA-slicer compilation; the new measurements retain their own host records. These observations disclose shared-host contention without attributing a particular amount of delay to it. Every timing miss remains a miss.

Every candidate proved exactly 15 baseline survivor workflows reused with fresh required catalog/shared checks. Numeric candidates reran the changed manifold workflow; prune candidates temporarily removed Bondtech while retaining its dedicated regression fixture. Each prune repetition removed one current entry; these are representative single-entry prune measurements, not an exact replay of M92’s historical four-entry removal. Per-candidate cleanup records confirm restoration of the baseline source and signed indexes.

- docs-only: 1.439s / 1.520s / 1.489s; <60s PASS.
- stale-count: 5.817s / 5.225s / 5.324s; <120s PASS.
- generated-order: 5.212s / 5.223s / 5.292s; <120s PASS.

The actual count-checker probe first rejected an appended 17-entry assertion, then passed after changing only 17 to 16. The normal product planner retained all 211 native, one golden and nine WASM obligations with their exact baseline keys, receipt paths and origins. All 211 current native scheduling locks became conservative while those semantic result keys remained unchanged. Source restoration and unchanged receipt indexes/control files/run inventory were verified. This is direct checker execution plus read-only product planning, not a repaired full gate or an actual failed overall product donor. Finalized failed-run donation is covered separately by the focused signed real-child fixtures.

Fast paths ran in an isolated worktree at `434d6d30d9d15dbe008298237393ab479fa160ed`, whose Git tree matches the qualified source. Every induced inventory failure stopped before compilation or semantic tests. Cold qualification began with an absent absolute Cargo target directory; Nix tools and npm/download caches remained warm. Cold-versus-warm wall time does not isolate scheduler speedup.

Parallel child CPU: 2654.789s user + 357.693s system; cumulative maximum child RSS 2,409,000 KiB. Serial child CPU: 13239.239s user + 878.113s system; cumulative maximum child RSS 2,798,948 KiB. RSS is not summed concurrent-process memory. The maximum-RSS counters are cumulative across each measurement driver’s children; the cold driver had already run candidate and count helpers. They are not isolated peak-memory comparisons between the two gates. Raw receipts retain stage resource/wait measurements; per-candidate generation and gate process records retain CPU/RSS costs.

## Coverage and commands

Independent extraction authenticated 9,875 parallel and 9,877 serial evidence files. Coverage includes 211 native stages / 2,629 selected native executions, 16 fresh browser sample workflows within 38 unique browser cases, 37 ordinary WASM cases, 3 optimized lifecycle cases and the 271-case golden.

Golden SHA-256: `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

Commands from completed measurement records (induced preflight failures are expected nonzero exits):

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --fresh'
nix-shell shell.nix --run 'CARGO_TARGET_DIR=/home/arduano/programming/geometric-constraint-solver/target/m93/cold-serial-cargo-08510fc ./scripts/release-gate.sh --fresh --jobs 1'
./scripts/release-gate.sh --docs-only --since HEAD
nix-shell shell.nix --run './scripts/release-gate.sh --preflight'
nix-shell shell.nix --run 'node packages/geosolve-sketch-code/scripts/generate-bundled-samples.mjs --write'
nix-shell shell.nix --run ./scripts/release-gate.sh
nix-shell shell.nix --run 'cargo run --locked -q -p geosolve-sketch-code --example generate_frontend_samples -- --write crates/geosolve-demo-web/frontend/src/data/samples.json'
```

Previously recorded focused checks are described in [M93_IMPLEMENTATION.md](M93_IMPLEMENTATION.md): 164 release regressions, the TypeScript cache smoke and `git diff --check`. Comparator and benchmark-coverage fixture results are separate helper evidence; they do not replace the full qualification. The actual TypeScript smoke compiled both cache configurations, then rejected an invalid source with TS2322 while preserving dependency-tree bytes.

Exact candidate source identities, generation/qualification commands, cache provenance and cleanup records are retained under `target/m93/recovery-qualification`. Archived measurement scripts have SHA-256 identities in `measurement-scripts/sha256.json`. `serial-parallel-comparison.json` records semantic/coverage parity; `frontend-output-stability.json` records that the later browser build preserved static dependency bytes. Earlier failures and timing misses remain in the implementation notes.

## Acceptance remains open

C1/C3/C7 qualification and selection evidence is recorded above. C2 has three repetitions of each induced failure; C4 has completed serial/parallel parity plus separately recorded recovery regressions. C6 has a warm fresh run and a measured cold serial fresh run; the warm target passed. These are evidence statements, not supervising-user acceptance.

**C5 edit/prune latency is not met and has not been waived.** Target misses: numeric-3.1, numeric-3.2, numeric-3.25, prune-1, prune-2, prune-3. Documentation latency and the count-assertion repair evidence are recorded separately above, with their scope limitations. Milestone closure remains open under [M93_GOALS.md](M93_GOALS.md). No timing tradeoff is accepted by this generated ledger.

## What remains slow

The first numeric edit reused 173 of 241 stages and 15 survivor workflows. Its browser stage
started about 475 seconds into the gate and took 274.099 seconds; optimized WASM lifecycle work
finished about 666 seconds in. Those approximate start/end positions use receipt-file timestamps
and are diagnostic chronology, while stage durations come from recorded receipts. Browser-only
savings would not eliminate the secondary lifecycle tail. The detailed read-only review is
`target/m93/recovery-qualification/m93-c5-latency-review.md`.

The next bounded experiment is earlier WASM/frontend admission within the existing one-Cargo-writer,
worker and memory limits, followed by the same complete coverage checks. Separating unchanged
harness fixtures from always-fresh inventory checks is another possible saving. Neither experiment
has run or has a promised speedup. Narrower sample reuse would require an explicit data boundary
because the current embedded catalog changes native and WASM executable bytes; those identities
must not be ignored. Shared-host load is reported without subtracting an estimated delay.

The required historical four-entry M92 prune replay also remains unproven. These benchmarks each
remove one current Bondtech entry and retain its dedicated regression fixture. That is measured
representative scope, not acceptance of an unexecuted historical workload.

## Count repair evidence and command

The count probe appended a deliberately stale assertion to the actual manifest checker at
`1b1b4395237aaf4651d8977207e8e359d398cc82`; the exact `16 !== 17` assertion failed.
Repair `ae1770e5b69eb93ea54b2c5c8dea60857804df5e` changed that probe to 16 and passed.
Normal planning and passive key inspection proved the 221 unchanged keys and original receipt
paths with conservative current locks. Direct checker calls took 1.820s and 1.819s; the baseline,
normal repaired plan and exact repaired-plan inspections took 148.512s, 136.272s and 186.944s.
These inspection costs are separate from the repeated five-second stale-count preflight target;
this is not repaired full-gate latency.

```bash
python3 /tmp/m93-product-count-repair.py --root /home/arduano/programming/geometric-constraint-solver --baseline-commit 08510fcf554d91829c2069a068450bbd4747a6c7 --baseline-run 20260906T203028-063974c6 --output /home/arduano/programming/geometric-constraint-solver/target/m93/recovery-qualification/count-repair --execute
```

The executed helper is archived with SHA-256
`a116703649e064fe6a4bbb5d66261643dcd23320d1bbc2417d235b7ed71256c6`.
Its independent review and 12 helper-only fixtures are separate from the 164 implementation
regressions. The actual product helper creates no failed overall donor and executes no product
tests. Finalized failed-run donation is covered by focused signed real-child fixtures, with the
original failed receipt retained. No broader product replay is claimed.

## Historical b43 checkpoint retained

Source `b43f54b4ed57745774503a12e4e410be8f985311` passed fresh parallel run
`20260906T183554-0a236cc5` in 1428.932 seconds (23m49s), with 241/241 stages fresh and the same
271-case golden bytes. Its representative measurements all qualified but missed C5:

| Representative change | Gate process seconds | Preparation + gate seconds | Qualification | <600s target |
|---|---:|---:|---|---|
| numeric-3.1 | 626.612 | 630.980 | PASS | MISS |
| numeric-3.2 | 670.217 | 674.095 | PASS | MISS |
| numeric-3.25 | 1092.067 | 1100.529 | PASS | MISS |
| prune-1 | 645.308 | 654.901 | PASS | MISS |
| prune-2 | 807.464 | 816.492 | PASS | MISS |
| prune-3 | 645.790 | 658.026 | PASS | MISS |


The historical preparation + gate figures also exclude later result checking, archival and restoration.
Its docs repetitions took 1.539 / 1.662 / 1.528s; stale-count failures 5.401 / 5.139 / 5.315s;
generated-order failures 5.118 / 5.537 / 5.768s. Source/index restoration passed after all six
candidates. Shared load exceeded 60/24 logical CPUs during one edit; all misses remain unadjusted.
The parallel child CPU was 2618.653s user + 395.579s system; cumulative child max RSS was
2620520 KiB, not total concurrent memory. Raw evidence remains in `target/m93/cache-qualification`
and `target/m93/cache-fast-paths.json`.

A subsequent actual-checker audit found that stale writer-review pins changed 209 native
scheduling locks and unnecessarily changed semantic result keys. The old driver stopped after
prune cleanup and before cold qualification; no cold/parity pass exists for b43. The corrected
085 source and recovery evidence above supersede that implementation checkpoint. Earlier failures
and profile measurements remain in [M93_IMPLEMENTATION.md](M93_IMPLEMENTATION.md).
