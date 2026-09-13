<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 prototype review — 2026-09-09

Historical review of prototype `cb581b5`, based on `d80bf22`. M97's source-native
metadata was subsequently integrated by `1584a5a`; its acceptance remains recorded
in [M97 closure](M97_CLOSURE.md). This review explains the transition from the
prototype to the [complete M98 plan](M98_IMPLEMENTATION_PLAN.md).

## Assessment

The useful boundary was ordinary source as durable intent, with Node handling
filesystem/HTTP transport and Rust owning source transactions, solving and accepted
geometry. The existing UI was reused. Baked profiles came from accepted production
topology rather than canvas tessellation.

The prototype had actual evidence: seven bake tests, 13 browser subtests plus their
parent, 20 follow-up frontend tests and three native sampler tests. Its initial
frontend slice passed 67 tests. These were focused checks, not an integrated release.
[Folder prototype](M98_HANDOFF.md) and [bake prototype](M98_BAKE_HANDOFF.md) retain the
original results and limits.

## Takeover priorities

The review identified five areas for hardening:

1. Integrate source-owned metadata and verify labels, overview flags, parameters,
   extraction and history through disk writeback.
2. Replace broad DOM capture and command-name guesses with explicit field lifecycle.
3. Expose folder capabilities before an unsupported interaction begins.
4. Define journaled recovery for interrupted filesystem publication.
5. Measure full snapshot, history serialization and dense-sketch navigation costs.

Multi-file imports and computed-profile export were separate necessary expansions
for the manifold. Exported arrangement faces still required explicit consumer
selection; a profile list did not imply solid or material-removal semantics.

## Fresh takeover observations

The initial prototype preserved useful source and conflict boundaries but did not
qualify dense navigation, multi-file source or complete point-drag persistence.
Those limitations motivated the subsequent implementation and documented findings.
[M98 qualification](M98_QUALIFICATION.md) supersedes this preliminary assessment;
[M99 cleanup](M99_CLEANUP.md) records consolidation of the resulting shared owners.
