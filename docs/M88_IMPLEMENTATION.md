<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 implementation ledger — Workflow-led authoring workbench redesign

Status: **complete and accepted on 2026-09-01**. The immutable M88-F004 React replacement remains
live as the accepted UAT identity. The supervising user's milestone-level close decision accepts
M88-U1 through M88-U10 without claiming a separately logged row-by-row replay. The obsolete
Rust-DOM compatibility surface is retired from source after that acceptance. The prior Rust-DOM candidate
was based on
`71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`; its fifteen-file implementation/test manifest was
`0090ca3e34b9dd09b08e40d7ba13aa30aadfd8026b13618185524c66ada6f035`. Its port-`8080` snapshot is
historical rollback evidence, not qualification of the current React tree.

## React replacement and closeout amendment

- [x] Add the React/Vite frontend under `crates/geosolve-demo-web/frontend/` and keep a versioned
  JSON adapter around the Rust/WASM workbench. Rust remains authoritative for solver, history,
  managed code, semantic interaction, picking and accepted scene; React owns DOM, focus, layout,
  browser files/clipboard/downloads and local presentation persistence.
- [x] Move accepted SVG composition into `geosolve-sketch-render`, including scoped scene styling,
  geometry, points, computed Fillets/Offset, construction, annotations, dimensions, inference,
  hover/selection and errors. The bridge returns the accepted SVG rather than duplicating scene
  equations or hit logic in React.
- [x] Implement the compact shell, searchable 37-sample surface, light-dismiss tool/file/diagnostic
  surfaces, collapsible Explorer, Inspector/Parameters/Problems, Design/Split/Code and a single
  CodeMirror instance that stays mounted through presentation changes. Split keeps a 520 px code
  minimum and Code hides the right Details panel to preserve the 1024x720 editor floor.
- [x] Implement exact source-owner navigation, source-positioned Problems, hidden file import,
  canonical project/reproduction/trace export surfaces and separate raw-draft download. Keep the
  previous accepted canvas when source application rejects.
- [x] Separate canonical browser project persistence, presentation preferences and local unapplied
  source drafts. Route every React-owned `localStorage` read/write/remove, including the
  resizable-panel library's storage adapter, through one guarded boundary; SecurityError, quota and
  cleanup failures now yield safe absence or a durable dismissible frontend alert.
- [x] Rebuild generated WASM bindings and pass the current focused bridge `12/12`, renderer `25/25`
  and frontend `17/17` tests. `npm run build:ui` passes, including TypeScript and Vite production
  assembly.
- [x] Implement middle-button canvas pan without allowing the gesture to enter semantic selection
  or drag. Captured terminal coordinates may leave the host, cancellation/reset clears the gesture,
  and pan cannot steal an already active semantic gesture.
- [x] Close command/sample/recents and interaction-parity gaps found with the real Rust/WASM
  adapter. The accessible bounded Recent section reuses the legacy presentation-only key, stores
  only canonical sample identity and never stores source/project/scene authority.
- [x] Replace mock-only Playwright coverage with actual-WASM regressions for sample opening,
  accepted SVG styling, Split/Code floors, CodeMirror caret/selection/scroll continuity,
  invalid-source accepted-canvas retention, parameter edit, persistence reload, reproduction
  interchange and absence of console/network/runtime errors. The real release-WASM matrix passes
  `6/6` using system Chrome.
- [x] Run formatting, warnings-denied workspace Clippy/tests, locked WASM checks, the applicable
  golden and release WASM plus React/Vite assembly from the final unchanged source.
- [x] Freeze and byte-verify a separately identified immutable React candidate without changing,
  rebuilding or replacing the existing port-`8080` rollback service. Initial exact eight-file
  snapshot `/tmp/geosolve-m88-react-uat.TAMXyz`, aggregate
  `91c3a2349f1466a64720cb1cfba8a4f7d18aed0be03e9eec256ccf8c4eb0f9de`, remains preserved at
  `http://100.94.63.83:18088/` but is superseded by M88-F001.
- [x] Resolve M88-F001: stop serializing `IntentGraphNodeKind` through Rust `Debug`; reuse the
  semantic node-family label, remove Explorer type/detail suffixes, retain one compact Inspector
  kind and keep Explorer row/group icons non-shrinking.
