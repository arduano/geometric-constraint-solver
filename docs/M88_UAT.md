<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 UAT — Workflow-led authoring workbench redesign

Status: **accepted and closed on 2026-09-01**. The immutable M88-F004 React replacement remains
live as the accepted UAT identity. The supervising user's milestone-level close decision accepts
M88-U1 through M88-U10 without claiming or inventing a separately logged row-by-row replay.

Historical rollback only: `/tmp/geosolve-m88-uat.nmhcRj`, preserved after reboot with its former
transient `http://100.94.63.83:8080/` listener gone, seven-file aggregate
`6c4af7e96e30687654d691b577954c22b051fb3ad37e9472c51e16c9c002a274`, Chrome
`151.0.7922.173`. Its automated browser preflight and five-run Gridfinity budget passed for the
pre-React Rust-DOM workbench. Do not use that endpoint as evidence for, or overwrite it with, the
React replacement.

React replacement: the initial full mechanical gate passed, M88-F001 received proportional
replacement qualification, M88-F002 received toolbar-boundary and visual qualification, and the
M88-F003 point-and-click and M88-F004 canvas-chrome corrections are proportionally qualified.
The frozen F004 candidate counts are bridge `23/23`, full demo-web `395/395`, frontend `26/26` and
real release-WASM Playwright `10/10`; focused all-feature
library Clippy, `npm run check`, distribution validation, formatting and the locked WASM check pass.
After acceptance, retiring the obsolete Rust-DOM compatibility host reduced the current demo-web
suite to `344/344`; final-source actual-WASM `2/2`, frontend `26/26`, release-WASM Playwright
`10/10`, the unchanged 271-row golden and the complete release gate pass separately. The accepted
F004 snapshot was not rebuilt or replaced.
The later explicit close decision supplies the milestone-level disposition below; it does not turn
those mechanical checks into fabricated individual human observations.

## Confirmed UAT finding M88-F001

The initial candidate deterministically exposed `IntentBootstrapMetadata` twice because the React
bridge serialized `IntentGraphNodeKind` with Rust `Debug` into Explorer and Inspector. The bridge
now reuses the semantic `node_family_label`; Explorer removes the type/detail suffix entirely,
Inspector retains one compact semantic kind, and Explorer row/group icons remain non-shrinking.
The exact bridge regression failed before the fix and passes after it. Real-WASM E2E rejects
`IntentBootstrapMetadata` and `Bootstrap {` and asserts the row/icon/Inspector contract.

## Confirmed UAT finding M88-F002

The next UAT finding was that the migrated rail had lost recognizable CAD icons and semantic
families, while a scissors-like miscellaneous bucket combined Fillet/Offset with unrelated display
actions. The repaired rail is Select, Sketch, Constraint, Dimension and Modify. Its exact inventory
is 25/13/5/2, Modify contains only Fillet and Offset, Profile/Construction is contextual, and
Grid/Fit/Origin are canvas-local. The icon/catalog data is one bounded static Rust read and does not
enter hot snapshots. View/role commands preserve the active authoring tool.

The final browser suite and two-size visual pass cover exact inventory/headings/icons, long labels,
bounded scroll, active category state, 12 px canvas-HUD insets, outside dismissal/destination click
delivery and Escape focus restoration. The only test correction was making the `Sketch` locator
exact so it did not also match `Fit sketch`; no product defect was involved.

The reboot removed all former transient HTTP units, while their immutable snapshots survived. The
withdrawn F002 snapshot remains preserved below. Its service has now been replaced only after the
corrected candidate was frozen and exact-verified; no partial earlier-candidate observation is
promoted to a scorecard pass.

## Confirmed UAT finding M88-F003

