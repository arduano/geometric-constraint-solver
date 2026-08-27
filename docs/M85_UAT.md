<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 focused UAT — Responsive retained workbench presentation

Status: **implemented with the affected final-code native suites passing at checkpoint `fd2c560`,
but pending the clean release gate, immutable candidate nomination, frozen-byte browser rerun and
human review**.
Do not run this scorecard against a mutable worktree or an unfrozen build, and do not record any
row as accepted before exact candidate source/tree/snapshot and served-byte evidence are added
here. Accepted M84 remains Pages authority.

## Candidate identity

- Qualified source: **pending complete clean release gate**.
- Qualified tree: **pending**.
- Immutable snapshot and modes: **pending**.
- Ordered manifest aggregate: **pending**.
- Temporary local and retained Tailscale endpoints/HTTP ledgers: **pending**.
- Focused five-test frozen-byte browser timing result and log SHA-256: **pending**. A provisional
  5/5 profile exists only for an unpinned pre-F003 ancestor and is not final-source evidence.
- Final-source flat-adapter U10 native compatibility/parity evidence: **pending qualification**.

## Candidate prerequisites

- Complete clean release gate and all M85 deterministic work-ledger, receipt, parity and ordinary-
  stack tests pass.
- Exact build output is frozen without rebuild and served from an immutable snapshot on both a
  temporary local listener and retained Tailscale endpoint.
- Local and Tailscale manifests/HTTP ledgers are byte-identical; the focused Chromium trace passes
  the fixed ordinary and visible-annotation manifold budgets.
- Both projectional/code and flat compatibility camera adapters pass direct regression coverage.
- Camera RAFs report only retained presentation. Any post-burst exact reprojection is separately
  authenticated and counted; pointer preview/terminal work comes from actual interaction/code
  receipts rather than event-label inference.
- No Pages publication or service retirement occurs before explicit supervising-user approval.

Test at approximately `1440x900` and `1024x720` on the exact retained candidate.

## Pending scorecard

| ID | Human action / evidence | Pass condition | Status |
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
| M85-U10 | Review final-source native compatibility/parity evidence for the flat retained-coordinator adapter. Persisted v1-v6 workspaces normalize into projectional authority, so there is no ordinary flat browser fixture to open. | Direct adapter and shared headless tests prove retained navigation/interaction semantics, final-camera parity and history/work neutrality without inventing a test-only browser bootstrap. | pending automated evidence |
| M85-U11 | Reload the retained candidate after camera-only motion; Copy repro before and after navigation and compare semantic content. | Camera-only motion does not mutate canonical workspace/repro authority; reload is valid and no stale global error appears. | pending |
| M85-U12 | Leave the manifold open, alternate pan, zoom, hover and selection for at least one minute. | No progressive slowdown, memory-driven blanking, stale selection, lost input, unexpected save or delayed geometry change is observed. | pending |

## Mechanical timing evidence required before UAT

For at least three warmed bursts and 120 samples per action class, record input callback p50/p95,
RAF CPU p50/p95, input-to-next-paint p50/p95/max, sustained frame rate, long tasks and actual-work
ledger counts. Ordinary navigation must meet `16.7 ms` p95/55 fps; visible manifold navigation
must meet `33.3 ms` p95/30 fps; no measured burst may contain a task over `50 ms`. Hover/drag must
meet `33.3 ms` browser p95, and exact terminal visibility/durability must meet `250 ms` ordinary or
`500 ms` code-coupled with no delayed movement.

## Pre-nomination implementation evidence

Committed implementation source `fd2c560c5c61338a96f145ecb87106af49e93749`, tree
`97591f3d8c268e36db2e3e728c52163dca87d055`, passes format and diff checks; warnings-denied
all-target Clippy for `geosolve-constraint-editor`, `geosolve-sketch-code` and
`geosolve-demo-web`; `geosolve-constraint-editor` at 756/756 passed with 3 ignored; the complete
`geosolve-sketch-code` crate; and `geosolve-demo-web --lib` at 300/300 in `98.61 s`. The exact PC
Water Manifold
`m85_large_unchanged_host_overlay_is_default_stack_and_history_neutral` regression passes on the
ordinary stack in `55.45 s`; the explicit 2 MiB partial and transitive managed-deletion pair passes
2/2 in approximately `2.04 s`.

M85-F003 preserves every pre-M85 public incremental-code signature. The unaudited public path
calls the private receipt-aware worker directly instead of building a large intermediate audited
return, while six optional Fillet/Profile Offset preview and gesture states are privately boxed.
This reduces `ProjectionalEditorSession` from 35,488 to 15,296 bytes and
`MaterializedCodeProject` from 36,320 to 16,128 bytes without a public replacement API or a thread-
stack increase. The experimental boxed public returns at `d2b7d38` are superseded and are not the
candidate contract.

A provisional browser profile exits `0`, 5/5 in `2.2m`: 1,200/1,200 camera-only admissions, zero
forbidden admissions/navigation long tasks, worst completed-presentation RAF p95 approximately
`0.8 ms`, frame-gap p95 `16.8 ms`, sustained navigation `59.6–61.3 fps`, and all ordinary/Compass
Rose/Rounded Polyline drag/terminal budgets passing with no delayed movement. Its harness SHA-256
is `0248cf43dfae7d49c2f834b2b900a5cd0faa8b85209e40f77cfc5b31cede0947`, configuration SHA-256 is
`2093af3a2dad08968ebfe64c6265b931c5b51215627ba90b4d0f36f1191c0079`, and captured log SHA-256 is
`8cab5533122f07667076ce0577115a0d00da88ac64cc8d85a7a4d90683559979`. The run predates both the
experimental F003 API change and its final repair, and its exact source was not pinned; it is useful
pre-F003 ancestor evidence only and must not be cited as final-source or frozen-candidate evidence.

This evidence does not nominate UAT bytes. The clean release gate, freeze, frozen-byte browser
profile, local/Tailscale service and byte verification, final-source M85-U10 evidence, human UAT
and Pages publication remain pending; the candidate fields above stay deliberately empty.

## Approval and publication

Explicit supervising-user approval is mandatory. After approval, commit the scorecard, publish
that accepted descendant through GitHub Pages, download and exact-verify the separately rebuilt
Pages artifact and all hosted paths, then retire the retained Tailscale service. Until those steps
pass, M84 remains final public-byte authority and M85 remains open.
