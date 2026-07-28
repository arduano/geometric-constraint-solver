<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M61 human UAT 3 — advanced geometry and topology

Status: remediated candidate ready for supervising-human review; approval not yet recorded

Candidate source: `1f5fd59` plus its documentation-only qualification commit.

## Candidate history

The first M61 candidate was withdrawn on 2026-07-29 after the supervising human found five
blocking gaps: fixed-only scenarios, missing representative mechanisms, third-level flyout
clipping, missing advanced-curve authoring, and no usable canvas camera. No approval from that
attempt is retained.

The replacement candidate:

- exposes ten movable public alpha fixtures with documented nonzero mobility and a preselected
  driver, including both scissor mechanisms and the Peaucellier linkage;
- routes projected selection/drag effects to the active ephemeral scenario coordinator without
  mutating or persisting the ordinary workspace;
- keeps recursive flyouts visible through every desktop nesting level;
- authors quadratic/cubic Beziers, ellipse/elliptical arc, rational quadratic conic, parabola,
  hyperbola, and clamped/periodic NURBS through the headless editor and public sketch APIs; and
- provides cursor-anchored wheel zoom, middle-drag pan, zoom buttons, scale feedback, and Fit.

No deleted playground, `/#/dev/lab`, browser E2E, CDP, or legacy UAT harness was restored.

## Entry point

The qualified shared endpoint is:

```text
http://100.94.63.83:8080/
```

For local use:

```bash
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env -u NO_COLOR trunk serve --open --release'
```

Use the top **Scenarios** selector and open **M61 Advanced geometry & topology**. Desktop branches
expand to the right on hover or keyboard focus. The subtree is:

```text
M61 Advanced geometry & topology
├── Interactive mechanisms
│   ├── Compact mechanisms
│   │   ├── drafting-compass
│   │   ├── bezier-c1-bridge
│   │   ├── twin-roller-bezier-cam
│   │   ├── tangent-orbit
│   │   ├── elliptic-trammel
│   │   ├── scotch-yoke
│   │   └── rotating-square
│   └── Linkage mechanisms
│       ├── scissor-jack
│       ├── five-stage-scissor-tower
│       └── peaucellier-linkage
└── Advanced curves & topology
    ├── advanced-all-families
    ├── nurbs-branch-topology
    ├── associative-companion-operations
    └── production-topology-trust
```

Each selection reconstructs deterministic ephemeral state. **Reset scenario** restores its exact
start and selected driver. **Exit scenario** restores the unchanged ordinary workspace. Scenario
work is never written to browser workspace persistence.

## Review scope

Allow 60–90 minutes. Objective construction, mobility, projected drag, reset, workspace isolation,
transactions, diagnostics, camera math, topology completeness, native/WASM behavior, and release
builds have direct qualification. Human review remains necessary for manipulation quality,
discoverability, branch clarity, topology trust, and perceived desktop responsiveness.

### A. Movable DOF mechanisms

Open at least these five leaves:

1. **Drafting compass · 1 DOF**
2. **Twin-roller Bezier cam · 2 DOF**
3. **Tangent orbit · 1 DOF**
4. **Scissor jack · 1 DOF**
5. **Five-stage scissor tower · 1 DOF**

For each, confirm one driver point is already selected, compare equality/bounded mobility in the
accepted diagnostics with the guide, and drag that point through several nearby targets. Use
**Reset scenario** before repeating.

Pass when dependent geometry follows solver-permitted motion, hard validity remains accepted,
branches do not flip accidentally, and reset exactly restores the start. Record a blocker if a
documented movable fixture cannot move, reports zero usable mobility, or changes the ordinary
workspace.

Then sample the trammel, Scotch yoke, rotating square, Bezier bridge, and Peaucellier linkage.
These provide representative curve-contact, ordinary-constraint, rigid-link, scissor, and exact
straight-line behavior from the retired demo without restoring its UI.

### B. Camera inspection

In the five-stage scissor tower:

- use wheel zoom at several cursor positions;
- middle-drag pan;
- use `−`, `+`, and **Fit**; and
- drag the selected base driver after changing the view.

Pass when the cursor anchor remains stable during zoom, panning follows the pointer, Fit contains
the whole tower, and camera changes do not alter selection, geometry, diagnostics, or persistence.

