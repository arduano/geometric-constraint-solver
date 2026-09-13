<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Release storage

The qualification runner manages disposable artifacts in this checkout's
`target/release-gate`. It prunes before and after execution, using the same
exclusive lock as qualification and preparation. It never cleans another
checkout, global Cargo/npm/Deno caches, installed products or project state.

## Inspect and clean

Run from the repository root with Python available:

```bash
python3 scripts/release_storage.py audit --build-cache --json
python3 scripts/release_storage.py verify
python3 scripts/release_storage.py prune --build-cache --apply --json
python3 scripts/release_storage.py verify
```

`audit` and `prune` default to a dry run. `--json` lists every candidate and
projected size. `--apply` computes a new plan under the lock; it cannot import a
saved deletion list. `--build-cache` additionally permits known Cargo output
directories to be removed when needed to meet the total target budget. They
will rebuild on demand. Neither command deletes all of `target`.

The tool rejects symlinked deletion roots and foreign paths, protects live
process executables, mappings, open files and configured asset roots, and
checks active paths again before deletion. Automatic process inspection uses
Linux `/proc`. Do not run independent Cargo/npm/preparation writers against
the same checkout during qualification or cleanup. A tool's lock cannot
serialize unrelated commands that do not participate in it.

## Retain a product or delivery

The latest run, latest qualified release and recent interrupted stages are
retained automatically. Explicitly pin accepted products and outstanding UAT
before a newer qualification replaces the automatic retention root:

```bash
python3 scripts/release_storage.py pin --label accepted --run RUN_ID
python3 scripts/release_storage.py pin --label uat --run OTHER_RUN_ID
```

A delivery that needs only its package/browser provenance can retain those
passing stages and their dependencies without retaining an entire superseded
qualification:

```bash
python3 scripts/release_storage.py pin --label installed-packages --run RUN_ID --stage package.m98
python3 scripts/release_storage.py pin --label installed-browser --run RUN_ID --stage prepare.browser
```

Stage pins preserve delivery evidence, not a claim that all other obligations
from that historical run remain locally verifiable. Use a whole-run pin when
complete qualification evidence is required. Installation/project bytes outside
the managed store are always excluded from deletion candidates.

Pins live in authenticated local `target/release-gate/retention.json`. A label
cannot silently change ownership. After retiring its product, installation or
UAT dependency, remove it explicitly and inspect a new dry run:

```bash
python3 scripts/release_storage.py unpin --label retired-delivery
python3 scripts/release_storage.py audit --build-cache --json
```

Keep the store authentication key with retained evidence. Cleanup follows
cross-run stage donors, original prepared inputs, captured output symlinks and
browser leaf/batch provenance. It retains complete required stage payloads,
including already-signed historical scratch. It never edits old receipts to
make smaller artifacts appear qualified. Small signed run summaries remain;
evicted cache results require fresh execution. `--resume RUN_ID` reuses only
available authenticated evidence, so a pruned old run can require rebuilding.

Planning authenticates retention metadata and provenance. The explicit
`verify` command additionally rehashes retained evidence and captured store
outputs, including nested output expectations. It does not requalify a product
or certify mutable external build outputs. Use it before and after deliberate
archival cleanup; ordinary stage reuse still independently checks actual bytes.

## Budgets and temporary data

[`scripts/release_storage_policy.json`](../scripts/release_storage_policy.json)
sets the checkout-local defaults:

| Setting | Default |
| --- | --- |
| Acceptance store | 128 GiB |
| Entire checkout `target` directory | 160 GiB |
| Filesystem free-space reserve | 32 GiB |
| Automatic latest runs / qualified releases | 1 / 1 |
| Unreported interrupted stage grace period | 24 hours |

The runner prunes disposable payloads and then checks these size budgets before
starting and after finishing. Protected evidence and unrelated target data are
never deleted to meet a ceiling; excess usage makes the command fail with an
actionable budget error. Retire obsolete pins deliberately or review a policy
change when the required retained set no longer fits.

These are admission and post-run budgets, **not filesystem quotas**. A running
qualification may exceed them while compiling and capturing new outputs. The
free-space reserve is checked before stages and approximately every second
while subprocesses execute; crossing it stops qualification with a failure.
Polling cannot prevent a sudden write burst or another application's disk use.
Use a filesystem quota or dedicated volume if an exact peak allocation boundary
is required. A policy change is a qualification input, not permission to weaken
any test assertion.

New golden cases use private temporary compiler caches outside their durable
observations and dispose of them on success or exception. Package/host stages
discard their private repository and npm cache on completion, rejection, timeout
or handled interruption, preserving selected logs, TAP, coverage, package proof
and browser diagnostics first. The parent runner also cleans its stopped M98
stage runtime before sealing evidence. A machine crash or uncatchable kill can
leave temporary files; no global temporary-directory sweep is performed.

Allocated directory sizes can include shared reflink extents. Compare the
reported target sizes with filesystem free space when measuring reclamation;
they need not decrease and increase by the same amount.