Ordinary point-and-click geometry authoring was independently reproduced as broken in frozen F002
snapshot `/tmp/geosolve-m88-react-uat.kdSCUU`, whose release-WASM SHA-256 is
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0` and source basis is
`71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`. Direct Rust bridge clicks already committed a Segment
and Center–Radius Circle. The React viewport instead canceled the staged interaction when the
browser emitted its normal `lostpointercapture` after `pointerup`; bridge cancellation also dropped
`ClearConstructionPreview` effects and could leave stale draft paint.

The repair retires the exact frontend pointer synchronously on `pointerup` before awaiting native
dispatch. Its expected subsequent capture-loss event is stale and does nothing; a genuine
`pointercancel` or unexpected capture loss cancels exactly once. The bridge now dispatches all
cancellation effects. Live provisional Fillet and Profile Offset drags cancel through their owning
restoration routes, and cleanup acknowledgements do not become false Problems. React normal-release/
genuine-loss coverage, direct Segment/Circle click-click parity, staged and live-drag cancellation,
and real release-WASM click-click authoring all pass.

## Confirmed UAT finding M88-F004

F003 UAT then exposed an unconditional Profile/Construction control, an action-error row that moved
the canvas vertically, and an enabled Finish button before Polyline had any publishable draft. The
repair makes these projections of retained authority: roles are hidden when irrelevant and label
new versus selected curves; errors are fixed overlays; Rust publishes exact Finish and Undo/Redo
readiness; and Enter shares Finish's canvas-scoped gate. Polyline readiness is false immediately,
false after one point, true after two points and false after completion. Computed tools additionally
require preview/candidate identity. Premature Finish is history/revision/project neutral.

The adjacent audit removes the same false affordance from one-shot preselection, camera/first-Escape
gesture cancellation, Code-only canvas tools, shortcut labels, dirty-draft project replacement and
exact reproduction, secondary-click routing, panel glyphs and trace loading. Ordinary/outer-code
history and local/Rust dirty-source blocking are exact. No solver or accepted-scene mathematics
changed.

## Current immutable M88-F004 replacement identity

- Frozen path: `/tmp/geosolve-m88-react-uat.KGhA7s`.
- Freeze/service start: `2026-09-01 00:55:59 AEST`.
- Tailscale endpoint: `http://100.94.63.83:18088/`.
- User service/PID: `geosolve-m88-react-uat-current.service` / `1021511`.
- Server log: `/tmp/geosolve-m88-react-uat.KGhA7s.http.log`.
- External sorted manifest: `/tmp/geosolve-m88-react-uat.KGhA7s.sha256`.
- Ordered file aggregate: `700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`.
- WASM SHA-256: `22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`.
- Evidence: `/tmp/geosolve-m88-f004-freeze-evidence.ijud9T`; staging/live HTTP ledger SHA-256
  `e838461921907cfe1252423f5a9ccf7db056dd62370698d90d6e47e0e7428531`.
- Contents/modes: exactly eight regular files, zero symlinks, directories `0555`, files `0444`.
- Build/browser identity: Cargo `--release`, wasm-bindgen and `wasm-opt -Oz`; Google Chrome
  `151.0.7922.173`. No debug candidate is nominated.

Build, frozen, Tailscale staging and Tailscale live manifests match exactly. Root, `/index.html` and
all eight files return HTTP 200. The live Chrome smoke proves right-click neutrality, exact
Polyline Finish transitions, one accepted curve, no canvas rail in Code mode and no page, console,
network or HTTP error. This endpoint is the accepted M88 identity and remains live. No public
deployment or service retirement was inferred from the close decision.

## Superseded immutable M88-F003 identity

- Frozen path: `/tmp/geosolve-m88-react-uat.QkVU1k`.
- Freeze/service start: `2026-08-31 23:02:01 AEST`.
- Tailscale endpoint: `http://100.94.63.83:18088/`.
- User service/PID: `geosolve-m88-react-uat-current.service` / `518679`.
- Server log: `/tmp/geosolve-m88-react-uat.QkVU1k.http.log`.
- External sorted manifest: `/tmp/geosolve-m88-react-uat.QkVU1k.sha256`.
- Ordered file aggregate: `e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167`.
- WASM SHA-256: `5f49f49a880dd8529982bfbcf68a3f1a92c79ee96f256ed56931254ca884b22a`.
- Evidence: `/tmp/geosolve-m88-f003-freeze-evidence.bbtXoS`; local and Tailscale HTTP ledger
  SHA-256 `340b5bfd3d7d661b877fad3d3ab97813bc9027f95a99962b6788781c68588ef5`.
- Contents/modes: exactly eight regular files, zero symlinks, directories `0555`, files `0444`.
- Build/browser identity: Cargo `--release`, wasm-bindgen and `wasm-opt -Oz`; Google Chrome
  `151.0.7922.173`. No debug candidate is nominated.

Final dist, frozen bytes, local HTTP and Tailscale HTTP match exactly. Root, `/index.html` and all
eight files return HTTP 200. Local and frozen-endpoint Chrome smokes commit an ordinary click-click
Segment and Center–Radius Circle, clear draft paint and report no runtime, console, network or HTTP
error. F004 now supersedes these bytes for continuing UAT; the snapshot remains preserved.