- [x] Pass the initially failing exact-leak regression, bridge `12/12`, full demo-web `384/384`,
  frontend `17/17`, real release-WASM Playwright `6/6`, focused all-feature library Clippy,
  `npm run check`, distribution validation, format and diff hygiene; freeze and exact-verify current
  replacement `/tmp/geosolve-m88-react-uat.nGkL4i`, aggregate
  `0bd35f3dba50c592c6eea34ed18afed1b6f908803d75feb9ca3bbb620a1572c4`, at
  `http://100.94.63.83:18089/`.
- [x] Resolve M88-F002 with the static Rust-owned CAD tool catalog, exact 25/13/5/2 semantic
  Sketch/Constraint/Dimension/Modify inventory, contextual role controls and canvas-local
  Grid/Fit/Origin. Freeze the exact post-reboot F002 snapshot at
  `/tmp/geosolve-m88-react-uat.kdSCUU`; preserve it after M88-F003 withdraws it from UAT.
- [x] Reproduce M88-F003 in that frozen release-WASM snapshot, distinguish ordinary browser
  `pointerup -> lostpointercapture` from genuine cancellation, repair exact frontend pointer
  retirement and bridge cancellation-effect dispatch, and regress React lifecycle, direct bridge
  Segment/Circle authoring, cancellation cleanup and real-WASM click-click authoring. Proportional
  qualification passes bridge `19/19`, full demo-web `391/391`, frontend `19/19` and Playwright
  `8/8`, together with `npm run check`, formatting, locked WASM and distribution validation. Live
  Fillet/Profile Offset cancellation restores exact provisional state and creates no false Problem.
- [x] Freeze and exact-verify the unchanged corrected release output, replace the still-live pre-fix
  F002 service only after verification and record the new immutable/service identity.
- [x] Resolve M88-F004 with authoritative Finish/history readiness, contextual new/selected geometry
  roles and non-layout-shifting feedback. Keep Enter on the same canvas-scoped Finish gate.
- [x] Close the adjacent false-affordance audit: one-shot preselection state, gesture-preserving
  camera cancellation, Code-only rail removal, real Open/Save shortcuts, dirty-draft replacement
  and reproduction guards, browser-owned secondary click, truthful panel icons and fresh trace
  loading. Freeze, stage, byte-verify and serve the exact release output.
- [x] Accept M88-U1 through M88-U10 at milestone scope under the supervising user's explicit close
  decision without inventing separate row observations, then delete the Rust-DOM compatibility
  surface and requalify the retained React/bridge architecture.

The remaining phases retain the original candidate ledger. Phase 0's Rust stack/performance and
core-owner results remain a shared baseline; the completed amendment and React qualification
record above/below identify the replacement evidence, while the separately labelled historical
record applies only to the pre-React Rust-DOM snapshot.

## Historical pre-React candidate — Phase 0 stability and measurement prerequisite

Complete these items in order before treating browser layout UAT as credible:

- [x] Record the exact release-mode browser build, toolchain, served-byte identity, reference
  machine and five fresh Gridfinity cold-open/edit timings. Require cold-open median/max at most
  2.0/2.5 s and edit median/max at most 1.25/1.75 s on that machine. Freeze timing boundaries from
  sample-open dispatch to accepted-frame presentation and from Apply dispatch to replacement
  accepted-frame presentation.
- [x] Reproduce the Gridfinity edit through the smallest public owning boundary. Freeze the actual
  browser/WASM stack contract and browser no-trap check separately from a one-MiB native thread-
  stack proxy regression. Reduce stack use on the shared owning path until both pass without
  weakening geometry validation; do not treat either test as proof of the other.
- [x] Remove duplicate cold-open checkpoint restore/validation and the immediate unchanged
  post-open encode/save. Prove accepted project, history, persistence, generated identity and
  failure retention remain byte/semantically equivalent as applicable.
- [x] Re-audit structural edit rehydrate/checkpoint work and remove only independently proven
  duplication.
- [x] At `geosolve-core`, reproduce full-hard-row rank behavior and add the smallest regression
  proving full row rank produces complete empty redundancy evidence. Implement a shortcut only
  behind that regression; retain independent residual validation unchanged.
- [x] Re-run release/native/WASM Gridfinity open and edit measurements and record the resulting
  profile evidence.

