<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M75 — hover and primary pointer-owner parity

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

Status: **complete (2026-08-16); accepted for scoped closure and exact-verified on GitHub Pages**.
The accepted product is the clean-qualified, byte-verified
post-F002 candidate below. the maintainer accepted that candidate, the focused F001/F002 hover recheck
and U1-U12 for scoped closure; this is not a claim that every prepared UAT step was individually
executed or logged. Exact public publication passes below. M76 subsequently reached scoped
approval, final qualification and exact public verification. Final M76 source
`a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
`248cba4509a992aeff7a02dd6d57a1a2481380a4`, passes Pages run `31961652265`, artifact
`9267811418` and deployment `5933831093`, superseding M75 and becoming current public authority
without changing any M75 evidence.

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

M75 also receives the deferred M74-U1 through M74-U8 hands-on scorecard. They were not
retroactively accepted by M74's scoped close decision. The later M75 close decision accepts those
carried items, together with M75-U9 through M75-U12, only for scoped M75 closure; it does not invent
a separate step-by-step human transcript.

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
- For uncaptured Fillet authoring only, the web adapter may enumerate the complete painted SVG stack
  to recover the exact `FeatureCorner` already resolved by the headless
  `SceneFilletHit::Radius` owner when native paint lies above it. One translation helper supplies
  the same reconciled hint to move and down. If no headless radius owner occurs in the stack, the
  top painted item remains the untrusted hint; no browser-side semantic priority is invented.
- The visible computed-radius grip, rail and spoke expose the same existing `FeatureCorner`
  identity to that translation; no pointer-active part of the authenticated radius surface may
  become an identity-free browser target.
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
  computed-radius/native paint-stack overlap, problem-forced annotations, multiple occurrences,
  exact deterministic ties, context-only corridors and every invalidation trigger.
- Run the same semantic matrix natively and under WASM. Add thin web tests only for event mapping,
  overlay ownership and headless-only painting; browser presentation is not the hit-test oracle.
- Prove independently that pointer-move is mutation-free and that hover/click agree without
  changing selection, history, accepted geometry, solver evidence or Fillet state before the
  pointer-down action itself.
- Pass formatting, warnings-denied workspace Clippy, locked all-feature tests, relevant
  native/WASM checks, unchanged golden check/clean, Trunk and the complete clean release gate.
- Freeze and byte-verify an immutable preview candidate. Run `docs/M75_UAT.md` at both supported
  desktop sizes and multiple zoom/tolerance fringes, including every deferred M74-U1 through
  M74-U8 item and accessibility review. Keep the candidate live through follow-up fixes.
- After explicit maintainer approval, deploy the exact accepted source through GitHub
  Pages and verify the hosted artifact byte-for-byte before closing M75.

Finding M75-F001 (2026-08-16): on that candidate, Fillet clicks accepted native lines and points
while uncaptured authoring pointer moves were deliberately discarded, leaving no
`geometry-hovered` target. The same omission affected ordinary relation/dimension authoring. The
replacement shares the exact authoring candidate resolution between move and down, carries a
painted computed-corner hint only as independently authenticated intent, and keeps captured radius
movement editor-owned. Focused native and WASM parity pass 11/11.

Finding M75-F002 (2026-08-16): in `fillet-workshop`, collecting point
`6600000000000000000000000000004f` and curve `66000000000000000000000000000038`
created a valid computed-radius grip overlap where a native point painted above the correct
`FeatureCorner`. The top-target-only adapter instead supplied native curve
`66000000000000000000000000000052`; an unchanged press destroyed the preview and captured no
radius gesture. The correction enumerates `elementsFromPoint` only for uncaptured Fillet
authoring, reconciles the exact headless `SceneFilletHit::Radius` owner through one helper used by
move and down, and otherwise falls back to the top painted item with no promoted owner. The
coordinator remains final authority. Demo-web passes 117/117, native/WASM M75 parity remains 11/11,
and focused Clippy, WASM, formatting, diff and unchanged-golden checks pass. Browser script
`m75_f001_browser_check.mjs` at SHA-256
`1109ad79c20534bfd7e862c07a313a78938ac062f1a49757f09ce740c5168f8e` passes 6/6 on the
provisional corrected local build.

Independent adapter review then found the same owner was absent from the pointer-active radius rail
and spoke markup, so those visible surfaces could not enter the paint reconciliation at all. The
shared radius-affordance group now carries the same `FeatureCorner` identity as its grip; the
presentation regression freezes grip, rail and spoke extraction, and the same browser run samples
the visible spoke and rail through hover/capture/release before replacement qualification.

Final public publication record (2026-08-16): documentation-only approval descendant
`f80235978fbcdccd58c45a08bccf3969a20110c9`, tree
`eb05b6496aa5c761e005a40da78d8fb96e84c16a`, deploys accepted product source
`553fd912730b1de3b39736c49b669e94cabdd2c3`, tree
`83df4efb99ca66cf0cebc0caec4515b61afd33cf`, through successful Pages run `31939764951`, artifact
`9261974799` and deployment `5929879555`. ZIP/tar SHA-256 values are
`8c031953dec4975c9b701a5ba30f060a95d5e0772286396f3c03ac74fb665fc0` and
`8ac419fbea39c306e6ee529309f2d3965c93d4ff0459fd2e21179714e9b89c1d`; the seven-file manifest
aggregate is `4c2da7d7860ac0bcadc64722007b5accb01aa999aa79f3046ba9d2868e86ef3b`. Every public file and
root exact-verifies, and M72/M74 two-size plus M75 6/6 Chromium checks pass. GitHub Pages was M75's
final public-byte authority at closure; completed M76 now supersedes it. The frozen M75 preview
snapshot remains accepted historical candidate evidence.

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
- The deferred M74 and new M75 scorecards are accepted for scoped closure on the immutable
  qualified candidate without claiming an individually logged replay. The focused F001/F002
  hover recheck and final human approval pass; exact accepted-source GitHub Pages publication and
  hosted-byte/browser verification pass.

## Non-goals

M75 does not retune hit or drafting-inference tolerances, redesign annotation placement, add a
second drawing-tool inference system, introduce grid snapping, change Fillet branches or radius
behavior, add a keyboard-driven canvas hit-test model, make mobile/tablet layout supported, release
sketch v5 or move any solver/scene authority into JavaScript.
