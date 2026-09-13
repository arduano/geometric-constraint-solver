<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation and review candidate

M98's authoring engine, local canvas, collaboration and full toolbar are mechanically
qualified. Human acceptance and closure remain open. This is a historical evidence
record; [Getting started](GETTING_STARTED.md) owns current setup, and
[M99 qualification](M99_QUALIFICATION.md) records the subsequent accepted cleanup.

## Qualified UAT repairs

Delivered on 2026-09-13 from clean source `b49e339e4fbeeebf8b0d513de109875983f2cb58`, tree
`9b977c33b298a08e6336d23f2592ca382fb377f7`. Run `20260913T020232-eaefaaaf` passes
**293/293 obligations: 44 fresh, 249 authenticated reused, 42m41.084s**. Every receipt is
independently authenticated; complete release, clean source and unchanged source all pass.
This replacement supersedes the earlier `b005b9e` delivery below. U02 remains **Fail pending
human recheck**; maintainer acceptance and M98 closure remain open.

F041 adds `SketchDocument::transport_unenforced_source_line_branches` and
`geosolve-sketch-code::transport_code_point_terminal_branches`. They authenticate native
origin/terminal and source-derived free Polyline references before transporting dormant
directions. Explicit/enforced branches, finite geometry, independent residual validation,
exact source/terminal identity and cold restoration remain strict. All fifteen original
gestures pass: nine formerly rejected corners now accept; six previously accepted model/input
identities remain exact. Hard residuals are zero and unrelated points remain stationary.
F042 reduces interactive raster and redundant hover/GPU preparation work while retaining
static supersampling, exact completed-frame evidence and recovery. F043's private
`SequenceAtom`/`derive_text_spans` path compares native operation IDs before encoding only
changed characters and Undo anchors; imported nonstandard text keeps the original fallback.
Complete span and checkpoint parity pass across Unicode, concurrency and restoration.
No solver equation, geometric tolerance, wire/schema, priority or golden expectation changed.

| Qualification | Outcome |
| --- | --- |
| Native workspace / headless | 243 / 7 stage obligations passed |
| Engine / folder Node / folder browser | 76 / 111 / 11 tests passed |
| Generator example / browser / offline packages | 2 / 1 / 2 tests passed |
| Collaboration runtime / package / frontend | 151 / 44 / 76 tests passed |
| Ordinary browser | 49 full workflows and 17 fresh opening checks; no skips or flakiness |
| Collaboration browser | 17/17 workflows; no failures, cancellations or skips |
| Golden | 271/271 clean; unchanged SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` |
| Performance | Passed in 222.7s with the original limits |

| Integrated browser case | Navigation p95 | Actual durable typing ACK |
| --- | ---: | ---: |
| Four editors, held solve | 273.7 ms | 88.6 ms |
| Manifold, held solve | 296.8 ms | 314.6 ms |
| Gridfinity, held solve | 143.0 ms | 200.1 ms |

The original 500 ms budgets and ten-second solve holds pass; navigation uses zero RPCs.
The eight-editor/24-viewer load case measures 319.3 ms typing ACK p95. First-use/warm drag
preview p95 is 351.1/83.3/83.1 ms, release-to-peer is 817.2/442.9/339.9 ms, and every
trial has zero reversals. The ACK observer records committed application events on the
same browser clock; observer-return intervals remain separate (100.2/466.3/292.9 ms).
Held Offset client/server navigation, recovery and negative ACK controls pass. These bounded
software-rendered measurements do not establish 60 Hz, arbitrary-size or unlimited-client scaling.

Signed qualification is `target/release-gate/runs/20260913T020232-eaefaaaf/qualification.json`.
Hash-checked summary: `target/m98/coordination/corner-drag/final-summary-20260913T020232-eaefaaaf.json`.
The failed prior nominations and focused attempts remain in [hardening](M98_HARDENING.md).
No independent harness failure was converted into a release pass or used to relax a budget.

All four authenticated archives are frozen and installed offline; all **284 installed files**
match. Production retains **23 exact files** and all three WASM modules; engine WASM is
**14,311,142 bytes**. Installation: `target/m98/installed-drag-preview-20260913T020232-eaefaaaf/installation.json`.
The installed cold-corner browser regression passes **2/2 in 53.689s**, including client/server
prediction, actual DOM terminal agreement, stationary other points, source retention, peer
visibility, personal Undo/Redo and reload. Its initial nominal-pointer and capture-target
helper failures remain documented separately; no production artifact changed for their repair.

Historical delivery verified shared and folder HTTP/MIME routes, actual WASM
readiness and exact source/draft/journal/personal-history retention across restart.
The generator website passed a fresh 1/1 workflow from 395 authenticated inputs;
its default footprint was 125.5 × 83.5 mm with 24 bores. Delivery did not reset
any journal or convert a failed human UAT row into a pass.

The final integrated command was:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260913T001723-3bb4f88c'
```