## Withdrawn M88-F002 reproduction identity

- Frozen path: `/tmp/geosolve-m88-react-uat.kdSCUU`.
- Freeze/start: `2026-08-31 21:18:19 AEST`.
- Tailscale endpoint: `http://100.94.63.83:18088/`.
- User service/PID: `geosolve-m88-react-uat-current.service` / `139684`.
- Server log: `/tmp/geosolve-m88-react-uat.kdSCUU.http.log`.
- External sorted manifest: `/tmp/geosolve-m88-react-uat.kdSCUU.sha256`.
- Ordered file aggregate: `d328fdded4ae963230eef2c64c5fb22459dec7a55467e0a7c051bed739b2cd47`.
- WASM SHA-256: `7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`.
- Exact verification evidence: `/tmp/geosolve-m88-react-freeze-evidence.sUELzR`; eight-path
  results SHA-256 `c5201b123ba2f06bcb9b0f6c370adfa2a9dc85c0caa750366d8b36ff33189726`.
- Contents/modes: exactly eight regular files, zero symlinks, directories `0555`, files `0444`.
- Build/browser identity: release WASM produced by Cargo `--release`, wasm-bindgen and
  `wasm-opt -Oz`; Google Chrome `151.0.7922.173`. No debug candidate is nominated.

At the F002 freeze, its then-final `dist`, frozen files and served files matched exactly. Every file
plus `/` and `/index.html` returned HTTP 200 with exact frozen bytes, and a frozen-endpoint Chrome
toolbar smoke passed before F003. PID `139684` was retired only after the corrected snapshot above
passed local/frozen verification; the F002 snapshot itself remains preserved as reproduction
evidence. Port `18088` now serves only the corrected identity above.

## Superseded React candidates — preserved, listeners gone after reboot

Snapshot `/tmp/geosolve-m88-react-uat.TAMXyz`, endpoint
`http://100.94.63.83:18088/`, service/PID
`geosolve-m88-react-uat-TAMXyz.service` / `1846522`, aggregate
`91c3a2349f1466a64720cb1cfba8a4f7d18aed0be03e9eec256ccf8c4eb0f9de` and WASM SHA-256
`9032a07bc6ac465816b3b3f3bb44c3881f815f290e39eece88890589f2cdfe5f` remain exact historical
evidence only. Do not use it for continuing UAT. Port `8080` and rollback PID `425555` are
historical records; those transient listeners no longer exist after reboot. M88-F001 snapshot
`/tmp/geosolve-m88-react-uat.nGkL4i`, aggregate
`0bd35f3dba50c592c6eea34ed18afed1b6f908803d75feb9ca3bbb620a1572c4`, WASM SHA-256
`beb76d47889055f9d8344ac4cdc8a01fc06a1dbd797979f636927031d3383996`, likewise remains preserved
but is not served and must not receive continuing UAT.

## Test conditions

The scorecard below records the accepted contract. Its status language is intentionally precise:
the supervising user accepted the milestone as a whole on 2026-09-01, but did not provide or ask us
to invent a separately logged observation for each row.

