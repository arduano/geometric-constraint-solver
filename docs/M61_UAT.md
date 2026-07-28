<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M61 human UAT 3 — advanced geometry and topology

Status: ready for supervising-human review; approval not yet recorded

Candidate source: the clean M60 completion commit recorded in Git after the complete release gate.

## Entry point

From the repository root:

```bash
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'trunk serve --open'
```

Use the top **Scenarios** selector, then open **M61 Advanced geometry & topology**. The nested menu
expands to the right on hover or keyboard focus. Do not use or recreate `/#/dev/lab`; it was
deleted in M50.

The four stable leaves are:

1. `advanced-all-families`
2. `nurbs-branch-topology`
3. `associative-companion-operations`
4. `production-topology-trust`

Each selection reconstructs deterministic ephemeral state. **Reset scenario** returns that leaf
to its fixed start. **Exit scenario** restores the unchanged ordinary workspace. Scenario work is
not written to browser workspace persistence.

## Review scope

Allow 45–60 minutes. Objective geometry, transactions, persistence, diagnostics, cancellation and
topology completeness have already passed direct Rust/WASM qualification. Human review is limited
to discoverability, manipulation intent, branch clarity, topology trust and perceived desktop
responsiveness.

### A. Advanced family clarity

Open **Advanced all-family gallery**.

- Inspect analytic, conic, Bezier, B-spline and NURBS geometry on the accepted canvas and tree.
- Compare the accepted scene with rank/mobility, Problems and production-topology presentation.
- Reset once.

Pass when the family variety is legible and every visible state appears to share one accepted
source of truth. Record a blocker if diagnostics or profile claims appear to describe different
geometry.

### B. Explicit NURBS branches and refinement

Open **NURBS branch & knot topology**.

- Note the initial semantic span, winding and contact neighborhood.
- Run **Advance periodic span**.
- Run **Insert NURBS knot**.
- Inspect the accepted scene, branch controls and diagnostics, then reset and repeat the span
  transition.

Pass when branch movement feels explicit and predictable, the seam does not jump unexpectedly,
and knot insertion reads as a topology edit without false or stale geometry.

### C. Associative and companion operations

Open **Associative & companion operations**.

- Inspect the initial fillet and parent trims.
- Run **Split visible support**, **Mirror exact source**, then **Create linear pattern**.
- Compare canvas, tree, diagnostics and production-topology card.
- Reset before trying another order if desired.

Pass when retained source identity, associated trim ownership and generated ordinary geometry form
one coherent story. Record a blocker for unexplained replacement, stale presentation or geometry
that appears accepted before the operation completes.

### D. Production-topology trust and cancellation

Open **Production topology trust**.

- Confirm the initial card labels only independently complete output as consumable.
- Run **Add open eligible support** and confirm no consumable profile remains.
- Run **Cancel topology query** and confirm cancellation is distinct from incomplete geometry.
- Run **Recover complete topology** and compare the recovered card with the initial state.

Pass when complete, incomplete, cancelled and recovered states are unmistakable and no partial or
stale topology looks consumable.

### E. Natural exploratory pass

Spend 10–15 minutes moving naturally between the four leaves. Use reset, the right-expanding
selector, Problems, stable diagnostics and typed evidence without following every instruction.

Pass when navigation remains quick, accepted-versus-attempted truth remains clear and interaction
feels responsive enough for a desktop diagnostic workbench.

## Scorecard

Record `Pass`, `Concern` or `Blocker` for each:

| Area | Rating | Notes |
| --- | --- | --- |
| Advanced-family discoverability and accepted-state coherence |  |  |
| Periodic NURBS span/winding clarity |  |  |
| Knot/refinement topology clarity |  |  |
| Associative fillet/trim ownership |  |  |
| Split/mirror/pattern operation coherence |  |  |
| Complete versus incomplete topology trust |  |  |
| Cancellation and recovery clarity |  |  |
| Selector navigation and perceived desktop responsiveness |  |  |
| Overall advanced-workflow trust |  |  |

M61 passes only after the supervising human explicitly approves it and no unresolved wrong-branch,
misleading-profile, advanced-interaction or responsiveness blocker remains.

## Finding policy

Give each finding an `M61-F###` identifier and record:

- selected stable scenario ID;
- action sequence and whether reset reproduced it;
- expected versus observed behavior;
- rating as objective defect, clarity concern or future scope; and
- any screenshot used only as visual context.

Objective defects receive a direct owning-layer regression before a targeted human recheck.
Clarity/layout changes require rebuilding the candidate and rechecking the affected scorecard
area. A material API, schema or primary-workflow change revokes this candidate and requires full
M60 requalification.
