<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 handover — Workflow-led authoring workbench redesign

Status: **historical closeout handover; M88 was accepted and closed on 2026-09-01**.
The supervising user's milestone-level close decision accepts M88-U1 through M88-U10 without
claiming a separately logged row-by-row replay. This file records that exact boundary rather than
reconstructing individual observations from chat history.

## Incoming authority

- Repository: `/home/arduano/programming/geometric-constraint-solver`.
- M87 clean-qualified source:
  `32c72892772ee09f8b904153484b02fd9923dc25`.
- M87 clean-qualified tree:
  `38f7175f93c87d11422f5de00e78208f8cf315bb`.
- The historical pre-React M88 implementation was based on
  `71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`; its fifteen-file implementation/test manifest was
  `0090ca3e34b9dd09b08e40d7ba13aa30aadfd8026b13618185524c66ada6f035`.
- Read-only snapshot `/tmp/geosolve-m88-uat.nmhcRj` remains preserved after reboot; its former
  transient `http://100.94.63.83:8080/` listener is gone. Its exact seven-file aggregate is
  `6c4af7e96e30687654d691b577954c22b051fb3ad37e9472c51e16c9c002a274` and its WASM SHA-256 is
  `cc1fc972fa6ae3cf70a1b8324b852ffa70cf65c187643798b7e08bcd6726ade9`. This is a rollback target,
  not the React replacement candidate; do not rebuild, overwrite or repoint it.
- The current shared dirty tree contains the React/Vite frontend at
  `crates/geosolve-demo-web/frontend/`, a versioned JSON/WASM bridge and renderer-owned interactive
  accepted SVG. Rust remains semantic authority; React owns browser presentation and platform work.
- The initial full gate passed. After M88-F001, semantic-toolbar M88-F002, point-and-click M88-F003
  and canvas-chrome M88-F004, the frozen candidate qualification passed bridge `23/23`, full
  demo-web `395/395`, frontend `26/26`, real release-WASM Playwright `10/10`, focused
  all-feature demo-web library Clippy, `npm run check`, distribution validation, formatting and the
  locked WASM check.
- Middle-button pan bypasses semantic gestures and preserves selection/revision/project authority.
  The accessible bounded Recent section stores only canonical sample identity through guarded
  presentation storage; it never stores source/project/scene authority.
- Browser project persistence, presentation preferences and local unapplied drafts use separate
  keys. All React-owned storage access, including panel-layout storage, is guarded against
  SecurityError, quota and removal failure; failures become safe absence or a durable alert.
- Current immutable M88-F004 snapshot `/tmp/geosolve-m88-react-uat.KGhA7s`, aggregate
  `700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`, release-WASM SHA-256
  `22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`, is exact-served at
  `http://100.94.63.83:18088/` by PID `1021511`. Every earlier snapshot remains preserved.
- React plus the instance-scoped `WorkbenchHandle`/`WorkbenchBridge` is now the sole browser
  presentation path. The obsolete Rust-DOM installer, static host assets, global callback
  registries and DOM-only dependencies/tests are retired from source after acceptance.
- Final post-cut source passes demo-web `344/344`, actual-WASM `2/2`, frontend `26/26`, current
  optimized release-WASM Playwright `10/10`, the unchanged 271-row golden, strict workspace
  Clippy/tests/WASM checks and the complete dirty-tree release gate. Its rebuilt local distribution
  is qualification evidence only and did not replace the accepted F004 bytes.
- M88 is closed. The F004 Tailscale service remains live and unchanged; no public publication or
  service retirement is inferred.

Preserve the twelve bundled code projects, especially the CNC joinery fit coupon and fully
constrained Gridfinity section added in M87. Preserve managed controls, exact-CAS outer history,
solver-instance overlays, shared rendering, headless inspect/edit/render, complete scene paint and
the full LOD removal.

## Mandatory read order

1. `AGENTS.md`, `START_HERE.md`, `ARCHITECTURE.md`, `PLAN.md`, `ACCEPTANCE.md` and
   `docs/SCENARIOS.md`.
2. ADR 0041 and ADR 0042 for optional managed-code and headless authority.
3. `docs/M88_AUDIT.md` for measured current behavior and workflow traces.
4. `docs/M88_GOALS.md` for required scope, authority boundaries and non-goals.
5. `docs/M88_IMPLEMENTATION.md` for the completed React amendment, historical Rust-DOM
   qualification and post-acceptance retirement record.
6. `docs/M88_UAT.md` before changing markup, focus behavior or presentation persistence.

## Historical closeout state

1. Preserve the historical port-`8080` snapshot exactly as rollback evidence; it is not the
   accepted React identity.
2. Preserve every earlier React snapshot and the current F004 service. F004 remains the accepted
   milestone identity at `http://100.94.63.83:18088/`.
3. The post-acceptance Rust-DOM compatibility cut is complete and the retained React/bridge path is
   mechanically requalified from the final source.
4. M88 is closed. A future fix requires its own scope; Pages/public deployment and retirement of
   the accepted service remain separate decisions.

## M88-F001 disposition

