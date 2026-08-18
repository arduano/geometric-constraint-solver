<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M79 — stable inference candidate cycling and recovery

Status: **implementation and pre-nomination qualification complete; clean release qualification
and human UAT pending**. M79 hardens the lifecycle of the existing M70/M71 drafting candidates
without adding a primitive, constraint family or solver equation.

## Product contract

- Tab cycles the complete deterministic candidate list for one exact stationary inference frame
  and wraps indefinitely. Every ID advertised by that cohort remains selectable until the pointer,
  authoring modifiers, tool/stage, accepted scene or viewport context changes.
- An explicit Tab choice changes only the selected candidate and its guides. It does not acquire or
  replace automatic point, midpoint, curve, datum, direction, concentric or tracking hysteresis.
- A preference from another frame remains fail-closed and cannot commit a different candidate.
  Stale output is not itself a replacement cohort; an ordinary hover may clear it and resolve once
  without a preference.
- The browser owns physical Tab/pointer/modifier translation and animation-frame ordering only.
  Candidate order, identity, applicability and stale authority remain headless Rust behavior.
- A candidate choice is retired on genuine pointer movement, pointer identity or modifier change,
  blur/leave, tool or stage change, Escape/Backspace, Undo/Redo, camera or accepted-scene change,
  import/reload or loss of drafting ownership. Pointer-down may consume one exact stationary choice
  once and then retires it.
- Before Tab reads a candidate list, the newest coalesced pointer sample is resolved. An ID from an
  earlier coordinate is never applied to a later queued coordinate.

## Redundant mixed-candidate precedence

An authenticated construction first trials the exact visible candidate. If independent accepted-
state redundancy evidence proves that every redundant auto source is a **fully** redundant
direction belonging to a bundle with stronger positional intent, the coordinator removes exactly
those directions and retries once from the original state. The final effective plan must pass the
ordinary solve, finite output, independent hard-residual and redundancy checks before one
publication/history entry.

Strong positional intent is point identity, origin/datum attachment, midpoint, point-on-curve,
semantic centre or a complete two-axis tracking intersection. A lone horizontal/vertical point or
midpoint guide is not strong positional intent: M73's established rule that the authored line's
own Horizontal/Vertical relation wins remains unchanged. Partial redundancy, a redundant
positional source, a direction-only candidate or a failed retry still rejects transactionally.

The original tokenized effect remains the acknowledgement authority; the pruned independently
validated plan is the replay/transcript truth. Direct public `apply_construction_plan` calls retain
their exact generic redundancy rejection.

## Confirmed findings

- **M79-F001 — stationary candidate cohort churn.** Selecting one advertised candidate changed
  anchor/direction/datum/concentric/tracking latches, filtering other candidates out at the same
  coordinate and eventually producing `StalePreferredCandidate`.
- **M79-F002 — browser preference poisoning.** The adapter retained an expired ID across movement,
  modifiers and lifecycle changes, including contexts that did not own geometry drafting.
- **M79-F003 — queued-move/Tab race.** Tab read the prior published resolution before draining the
  latest animation-frame pointer sample, then applied the old ID at the newer coordinate.
- **M79-F004 — unpublishable ranked default.** In the exact centre-rectangle/right-edge-midpoint
  scenario, `Midpoint + Horizontal` ranked first but retained publication rejected its already-
  implied Horizontal source. Midpoint alone published successfully.

The independent public-boundary reproduction used source
`077b428effb18958928531cd27c284b513f845fa`. Retained log
`/tmp/m79_exact_repro.log` has SHA-256
`0a898a60b62a229d5ddfa1917c8b9bef3151b3ede18e107d76dd0f9e95d1fdf2`.

## Acceptance gate

- Focused inference-engine tests cycle equal and ranked alternatives through two wraps with stable
  IDs/order/guides and prove genuine stale preferences remain noncommittable.
- A public coordinator regression creates a centre rectangle at Origin, starts a Midpoint Line at
  Origin and targets the right-edge midpoint. Cycling is state-neutral, and the default commit
  retains Midpoint while omitting only the proven-redundant Horizontal source.
- Thin demo tests cover queued movement before Tab, exact stationary-choice invalidation,
  modifiers, blur/leave, tool/history/camera/context transitions, non-owner samples and one-shot
  pointer-down forwarding.
- Focused native/WASM parity, demo tests, formatting, warnings-denied Clippy, workspace tests, the
  unchanged golden survey/check/clean modes and the complete clean release gate pass before an
  immutable no-rebuild Tailscale candidate is nominated.
- Human UAT explicitly accepts the frozen candidate before GitHub Pages publication and milestone
  closure.

M79 adds no solver residual, Jacobian, persistence version, browser geometry policy, mobile work,
new candidate family, weighted priority substitute or broad golden expansion.
