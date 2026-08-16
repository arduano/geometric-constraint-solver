<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 — hover and primary pointer-owner parity

Status: **active; the initial immutable candidate was withdrawn after confirmed finding M75-F001;
the correction is implemented and replacement qualification is pending as of 2026-08-16**. This
document freezes the amended approved contract. Human review and the public M75 artifact remain
pending.

## Goal

Make Select hover a truthful preview of the primary semantic owner that the same pointer sample
would receive on pointer-down. The presentation-independent editor owns candidate construction,
precedence, deterministic annotation occurrence choice, related-context reveal and invalidation.
The browser only maps input and paints the returned state.

When an ordinary relation/dimension tool or grouped Fillet authoring owns the canvas, hover must
instead preview the exact compatible native operand that an unchanged press would consume. A
current painted Fillet preview arc may preview its independently validated radius owner. An
inapplicable nearer item yields to the same applicable fallback as click rather than suppressing
feedback or publishing unrelated Select hover.

M75 also receives the deferred M74-U1 through M74-U8 hands-on scorecard. Those items remain
unexecuted and are not retroactively accepted by M74's scoped close decision.

## Accepted work

### M75-R001 — one Select primary-owner order

- Use one headless resolution path for Select pointer-move prediction and primary pointer-down
  ownership under the same tool, accepted scene, viewport, visibility, problem and pointer input.
- Resolve the first applicable candidate in this exact order:
  1. Fillet radius surface or grip;
  2. draggable geometry: stored points and semantic centers;
  3. visible constraint or dimension annotation occurrence;
  4. other native or computed geometry;
  5. intrinsic datum;
  6. no primary target.
- Return the existing semantic owner that pointer-down will consume. Selection modifiers may alter
  membership after that resolution but do not create another hit order.
- Preserve all established hit tolerances and native/computed/Profile/Construction role ordering
  inside their existing priority class. M75 reuses those candidates; it does not retune them.

### M75-R002 — visible annotation parity and deterministic ties

- Include every currently visible annotation occurrence in the shared resolver, including an
  occurrence made visible only by current problem state. Removing the problem-owned visibility
  removes hit eligibility with the next authenticated scene.
- Resolve crowded annotation occurrences by finite screen distance, then stable semantic item
  identity, then occurrence identity. Candidate insertion order, browser paint order and hash-map
  order may not affect the result.
- Keep annotation visibility, placement, fan-out, leader geometry and tolerance policy unchanged.
  M75 aligns hover with the already-visible semantic surface; it does not redesign annotations.

### M75-R003 — context is not a primary target

- Preserve contextual geometry/annotation corridors that reveal related annotations or operands.
  When a sample lies only in such a corridor, publish that context with primary target `None`.
- A corridor-only reveal cannot promise an annotation or geometry click owner. Pointer-down uses
  the same primary result and therefore cannot select an item solely because context is visible.
- Moving from a corridor into a real hit envelope may add a primary target without discarding
  valid related context.

### M75-R004 — state lifecycle and browser authority

- Revoke stale hover on active-tool changes, selection/annotation-visibility changes, camera
  changes, accepted-scene replacement and overlay/dialog/input-ownership changes. Existing
  pointer-leave, visibility and cancellation paths remain coherent with the same rule.
- Suppress unrelated Select resolution while ordinary or computed-feature authoring owns
  uncaptured canvas input. Route that movement to the active authoring resolver so only the exact
  compatible next operand is highlighted. An already captured Fillet-radius gesture continues to
  receive its matching movement and terminal samples.
- Returning to Select or canvas ownership does not resurrect a revoked result; a new valid mapped
  pointer sample is required.
- Paint hover, related operands and contextual annotations in the browser only from current
  headless state. DOM/SVG event targets, CSS `:hover`, painted stroke order, local distance checks
  or browser caches may not choose or retain a second semantic owner.

### M75-R005 — authoring hover predicts the next accepted operand

- Ordinary constraint and dimension authoring hover and click share one ordered, bounded native
  candidate resolver. Point, curve, datum and other operand eligibility remains tool/stage owned.
- Grouped Fillet native point/corner expansion and curve collection use the same shared resolver
  for hover and click, including duplicate/inapplicable overlap fallback and empty-canvas clearing.
- A painted current-preview `FeatureCorner` is only an intent hint. Both hover and pointer-down
  authenticate the complete candidate, retained preview, accepted/design/computed scene stamps,
  geometry policy and exact radius hit before publishing or consuming it. A stale/spoofed hint
  fails closed and never falls through to an underlying native operand.
- Authoring hover changes only the existing editor hover DTO/effect. It does not mutate authoring
  state, preview candidate/snapshot, selection, gesture, history/transcript or accepted geometry.

