<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# GeoSolve active handoff

**Next milestone: [M100 final cleanup and pause readiness](docs/M100_FINAL_CLEANUP.md).**
The supervising user accepted and closed M99 on 2026-09-13 and requested M100
preparation before a temporary project pause. Its bounded plan is prepared;
implementation has not started. Continue in the `m98/file-workspace` worktree at
`/home/arduano/programming/geometric-constraint-solver-worktrees/m98-file-workspace`.
Preserve the primary checkout, existing previews, user source, drafts and history.

**M99 is accepted and closed:** [closure](docs/M99_CLOSURE.md). Qualified product
`eb4d2e06933a7ed5e02533f0359abff28040991a`, run `20260913T155413-41e9f715`, passes
297/297 obligations (32 fresh, 265 authenticated reused). Four exact archives were
installed offline and all 285 shipped files byte-verified. MiniCAD's consumer
migration `46c2662` passes both final pipelines at descendant `dde56658`, with exact
consumer source hashes and evidence in `output/bridge-m99` and `output/pi-case-m99`.
[M99 qualification](docs/M99_QUALIFICATION.md) records the commands, identities
and accepted limitations. M98's separate human acceptance and existing previews
remain unchanged; M99 signoff does not retroactively pass M98's human rows.

M100 finishes small shared-code boundaries, maintained qualification/install
tooling, current documentation, audited artifact retention and a reproducible
restart handoff. Preserve M99's architecture and exact regression coverage.
Use the ordered work and completion criteria in its milestone document; do not
turn this final cleanup into a new feature or protocol redesign.

## Product and ownership

GeoSolve is a pure Rust sketch and linkage solver with public authoring, interaction,
source and embedding libraries. `geosolve-sketch` and `geosolve-linkage` remain separate
domains over `geosolve-core`. The WASM demo consumes public domain/audit APIs.

M99 consolidates source publication, native tool catalogs, terminal validation,
accepted browsing and worker mechanics. Hosts retain distinct storage, history and
publication policies. Keep working source, accepted authority, provisional geometry
and personal presentation separate. Navigation stays local while expensive authoring
or solving runs independently; all accepted edits require independent residual validation.

## Read before implementation

Read [AGENTS.md](AGENTS.md), [ARCHITECTURE.md](ARCHITECTURE.md), [PLAN.md](PLAN.md),
[ACCEPTANCE.md](ACCEPTANCE.md) and [docs/SCENARIOS.md](docs/SCENARIOS.md).
`PLAN.md` owns execution order. Then read the active milestone document and the
relevant subsystem contracts; historical milestone handoffs are reference material.

Use the repository `geosolve-harden-defect` skill for every solver, domain or headless
interaction defect and any golden expansion. Preserve explicit branches, hard/soft
semantics, exact accepted-scene authority and transactional failure retention.
Every new residual needs a finite-difference Jacobian test and a readable audit.
No unsafe code or solver FFI is authorized. Commit only when the caller permits it.

## Qualification

Follow [RELEASE_QUALIFICATION.md](docs/RELEASE_QUALIFICATION.md). Use focused owning-layer
checks during implementation. Nominate a clean source and run the integrated gate for
a complete release claim; its runner authenticates any unchanged-input evidence reuse.
The gate owns formatting, Clippy, native tests, WASM builds, golden, package, browser
and performance coverage. Do not weaken assertions or repeat the entire gate after
an independent harness failure. `--fresh` remains available.

Use the pinned environment on this machine:

```bash
nix-shell shell.nix \
  -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source \
  --run 'COMMAND'
```

Report files/APIs, mathematical behavior, exact commands and results, acceptance
criteria and remaining limitations. Mechanical readiness and human acceptance are
separate records.

## Preserved M98 delivery

M98 is mechanically qualified and delivered; supervising-user acceptance and closure
remain open. [M98_UAT.md](docs/M98_UAT.md) retains U02 **Fail** pending human recheck of
the repaired cold corner drag. Other human rows remain **Not run**. The launcher is
[http://100.94.63.83:18120/](http://100.94.63.83:18120/).

Qualified product `b49e339`, run `20260913T020232-eaefaaaf`, passes 293/293 obligations
(44 fresh, 249 authenticated reused). See [M98 qualification](docs/M98_QUALIFICATION.md)
and [tool parity](docs/M98_TOOL_PARITY.md). This includes all 25 geometry variants,
13 constraint tools, five dimensions, Fillet, Offset and source-authoritative metadata.
The reviewed 271-case golden is unchanged.

UAT services 18121–18125 and retained shared previews 18111/18112 serve frozen installed
artifacts. Preserve their invitations, journals and personal histories. Restart a
shared folder with its existing state, omitting `--initialize true`. Existing location
records remain in `target/m98/`; do not print private invitation URLs or tokens.
The original 18108 manifold retains the user's 40 mm reservoir and 12 mm channels.
M99 has not replaced these services.

Local canvas navigation, selection and provisional reprojection remain available while
solving. A loading veil appears after 500 ms. Current performance evidence is bounded
and does not claim 60 Hz or arbitrary-size responsiveness. Full protocol/authority
contracts are in [M98_COLLABORATION.md](docs/M98_COLLABORATION.md).

## Accepted history

M97 is accepted and closed: [M97_CLOSURE.md](docs/M97_CLOSURE.md). It owns focused
measurement presentation and source-native `isKeyConstraint`, `isKeyParameter` and
`dimensions.areKeyConstraintsByDefault`. M96's accepted channels are recorded in
[M96_CLOSURE.md](docs/M96_CLOSURE.md).

The former 1,848-line handoff, including earlier services, scoped acceptance decisions,
limitations and superseded commands, is preserved in
[the historical handoff](docs/history/START_HERE_PRE_M99.md). Individual milestone
reports, ADRs and qualification artifacts remain the authoritative historical evidence.
