<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M40 core sketch interaction UAT

## Status

Complete and archived as of 2026-07-26. The supervising human approved M40.7 after
the F1-F5 targeted rechecks below. The then-current release-browser qualification
passed 14/14, and no correctness, data-loss, misleading-state or basic-interaction
blocker remained.

This is a compact historical decision record, not an executable UAT script. M48
replaced the durable objective claims with direct editor/workbench tests and retired
the browser harness, serving path and finding downloads. M52-M53 own the
post-cleanup candidate and human review.

## Targeted Recheck UAT-C1-F1

Passed. The recheck covered live line placement, discoverable polyline completion
and endpoint snapping without a visible accepted-position jump.

## Targeted Recheck UAT-C1-F2

Passed. Point and constrained-line drags displayed a solved preview continuously,
committed the previewed branch on release, created one Undo step and discarded
cancelled motion.

## Targeted Recheck UAT-C1-F3

Passed. Persistent multi-selection, forgiving curve picking, click-versus-drag
separation and projected counterclockwise arc placement remained coherent between
preview and accepted state.

## Targeted Recheck UAT-C1-F4

Passed. Constrained geometry retained the exact nearby valid preview branch on
release instead of realigning to another valid solution; the headless regression
also required one-step Undo.

## Targeted Recheck UAT-C1-F5

Passed. Polyline previews remained unfilled, circle/arc center and radius guides
remained visible at the required stages, and finishing a polyline committed only
placed segments while clearing every provisional marker.

## Historical finding capture

The completed review used a deterministic JSON finding envelope and accepted-scene
SVG. Those browser download routes were temporary M40 evidence and were removed by
M48. Their durable semantic owners are recorded in
`docs/M46_DIRECT_TEST_REPLACEMENT.md` and `docs/M48_IMPLEMENTATION.md`; they must not
be restored as a current qualification path.

## Scorecard

The archived source record did not preserve individual per-row ratings. It did
preserve the supervising-human decision after the five targeted rechecks:

| Review context | Recorded result |
| --- | --- |
| Candidate | `geosolve-demo-web/0.2.0` pre-cleanup M40.7 release candidate |
| Objective browser qualification | 14/14 at that checkpoint; retired by M48 |
| Targeted findings | UAT-C1-F1 through UAT-C1-F5 passed |
| Decision | **Approve** |
| Supervising human/date | supervising caller, 2026-07-26 |
| Approval statement | No unresolved correctness, data-loss, misleading-state or basic-interaction blocker remained. |

The temporary network address, browser-download instructions and blank working
scorecard were intentionally removed during M53 repository reconciliation because
they were neither durable evidence nor valid current instructions.
