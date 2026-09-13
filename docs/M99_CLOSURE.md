<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M99 acceptance and closure — 2026-09-13

**M99 is accepted and closed as of 2026-09-13.** The maintainer accepted the
delivered cleanup and its documented limits, then requested M100 preparation.
This milestone-level acceptance does not claim a separate row-by-row human replay.

## Accepted product and evidence

| Item | Accepted identity |
| --- | --- |
| Product commit | `eb4d2e06933a7ed5e02533f0359abff28040991a` |
| Product tree | `24618ad2b40f85f5d8dfebc261b9ee3dc995edd3` |
| Integrated run | `20260913T155413-41e9f715` |
| Integrated result | 297/297 obligations, 32 fresh and 265 authenticated reused |
| Implementation closeout | `6408764` |
| Golden | 271 cases unchanged |
| Offline installation | Four exact archives, 285 shipped files verified |
| downstream CAD consumer migration | `46c2662`, qualified consumer sources at descendant `dde5665` |

[Qualification](M99_QUALIFICATION.md) records exact commands, receipts, archive and
consumer proof hashes, browser readiness and measured limits.
[The cleanup record](M99_CLEANUP.md) maps removed implementations to their surviving
owners and coverage. Both final downstream CAD pipelines pass against the qualified CLI;
each case variant passes 111 STL inspections and 1707 geometry checks.

M99 shares native authoring catalogs/receipts, source terminal validation,
engine transactions, accepted browsing, local interaction and persistence across
standalone, folder and collaborative hosts. Node hosting belongs to the CLI;
hosts retain separate publication/storage/history policies. No solver mathematics,
primitive set, branch semantics, priority rules or golden expectations changed.

## Scope and continuation

M99 has no remaining implementation or signoff blocker. Qualification and deployment are separate; this closure did not deploy new
artifacts. Dense startup remains a known cost (earlier focused manifold opening:
6.484 s). Passing bounded browser measurements do not imply 60 Hz or unrestricted
multi-client scale. downstream CAD case rendering and physical fit/thermal review were
outside the consumer integration check.

M98's separate human acceptance is unchanged: U02 remains **Fail pending human
recheck**, and unperformed rows remain **Not run**. This M99 close decision does
not retroactively pass those rows. [M100 preparation](M100_FINAL_CLEANUP.md) carries
their truthful disposition into the pause handoff.

This is a documentation-only acceptance record. Qualification remains tied to
`eb4d2e0`; no build, solver test replay, branch merge, deployment or service retirement
is required for this prose. Closeout uses:

```bash
git diff --check
./scripts/release-gate.sh --docs-only --since 6408764
```

The command passed; its historical result was recorded in
`target/m100/preparation-docs.log`. [M100](M100_FINAL_CLEANUP.md) owns the subsequent
cleanup status.
