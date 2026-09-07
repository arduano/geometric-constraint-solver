<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M95 acceptance and fresh-session handoff — 2026-09-07

**M95 is accepted and closed.** The supervising user stated: “I accept the UAT, please close it
off and then ensure repo state is clean enough to hand over to a fresh session (docs and all)”.
This accepts the delivered connected code/Explorer/canvas navigation and its documented limits.
It does not assert an unrecorded exhaustive human replay. No next milestone is scoped or authorized.

## Accepted checkpoint

| Item | Value |
| --- | --- |
| Repository | `/home/arduano/programming/geometric-constraint-solver` |
| Branch | `m92/integration` (inherited name; work is complete through M95) |
| Accepted product source | `f18ff9ebeff2a0dcf6697aae43ade12dc5a05f21` |
| Accepted product tree | `7e6acedd231d42685ef8b916ced8dc9f44dd27cb` |
| Prior documentation checkpoint | `e57b178` |
| Complete passing gate | `20260907T193542-d9094458` |
| Accepted M95 endpoint | `http://100.94.63.83:18100/` |
| Retained accepted M94 / M92 | `http://100.94.63.83:18096/` / `http://100.94.63.83:18092/` |
| Frozen manifest | `/tmp/geosolve-m95-uat.zkdiegw4/production.json` |
| Frozen files | `/tmp/geosolve-m95-uat.zkdiegw4/geosolve-production` |
| Files SHA-256 | `43fce481d4966b1694192a1301061b93286d3c8f2b2e3eae152c8150fd772545` |

The closure commit is a documentation descendant of `e57b178`; `git log -1` identifies it without
changing accepted product bytes. No branch rename, history rewrite, merge, push or Pages deployment
is part of this closeout. Historical worktrees and branches remain preserved; none is an M95 task
awaiting integration. The primary worktree is the continuation point.

## Delivered scope and accepted limits

Native navigation APIs resolve owned outputs separately from singular mutation selection. Accepted
source/expansion indexes connect ordinary and generated declarations, exact members, groups and
multi-selection. The bridge sends transient selection updates; CodeMirror decorates relevant source
without changing cursor/focus. Ordinary selection preserves layout. Explicit Show in canvas/Show
in code reveals hidden destinations, with Fit separate. Dirty/stale/Unicode requests, hidden rows,
active tools/captures and Apply/history/replacement retain truthful ownership. Dense visibility
batching removes repeated whole-project declaration projection.

All mathematical behavior remains unchanged: no equations, tolerances, branches, priority rules,
primitive families or canonical persistence formats changed. The acceptance evidence includes
241 passing gate stages (24 fresh, 217 authenticated reused), 181 frontend tests, the complete
45-case browser inventory and all 271 unchanged golden rows. The fresh native workbench suite
passes 323 tests with one pre-existing ignored test. Detailed files/APIs, exact qualification
commands and results are in [M95_QUALIFICATION.md](M95_QUALIFICATION.md); the chronological
[M95_IMPLEMENTATION.md](M95_IMPLEMENTATION.md) retains the failed and interrupted attempts.

Accepted limits remain explicit:

- Source navigation covers authenticated builder expressions. A full declaration-line selection
  intersects its expression; a cursor only in `const name =` or an arbitrary reference is unmatched.
- Dense editing remains above a 16.7 ms frame budget. Source compilation and full history
  restoration remain expensive; final point previews are about 28–30 ms and terminal bridge work
  about one second. Navigation measurements and run-to-run variation are recorded in qualification.
- Selection preserves the camera, so off-screen geometry may need explicit Fit. M95 adds no desktop
  layout redesign or automatic Pages publication.

There is no outstanding M95 blocker or acceptance action. Further work needs the user's next task.

## Closure verification

The read-only closure audit authenticates all 241 stage receipts, checks the signed browser
preparation against the original and frozen manifests, and rechecks all 12 read-only files plus
HTTP `/` against the accepted bytes. It passes at the existing endpoint; its script/report are
`target/m95/closure/audit.py` and `target/m95/closure/evidence.json`.

Executed closeout commands:

```bash
python3 target/m95/closure/audit.py
./scripts/release-gate.sh --docs-only --since f18ff9e
git diff --check
git status --porcelain=v1 --untracked-files=all
```

The documentation check passes for nine prose files and 26 added links; `git diff --check` passes.
The accepted complete format,
Clippy, native, WASM, golden and browser gate remains the product evidence under
[RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md); closing documentation requires no rebuild
or repeat solver/browser qualification. The post-commit status check establishes a clean worktree.
Original nomination/qualification receipts retain their historical awaiting-acceptance
status; this document supplies the subsequent explicit user acceptance.

## Fresh-session instructions

Start with [START_HERE.md](../START_HERE.md), this handoff and the active M95 sections of
[PLAN.md](../PLAN.md), [ARCHITECTURE.md](../ARCHITECTURE.md),
[ACCEPTANCE.md](../ACCEPTANCE.md) and [SCENARIOS.md](SCENARIOS.md). Before implementation, obey
`AGENTS.md` and read its required documents. Use the repository defect-hardening skill for solver
or headless-interaction defects. Do not restart M92, M94 or M95 qualification from historical notes.
The external `geometric-constraint-solver-M92-HANDOVER.md` now carries a superseded banner.

No Cargo, browser test, release gate or development task is left running by this closeout.
Obsolete local development servers on ports 18097, 18098, 18108, 18109 and 18110 have stopped.
Accepted M95 remains on port 18100 (PID 1937583 at closeout); M94 and M92 remain available.
Check process identity before acting on recorded PIDs in a later session.

Local receipts, screenshots and probes live under ignored `target/m95/`; authenticated gate inputs
and receipts live under `target/release-gate/`. Frozen artifacts live under `/tmp` and are local,
not committed or guaranteed across reboot/cleanup. Preserve them for continuity; the tracked
qualification and this closure record retain the product identity if local evidence is lost.

If the accepted M95 server has stopped but its frozen files remain, this repository-root command
restarts the exact authenticated files on localhost and Tailscale without rebuilding:

```bash
M95_MANIFEST=/tmp/geosolve-m95-uat.zkdiegw4/production.json \
M95_DIRECTORY=/tmp/geosolve-m95-uat.zkdiegw4/geosolve-production \
node target/m95/serve-frozen.mjs
```

The local helper imports `serveArtifact` from the tracked frontend `scripts/serve-artifact.mjs`
and binds hosts `127.0.0.1` and `100.94.63.83` at port 18100. If moving or restarting the artifact,
repeat the `verify:artifact` command in the qualification report. If the frozen files are missing,
a rebuilt artifact needs qualification before being described as the same accepted bytes.
