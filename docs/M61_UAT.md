<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M61 human UAT 3 — advanced geometry and topology

Historical record: the approved selector paths and temporary candidate endpoint below describe the
M61 review build. M64 later flattened retained fixtures into the ordinary editable **Samples**
catalog, and the endpoint is not expected to be live.

Status: complete; explicitly approved by the supervising human on 2026-07-29 for the recorded M61
scope

Candidate source: `5140f85` for the latest `M61-F005` interaction repair, with the prior
replacement candidate and `M61-F001`-`M61-F004` repairs retained in its history, plus the
documentation-only qualification commit.

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

After the replacement candidate entered human review, `M61-F001` found that
`twin-roller-bezier-cam` omitted its public fixture's passive-roller stability target in the
workbench interaction adapter. The unconstrained independent roller could therefore move along
its own cam-contact DOF while the selected roller was dragged. Commit `1c314e9` routes the
scenario's persistent active/passive identities through a headless coordinator API that reads the
accepted passive position and constructs the transient stability target. Repeated bidirectional
drag is directly regressed; only the targeted human recheck remains.

`M61-F002` then found that first-use dynamic contact controls visually exposed the headless
point-on-circle defaults but restored an empty or obsolete select value after replacing their
option set. Apply therefore stopped in the browser adapter before dispatch. The adapter now
retains a prior value only if it remains in the current headless choices; otherwise the first
published default is selected. A direct editor regression executes the exact authored line
endpoint plus circle contact, and a presentation-policy regression covers empty, obsolete and
still-valid option values.

`M61-F003` came from a supplied persisted workspace built by stacking contacts and constraints.
Its retained design correctly rejects an ambiguous contact neighborhood, but raw browser
`pointermove` events each ran a synchronous projected solve on the WASM main thread. Expensive
samples allowed stale pointer events to accumulate and made the tab appear permanently frozen.
The adapter now keeps only the latest pending sample for each animation frame and flushes at most
that sample before pointer-up. The solver's ambiguity validation remains unchanged.

The first `M61-F003` recheck exposed separate `M61-F004`: restoring the exact graph in optimized
WASM takes about 171 ms, but the host-state sidebar also ran legacy full visual-profile analysis
on the accepted graph during every render. That duplicate analysis took about 2.3 seconds per
render even in optimized native code. The sidebar now exposes only cheap accepted geometry-role
declarations and explicitly defers consumability to the separately qualified production-topology
card.

`M61-F005` then found that the compact Tangent and Normal authoring story was misleading for
circles. Tangent already lowered to true shared contact plus tangent alignment, but the contextual
line/curve Parallel and Perpendicular paths lowered to direction at a free curve parameter. On a
full circle that parameter can move to satisfy any direction without line contact. Compact
authoring now keeps Parallel for line pairs, keeps Tangent as true generic tangency, and resolves a
line plus circle/arc Perpendicular / Normal to radial centre-on-line incidence. The public
domain-level direction relation remains available but is no longer presented as this authoring
intent.

## Historical entry point

The qualified shared endpoint at review time was:

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

For **Twin-roller Bezier cam**, drag the left roller repeatedly and confirm the right roller
remains stationary. Then select and drag the right roller and confirm the left remains stationary.
Record a blocker for passive motion, contact jumping, or pointer-event lag that makes the
interaction unusable.

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

## Recorded findings

### M61-F001 — passive twin-roller motion and drag lag

- Scenario: `twin-roller-bezier-cam`.
- Reproduction: repeatedly drag either roller along the quadratic Bezier cam.
- Expected: only the selected roller follows its independent contact DOF; the passive roller stays
  at its current accepted position.
- Original observation: the passive roller could jump along its independent contact and repeated
  synchronous solves/redraws became severely laggy.
- Classification: objective defect.
- Cause: the generic workbench projected-drag route disabled previous-state preferences but did
  not forward the `MotionCam` fixture's transient stability target.
- Resolution: `1c314e9` adds a headless stabilized-projection seam, bidirectional scenario
  active/passive metadata, and a repeated-drag regression proving the passive center remains
  within `1e-9` of its accepted position.
- Status: mechanically requalified and closed under the final scoped M61 approval.

### M61-F002 — untouched point-on-circle defaults did not dispatch

- Reproduction: author a circle and line, select a line endpoint and the circle circumference,
  leave the first published contact branch values untouched, and apply **Coincident**.
- Expected: the contextual intent resolves to a periodic point-on-curve contact and dispatches
  with the headless defaults.
- Original observation: the browser retained an empty or obsolete option value and stopped before
  dispatch.
- Resolution: dynamic selectors restore a value only while it remains in the current published
  choices; otherwise they select the first current default.
- Status: mechanically requalified and closed under the final scoped M61 approval.

### M61-F003 — pathological retained contact workspace froze on interaction

- Reproduction payload: retained revision 42/attempt 44 over accepted revision 41, containing five
  points; two circles, two lines and one quadratic Bezier; four contacts; two point-on-circle
  constraints; one line/Bezier tangency; and two driving line-length dimensions.