The final Chrome 151 five-process runs used fresh temporary profiles, disabled HTTP cache and the
dispatch-to-first-RAF accepted-replacement boundary. Cold opens were `1770.1`, `1795.7`, `1561.9`,
`1455.7` and `1551.1` ms (median `1561.9`, maximum `1795.7`); exact-CAS
`baseBottomWidth: 35.6 -> 20` edits were `1314.3`, `928.8`, `897.1`, `871.0` and `855.0` ms
(median `897.1`, maximum `1314.3`). Every run retained 31 finite points, 36 curves, 31 constraints,
four Current features and eleven edges. Independently validated maximum normalized Hard residuals
were `2.8866e-15` on open and `4.8486e-12` after edit. There were no page, console, request,
crash or trap failures. The one preload-SRI capability warning per Chrome process is not a
GeoSolve runtime error.

## Phase 1 — Workflow and information-architecture freeze

- [x] Freeze a machine-readable manifest of every current command and variant, then classify every
  entry as primary, contextual, advanced or diagnostic using the audited start/sketch/code/
  ownership/problem flows.
- [x] Freeze the Design, Split and Code layout contract at 1920x1080, 1440x900 and 1024x720.
- [x] Freeze focus order, keyboard navigation, pane minimums, reset behavior and presentation-only
  persistence schema.
- [x] Freeze one project identity/status model and one durable Problems model before markup changes.

## Phase 2 — Shell and layout foundation

- [x] Implement the compact app bar and true project title/accepted-dirty-failed state.
- [x] Implement Design, Split and Code modes with pointer and keyboard-resizable panes.
- [x] Persist only bounded presentation preferences; never put them in canonical project/repro
  authority.
- [x] Prove resize/layout callbacks perform no solve, code expansion, history publication or
  semantic workspace save and do not strand accepted paint.
- [x] Implement one collapsible Explorer and right Inspector/Parameters/Problems tabs.

## Phase 3 — Primary navigation

- [x] Replace the 33-button permanent palette with the narrow primary tool rail.
- [x] Put every manifest entry in explicit click/keyboard navigation with last-used affordances and
  prove one-for-one parity; no registered command or variant may disappear behind the redesign.
- [x] Restore recognizable CAD icons and semantic families after M88-F002: Sketch 25,
  Constraint 13, Dimension five and Modify two. Keep Profile/Construction contextual and move
  Grid/Fit/Origin onto a canvas-local toolbar without replacing the active authoring tool.
- [x] Implement the searchable start/open surface for New, Start from code, all 37 samples,
  project/repro import and recents.
- [x] Move reproduction/trace commands to Diagnostics and add file-first large-payload download.

## Phase 4 — First-class Code workspace

- [x] Move Code from the narrow Design tab into the central Design/Split/Code workspace.
- [x] Add sticky Apply/Revert/status and source tabs with at least 12 px source text and at least
  `720 x 500` CSS px of usable editor in Code mode at `1024 x 720`.
- [x] Give Parameters and Problems one state model each, rehosted between the Design/Split right
  tabs and Code secondary tabs rather than duplicated. Keep Generated and Artifacts as separate
  secondary surfaces; collapse and search large generated inventories by default.
- [x] Reconcile durable panel updates without replacing the active editor DOM.
- [x] Preserve selected file, cursor, text selection, scroll and dirty draft through canvas
  selection, resize and layout switching.

## Phase 5 — Ownership and Problems cohesion

- [x] Expose one exact Open-in-code action for every modifiable managed owner and focus its source
  span without losing canvas selection.
- [x] Use consistent Modifiable in source / Modifiable instance / Encoded / Blocked language across
  Inspector, Parameters, Code and Problems.
- [x] Give retained source failures one durable Problems entry with exact line/column focus while
  the prior accepted canvas remains visible.
- [x] Keep generated, branch and artifact detail available through disclosures without making it
  primary chrome.

## Phase 6 — Browser/headless handoff

- [x] Export canonical `project.json` and `sketch.ts` from the browser without copying solver state
  into a second format. Return a typed refusal whenever a dirty or invalid unapplied draft exists;
  provide raw draft-source download as a separate non-canonical action.
- [x] Atomically import a canonical headless-emitted project after complete decode, expansion,
  materialization and independent validation.
- [x] Prove browser and headless project/source/control digests agree before and after one exact-CAS
  edit.
- [x] Keep custom patch source byte-identical and read-only; do not add TypeScript execution, AI
  chat or a stateful service.

## Phase 7 — Cleanup and qualification