The final signed receipt and stage outputs are under
`target/release-gate/runs/20260913T020232-eaefaaaf/`. Generated installation and
browser evidence were retained under `target/m98/coordination/corner-drag`.
These historical paths identify evidence; current setup uses
[Getting started](GETTING_STARTED.md).

## Qualified shared toolbar parity

Delivered on 2026-09-11 from clean source `b005b9e1bdb1a120ec9363d4c8684d1f8f5c3d00`, tree
`5381978256660eaaf9f6bb648a36787309a9eb51`. Run `20260911T164104-576e7789` passes **293/293 obligations**:
**20 fresh, 273 authenticated reused**, **37m48.118s**. Independent
verification authenticates every passing receipt and confirms complete release, clean
source and unchanged source. Signed qualification: `target/release-gate/runs/20260911T164104-576e7789/qualification.json`.
The hash-checked detailed summary is `target/m98/coordination/tool-parity/final-summary-20260911T164104-576e7789.json`.

The [toolbar amendment](M98_TOOL_PARITY.md) exposes all 45 current React workbench tools:
25 geometry variants, 13 constraints, five dimensions, Fillet and Profile Offset.
Contextual options, explicit branches, preselection, Explorer operands and geometry roles
use native authoring. The engine's `beginToolOperation` prediction exposes `initialFrame`,
ordered `advance`, paint-only `presentationJSON`, `finish` and `cancel`. Server
prepare/replay/resolve/apply APIs authenticate original operands and target lifetimes,
replay against the latest accepted model and require genuine compiler receipts and
independent residual validation before durable publication. Traces retain exact native
curve occurrences and are cumulatively bounded to 1 MiB. Personal Undo/Redo preserves
peer contributions. Camera, picking, selection and provisional reprojection remain local
with either client or server prediction.

M98-F031 repairs generated-polyline point history through its semantic source lens.
F032–F035 preserve defining samples, explicit arc orientation and exact source projection;
F036 adds native operation paint cues; F037 bounds traces; F038 preserves collector recovery
and independently accepted authority after refused Apply. F039 replaces synchronous GPU
completion waits with WebGL2 fences and scheduled zero-timeout polling. One immutable draw
is in flight; coalesced inputs cannot relabel old pixels. Only completed work followed by
existing GL error/context validation advances the exact frame/surface/context witness.
Context loss, disposal, failed waits and still-unsignaled five-second deadlines cannot
publish late frames. A signaled fence observed after background throttling may complete.
Final polling is 4 ms. F040 submits already-coalesced input immediately after asynchronous
validation and initializes the exact blur shader programs during the first ordinary render.
It repeats preparation after restoration; actual batch and horizontal/vertical shader-loss
cases retain line/text pixel evidence. No synthetic offscreen draw is used, and shader
initialization is included in canvas readiness. Synchronous draws retain RAF coalescing;
both paths preserve one in-flight draw, hidden suspension, failure latching and context
epochs. The private release engine staging build applies `wasm-opt -Oz`, keeping the
unchanged 20 MiB ceiling.

