<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 focused UAT — Responsive retained workbench presentation

Status: **mechanically qualified and nominated on exact immutable bytes; hands-on
M85-U1-U9/U11-U12 and supervising-user approval remain pending**. Automated native M85-U10 passes.
Run the remaining scorecard only against the retained frozen candidate below. Accepted M84 remains
Pages authority; M85 has not been published.

## Candidate identity

- Qualified source: `5c265e211e20dabc8a27f6402d530f5d645ff15c`.
- Qualified tree: `b55d012443f4dbf7551e30041da2912e666de9db`.
- Clean release gate: exit `0`, 6,993 lines and 456,580 bytes, from 06:40:49 through 07:13:16 AEST
  on 2026-08-28. Log `/tmp/geosolve-m85-gate.8vK5wDu4/release-gate.log` has SHA-256
  `0b09720dfd4491575ab10bd3baba2f8f6e7fae9e8e90954ff0026a64de4458eb`.
- Immutable snapshot: `/tmp/geosolve-m85-uat.QX8fU3Q6`; directories/files `0555`/`0444`, exactly
  seven regular files and zero symlinks.
- Ordered-manifest aggregate:
  `dc729ce5fa28929dba2aa086e49246aac7d5173a0583d3b0b64748ffbbb7fda5`.
- Complete freeze/nomination evidence: `/tmp/geosolve-m85-freeze-evidence.uj9HviX1`.
- Local endpoint: `http://127.0.0.1:18100/`, PID `2008536`, invocation
  `d9fdcccfa9ce46deafcf46f7b6148e6e`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`, PID `2008538`, invocation
  `2c70402e2b2a45a5810ea29722f25f95`.
- Both endpoints serve the same snapshot. Their complete eight-path HTTP ledgers are byte-identical
  at SHA-256 `305eccfc8fa60786aabfae59edd612e695ce3c15b7224abbf3be0d0852ae0d27`.
- Focused frozen-byte browser timing: 5/5 in `2.2m`; log SHA-256
  `d1174515c320e1f3a0e006bbaad47cc47ba0aeda52fe9bf05c956d91750951c7`; summary SHA-256
  `db508a28f46774fbc74f9bdebfc20c513c46934bfd29a7b0775d96962caa63e6`.
- Final-source flat-adapter U10 native compatibility/parity evidence: 19/19 exact tests pass at
  `/tmp/geosolve-m85-u10-final.D1auvz5d`; command/result/manifest SHA-256 values are
  `5da8bf46936228d22034f4195e9e571f6f0227510db257f943ef27686dee6545`,
  `5efaa7d874d1c07189ab8ec7398863945abd1d15bb50933e55f47bac888f7f79` and
  `ec2b205710bf6d79c09e696fb6023b01ebcac1f922bcae04c2e8486c80702189`.

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
| M85-U10 | Review final-source native compatibility/parity evidence for the flat retained-coordinator adapter. Persisted v1-v6 workspaces normalize into projectional authority, so there is no ordinary flat browser fixture to open. | Direct adapter and shared headless tests prove retained navigation/interaction semantics, final-camera parity and history/work neutrality without inventing a test-only browser bootstrap. | automated pass: 19/19 exact tests |
| M85-U11 | Reload the retained candidate after camera-only motion; Copy repro before and after navigation and compare semantic content. | Camera-only motion does not mutate canonical workspace/repro authority; reload is valid and no stale global error appears. | pending |
| M85-U12 | Leave the manifold open, alternate pan, zoom, hover and selection for at least one minute. | No progressive slowdown, memory-driven blanking, stale selection, lost input, unexpected save or delayed geometry change is observed. | pending |

## Mechanical timing evidence required before UAT

For at least three warmed bursts and 120 samples per action class, record input callback p50/p95,
RAF CPU p50/p95, input-to-next-paint p50/p95/max, sustained frame rate, long tasks and actual-work
ledger counts. Ordinary navigation must meet `16.7 ms` p95/55 fps; visible manifold navigation
must meet `33.3 ms` p95/30 fps; no measured burst may contain a task over `50 ms`. Hover/drag must
meet `33.3 ms` browser p95, and exact terminal visibility/durability must meet `250 ms` ordinary or
`500 ms` code-coupled with no delayed movement.

The final frozen-byte profile satisfies these prerequisites. Across 30 navigation bursts and 1,200
camera samples, all 1,200 admissions are camera-only, with zero forbidden admissions and zero
navigation long tasks. Worst callback/camera-RAF/completed-presentation p95 is
`0.2`/`0.6`/`0.8 ms`; worst frame-gap p95 is `16.8 ms`; minimum sustained rate is
`59.8249 fps`; and worst hover p95 is `3.5 ms`. Ordinary, Compass Rose and Rounded Polyline drag
preview p95/terminal maximum is respectively `5.4`/`87.23 ms`, `8.7`/`175.50 ms` and
`6.6`/`184.93 ms`. Every measured drag has preview/terminal parity, no delayed movement and one
save.

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

That provisional profile remains historical and does not own nomination. The exact clean gate,
immutable freeze, local/Tailscale byte verification, final frozen-byte profile and final-source
M85-U10 evidence recorded above do nominate the current UAT bytes. Hands-on M85-U1-U9/U11-U12,
explicit supervising-user approval and Pages publication remain pending.

## Approval and publication

Explicit supervising-user approval is mandatory. After approval, commit the scorecard, publish
that accepted descendant through GitHub Pages, download and exact-verify the separately rebuilt
Pages artifact and all hosted paths, then retire the retained Tailscale service. Until those steps
pass, M84 remains final public-byte authority and M85 remains open.