- [x] Delete the superseded Rust-DOM/narrow-Code host, markup, CSS, routes and DOM-only dependencies
  after React replacement coverage and explicit acceptance pass.
- [x] Run formatting, warnings-denied Clippy, workspace tests, relevant native owner tests, locked
  WASM build/tests, TypeScript package checks and release React/Vite assembly.
- [x] Record M88-U1 through M88-U10 as accepted by the supervising user's 2026-09-01 milestone-level
  close decision, explicitly without claiming a separate row-by-row replay; retain the exact F004
  browser/build identity.
- [x] Record known limitations and the explicit supervising-user close disposition. Publication
  and accepted-service retirement remain separate and unrequested.

## Work and performance invariants

- Presentation-only actions must record zero parse, expand, materialize, solve, history and
  semantic-save work.
- Invalid or rejected source retains complete prior accepted scene, project, revision, generated
  identity and history authority.
- Any solver optimization receives an owning-layer regression and independent residual oracle; UI
  timing is not mathematical correctness evidence.
- Browser snapshots remain presentation/UAT evidence, never a solver oracle.
- No phase may restore LOD or hide accepted geometry as a performance substitute.

## Initial React mechanical qualification record

The initial pre-M88-F001 working-tree source passed the following mechanical qualification on
2026-08-31. These checks alone are source/test evidence; the next section separately records the
initial immutable served bytes. Neither section records human UAT.

- Formatting and diff hygiene pass. Warnings-denied workspace Clippy passes.
- The locked all-feature workspace suite passes, including demo-web `383/383`, renderer `25/25`,
  bridge `11/11`, the one-MiB Gridfinity proxy, the twelve code projects, headless corpus and all
  doctests.
- The locked `wasm32-unknown-unknown` check passes. Actual-WASM tests pass `4/4`, including the
  Gridfinity open/exact-CAS edit contract.
- Golden `--survey`, `--check` and `--require-clean` all pass; every reviewed row remains `PASS`
  and the checked fixture is exact.
- A clean frontend install followed by `npm run check` passes the 37-sample/command manifest,
  runtime-license and path/build-contract checks, frontend `17/17` and release-WASM Vite assembly.
  The separately run current-distribution validator also passes.
- Real release-WASM Playwright passes `6/6` with system Chrome. It covers real Typed Panel opening,
  renderer-owned authoritative SVG, dimensional floors, CodeMirror continuity, dirty canonical-
  export refusal, rejected-source accepted-frame retention with one positioned Problem, managed
  radius persistence, reproduction interchange, outside-click destination delivery and absence of
  page/console/network/HTTP errors.

The actual-WASM browser command requires
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome` inside `nix-shell shell.nix`.
Target-only dead-code warnings arise because the legacy Rust-DOM compatibility surface is
deliberately retained; native warnings-denied Clippy remains clean.

## Superseded initial React candidate record

At `2026-08-31 18:04:53 AEST`, the unchanged Vite distribution was copied without rebuilding to
`/tmp/geosolve-m88-react-uat.TAMXyz`. The snapshot contains exactly eight regular files and zero
symlinks; directories are mode `0555` and files are mode `0444`. External manifest
`/tmp/geosolve-m88-react-uat.TAMXyz.sha256` is the `LC_ALL=C` relative-path-sorted sequence of
`<sha256><two spaces><relative-path>\n`; its aggregate SHA-256 is
`91c3a2349f1466a64720cb1cfba8a4f7d18aed0be03e9eec256ccf8c4eb0f9de`. The WASM file SHA-256 is
`9032a07bc6ac465816b3b3f3bb44c3881f815f290e39eece88890589f2cdfe5f`. The current distribution
manifest and frozen manifest are identical.

User unit `geosolve-m88-react-uat-TAMXyz.service`, PID `1846522`, historically served only those bytes at
`http://100.94.63.83:18088/`; its log is
`/tmp/geosolve-m88-react-uat.TAMXyz.http.log`. Every frozen file plus `/` and `/index.html` returns
HTTP 200 with exact bytes, digest, length and media type. Served root, served index and frozen
`index.html` are identical. Build/browser identity is rustc `1.95.0 (59807616e 2026-04-14)`, Cargo
`1.95.0 (f2d3ce0bd 2026-03-21)`, Node `v24.19.0`, npm `11.17.0`, wasm-bindgen `0.2.121` and Google
Chrome `151.0.7922.173`.