No solver primitive, equation, geometric tolerance, priority semantics, implicit branch
or reviewed golden expectation changed. Format, warnings-denied Clippy, native/headless,
WASM, optimized lifecycle, package/license, browser, golden and performance requirements
pass through the integrated runner:

| Qualification | Outcome |
| --- | --- |
| Native workspace | 243 stage obligations passed |
| Headless | 7 stage obligations passed |
| Dedicated engine / actual WASM | 76/76 tests passed |
| Folder Node | 111/111 tests passed |
| Folder browser | 11/11 tests passed |
| Generator example | 2/2 tests passed |
| Browser example | 1/1 tests passed |
| Offline M98 packages | 2/2 tests passed |
| Collaboration runtime | 149/149 tests passed |
| Collaboration package | 44/44 tests passed |
| Collaboration frontend | 75/75 tests passed |
| Ordinary browser | 49/49 full workflows; 17 fresh opening checks |
| Collaboration browser | 16/16 workflows passed |
| Golden | 271/271 clean; unchanged expected bytes |
| Performance | Passed in 181.7 s |

The reviewed golden remains **271/271 clean**, with unchanged expected bytes.
Expected and observed SHA-256: `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

Final integrated browser measurements include Chromium software rendering and event
delivery:

| Drag trial | Event-to-preview p95 | Release-to-peer | Backward jumps |
| --- | ---: | ---: | ---: |
| First use | 417.2 ms | 828.4 ms | 0 |
| Warm 1 | 142.5 ms | 476.3 ms | 0 |
| Warm 2 | 116.7 ms | 419.5 ms | 0 |

Held Offset client prediction: wheel/pan/resize 142.9 / 188.0 / 294.8 ms; zero navigation RPCs.

Held Offset server prediction: wheel/pan/resize 106.9 / 191.7 / 280.1 ms; zero navigation RPCs.

Four editors navigate during a ten-second held solve at 363.4 ms p95; manifold/Gridfinity navigation measures 373.1/250.7 ms p95, with zero navigation RPCs. Eight editors and 24 viewers acknowledge text at 248.9 ms p95. Lost-acknowledgement recovery, server restart, invalid-source editing and bounded stalled-TCP recovery pass.

All five new toolbar workflows execute within the full collaboration suite: catalog
availability with advanced conic/reference-dimension history, native Parallel preselection
across zoom, Fillet options and selected roles, and client/server Offset with Explorer
operands, Escape and held publication. Existing recovery, concurrent editing, dragging,
held-solve navigation, bounded transport and load regressions remain required. These
measurements do not establish 60 Hz, compositor scan-out timing, arbitrary document size
or unlimited editors. GPU driver/shader IPC still contributes to latency.

Exact production (**23 files**) and four archives are frozen and
installed offline without rebuilding. All **284 installed package files**
match authenticated archive hashes. Final nominated engine WASM: **14303000
bytes**. Both existing previews pass **24 HTTP/MIME routes** and two-editor
readiness with all three exact nominated WASM modules, accepted-presentation provenance
and zero page errors. Delivery readiness visits editors sequentially; concurrent-edit
coverage belongs to the integrated suite. Verification makes no authored edits.

## Qualified shared dragging repair

The following historical qualification and delivery describe the preceding product. The
toolbar amendment above supersedes its current-service and restricted-tool statements.

Delivered on 2026-09-11 from clean source
`9b64c29e188b1e74f5802d170ca40ced107b28b1`, tree
`9a58a5d7401741b60f142d5af5dfe00100ce5254`. Run
`20260910T233626-b8af5961` passes **288/288 obligations**: **34 fresh, 254 authenticated
reused**, **36m7.710s**. Independent verification authenticates every linked passing
receipt and the complete-release, clean-source and unchanged-source flags.
Evidence: `target/m98/coordination/drag-repair/final-qualification-r3.json`.

M98-F028–F030 are recorded in the hardening report:
retained authenticated document workers remove repeated cold initialization; bounded
native result ownership prevents result/history exhaustion. Genuine compiler receipts,
historical source/target authentication, independent residual validation and durable
publication remain required. Local provisional geometry owns paint through gesture
completion and accepted reconciliation; both client and server prediction forward the
same opaque native presentation to the independent navigation worker. Native partial
source-symbol mapping preserves surviving selection, exact curve occurrences and pins,
while dropping deleted, incompatible or ambiguous owners. Strict prediction mapping
and shared target lifetimes remain unchanged. No equation, tolerance, branch or reviewed
golden expectation changed.

The integrated gate passes format, strict Clippy, workspace/headless tests, release
WASM, optimized lifecycle, native/WASM parity, golden, package, license and performance
requirements. The fresh native workbench stage passes all 363 ordinary cases (the existing
external-payload-only M92 legacy test remains ignored). All 271 reviewed golden cases
remain clean. Browser prefix **17/17** and full workflows **49/49** pass without failures,
skips or retries. Engine **22**, folder Node **111**, folder browser **11**, examples and
offline package checks pass. Collaboration runtime **140**, package **44**, frontend
**62** and browser **11** pass without failed/skipped/cancelled/todo assertions.

Final integrated measurements include Chromium software rendering and event delivery:

| Measurement | Before repair | Qualified repair |
| --- | ---: | ---: |
| Backward jumps in three straight drags | 5 / 2 / 4 | 0 / 0 / 0 |
| Warm event-to-preview p95 | Visible repeated accepted-frame flicker | 63.0 / 75.8 ms |
| Warm release-to-peer | 3,251–3,740 ms | 351.1 / 383.3 ms |
| First drag event-to-preview p95 | — | 426.8 ms |
| Pan/wheel/resize with independently held authoring or terminal | Previously blocked | 59.7–166.0 ms; zero navigation RPCs |

The first trial's release-to-peer is 744.2 ms. First-use browser/GPU cost remains;
the unachieved 200 ms cold aspiration and fully reverted warmup experiment are retained
in the hardening record. The accepted regression budgets are 500 ms first use, 200 ms
warm preview and 1,000 ms warm peer publication. No 60 Hz or arbitrary-size claim is made.
Four editors during a held ten-second solve navigate at 431.7 ms p95; manifold/Gridfinity
at 321.5/234.4 ms. Eight editors and 24 viewers acknowledge text at 267.6 ms p95; bounded
stalled-TCP recovery passes. Detailed diagnostics:
`target/m98/coordination/drag-repair/integrated-metrics-r3.json` and the gate's
`stages/collaboration.browser/output.log`.

The 23-file production artifact and four archives installed offline; all 283
installed package files matched. Exact HTTP/MIME and real-WASM readiness passed,
with accepted/working source and personal history retained across restart.

## Qualified multi-editor collaboration

Candidate `513463f822083385edaccded05d8ed901e479dab`, tree
`ef01e9bf183e7e5712a7d1a82dab2ff6c8026d98`, passes **288/288 obligations** in
`20260910T204328-4a05c31a`: **32 fresh and 256 authenticated reused results**,
**44m4.890s**. Independent receipt verification confirms complete release,
clean-source and unchanged-source flags and every linked passing receipt.
Evidence: `target/m98/collaboration-final-qualification.json`.

The reusable Rust collaboration crate, dedicated WASM adapter and TypeScript package
compose shared raw Automerge text with server-ordered validated semantic operations.
Immutable Apply, checked personal Undo/Redo, stable target lifetimes, durable operation
outcomes, lost-ACK recovery, browser outboxes, external mirrors and the CLI use the
same authority. Each editor retains local Rust/WASM navigation, selection and tools;
client/server authoring prediction uses shared native semantics. Accepted model,
unfinished working source and provisional presentation remain separate.
No solver equation, tolerance, hard/soft priority, implicit branch or golden expectation
changed. Compiler receipts and independent residual validation still control acceptance.

The integrated gate passes formatting, strict Clippy, workspace/headless tests,
release WASM, optimized-WASM lifecycle, seven native/WASM parity suites, licensing,
package checks and all 271 unchanged golden cases. The performance stage passes in
188.4 s; the independently validated 256-moving-body sparse crossover takes 129.39 s.
All 49 ordinary browser workflows retain authenticated unchanged-input evidence.
Fresh M98 coverage passes 22 engine cases, 111 folder Node cases, 11 folder browser
workflows, two generator cases, one browser-embedding case and two offline-package
cases. Collaboration passes 132 runtime cases, 44 actual-WASM package cases,
50 frontend assertions and all eight real-browser recovery/load scenarios, with
no failed, skipped, cancelled or todo cases.

Integrated collaboration browser measurements on this host:

| Case | Local navigation p95 | Text acknowledgement | Evidence |
| --- | ---: | ---: | --- |
| Four concurrent browser editors | 370.34 ms | 87.92 ms | Ten-second held solve; zero navigation computation RPCs |
| Dense manifold | 317.51 ms | 304.10 ms | Ten-second held solve; peer-visible typing |
| Gridfinity | 130.38 ms | 320.71 ms | Ten-second held solve; retained source metadata |
| Eight editors and 24 viewers | — | p95 302.24 ms | 24 text operations; disconnected subscriber recovery |
| Server authoring prediction | 88.61 ms | — | Held preview reply; one durable point terminal and cold restart |

The 32-client case sends 34,111 bytes and receives 2,855,265 bytes, including
891,654 SSE bytes. Peak ingress is eight; sampled process RSS grows from
664,215,552 to 701,243,392 bytes. A genuinely paused TCP reader reaches 120,128 bytes
under its 131,072-byte subscriber bound, is retired, and reconnects successfully while
nine healthy subscribers continue. These are bounded reference-host observations,
including Chromium software rendering and event delivery; they do not establish
60 Hz, arbitrary document size or unlimited users. Detailed diagnostics live in
`target/release-gate/runs/20260910T204328-4a05c31a/stages/collaboration.browser/output.log`.

Four matching offline archives and the 23-file production artifact were verified
without rebuilding. HTTP/MIME and actual-WASM readiness passed. The initial GUI
catalog was limited to four construction tools; the later toolbar amendment
superseded that limit. Server identity and full offline reconciliation remain
bounded reference-host responsibilities.

## Qualified local canvas boundary

Preceding candidate `d5f9e4048b428939a0bcd40c8a3433d4601fbd9b`, tree
`54975cf96815643f74f6b1b67e1e93883e07ca6b`, passes **261/261 obligations** in
`20260910T022428-a1d7c652`: **23 fresh and 238 authenticated reused results**, in
**31m30.382s**. Every linked passing receipt authenticates, and complete release,
clean-source and unchanged-source flags are true. Evidence:
`target/m98/local-canvas-final-qualification.json`.

The browser owns camera, analytic reprojection, hover/picking, selection and dimension
presentation in a dedicated Rust/WASM worker. The server owns compilation, solving,
sketch edits, history and persistence. Delayed server replies retain newer local camera
and selection state. Detached scene transport grants presentation capability only;
source, scene, revision, epoch and editing-lease guards still protect server mutations.
[Implementation and exact focused commands](M98_LOCAL_CANVAS.md) records the boundary.
No solver equations, priority semantics or explicit mathematical branches changed in this amendment.

All 17 sample-opening checks and 49 standalone browser workflows pass without failures,
skips or retries. Folder coverage passes 110 Node cases and 11 browser workflows;
engine coverage passes seven Node cases, with generator, browser embedding and offline
package checks also passing. Formatting, strict Clippy, native/headless tests, optimized
WASM lifecycle, seven native/WASM parity suites, licensing and the unchanged 271-case
golden corpus pass. Performance passes in 250.4 s, including independently validated
256-moving-body sparse crossover in 148.46 s.

The integrated stalled-server browser case presents local zoom in **133.4 ms** while
holding the edit reply for **1,580.4 ms**. All 20 mixed wheel samples reach local Rust,
with **zero navigation RPCs**, one accepted edit write, and the latest camera/selection
preserved after publication. Real drawing, cancellation, dragging, source Undo/Redo and
external generator failure retention pass. Evidence:
`target/release-gate/runs/20260910T022428-a1d7c652/stages/folder.browser/scratch/m98/browser/local-folder-navigation.json`.

Installed-manifold observations measured zoom at 302.9 ms and pan at 219.4 ms,
including browser delivery, polling and software rendering. Read-only verification
preserved all authored files and source authority. This established the local
navigation boundary rather than a 60 Hz or hardware-independent latency promise.

## Preceding qualified navigation latency repair

Candidate `b1243a6deec4eddad7dd0a0f941b29ea47e538a4`, tree
`9934cc679fe2a53ec12e1b7e62b5ef27ee5271b7`, passed **261/261 obligations** in
`20260909T221531-9189674e`: 11 fresh, 250 authenticated reused, 16m14.099s.
It included 101 folder Node tests, nine folder browser cases, 49 complete ordinary
browser workflows and 17 opening checks. Performance passed in 122 s, including
the 256-moving-body case in 114.81 s. The 271-case golden remained unchanged.

[M98-F016](M98_NAVIGATION_LATENCY.md) bounded stale input queues and compressed
exact full replies. A representative response shrank from 628,107 to 59,826 bytes
with decoded-byte equality. This improved network behavior but retained remote
navigation; the later local-canvas boundary superseded that architecture.

## Preceding qualified loading-feedback amendment

Candidate `a68fffa7ddd97d8b86f17eb98a7bf01c58db3fda` passed **261/261 obligations**
in `20260909T173058-0f35c852`: 16 fresh, 245 authenticated reused, 16m53.659s.
The [loading amendment](M98_LOADING_FEEDBACK.md) introduced a 500 ms delayed veil
and retained accepted geometry while solving ran independently. Its initial input
blocking was superseded by local browsing and authoring worker separation.

## Delivered authoring surface

- `packages/geosolve-sketch-code`: ordinary TypeScript generator recorder, optional
  `defineGenerator` inputs and existing equation-free SDK builders and custom patches.
- `crates/geosolve-sketch-engine`, dedicated WASM crate and `packages/geosolve-engine`:
  Node/browser evaluation, immutable accepted results, editable sessions, semantic design
  export and complete computed-profile export.
- `packages/geosolve-cli/runtime` (relocated from prototype scripts in M99):
  complete local dependency snapshots, revision-bound source and parameter edits, one
  canonical Linux folder owner and editing lease, recoverable publication and operation retries.
- Workbench folder/editable/generator flows, the plaintext manifold folder and the
  standalone Gridfinity-style generator website. The authoring quickstart supplies a
  complete CLI handoff/edit/retry example and a small generator host example.

Source owns dimension/parameter intent, labels, groups and generator input declarations.
The manifest selects entry and mode. Necessary semantic overrides live in the inspectable
`design.json`; derived cache/history cannot override independently recompiled source.

No solver equations, primitive families, priority semantics or golden bytes changed.
M98-F007 fixes certified profile containment for curve sections behind the rightward ray
origin, preserving rejection of true touching/uncertain topology. The manifold exports
18 bounded arrangement regions at 12, 10 and 11 mm channel widths; complete signed area is
28,800 mm², including retained holes and unchanged five radius-3 bores.

## Original qualification

The preceding product source is `6509e9c160a74c499668b63a5ef9bddefa593223`, tree
`f71ecd449ae6f911f2616fdc6a3331d628e6ad33`. Authenticated run
`20260909T144235-377abb33` passes **261/261 obligations** in **31m11.858s**:
23 freshly executed stages and 238 authenticated unchanged-input results. Every deferred
inventory expanded; `complete`, `qualified_release`, `clean_source` and `source_unchanged`
are true. Every linked stage receipt was independently unsealed and matched its stage/key.

Executed final command:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260909T144055-07e2eff5'
```