Initial UAT deterministically exposed `IntentBootstrapMetadata` twice because the bridge serialized
`IntentGraphNodeKind` using Rust `Debug` for Explorer and Inspector. The bridge now reuses semantic
`node_family_label`; Explorer omits the type/detail suffix entirely, Inspector keeps one compact
semantic kind, and Explorer row/group icons are non-shrinking. The exact regression initially
failed and now passes. Real release-WASM E2E rejects `IntentBootstrapMetadata` and `Bootstrap {`
and asserts the Explorer row/icon/Inspector contract. All UAT rows were still pending at this
historical F001 checkpoint.

## M88-F002 disposition

UAT next found that the migrated rail had lost recognizable CAD icons and semantic grouping, while
a scissors-like bucket combined Fillet/Offset with unrelated display actions. The current rail is
Select, Sketch, Constraint, Dimension and Modify with exact 25/13/5/2 inventories. Modify contains
only Fillet and Offset; Profile/Construction is contextual; Grid/Fit/Origin are canvas-local. A
bounded one-time Rust catalog owns command/icon identity and is absent from hot snapshots. View and
role commands preserve the active authoring tool. Browser qualification also covers long labels,
bounded scrolling, outside dismissal, destination-click routing, focus restoration and active
category state at both required desktop sizes. All human rows were still pending at this
historical F002 checkpoint.

## M88-F003 disposition

The frozen F002 release-WASM candidate independently reproduced ordinary point-and-click authoring
failure for Segment and Center–Radius Circle. Snapshot
`/tmp/geosolve-m88-react-uat.kdSCUU`, WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`, was built from the shared
tree based on `71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`. Direct bridge click-click authoring already
worked. The React viewport unconditionally canceled `lostpointercapture`, although browsers
normally emit it after `pointerup`; the bridge cancellation path independently discarded the
editor's `ClearConstructionPreview` effect.

The viewport now remembers the exact pointer and retires ownership synchronously on `pointerup`
before awaiting the adapter. The expected later capture loss is stale and harmless; genuine
`pointercancel`/capture loss cancels exactly once. Bridge cancellation now dispatches every returned
effect. React normal-release/genuine-loss, direct bridge Segment/Circle parity, direct cancellation-
preview cleanup and real release-WASM regressions pass. Final F003 proportional totals were bridge
`19/19`, full demo-web `391/391`, frontend `19/19` and Playwright `8/8`; `npm run check`, focused
all-feature demo-web Clippy, formatting, locked WASM and distribution validation passed. Immutable
F003 snapshot `/tmp/geosolve-m88-react-uat.QkVU1k`, aggregate
`e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167`, remains preserved after
F004 replaced its service. All human rows were still pending at this historical F003 checkpoint.

## M88-F004 disposition and current identity

F003 UAT found unconditional geometry-role chrome, flow-layout error feedback and a Finish action
enabled without a completable draft. Rust now publishes exact Finish and Undo/Redo readiness;
new-curve and selected-curve roles are contextual; feedback is a fixed overlay; and canvas Enter
shares Finish's gate. One-shot preselection returns to Select, camera/first-Escape cancellation
preserves the collector, Code-only mode omits canvas tools, Ctrl-O/Ctrl-S are real, dirty drafts
block replacement/reproduction, and secondary click remains browser input.

- Frozen path: `/tmp/geosolve-m88-react-uat.KGhA7s`.
- Freeze/service start: `2026-09-01 00:55:59 AEST`.
- Endpoint/service/PID: `http://100.94.63.83:18088/` /
  `geosolve-m88-react-uat-current.service` / `1021511`.
- Manifest/aggregate: `/tmp/geosolve-m88-react-uat.KGhA7s.sha256` /
  `700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`.
- WASM SHA-256: `22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`.
- Evidence: `/tmp/geosolve-m88-f004-freeze-evidence.ijud9T`; equal staging/live HTTP ledger
  SHA-256 `e838461921907cfe1252423f5a9ccf7db056dd62370698d90d6e47e0e7428531`.
- Exactly eight regular files, zero symlinks, directories/files `0555`/`0444`; Cargo `--release`,
  wasm-bindgen, `wasm-opt -Oz`, Chrome `151.0.7922.173`.

Build, frozen, staging and live HTTP manifests match. Live Chrome proves right-click neutrality,
Polyline Finish false → false → true → false, one accepted curve, hidden Code-only canvas tools
and zero runtime errors. Mechanical totals are bridge `23/23`, demo-web `395/395`, frontend
`26/26`, Playwright `10/10`. The user's 2026-09-01 milestone-level decision accepts this identity
and M88-U1 through M88-U10 without claiming a separate row replay.

## Withdrawn immutable M88-F002 reproduction identity