The candidate was built from the dirty working tree based on HEAD
`71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`; this records exact working bytes, not a clean-source
claim. M88-F001 supersedes it for UAT; the snapshot remains preserved while service PID `1846522`
and historical port-`8080` rollback PID `425555` disappeared with their transient units on reboot.

## M88-F001 historical replacement qualification

The first UAT open deterministically showed `IntentBootstrapMetadata` twice because the bridge
serialized `IntentGraphNodeKind` with Rust `Debug` for both Explorer and Inspector. It could also
surface payload-shaped `Bootstrap { ... }` text. The repair reuses `node_family_label`, removes the
Explorer type/detail suffix entirely, keeps only the compact semantic kind in Inspector and makes
Explorer row/group icons non-shrinking.

The exact bridge regression initially failed on the leak and passes with the repair. Real
release-WASM E2E rejects `IntentBootstrapMetadata` and `Bootstrap {` and asserts the Explorer
row/icon plus Inspector contract. Proportional replacement qualification passes bridge `12/12`,
full demo-web `384/384`, frontend `17/17`, real release-WASM Playwright `6/6`, focused all-feature
demo-web library Clippy, `npm run check`, `validate:dist`, formatting and diff hygiene. The release
WASM remains built by Cargo `--release`, wasm-bindgen and `wasm-opt -Oz`; no debug candidate was
created or nominated.

At `2026-08-31 18:46:39 AEST`, the replacement distribution was copied without rebuilding to
`/tmp/geosolve-m88-react-uat.nGkL4i`. It contains exactly eight regular files, zero symlinks,
directories mode `0555` and files mode `0444`. External manifest
`/tmp/geosolve-m88-react-uat.nGkL4i.sha256` has aggregate SHA-256
`0bd35f3dba50c592c6eea34ed18afed1b6f908803d75feb9ca3bbb620a1572c4`; the WASM SHA-256 is
`beb76d47889055f9d8344ac4cdc8a01fc06a1dbd797979f636927031d3383996`.

User unit `geosolve-m88-react-uat-nGkL4i.service`, PID `2160046`, historically served the replacement at
`http://100.94.63.83:18089/`; log `/tmp/geosolve-m88-react-uat.nGkL4i.http.log`. Current dist,
frozen files and served files match exactly; every file plus `/` and `/index.html` returns HTTP 200
with the verified bytes. This was the current UAT candidate before M88-F002 and the later reboot;
its snapshot remains historical evidence while those transient listeners are gone. Every human row,
legacy Rust-DOM deletion, public publication and milestone closure remain pending.

## M88-F002 toolbar hierarchy and post-reboot replacement

The supervising user's next UAT finding was presentation hierarchy rather than missing domain
capability: the React migration had discarded recognizable CAD icons and meaningful families, and
its scissors-like miscellaneous bucket mixed Fillet/Offset with unrelated display and camera
actions. The repair keeps Rust as command identity authority and exposes its static tool metadata
through one versioned `WorkbenchHandle.toolCatalog()` read. The catalog is bounded, validated by
React as a closed SVG subset, rendered as ordinary JSX elements and intentionally absent from every
hot mutable snapshot.

The rail is now Select, Sketch, Constraint, Dimension and Modify. Sketch contains all 25 geometry
variants grouped by existing geometry family. Constraint contains all 13 tools under Placement,
Orientation, Equality & symmetry and Curve join. Dimension contains all five tools under Linear,
Circular and Angular. Modify contains only Fillet and Offset. Profile/Construction is presented in
contextual tool settings, and Grid/Fit/Origin are a three-button canvas-local toolbar. Rust snapshot
presentation publishes `activeTool` and `gridVisible`; `geometry.role.toggle`, `view.grid.toggle`,
`view.fit` and `view.origin` never replace the active authoring tool. Retained `tool.select` aliases
remain compatible.

Focused bridge tests pass 14/14, including the initially failing presentation-action regression
and the static-catalog bound/absence regression. Complete demo-web passes 386/386, frontend passes
18/18 and final real release-WASM Playwright passes 7/7. `npm run check`, distribution validation,
focused warnings-denied library Clippy, `cargo fmt --all -- --check` and `git diff --check` pass.
Visual/browser qualification at `1024 x 720` and `1440 x 900` covers long labels, bounded catalog
scrolling, active state, 12 px canvas insets, outside-click destination delivery and Escape focus
restoration with no runtime/console/network errors.

