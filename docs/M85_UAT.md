<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 focused UAT — Responsive retained workbench presentation

Status: **pending implementation, clean qualification and immutable candidate nomination**. Do not
run this scorecard against a mutable worktree and do not record any row as accepted before exact
candidate source/tree/snapshot and served-byte evidence are added here. Accepted M84 remains Pages
authority.

## Candidate prerequisites

- Complete clean release gate and all M85 deterministic work-ledger/parity tests pass.
- Exact build output is frozen without rebuild and served from an immutable snapshot on both a
  temporary local listener and retained Tailscale endpoint.
- Local and Tailscale manifests/HTTP ledgers are byte-identical; the focused Chromium trace passes
  the fixed ordinary and visible-annotation manifold budgets.
- Both projectional/code and flat compatibility camera adapters pass direct regression coverage.
- No Pages publication or service retirement occurs before explicit supervising-user approval.

Test at approximately `1440x900` and `1024x720` on the exact retained candidate.

## Pending scorecard

| ID | Human action | Pass condition | Status |
|---|---|---|---|
| M85-U1 | Open the ordinary authored rectangle-plus-diagonal scene. Middle-button pan continuously for several seconds, reverse direction, release, then repeat with short bursts. | Motion tracks the pointer smoothly, the newest position wins, release causes no delayed jump, and geometry/selection/history are unchanged. | pending |
| M85-U2 | On the same scene, wheel zoom rapidly in and out around corners, the Origin and empty canvas space. | Zoom remains anchored under the pointer, ordered deltas are not lost, labels/hit sizes remain usable and no late camera correction is visible. | pending |
| M85-U3 | Exercise toolbar +/−, Fit and Origin after arbitrary pan/zoom. | All controls use the same camera semantics; Fit contains the sketch, Origin centres the protected datum, and no document/history entry is created. | pending |
| M85-U4 | Open the exact PC Water Manifold with annotations visible and repeat sustained pan plus wheel bursts. | The dense scene remains at least subjectively 30 fps, with no multi-hundred-millisecond stalls, blank canvas, markup flash or annotation disappearance. | pending |
| M85-U5 | Hide manifold annotations and repeat, then restore them. | The toggle remains paint/pick-only, both modes navigate smoothly, restoration is exact and hidden mode is not required to achieve the visible-mode budget. | pending |
| M85-U6 | After a large pan/zoom, immediately hover and click screws, channels, Fillets, axes, points and annotations near the pointer. | Hover and click target exactly the painted item; there is no stale-coordinate pick, first-interaction pause or visual rebase jump. | pending |
| M85-U7 | In the ordinary scene, drag a free point through several frames, release, then immediately pan and drag again. Test no-motion release and Escape cancellation. | Preview is responsive; exact release stays fixed through at least 1 s; one mutating history entry is added, while no-motion/cancel add none. | pending |
| M85-U8 | Open Compass Rose and Rounded Polyline. Repeatedly drag their code-owned controls, including rapid consecutive drags, then pan/zoom between attempts. | Coupled geometry remains attached, each terminal is deterministic, no delayed snap occurs and code/session history remains coherent. | pending |
| M85-U9 | Begin a drag or authoring preview, then initiate a camera gesture. | The semantic gesture is canceled or retained according to existing policy exactly once; repeated wheel samples do not repeatedly rebuild/cancel state and accepted geometry remains finite. | pending |
| M85-U10 | Repeat representative navigation and drag checks in a restored flat compatibility workspace/reproduction. | Presentation feel, final camera semantics and history neutrality match the projectional route. | pending |
| M85-U11 | Reload the retained candidate after camera-only motion; Copy repro before and after navigation and compare semantic content. | Camera-only motion does not mutate canonical workspace/repro authority; reload is valid and no stale global error appears. | pending |
| M85-U12 | Leave the manifold open, alternate pan, zoom, hover and selection for at least one minute. | No progressive slowdown, memory-driven blanking, stale selection, lost input, unexpected save or delayed geometry change is observed. | pending |

## Mechanical timing evidence required before UAT

For at least three warmed bursts and 120 samples per action class, record input callback p50/p95,
RAF CPU p50/p95, input-to-next-paint p50/p95/max, sustained frame rate, long tasks and actual-work
ledger counts. Ordinary navigation must meet `16.7 ms` p95/55 fps; visible manifold navigation
must meet `33.3 ms` p95/30 fps; no measured burst may contain a task over `50 ms`. Hover/drag must
meet `33.3 ms` browser p95, and exact terminal visibility/durability must meet `250 ms` ordinary or
`500 ms` code-coupled with no delayed movement.

## Approval and publication

Explicit supervising-user approval is mandatory. After approval, commit the scorecard, publish
that accepted descendant through GitHub Pages, download and exact-verify the separately rebuilt
Pages artifact and all hosted paths, then retire the retained Tailscale service. Until those steps
pass, M84 remains final public-byte authority and M85 remains open.