- Frozen path: `/tmp/geosolve-m88-react-uat.kdSCUU`.
- Freeze/start: `2026-08-31 21:18:19 AEST`.
- Tailscale endpoint: `http://100.94.63.83:18088/`.
- User service/PID: `geosolve-m88-react-uat-current.service` / `139684`.
- Server log: `/tmp/geosolve-m88-react-uat.kdSCUU.http.log`.
- External sorted manifest: `/tmp/geosolve-m88-react-uat.kdSCUU.sha256`.
- Ordered file aggregate: `d328fdded4ae963230eef2c64c5fb22459dec7a55467e0a7c051bed739b2cd47`.
- WASM SHA-256: `7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`.
- Evidence: `/tmp/geosolve-m88-react-freeze-evidence.sUELzR`; eight-path results SHA-256
  `c5201b123ba2f06bcb9b0f6c370adfa2a9dc85c0caa750366d8b36ff33189726`.
- Contents/modes: exactly eight regular files, zero symlinks, directories `0555`, files `0444`.
- Build/browser identity: release WASM built by Cargo `--release`, wasm-bindgen and
  `wasm-opt -Oz`; Google Chrome `151.0.7922.173`. No debug candidate is nominated.

At the F002 freeze, its then-final `dist`, frozen files and served files matched exactly. Every file
plus `/` and `/index.html` returned HTTP 200, and a real-browser frozen-endpoint toolbar smoke passed
before F003. The immutable snapshot remains exact reproduction evidence; its service is retired and
it is not a current UAT endpoint.

## Superseded React candidates — preserved, listeners gone

- Frozen path: `/tmp/geosolve-m88-react-uat.TAMXyz`.
- Endpoint/service/PID: `http://100.94.63.83:18088/` /
  `geosolve-m88-react-uat-TAMXyz.service` / `1846522`.
- Aggregate: `91c3a2349f1466a64720cb1cfba8a4f7d18aed0be03e9eec256ccf8c4eb0f9de`.
- WASM SHA-256: `9032a07bc6ac465816b3b3f3bb44c3881f815f290e39eece88890589f2cdfe5f`.

These bytes remain historical M88-F001 evidence and must not receive further UAT. M88-F001 snapshot
`/tmp/geosolve-m88-react-uat.nGkL4i`, aggregate
`0bd35f3dba50c592c6eea34ed18afed1b6f908803d75feb9ca3bbb620a1572c4`, also remains preserved.
All former `8080`, `18088` and `18089` transient listener PIDs disappeared after reboot; do not
confuse the newly reused current `18088` endpoint with the historical TAMXyz bytes.

## Frozen UX target

- Compact File/project app bar with Undo/Redo, true title/status, Design/Split/Code and Export;
  repro/trace under Diagnostics.
- Narrow Select/Sketch/Constraint/Dimension/Modify rail with last-used state and click/keyboard
  submenus for every current variant.
- One collapsible Explorer instead of simultaneous tree and Design hierarchy.
- Design canvas, resizable Split and first-class central Code modes.
- Right Inspector, Parameters and Problems tabs in Design/Split; the latter two rehost their one
  logical state into Code mode rather than creating duplicate views.
- Sticky Apply/Revert/status, at least 12 px source text, at least `720 x 500` CSS px of editor at
  `1024 x 720`, source tabs and secondary Parameters, Problems, Generated and Artifacts surfaces.
- Searchable New/Start from code/sample/import/recents surface covering all 37 samples.
- Exact managed-owner Open in code and canonical `project.json`/`sketch.ts`
  browser-to-headless-to-browser handoff.

## Historical performance baseline to retain

Gridfinity remains a valid 62x62 full-rank, zero-DoF system with no `FixedPoint` and one Y
`FixedCoordinate`. The actual WASM open/edit contract passes `4/4` together with the final
one-MiB native proxy regression. Five independent Chrome 151 processes measured cold opens at
`1770.1, 1795.7, 1561.9, 1455.7, 1551.1` ms (median/max `1561.9/1795.7`) and exact-CAS edits at
`1314.3, 928.8, 897.1, 871.0, 855.0` ms (median/max `897.1/1314.3`). Open/edit independently
validated maximum normalized Hard residuals were `2.8866e-15` and `4.8486e-12`; all accepted
geometry was finite and no browser error, trap or crash occurred. Native rank/DoF owner evidence,
not browser DOM, owns the full-rank/zero-DoF claim.

## Boundaries and cautions

- Presentation preferences never enter canonical project, repro, solver, accepted-scene or history
  authority. Resizing and mode changes do no semantic work.
- Invalid code retains prior accepted paint and one outer transaction boundary.
- Canonical project export returns a typed refusal while any unapplied draft exists; raw draft-
  source download remains separate and claims no accepted-project authority.
- Do not execute custom TypeScript in Rust/WASM or add an AI chat/stateful service.
- Do not build a general IDE, add new sketch mathematics or weaken independent residual checks.
- Do not restore LOD or hide accepted geometry for performance.
- Preserve existing dirty user work if implementation resumes in a non-clean tree; inspect before
  editing and make small, owning-boundary changes.

## Historical M88 closeout documents

- `docs/M88_AUDIT.md`
- `docs/M88_GOALS.md`
- `docs/M88_IMPLEMENTATION.md`
- `docs/M88_UAT.md`
- `docs/M88_HANDOVER.md`