The reboot removed every former transient HTTP unit but preserved its immutable `/tmp` snapshots.
After the final release build, the unchanged eight-file distribution was race-checked and frozen at
`/tmp/geosolve-m88-react-uat.kdSCUU`, with directories/files `0555`/`0444`, zero symlinks, external
manifest `/tmp/geosolve-m88-react-uat.kdSCUU.sha256`, aggregate
`d328fdded4ae963230eef2c64c5fb22459dec7a55467e0a7c051bed739b2cd47` and WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`. Exact verification evidence
is `/tmp/geosolve-m88-react-freeze-evidence.sUELzR`; its eight-path ledger SHA-256 is
`c5201b123ba2f06bcb9b0f6c370adfa2a9dc85c0caa750366d8b36ff33189726`.

Transient user unit `geosolve-m88-react-uat-current.service`, PID `139684`, serves only that
snapshot on the Tailscale interface at `http://100.94.63.83:18088/`. Root/index/all eight files
return HTTP 200 and exactly match frozen bytes; an actual Chrome frozen-endpoint toolbar smoke also
passes. The candidate comes from the shared dirty tree at HEAD `71a51ee5`; M88-F003 now withdraws
it from continuing UAT. It remains immutable pre-fix reproduction evidence, not a clean-source
claim, human acceptance, public publication or milestone closure.

## M88-F003 point-and-click authoring correction

The failure was reproduced independently against frozen F002 snapshot
`/tmp/geosolve-m88-react-uat.kdSCUU`, release-WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`, source basis
`71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`. Selecting Segment or Center–Radius Circle and using
ordinary clicks painted an intermediate stage but could not commit it. The same click sequences
already committed through the direct Rust bridge, ruling out the presentation-independent geometry
stager.

The React viewport unconditionally mapped `lostpointercapture` to cancellation. Browsers normally
emit that event after `pointerup`, so each successfully dispatched release was immediately undone.
The bridge's `cancel_active_interaction` path independently discarded the editor's returned effects,
including `ClearConstructionPreview`, leaving canceled construction paint capable of surviving the
state transition.

The viewport now keeps the exact captured pointer in a ref and retires it synchronously on
`pointerup` before awaiting the adapter. Its expected later capture-loss notification is therefore
a no-op. A genuine `pointercancel` or capture loss while ownership remains active cancels exactly
once. Bridge cancellation dispatches all editor effects, so construction previews clear with the
authoring state. Provisional Fillet and Profile Offset drags use their specialized projectional
cancellation routes before the returned cleanup effects are consumed; cleanup acknowledgements are
nondurable and cannot manufacture a Problem.

Regression coverage includes failing-before-fix React normal-release versus genuine-loss behavior,
direct bridge click-click parity for Segment and Center–Radius Circle, direct cancellation-preview
cleanup, live Fillet-radius and provisional Profile Offset cancellation, and an ordinary-click real
release-WASM test for both geometries. Bridge tests pass `19/19`, the full demo-web library passes
`391/391`, frontend tests pass `19/19` and Playwright passes `8/8`.
`npm run check`, focused all-feature demo-web library Clippy, `cargo fmt --all -- --check`, the
locked WASM check and distribution validation pass. No solver equation, geometry recipe, branch,
persistence format or accepted-scene authority changed.

The final unchanged release distribution is frozen at `/tmp/geosolve-m88-react-uat.QkVU1k` with
exactly eight regular files, zero symlinks, directories/files `0555`/`0444`, external sorted manifest
`/tmp/geosolve-m88-react-uat.QkVU1k.sha256`, aggregate
`e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167` and optimized release-WASM
SHA-256 `5f49f49a880dd8529982bfbcf68a3f1a92c79ee96f256ed56931254ca884b22a`.
Evidence directory `/tmp/geosolve-m88-f003-freeze-evidence.bbtXoS` records identical final dist,
frozen, local-HTTP and Tailscale-HTTP bytes; both HTTP ledgers have SHA-256
`340b5bfd3d7d661b877fad3d3ab97813bc9027f95a99962b6788781c68588ef5`.

User unit `geosolve-m88-react-uat-current.service`, PID `518679`, started at
`2026-08-31 23:02:01 AEST` and serves only that snapshot at `http://100.94.63.83:18088/`. Root,
index and all eight files return exact HTTP 200 bytes. Local and served Chrome smokes commit Segment
and Center–Radius Circle with `1/2` then `2/3` accepted curves/points, zero draft paint and zero
runtime errors. All prior snapshots remain preserved. Human UAT remains paused and every scorecard
row stays pending.