| Row | Exercise | Pass condition | Status |
|---|---|---|---|
| M88-U1 | Open the application with no prior workspace. Use the start/open surface to create a native sketch, Start from code, find one native and one code sample by typing, import a project/repro and inspect recents. | Every entry is reachable within two deliberate actions. All 25 native and 12 code samples are keyboard-searchable without hover. The opened project title and accepted/dirty/failed state agree in one app-bar location. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U2 | At 1440x900 switch Design -> Split -> Code, resize both Split panes by pointer and keyboard, reset them, reload, then return to Design. | Split provides at least 520x500 px of editable source. The canvas and editor remain usable, sizes reset predictably and bounded presentation preferences reload without entering project/repro authority. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U3 | At 1024x720 open Typed Panel directly in Code mode, switch files and secondary Code surfaces, then return through Split and Design. | Code never disappears and provides at least `720 x 500` CSS px of usable editor. Text is at least 12 px, stays inside its parent and has no fixed 27 rem height cap. Parameters and Problems retain one logical state while rehosting; no duplicate copy diverges. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U4 | Put the caret in `sketch.ts`, select text, scroll deeply and create an unapplied dirty draft. Select several canvas items, resize panes and switch Design/Split/Code repeatedly. | Selected file, cursor, text selection, scroll and dirty draft remain exact. Sticky Apply/Revert/status remains visible whenever relevant; no durable render replaces the active editor state. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U5 | Observe the presentation work ledger while resizing/collapsing panes, switching layout and changing right/Explorer tabs. Repeat while an accepted dense scene is visible. | Every presentation-only action performs zero solve, code expansion, history publication and semantic workspace save. Accepted complete paint remains visible and future camera/selection input works; no LOD or reduced-paint state exists. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U6 | Generate the frozen pre-redesign command/variant manifest, inspect the CAD icons and semantic Sketch/Constraint/Dimension/Modify headings, traverse every entry by pointer and keyboard, commit a Segment and Center–Radius Circle with ordinary click-click input, then semantically exercise Rectangle, Coincident, Distance, Fillet, Conics, Splines, Continuity and canvas display options. Exercise one genuine pointer-cancel/capture-loss path after staging geometry. | The post-redesign inventory matches the manifest one for one. Sketch/Constraint/Dimension/Modify contain exactly 25/13/5/2 tools, Modify contains only Fillet/Offset, and Grid/Fit/Origin stay canvas-local without changing the active authoring tool. Frequent families are one action away; every advanced entry is within two actions, retains its current semantics and never requires hover. Ordinary releases commit once; genuine cancellation occurs once and clears draft paint. Finish/Cancel and current tool state remain visible when applicable. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U7 | In Typed Panel select either generated Fillet, invoke Open in code, edit `radius: mm(4)` to `mm(2)` and Apply. Then inspect a solver-instance point, encoded field and blocked dirty-draft field. | Open in code focuses the exact authenticated source owner and preserves canvas selection. Both Fillets update in one outer history row. Source/instance/encoded/blocked language and permitted actions remain consistent across Inspector, Parameters and Code. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U8 | Introduce invalid managed source at a known line/column, navigate among canvas, Inspector and Code, then correct and Apply it. | The prior accepted canvas remains complete. Exactly one persistent Problems entry identifies and focuses the source position without losing the draft; correction clears/resolves it through one accepted outer transaction. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U9 | With an unapplied valid draft and then an invalid draft, attempt canonical export and separately download raw draft source. Apply/Revert to clean state, export `project.json` and `sketch.ts`, inspect with `geosolve-headless`, perform one exact-CAS edit, render, and atomically import the emitted project. Download a deliberately large repro/trace rather than copying it. | Dirty and invalid drafts each produce a typed canonical-export refusal; raw draft download claims no accepted-project authority. After acceptance, browser/headless source, project and control identities agree. The edited project imports only after complete validation and matches the headless scene. Large diagnostics have a file route under Diagnostics. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |
| M88-U10 | With exact build identity recorded, perform five fresh release cold-opens of Gridfinity and five `baseBottomWidth: 35.6 -> 20` edits. Run the actual browser/WASM stack contract/no-trap check and separate one-MiB native proxy regression. Maximize, restore and resize while inspecting the complete profile. | Cold-open median/max are at most 2.0/2.5 s and edit median/max at most 1.25/1.75 s on the recorded machine. Both distinct stack checks pass. Geometry is finite, zero-DoF/full-rank authority is truthful, independently validated hard residual is at most `1e-9`, and full accepted paint remains interactive. | accepted by 2026-09-01 milestone-level close decision; not separately replayed |

## Required mechanical evidence before disposition

- Exact owner regressions for stack reduction, checkpoint/open deduplication and any full-row-rank
  redundancy shortcut, with separate native-proxy and actual browser/WASM stack evidence.
- Independent finite and hard-residual validation for Gridfinity after open and edit.
- Locked native/WASM tests covering canonical browser/headless project interchange.
- Presentation work-ledger tests proving pane/layout actions have no semantic work.
- Editor-state tests covering durable canvas selection and layout transitions.
- Frozen command/variant manifest parity plus keyboard/focus checks for every entry, pane
  separators, tabs, start/search and Open in code.
- Formatting, warnings-denied workspace Clippy/tests, relevant TypeScript checks, Rustdoc, locked
  WASM and release React/Vite assembly before nomination.

Human review may accept visual hierarchy and interaction feel. It cannot replace the owning-layer
stack, solver, accepted-scene, transaction or headless parity evidence.