### C. Advanced authoring from an ordinary workspace

Exit scenario mode and start **New**. Author at least:

- one quadratic and one cubic Bezier;
- an ellipse and directed elliptical arc with edited ratio/angles/sweep;
- one rational quadratic conic with an explicit positive middle weight;
- a trimmed parabola and a chosen-branch trimmed hyperbola; and
- one clamped and one periodic NURBS.

For NURBS, set form, positive degree, optional comma-separated weights, and a gauge index whose
weight is exactly `1`; place more controls than the degree, then use **Finish**. Empty weights mean
unit weights. Invalid options or invalid terminal topology must retain the draft/document rather
than partially committing it.

Pass when staged control polygons and sampled curve previews match the committed accepted curve,
each tool commits atomically once, branch/topology options are explicit, and Undo/Cancel remain
coherent. Record a blocker for missing families, browser-owned substitute geometry, partial
invalid commits, or a curve that cannot be inspected after authoring.

### D. Explicit NURBS branches and refinement

Open **NURBS branch & knot topology**:

- note the initial semantic span, winding, side, and neighborhood;
- run **Advance periodic span**;
- run **Insert NURBS knot**; and
- inspect accepted scene, branch controls, and diagnostics before resetting.

Pass when span movement is explicit and predictable, the seam does not jump unexpectedly, and
knot insertion reads as a topology edit without false or stale geometry.

### E. Associative and companion operations

Open **Associative & companion operations**:

- inspect the initial fillet and parent trims;
- run **Split visible support**, **Mirror exact source**, and **Create linear pattern**; and
- compare canvas, tree, diagnostics, and production-topology card.

Pass when retained source identity, associated trim ownership, and generated ordinary geometry
form one coherent accepted story.

### F. Production-topology trust and cancellation

Open **Production topology trust**:

- confirm only independently complete output is labelled consumable;
- run **Add open eligible support** and confirm no consumable profile remains;
- run **Cancel topology query** and distinguish cancellation from incompleteness; and
- run **Recover complete topology**.

Pass when complete, incomplete, cancelled, and recovered states are unmistakable and no partial
or stale topology looks consumable.

### G. Natural exploratory pass

Spend 10–15 minutes moving naturally between mechanisms, authoring, advanced branch actions,
Problems, diagnostics, and typed evidence without following every instruction.

Pass when navigation stays quick through third-level flyouts, accepted-versus-attempted truth is
clear, and interaction feels responsive enough for a desktop diagnostic workbench.

## Scorecard

Record `Pass`, `Concern`, or `Blocker` for each:

| Area | Rating | Notes |
| --- | --- | --- |
| Nonzero DOF visibility and projected mechanism drag |  |  |
| Scissor jack/tower propagation and reset |  |  |
| Representative old-demo mechanism coverage |  |  |
| Third-level right-expanding selector navigation |  |  |
| Wheel zoom, middle-pan, Fit, and large-scene inspection |  |  |
| Bezier authoring and preview coherence |  |  |
| Conic authoring, trims, sweep, and branch clarity |  |  |
| Clamped/periodic NURBS authoring and gauge clarity |  |  |
| Periodic NURBS span/winding and refinement clarity |  |  |
| Associative/companion operation coherence |  |  |
| Complete/incomplete/cancelled topology trust |  |  |
| Accepted-state and ordinary-workspace isolation |  |  |
| Overall advanced-workflow trust and responsiveness |  |  |

M61 passes only after the supervising human explicitly approves it and no unresolved wrong-branch,
misleading-profile, immovable-scenario, authoring, navigation, camera, or responsiveness blocker
remains.

## Finding policy

Give each finding an `M61-F###` identifier and record:

- selected stable scenario ID or authored curve family;
- action sequence and whether reset reproduced it;
- expected versus observed behavior;
- rating as objective defect, clarity concern, or future scope; and
- any screenshot used only as visual context.

Objective defects receive a direct owning-layer regression before targeted human recheck.
Clarity/layout changes require rebuilding the candidate and rechecking the affected scorecard
area. A material API, schema, solver, or primary-workflow change revokes this candidate and
requires complete requalification.
