<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M93 qualification checkpoint and remaining work

M93 remains open. Source `b43f54b4ed57745774503a12e4e410be8f985311` passed a complete fresh
parallel gate and all six representative edit/prune qualifications. The ten-minute edit/prune
target was missed. A subsequent audit found a count-checker repair invalidation defect; its
repair and a further scheduling optimization are in progress. Cold serial qualification and
parity have not run on this source. No target was waived or milestone acceptance granted.

## Measured checkpoint

Host: `main-pc`, AMD Ryzen 9 3900X, 24 logical CPUs, 64 GiB installed RAM. Stage limits:
three workers, two memory-marked stages, one Cargo writer with four compiler jobs, and two
native test threads per stage. Existing Nix, npm/download and Cargo build caches were warm.
Unrelated work shared this host; these measurements are not isolated scheduler comparisons.

The command `nix-shell shell.nix --run './scripts/release-gate.sh --fresh'` passed all
**241/241 stages**, each freshly executed, in **1,426.210 seconds** (gate report) and
**1,428.932 seconds / 23m49s** (gate process). This meets the 45-minute warm full-gate target.
Independent extraction authenticated 9,837 evidence files, 211 native stages / 2,629 selected
native executions, all 16 fresh browser sample workflows, 37 ordinary WASM cases, three
optimized lifecycle cases and the unchanged 271-case golden. Golden SHA-256:
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

| Representative change | Gate process seconds | Preparation + gate seconds | Qualification | <600s target |
|---|---:|---:|---|---|
| numeric-3.1 | 626.612 | 630.980 | PASS | MISS |
| numeric-3.2 | 670.217 | 674.095 | PASS | MISS |
| numeric-3.25 | 1092.067 | 1100.529 | PASS | MISS |
| prune-1 | 645.308 | 654.901 | PASS | MISS |
| prune-2 | 807.464 | 816.492 | PASS | MISS |
| prune-3 | 645.790 | 658.026 | PASS | MISS |

Preparation includes generation, temporary candidate commit and baseline-index setup. The
combined figures exclude subsequent result checking, evidence archival and final restoration.
Each candidate reused exactly 15 authenticated baseline survivor browser workflows after fresh
opening-state witnesses; numeric edits reran the changed manifold workflow. Each prune removed
one current Bondtech entry while retaining its private regression fixture. These are single-entry
prune measurements, not an exact replay of M92's historical four-entry removal. All helpers
restored baseline source and signed indexes. Deployment is outside these timings.

Documentation checks took **1.539 / 1.662 / 1.528 seconds**. Stale-count failures took
**5.401 / 5.139 / 5.315 seconds**; generated-order failures **5.118 / 5.537 / 5.768 seconds**.
All induced failures stopped in inventory preflight before product compilation or semantic tests.
These pass the one-minute documentation and two-minute cheap-failure targets over three repeats.
The isolated worktree had the same Git tree as the qualified source.

The third numeric run encountered load averages above 60 on 24 logical CPUs. Prune-2 also
recorded substantial load and unrelated OCCT/SLA-slicer work. These observations explain why
host conditions must be reported; no contention-adjusted timing or waived miss is claimed.
Parallel child CPU was 2,618.653 seconds user plus 395.579 seconds system, with cumulative
maximum child RSS 2,620,520 KiB. That RSS is not total concurrent process memory.

Raw evidence, exact commands and cleanup records are under `target/m93/cache-qualification`;
fast-path records are `target/m93/cache-fast-paths.json`. Measurement scripts are archived with
SHA-256 identities in `measurement-scripts/sha256.json`. The warm run is
`20260906T183554-0a236cc5`. `frontend-output-stability.json` proves that later browser preparation
preserved the exact installed dependency bytes from frontend static checks. Earlier failures
and implementation measurements remain in [M93_IMPLEMENTATION.md](M93_IMPLEMENTATION.md).

## Remaining qualification and confirmed recovery issue

A checker-only edit to `frontend/scripts/check-sample-manifest.mjs` invalidates the reviewed
native writer boundary. The old runner then changes 209 of 211 native stages' `build_lock`
values, and includes that scheduling flag in semantic receipt keys. Those tests rerun despite
unchanged native source, executable/runtime bytes and case inventories. Golden and WASM keys
remain eligible. Generic recovery fixtures had not exercised this combined path.

The correction separates only native scheduling admission from semantic result identity.
Current execution and full recorded contracts must retain conservative locks; source, cases,
profiles, runtime, tools and artifacts must still invalidate changed evidence. Focused combined
checker-repair/failed-donor regressions and integrated qualification are required. The subsequent
optimization allows prepared frontend bundling to overlap Cargo only under an independently
reviewed complete writer/input boundary.

After the final prune restored baseline source/indexes, the automation driver was terminated
before its cold gate. `measurement-stop.json` records that intentional stop; no cold or parity
PASS is claimed. Both must qualify the corrected runner. The preserved warm measurement is a
checkpoint, not qualification of subsequent changes. C5 timing, exact count-repair evidence,
remaining integrated qualification and supervising-user acceptance stay open.

The accepted M92 catalog remains 16 samples, and its service and publication are unchanged.