The full gate includes formatting, strict native/WASM Clippy, native workspace/domain and
headless tests, both release WASM builds, TypeScript/frontend checks, package/archive/license
checks, the unchanged **271-case clean golden**, actual browser workflows and performance.
All seven native/WASM interaction-parity groups pass. The 256-moving-body linkage test
passes in 139.26 seconds; the full performance stage passes in 393.5 seconds.

Final M98 group counts are 97 folder Node tests, seven folder browser tests, seven engine
Node tests, two generator Node tests, one complete generator browser workflow and two offline
package tests. Main browser evidence has 17 catalog/render prefixes and 48 complete workflows;
it is authenticated unchanged evidence from the first run, without skips or flaky outcomes.

## Original frozen review candidate (superseded)

The original frozen candidate and temporary preview endpoints were superseded.
Its product identity and automated results are retained above; session URLs and
process inventories are not part of the public qualification contract.

Earlier failed or interrupted nominations remain separate from qualified runs.
Only authenticated unchanged-input successes were reused:

| Run | Disposition |
| --- | --- |
| `20260909T132628-47a2b33d` | Failed: installed CLI generator timeout; 246 stages passed, 14 downstream stages not run |
| `20260909T142852-ee5601c1` | Failed: Split camera-baseline synchronization |
| `20260909T144055-07e2eff5` | Failed: preflight process-cleanup race |
| `20260909T213304-b37d7894` | Preceding browser evidence reused by the navigation repair |
| `20260910T005858-8cb8b936` | Failed: invalid-source browser completion witness |
| `20260910T224255-cf8bbde7` | Interrupted drag-repair nomination |
| `20260910T224940-a860c18d` | Failed: provisional-navigation witness |
| `20260911T130620-4b8e58fc` | Failed: engine WASM exceeded the unchanged 20 MiB ceiling |
| `20260911T133551-a403dc7b` | Failed: four collaboration browser cases; ordinary workflows passed 49/49 |
| `20260911T143755-117813be` | Interrupted; browser stage produced no completed receipt |
| `20260911T152048-7157230d` | Failed before browser assertions because a test server port was occupied |
| `20260911T153007-f3ebd337` | Failed: manifold navigation 506.9 ms exceeded the 500 ms limit; ordinary 49/49 and collaboration 15/16 passed |