## M88-F004 canvas-chrome and adjacent-affordance correction

F003 UAT exposed three distinct presentation lies. Profile/Construction was always shown even when
neither current authoring nor selected geometry could use it. Action failures were a flex child and
changed the canvas height. Finish was enabled for every non-Select tool even when its retained owner
had no publishable draft. The bridge also omitted authoritative Undo/Redo availability, leaving the
frontend to infer history from unrelated state.

The bridge snapshot now publishes `canFinish`, `canUndo` and `canRedo`. Polyline and NURBS delegate
to exact retained draft readiness. Computed Fillet and Profile Offset require their retained
candidate to match the painted preview. Premature Finish is revision/history/project neutral, and
canvas-scoped Enter uses the same gate. Geometry role is contextual: applicable authoring labels
“New curves”, selected curve state labels “Selected curves”, and irrelevant state renders no
control. Action feedback is a fixed bottom overlay and preserves exact canvas bounds. Ordinary
projects use Intent history; code projects use clean outer `SketchCodeSession` history; local or
Rust-owned dirty source disables Undo/Redo with an exact explanation.

The adjacent audit corrected the same failure class elsewhere. A relation/dimension with complete
preselection applies or rejects once and returns to Select rather than painting an inactive
collector. Camera and first-Escape cancellation restore only the live gesture and preserve the
authoring mode. Code-only layout omits canvas tools. Ctrl-O/Ctrl-S are real routes. Dirty source
blocks project replacement and exact reproduction export. Right-click remains browser input and is
not captured or dispatched semantically. Explorer/Details glyphs reflect open/closed state, and a
reopened trace dialog shows loading rather than stale prior payload.

Qualification passes bridge `23/23`, complete demo-web `395/395`, frontend `26/26`, real optimized
WASM Playwright `10/10`, focused all-feature warnings-denied Clippy, locked WASM check,
`npm run check`, distribution validation, formatting and diff hygiene. The release build uses Cargo
`--release`, wasm-bindgen `0.2.121` and `wasm-opt -Oz`; no debug candidate is nominated. No solver
equation, residual, Jacobian, priority, branch, persistence format or accepted-scene validation
changed.

The unchanged eight-file distribution was frozen at `/tmp/geosolve-m88-react-uat.KGhA7s` with zero
symlinks, directory/file modes `0555`/`0444`, external manifest
`/tmp/geosolve-m88-react-uat.KGhA7s.sha256`, aggregate
`700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071` and optimized release-WASM
SHA-256 `22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`. Evidence directory
`/tmp/geosolve-m88-f004-freeze-evidence.ijud9T` records equal final-dist, frozen, Tailscale staging
and Tailscale live manifests; staging/live HTTP ledgers both hash to
`e838461921907cfe1252423f5a9ccf7db056dd62370698d90d6e47e0e7428531`.

User unit `geosolve-m88-react-uat-current.service`, PID `1021511`, started at
`2026-09-01 00:55:59 AEST` and serves only this snapshot at `http://100.94.63.83:18088/`. Root,
index and all eight files return exact HTTP 200 bytes. Live Chrome proves right-click neutrality,
Polyline Finish false → false → true → false, one accepted curve, hidden Code-only canvas tools and
zero page/console/network/HTTP errors. F003 and every earlier snapshot remain preserved. The user's
2026-09-01 close decision accepts F004 and M88-U1 through M88-U10 at milestone scope. The
compatibility surface is retired from source; public publication and accepted-service retirement
remain unrequested.

## Post-acceptance compatibility retirement qualification

After the supervising user's close decision, the obsolete Rust-DOM host was removed without
changing the accepted React/bridge authority. The static Trunk host, DOM installer, legacy panel/
platform/performance/PNG modules, global callback registries and DOM-only dependencies/tests are
gone. The retained public surface is the instance-scoped `WorkbenchHandle`/`WorkbenchBridge`,
persistence and reproduction transports, headless APIs, samples and semantic owner regressions.