- Expected: the ambiguous retained attempt remains visibly rejected, while a subsequent pointer
  interaction is processed without an unbounded backlog of stale positions.
- Original observation: native replay truthfully reported
  `AmbiguousContactNeighborhood`; in the browser, every raw move synchronously triggered a solve,
  so expensive centre projections queued more moves and eventually made the tab unresponsive.
- Resolution: projected pointer moves are latest-sample coalesced to animation frames. Pointer-up
  processes at most the latest pending sample once before commit, cancellation discards it, stale
  frame callbacks are invalidated, and scheduling failure allows a later move to retry.
- Regression: the exact graph transitions from its accepted snapshot to the retained rejected
  design, asserts ambiguity, then accepts a small projected centre retry. A pure queue regression
  covers replacement, terminal drain, stale-frame invalidation and scheduling retry.
- Solver impact: none. No residual, equation, branch rule, solver policy or persisted schema
  changed; ambiguity is not converted into success.
- Status: mechanically requalified and closed under the final scoped M61 approval.

### M61-F004 — persisted workspace locked during initial and repeated render

- Reproduction: retain the `M61-F003` payload in local storage and force-refresh the workbench.
- Expected: restore and first presentation complete promptly; ordinary renders do not rerun
  expensive topology analysis.
- Original observation: the main thread remained locked after the cached workspace appeared, even
  with pointer-event coalescing active.
- Isolation: exact optimized `wasm32` restore under Node takes about 171 ms. In contrast,
  `host_state_markup` took about 2.3 seconds in optimized native code because it synchronously ran
  legacy accepted visual-profile analysis; the qualified production-topology query itself took
  only microseconds for this graph.
- Resolution: remove full visual-profile analysis from ordinary host-state rendering. The card now
  lists accepted profile/construction role declarations, clearly states that these do not prove
  consumability, and leaves that decision to **Production topology**.
- Measured result: exact native host-state generation falls from about 2.3 seconds to about
  0.12 ms. Solver, accepted geometry and production-topology semantics are unchanged.
- Status: mechanically requalified and closed under the final scoped M61 approval.

### M61-F005 — circle tangent and normal authoring was direction-vacuous

- Reproduction: author a line and circle, then compare the UI-facing Tangent and
  Perpendicular / Normal actions.
- Expected: Tangent establishes shared line/circle contact and aligned tangent directions; Normal
  makes the line radial by constraining it through the circle or circular-arc centre.
- Original observation: direction-only line/curve dispatch exposed a movable curve contact. On a
  full circle the contact could move to whichever parameter has the requested direction, so the
  result neither established line contact nor clearly represented a radial normal.
- Resolution: Tangent remains `CurveCurveTangency`; circle/arc Normal lowers atomically to
  centre-on-line `PointOnCurve`; Parallel is line-pair only; arbitrary nonlinear direction-only
  Parallel/Perpendicular is disabled in compact authoring.
- Regression: the focused editor test evaluates tangent contact positions and derivatives and
  evaluates the radial line contact against the persistent circle centre. The reusable
  `circle-tangent-normal` scenario records verification point P29 and explains both meanings.
- Solver impact: none. Existing validated residuals are reused; no equation, schema, tolerance, or
  branch inference changed.
- Status: mechanically requalified and closed under the final scoped M61 approval.

## Scorecard

Record `Pass`, `Concern`, or `Blocker` for each:

| Area | Rating | Notes |
| --- | --- | --- |
| Nonzero DOF visibility and projected mechanism drag | Pass | Approved for recorded M61 scope. |
| Scissor jack/tower propagation and reset | Pass | Approved for recorded M61 scope. |
| Representative old-demo mechanism coverage | Pass | Approved for recorded M61 scope. |
| Third-level right-expanding selector navigation | Pass | Approved for recorded M61 scope. |
| Wheel zoom, middle-pan, Fit, and large-scene inspection | Pass | Approved for recorded M61 scope. |
| Bezier authoring and preview coherence | Pass | Approved for recorded M61 scope. |
| Conic authoring, trims, sweep, and branch clarity | Pass | Approved for recorded M61 scope. |
| Clamped/periodic NURBS authoring and gauge clarity | Pass | Approved for recorded M61 scope. |
| Circle tangent and radial-normal authoring clarity | Pass | Approved for recorded M61 scope. |
| Periodic NURBS span/winding and refinement clarity | Pass | Approved for recorded M61 scope. |
| Associative/companion operation coherence | Pass | Approved for recorded M61 scope. |
| Complete/incomplete/cancelled topology trust | Pass | Approved for recorded M61 scope. |
| Accepted-state and ordinary-workspace isolation | Pass | Approved for recorded M61 scope. |
| Overall advanced-workflow trust and responsiveness | Pass | Approved for recorded M61 scope. |

Approval record: on 2026-07-29 the supervising human explicitly closed M61 as approved for the
scope captured by this scorecard and findings ledger. Future UI improvements and cleanup are new
milestone scope, not amendments to M61.

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