The [hardening ledger](M98_HARDENING.md) records each owning repair and later
F041–F043 attempts. Budgets, geometry assertions and golden expectations were not
relaxed to obtain the final pass.

## Review scope and limitations

M98 remains mechanically qualified with **human acceptance open**. U02 is **Fail
pending human recheck**; all other unperformed rows remain **Not run** in
[M98 UAT](M98_UAT.md). M99's later acceptance does not change that ledger.

The workbench exposes the current native catalog, with writable-source restrictions
for generated roles/properties. The archived native-profile Fillet output action
is outside that catalog. The 500 ms navigation/text budgets, 500/200 ms cold/warm
drag-preview budgets and zero-reversal checks cover specified workloads. They do
not establish 60 Hz, arbitrary document size or unlimited clients. Dense startup
and semantic edits retain substantial cost; earlier local manifold Fit and
label-edit/Undo/Redo observations were 2.7 s and 17.9/7.3/7.4 s respectively, before
the final local navigation architecture.

Folder recovery targets Linux and cannot make filesystem CAS atomic against an
independent writer retaining an old descriptor. Generator workers execute trusted
code and are not security sandboxes; an already-running inline callback cannot be
preempted. Bounded 2D arrangement faces do not infer material removal, pocket depth
or manufacturing validity. New folders use v2; the v1 fixture retains a narrower
single-file compatibility workflow.

- [ ] Maintainer accepts the M98 candidate.
- [ ] Close M98 after acceptance.