The final post-cut source passes strict all-workspace Clippy, the locked all-feature workspace and
doc-test suite, demo-web `344/344`, actual-WASM `2/2`, the locked WASM check, frontend `26/26`, the
eight-file release distribution validator and real optimized release-WASM Playwright `10/10`.
The 271-row authoring/scene oracle surveys cleanly and matches its unchanged SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`; `--require-clean` passes
inside the complete release gate. Formatting and diff hygiene also pass. The canonical frontend
build uses Cargo `--release`, wasm-bindgen `0.2.121` and wasm-opt `131` with `-Oz`.

The complete final-source command is:

```text
GEOSOLVE_ALLOW_DIRTY=1 NO_COLOR=true nix-shell shell.nix --run \
  './scripts/release-gate.sh'
```

Standalone golden `--survey` and `--check`, canonical frontend `npm run check`,
`npm run validate:dist -- ../dist ./` and `npm run test:e2e` also exit `0`. The rebuilt local
distribution is qualification evidence only. It did not replace, rebuild or mutate accepted
snapshot `/tmp/geosolve-m88-react-uat.KGhA7s`; that immutable F004 identity remains live on
Tailscale exactly as accepted.

## Historical pre-React qualification record

The following commands passed for the port-`8080` Rust-DOM snapshot and identify that historical
qualification rather than the separate current React run recorded above:

```text
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked -p geosolve-core --test m10 \
  full_hard_row_rank_proves_redundancy_empty_without_deletion_rank_trials -- --exact
cargo test --locked -p geosolve-sketch-code --test gridfinity_stack
cargo test --locked -p geosolve-demo-web --lib
cargo test --locked --workspace --all-features
(cd packages/geosolve-intent && npm ci --ignore-scripts && npm test)
(cd packages/geosolve-sketch-code && npm ci --ignore-scripts && npm test)
env NO_COLOR=true nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
   cargo test --locked -p geosolve-demo-web --lib actual_wasm_ \
   --target wasm32-unknown-unknown -- --nocapture'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features \
   --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release \
   --dist /tmp/geosolve-m88-final-build.AHuCxA'
diff -qr /tmp/geosolve-m88-final-build.AHuCxA /tmp/geosolve-m88-uat.nmhcRj
```

The demo-web native library reports `372/372`; the TypeScript packages report `40/40` and `14/14`
runtime tests plus their fixture/type/managed-source checks. The final actual-WASM run reports
`4/4`, including
the real Gridfinity cold-open/exact-CAS stack contract. The full workspace and doc-test command
exits `0`. The one-MiB native proxy and focused full-row-rank owner regressions each report `1/1`.
The actual-WASM test build emits only the target-specific dead-code warnings already permitted by
the release gate; warnings-denied workspace Clippy is clean.

The Nix release toolchain is Rust `1.97.1`, Cargo `1.97.0`, Trunk `0.21.14`,
`wasm-bindgen-test-runner 0.2.121` and `wasm-opt 131`. Release Trunk assembly produced seven files
in `/tmp/geosolve-m88-final-build.AHuCxA`. The read-only no-rebuild UAT snapshot is
`/tmp/geosolve-m88-uat.nmhcRj`, served at `http://100.94.63.83:8080/`; root and every named file
byte-match. Its ordered-manifest aggregate is
`6c4af7e96e30687654d691b577954c22b051fb3ad37e9472c51e16c9c002a274`; the WASM SHA-256 is
`cc1fc972fa6ae3cf70a1b8324b852ffa70cf65c187643798b7e08bcd6726ade9`.

The historical browser audit also closed two late presentation findings. Returning an invalid managed
draft byte-identically to accepted source now clears its stale source-only Problems entry without
requiring Apply. Escape from a focused non-modal tool-options control now reaches active authoring,
closes Point options, activates Select and restores focus to the Select rail button. Focused native
tests, a clean isolated release build and trusted Chrome keyboard input pass. The final release
browser reports 25 native/12 code samples, no LOD state, a `722.21875 x 506.28125` editor with
12 px text at `1024 x 720`, exact textarea identity through source recovery, and no unexpected
page, runtime, console, network or HTTP errors.

The historical record above qualifies only the Rust-DOM rollback snapshot. The React full gate and
M88-F001 through M88-F004 replacement qualification are recorded separately above. The user's
2026-09-01 close decision accepts every `docs/M88_UAT.md` row at milestone scope without claiming
a separate replay. The port-`8080` rollback and superseded React snapshots remain untouched; the
Rust-DOM compatibility host is retired from source. No public deployment or accepted-service
retirement is inferred.