### M75-C001 — compatibility boundary

- Limit additive public API changes to problem-aware Select and domain-aware authoring pointer-move
  wrappers that consume and return existing scene/selection/input DTOs. Existing pointer-leave,
  cancellation and retained-state paths revoke host-side camera, scene and input-owner context.
  Shared candidate helpers and ordering remain private.
- Add no solver equation, residual, constraint or dimension family, branch policy, rank/DOF rule,
  persistence field/version or supported draft-v5 claim.
- Preserve canonical sketch v1-v4 bytes, unsupported draft-v5 handling, current action semantics,
  all hit tolerances and the reviewed authoring/scene golden bytes.

### M75-Q001 — qualification and publication

- Add focused editor regressions for every adjacent Select priority edge, ordinary/Fillet
  authoring point and curve operands, inapplicable overlap fallback, computed-radius ownership,
  problem-forced annotations, multiple occurrences, exact deterministic ties, context-only
  corridors and every invalidation trigger.
- Run the same semantic matrix natively and under WASM. Add thin web tests only for event mapping,
  overlay ownership and headless-only painting; browser presentation is not the hit-test oracle.
- Prove independently that pointer-move is mutation-free and that hover/click agree without
  changing selection, history, accepted geometry, solver evidence or Fillet state before the
  pointer-down action itself.
- Pass formatting, warnings-denied workspace Clippy, locked all-feature tests, relevant
  native/WASM checks, unchanged golden check/clean, Trunk and the complete clean release gate.
- Freeze and byte-verify an immutable Tailscale candidate. Run `docs/M75_UAT.md` at both supported
  desktop sizes and multiple zoom/tolerance fringes, including every deferred M74-U1 through
  M74-U8 item and accessibility review. Keep the candidate live through follow-up fixes.
- After explicit supervising-human approval, deploy the exact accepted source through GitHub
  Pages and verify the hosted artifact byte-for-byte before closing M75.

Initial mechanical nomination record (2026-08-16; withdrawn for hands-on use by M75-F001): exact
clean product source
`f3affff1b62b1cb484a59647c4072c94c3b12ada`, tree
`7662abc8b7c71130f54fbf2745afa60f0d286431`, passes the complete release gate, including the
unchanged 270-row golden oracle and native/WASM M75 parity. The gate-produced distribution was
copied without rebuilding to seven-file read-only snapshot `/tmp/geosolve-m75-uat.hUSaG7`, whose
C-locale ordered-manifest aggregate is
`69425a504453eda6645c96b6163b5b899ab455f40828f3cdecc73b90ff3c41d9`. PID `3801058` serves it at
`http://100.94.63.83:8080/`; direct byte/media verification and both retained two-size Chromium
checks pass. This record satisfies only the mechanical portion of M75-Q001. M74-U1 through M74-U8,
M75-U9 through M75-U12, final human approval and GitHub Pages publication remain open.

Finding M75-F001 (2026-08-16): on that candidate, Fillet clicks accepted native lines and points
while uncaptured authoring pointer moves were deliberately discarded, leaving no
`geometry-hovered` target. The same omission affected ordinary relation/dimension authoring. The
replacement shares the exact authoring candidate resolution between move and down, carries a
painted computed-corner hint only as independently authenticated intent, and keeps captured radius
movement editor-owned. Focused native and WASM parity now pass 11/11; a new immutable candidate is
required before human UAT resumes.

## Acceptance

- Hover predicts exactly one primary pointer-down owner under the shared order, or truthfully
  reports none.
- Problem-forced annotations, crowded occurrence ties and context-only corridors obey the exact
  headless rules above natively, under WASM and through the thin browser adapter.
- Stale hover cannot survive a change to its owning tool, selection/visibility, camera, scene or
  overlay/input context; browser paint has no independent semantic hover authority.
- Active relation/dimension and Fillet authoring hover predicts the exact compatible operand or
  preview-radius owner that unchanged pointer-down will consume, including fallback and no-target
  cases, without mutating retained state.
- Existing tolerances, role order, schemas, equations, solver/golden behavior and public semantics
  outside the additive pointer-move wrappers remain unchanged.
- The complete deferred M74 and new M75 human scorecards pass on an immutable qualified candidate,
  then the accepted source is exact-verified on GitHub Pages.

## Non-goals

M75 does not retune hit or drafting-inference tolerances, redesign annotation placement, add a
second drawing-tool inference system, introduce grid snapping, change Fillet branches or radius
behavior, add a keyboard-driven canvas hit-test model, make mobile/tablet layout supported, release
sketch v5 or move any solver/scene authority into JavaScript.
