<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Roadmap

GeoSolve's current milestone is **M100: final cleanup and pause readiness**.
[M99](docs/M99_CLOSURE.md) is accepted and closed. The detailed
[M100 plan](docs/M100_FINAL_CLEANUP.md) owns the bounded worklist and acceptance
criteria. [Documentation results](docs/M100_FINAL_CLEANUP.md#documentation-pass--completed)
record the completed documentation slice. Completed milestone plans live in the [historical roadmap](docs/history/ROADMAP.md)
and [milestone index](docs/history/README.md).

## M100 — final cleanup and pause readiness

The documentation pass now covers the whole repository: a public product README,
current build/authoring/architecture guides, package instructions, design records,
and consolidated historical notes. Detailed mathematical contracts, regression
IDs, licences and genuine acceptance outcomes remain preserved.

- [x] Audit maintenance seams and prepare the implementation/acceptance plan.
- [x] Complete and validate the repository-wide public documentation pass.
- [x] Bound acceptance storage, preserve delivery evidence and reclaim disposable caches.
- [ ] Freeze the remaining ownership, preservation inventory and focused check routing.
- [ ] Finish shared decoding/compiler mechanics and proven redundant adapter cleanup.
- [ ] Make qualification, exact offline installation and source/installed startup reproducible.
- [ ] Prepare restart guidance and audit artifact retention before pruning disposable copies.
- [ ] Pass clean-source qualification, installed consumers and copied-state restart checks.
- [ ] Record final evidence, human dispositions and the pause handoff for signoff.

Keep M99's accepted owners and distinct host policies. No new mathematics, tool
families, durable formats or protocol redesign is scoped. The completed documentation pass
alone does not close M100 or qualify new product archives.

The [storage slice](docs/M100_FINAL_CLEANUP.md#acceptance-storage--implementation-and-cleanup)
reduced target allocation from 472.5 to 115.5 GiB and recovered 196.3 GiB of
filesystem space. Automatic retention, disposable scratch cleanup and explicit
budgets now preserve accepted evidence without accumulating every old build.

## Accepted baseline and outstanding review

M99 qualified product `eb4d2e06933a7ed5e02533f0359abff28040991a` passes 297/297
obligations in run `20260913T155413-41e9f715`. Four offline archives and all 285
installed files were verified. [Qualification](docs/M99_QUALIFICATION.md) records
consumer checks and bounded performance. M99 introduced shared authoring/host
infrastructure cleanup without changing the 271-case reviewed golden.

M98 is mechanically qualified and delivered, with human review still open.
[M98 UAT](docs/M98_UAT.md) preserves U02 **Fail pending human recheck** and other
unperformed human rows as **Not run**. Its automated corner-drag regressions pass;
that does not constitute a new human result.

M97's [focused dimensions and source metadata](docs/M97_CLOSURE.md) and M96's
[manifold channels](docs/M96_CLOSURE.md) are accepted. Their behavior remains part
of the baseline for cleanup.

## Explicit non-goals

Separate product decisions are required for solid modeling/B-rep booleans,
3D sketch curves, global root enumeration, arbitrary curve/manifold plugins,
user residuals or soft ordinary constraints, a C/C++ ABI, mobile support, physical
contact, collision, loads, forces, dynamics or time integration. Final maintenance
must not silently expand into these features.
