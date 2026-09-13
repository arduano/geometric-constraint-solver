<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M95 navigation: acceptance and closure — 2026-09-07

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

**M95 was accepted and closed on 2026-09-07.** Maintainer acceptance covers the
connected code, Explorer and canvas navigation behavior and its documented limits.
A separate exhaustive human replay was not recorded.

## Accepted checkpoint

| Item | Value |
| --- | --- |
| Accepted product source | `f18ff9ebeff2a0dcf6697aae43ade12dc5a05f21` |
| Accepted product tree | `7e6acedd231d42685ef8b916ced8dc9f44dd27cb` |
| Prior documentation checkpoint | `e57b178` |
| Complete passing gate | `20260907T193542-d9094458` |
| Files SHA-256 | `43fce481d4966b1694192a1301061b93286d3c8f2b2e3eae152c8150fd772545` |

The documentation-only closure preserved the accepted product bytes.

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

No M95 implementation blocker or acceptance action remains.

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

M95's acceptance and qualified source are historical records. Current setup, development
and release instructions are maintained in the [documentation index](README.md) and
[release guide](RELEASE_QUALIFICATION.md). Local receipts and old preview processes
are not part of the public repository or a supported deployment contract.
