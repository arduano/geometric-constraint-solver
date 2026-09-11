<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation and review candidate

M98, its local canvas boundary, multi-editor collaboration and full current toolbar
amendment are mechanically qualified and delivered. Supervising-user acceptance and
milestone closure remain open. Work is isolated on `m98/file-workspace`; accepted M97
remains unchanged.

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

Fresh captures immediately before each restart and final comparisons after browser
verification preserve source/model, shared draft text, target lifetimes, identity/epoch,
invitations, journals, retained files and every user's text/semantic history. Only transient
presentation point IDs and the three runtime lock/session files are excluded.

| Existing preview | Preserved accepted revision | Retained files | Users |
| --- | ---: | ---: | --- |
| Playground, port 18112 | 12 | 59 | tab-a, tab-b, tab-c |
| Shared manifold, port 18111 | 1 | 25 | arduano, review-editor, review-viewer |

Original private invitation URLs are unchanged. Current process/link records are
`target/m98/playground-preview-location.json` and `target/m98/manifold-tool-parity-preview-location-20260911T164104-576e7789.json`.
The playground was completely verified before replacing the manifold. Each existing
folder reopened with the same invitations and journal, without initialization. Static
18110 remains on `513463f`; the original 18108 manifold and its 40 mm reservoir/12 mm
channels remain preserved.

Executed commands use the pinned environment:

```sh
nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'COMMAND'
```

The inner qualification and delivery commands were:

```sh
./scripts/release-gate.sh --since 0af5b82
./scripts/release-gate.sh --resume 20260911T143755-117813be --since 0af5b82
./scripts/release-gate.sh --resume 20260911T152048-7157230d --since 0af5b82
./scripts/release-gate.sh --resume 20260911T153007-f3ebd337 --since 0af5b82
python3 target/m98/verify-qualification.py 20260911T164104-576e7789
python3 target/m98/coordination/tool-parity/extract-final-evidence.py 20260911T164104-576e7789 target/m98/coordination/tool-parity/final-summary-20260911T164104-576e7789.json
python3 target/m98/freeze-preview.py 20260911T164104-576e7789
python3 target/m98/coordination/drag-repair/install-qualified.py 20260911T164104-576e7789
python3 target/m98/coordination/tool-parity/deliver-one.py 20260911T164104-576e7789 playground
python3 target/m98/coordination/tool-parity/deliver-one.py 20260911T164104-576e7789 manifold
```

Per-service executed records retain every actual helper argument:
`target/m98/coordination/tool-parity/playground-executed-20260911T164104-576e7789.json` and `target/m98/coordination/tool-parity/manifold-executed-20260911T164104-576e7789.json`. The reviewed sequence is fresh
capture, authenticated restart, capture/comparison, exact transport/browser readiness,
then final capture/comparison. Final receipts:

- `target/m98/coordination/tool-parity/playground-delivery-20260911T164104-576e7789.json`
- `target/m98/coordination/tool-parity/manifold-delivery-20260911T164104-576e7789.json`
- `target/m98/coordination/tool-parity/playground-final-preservation-20260911T164104-576e7789.json`
- `target/m98/coordination/tool-parity/manifold-final-preservation-20260911T164104-576e7789.json`

The initial plain gate invocation selected documentation-only verification because its
`HEAD^` contained only prose. That receipt was not product qualification; explicit
`--since 0af5b82` selected the changed-product inventory.

First product run `20260911T130620-4b8e58fc` on `94a24c4` failed browser preparation:
the engine module was 21,842,414 bytes, above the unchanged 20 MiB ceiling. Native,
headless and optimized-WASM lifecycle checks had passed. `ec9362b` added the existing
release optimizer to private engine staging. The focused module became 14,068,507 bytes
and all 66 actual-WASM construction/operation/preview checks passed. The failed run
remains failed; only authenticated unchanged-input passing receipts can supply reuse.

Second run `20260911T133551-a403dc7b` on `ec9362b` passed optimized preparation,
engine/folder/runtime/package/frontend checks and all 49 ordinary browser workflows
(1,824.8 seconds), but failed four collaboration browser checks. The Fillet check read
paint before its asynchronous queue caught up; waiting for the actual provisional Fillet
item retains the original assertion. Collaboration latency now runs exclusively within
the gate, because concurrent ordinary browser work contaminated its measurements.
No thresholds or geometric assertions were relaxed.

Isolated repetition still failed cold drag, server Offset and manifold timing, exposing
synchronous GL error-query stalls up to 455.8 ms. The previous qualified frontend also
failed under the same host conditions, with a 421.5 ms query stall; that comparison does
not establish a new toolbar/solver regression. F039 addresses this presentation bottleneck.
Its 8 ms polling trial passed Fillet, server Offset and manifold but failed cold drag at
500.8 ms. Final 4 ms polling passed focused drag at 416.2 ms cold and 129.5/116.8 ms warm,
zero reversals, and 894.5/443.4/358.7 ms release-to-peer. All 39 renderer unit tests,
TypeScript and four actual-renderer browser workflows passed. These focused results
remain separate from the final integrated measurements above.

Third run `20260911T143755-117813be` on `8ac6d11` was externally interrupted
when its launcher exited with status 143. It had no reported failed assertions, but its
ordinary browser stage lacked a completed receipt and the run never qualified. The
orphaned browser process was terminated before resuming in a process independent of
the terminal session. The final run reuses only runner-authenticated completed results
and reruns the interrupted browser stage; no receipt was manufactured for unfinished work.

The first resumed run `20260911T152048-7157230d` recovered completed evidence but failed
browser startup because the interrupted run had left its separately spawned test HTTP
server on loopback port 4173. No browser assertions ran in that attempt. After the gate
settled, the exact orphan was authenticated by PID/start time, command, working directory
and its original browser-context record, then terminated. The final resume retains this
failed harness attempt and reuses only eligible completed evidence. No product, assertion
or timeout changed to address the port collision.

Run `20260911T153007-f3ebd337` on `8ac6d11` passed 49/49 ordinary browser workflows and 15/16
collaboration workflows but failed manifold navigation at 506.9 ms against the unchanged
500 ms budget. F040's immediate queued submission alone passed manifold at 438.5 ms but
failed four-editor navigation at 560.3 ms. Exact shader-source traces identified blur
program compilation before wheel delivery; startup preparation removed that input-time
compilation. Three focused shader setup/restoration/failure regressions failed before the
repair; all 44 renderer unit tests and TypeScript checking passed afterward. The final
focused three-workflow run passed: cold/warm drag p95 450/173.9/116.5 ms with zero reversals,
four-editor navigation 363.1 ms p95, and manifold navigation 461.8 ms p95 with 351.9 ms text
acknowledgement. All four actual-browser renderer workflows passed in 56.9 seconds,
including independent batch and horizontal/vertical blur shader-loss injections with
line/text pixel evidence. These focused measurements remain separate from the final
integrated measurements above. Two preceding renderer harness attempts failed before
assertions because of a relative manifest path and missing bundled-Chromium host libraries;
the passing attempt used the existing artifact and the same installed Chrome as collaboration
qualification. No assertion or artifact was changed for those harness corrections.

The final integrated nomination `20260911T164104-576e7789` on `b005b9e1bdb1a120ec9363d4c8684d1f8f5c3d00` resumes only eligible
results from `20260911T153007-f3ebd337`; all failed attempts remain failed evidence.

Two earlier focused toolbar attempts had locator errors: Dimension matched a toolbar
button and Branch target missed a nested select. Exact combobox locators fixed only
those queries; native traces already showed a complete candidate. The failed observations
remain recorded under `target/m98/coordination/tool-parity/`, including the failed/interrupted product
logs and authenticated resume, isolated/prior-artifact comparisons, renderer before/after and final focused logs.

Parity covers the current React catalog and controls. The archived native-profile Fillet
output action remains separate work; generated role changes require a writable source
role path. Trusted invitation identities and no full offline semantic reconciliation
remain protocol limits. Supervising-user acceptance and M98 closure remain open.

Documentation-only handoff uses
`./scripts/release-gate.sh --docs-only --since b005b9e1bdb1a120ec9363d4c8684d1f8f5c3d00`.
It preserves this exact qualified product and installed bytes; no rebuild or milestone
acceptance follows documentation changes.

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

Exact production (23 files) and four archives are frozen and installed offline without
rebuilding; all **283 installed package files** match authenticated archive hashes.
The existing **18112 playground** and **18111 shared manifold** each pass **24 HTTP/MIME
routes**, two-editor readiness and the exact three nominated WASM modules with zero page
errors. Browser verification makes no authored edits. A final comparison after browser
verification preserves all retained disk files, accepted/working source, native model,
shared target lifetimes, document identity/epoch, invitations and each user's text and
semantic history. Transient presentation-native point-target IDs and process locks/session
metadata are excluded from equality; none of them confer durable authoring authority.

| Existing preview | Preserved accepted revision | Retained files | Users |
| --- | ---: | ---: | --- |
| Playground, port 18112 | 6 | 32 | tab-a, tab-b, tab-c |
| Shared manifold, port 18111 | 1 | 25 | arduano, review-editor, review-viewer |

Original private links remain unchanged. Current process/invitation records:
`target/m98/playground-preview-location.json` and
`target/m98/manifold-collaboration-preview-location-20260910T233626-b8af5961.json`.
Complete settled folders and external invitations were backed up before reopening the same
journals, without initialization. Static port 18110 remains on preceding `513463f`; the old
combined location record is historical and still owns that static process. The original
18108 manifold and its 40 mm reservoir/12 mm channels remain preserved.

Qualification and delivery use the pinned Nix shell shown below. Executed gate/install
commands (all passed):

```sh
./scripts/release-gate.sh --since 513463f
python3 target/m98/verify-qualification.py 20260910T233626-b8af5961
python3 target/m98/freeze-preview.py 20260910T233626-b8af5961
python3 target/m98/coordination/drag-repair/install-qualified.py 20260910T233626-b8af5961
```

The executed delivery sequence and exact per-service arguments are recorded in
`target/m98/coordination/drag-repair/delivery-executed.md`. Each service ran fresh
`capture-preview.mjs`, `restart-preview.py` with that capture, `compare-preview.mjs`,
`verify-delivery.mjs`, then a final capture/comparison. All final checks pass. Receipts
under `target/m98/coordination/drag-repair/` include:

- `playground-delivery-r2-20260910T233626-b8af5961.json`
- `manifold-delivery-20260910T233626-b8af5961.json`
- `playground-final-preservation-20260910T233626-b8af5961.json`
- `manifold-final-preservation-20260910T233626-b8af5961.json`

The initial delivery helper failed before stopping a service because it passed `opener`
to `Path.open`; the corrected helper uses Python's built-in `open`. The first playground
verifier expected standalone `accepted` instead of the existing detached
`accepted-presentation` provenance. Correcting only the ignored helper preserves the
strong ready/authority/byte assertions, and the same installed bytes pass. Both failed
helper observations remain recorded; no product rebuild or full-gate rerun followed them.
The interrupted `20260910T224255-cf8bbde7` and failed `20260910T224940-a860c18d`
nomination attempts remain historical, with only authenticated unchanged-input successes
eligible for reuse.

At this preceding checkpoint, generated-polyline Undo `.invocation` and broader GUI
authoring remained open. F031 and the qualified full current toolbar amendment above
supersede those limits. Production identity providers and full offline reconciliation
remain separate work. **M98 supervising-user acceptance and closure remain open.**
Documentation-only closeout uses `./scripts/release-gate.sh --docs-only --since 9b64c29`
and preserves the qualified product and installed bytes.

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

The new static preview is `http://100.94.63.83:18110/`; the invited collaborative
manifold is on `http://100.94.63.83:18111/`. Private editor/viewer invitation URLs
and process records are in
`target/m98/collaboration-preview-location-20260910T204328-4a05c31a.json`.
The four matching offline archives and 23-file production artifact are frozen from
authenticated gate receipts, installed offline with an empty npm cache and verified
file by file, without rebuilding. Both endpoints pass all 24 exact HTTP/MIME routes.
Static actual-WASM manifold readiness passes; collaborative editor/viewer readiness
loads all three nominated WASM modules and presents 182 geometry items without errors.
Local zoom is observed at 84.88 ms with an independent peer camera, zero navigation
RPCs and zero document mutations.

The original seven authored manifold files are preserved byte for byte, including
the user's 40 mm reservoir and 12 mm channels. The shared preview uses a separate
copy and a fresh collaboration journal; legacy semantic overrides/history are not
migrated. Its copied legacy overrides were empty. Initial shared accepted revision
is zero and input is `f9717c6194f43f064b5caf8a723d6ef8e7a8fdff4e62d1bb59edc503d308c126`.
Verification preserves accepted/working source, model and authority. The original
18106/18108 services remain live with original current/accepted hash
`01fe512c5221bccbc1a032eb32480101005c9ca3571f99e504a1619647b3d59a`.

Delivery evidence:

- `target/m98/installed-collaboration-preview-20260910T204328-4a05c31a.json`
- `target/m98/collaboration-static-verification-r1.json`
- `target/m98/installed-collaboration-preview-20260910T204328-4a05c31a/collaboration-verification-r2.json`
- `target/m98/installed-collaboration-preview-20260910T204328-4a05c31a/collaboration-preview.png`
- `target/m98/collaboration-delivery-preservation.json`

The gate, installation, launch and browser commands use the pinned shell; the
receipt verifier and freeze helper run directly with the same available Python:

```bash
nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'COMMAND'
```

```bash
./scripts/release-gate.sh --plan --since d5f9e40
./scripts/release-gate.sh --since d5f9e40
python3 target/m98/verify-qualification.py 20260910T204328-4a05c31a
python3 target/m98/freeze-preview.py 20260910T204328-4a05c31a
python3 target/m98/install-collaboration-preview.py 20260910T204328-4a05c31a
python3 target/m98/launch-collaboration-preview.py 20260910T204328-4a05c31a
env GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome node target/m98/verify-collaboration-preview.mjs target/m98/installed-collaboration-preview-20260910T204328-4a05c31a/verification-config-r2.json
```

The exact static `npm --prefix crates/geosolve-demo-web/frontend run verify:artifact`
invocation, including absolute manifest/directory/receipt paths and endpoint, is retained
in `target/m98/collaboration-static-verification-r1.log`. Every command above passed.
The first two integrated attempts remain failed historical runs: the M98 owning-file
inventory and folder camera-baseline harness repairs are documented in
[M98_HARDENING.md](M98_HARDENING.md). The initial delivery verifier incorrectly expected
standalone scene provenance `accepted`; local presentation intentionally reports
`accepted-presentation`. Correcting only the ignored helper makes the same bytes pass;
the failed receipt remains preserved. No qualified product changes followed nomination.

At this initial collaboration checkpoint, GUI construction supported Segment, Polyline,
Center-radius Circle and Two-point aligned Rectangle, plus native point dragging. Other
GUI routes disclosed unavailable actions. The full current toolbar qualification above
supersedes that limit. Trusted invitation roles remain the reference identity boundary;
production identity providers and full offline semantic reconciliation remain separate work. Restart the shared folder with
its existing invitations and journal, omitting the first-launch `--initialize true`.
**Supervising-user acceptance and M98 closure remain open.**

The documentation-only handoff is checked with
`./scripts/release-gate.sh --docs-only --since 513463f`; it preserves this qualified
product identity and the installed preview bytes.

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

The static preview `http://100.94.63.83:18106/` and editable manifold on Tailscale port
18108 serve the exact frozen/installed qualified artifacts, without a subsequent rebuild.
Both sets of 14 HTTP routes match qualified bytes; static actual-WASM manifold readiness
passes. The folder verifier removes only the existing documented HTTP compatibility shim
for its index comparison. It verifies 372 finite presented items, working SSE, exact
identity/gzip decoded equality and completed local zoom, pan and click with zero navigation
RPCs. All 12 observed local worker requests receive replies, with none pending.

On the installed manifold, zoom is visible in **302.9 ms** and pan in **219.4 ms**. These
bounded Playwright observations include event delivery, polling and software-rendered
Chromium; they do not establish 60 Hz or the user's hardware performance. Selection
highlights are local; Inspector details and semantic edits may still await the server.

The original folder at `target/m98/installed-preview-20260909T144235-377abb33/manifold`
and all seven authored files remain unchanged. Current and accepted source identity remain
`01fe512c5221bccbc1a032eb32480101005c9ca3571f99e504a1619647b3d59a`.
Read-only verification does not claim the new editing lease or change source, authority or
writes. The current session URL is in `target/m98/tailnet-folder-location.json` and
`target/m98/local-canvas-preview-verification.json`. Static evidence is
`target/m98/local-canvas-static-preview-verification.json`; service/file preservation is
recorded in `target/m98/local-canvas-preview-processes.json`.

Executed qualification and delivery commands use the pinned Nix shell documented in
[M98 local canvas](M98_LOCAL_CANVAS.md):

```bash
./scripts/release-gate.sh --resume 20260910T005858-8cb8b936
python3 target/m98/verify-qualification.py 20260910T022428-a1d7c652
python3 target/m98/freeze-preview.py 20260910T022428-a1d7c652
python3 target/m98/install-preview.py 20260910T022428-a1d7c652
python3 target/m98/upgrade-local-canvas-preview.py 20260910T022428-a1d7c652
node target/m98/verify-local-canvas-preview.mjs
```

The static `npm run verify:artifact` command, manifest, moved directory and endpoint are
retained exactly in its receipt. It requires
`GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome` on this host.
An initial invocation could not launch the bundled Chromium because its Linux libraries
were unavailable; a second could not overwrite the deliberately exclusive receipt path.
Both failures remain recorded. The final invocation uses host Chromium and a fresh receipt,
and passes against the same served bytes. No product change or full gate rerun followed
these local verifier setup errors.

M98 supervising-user acceptance and milestone closure remain open.

## Preceding qualified navigation latency repair

The preceding candidate `b1243a6deec4eddad7dd0a0f941b29ea47e538a4`, tree
`9934cc679fe2a53ec12e1b7e62b5ef27ee5271b7`, passes **261/261 obligations** in
`20260909T221531-9189674e`: 11 fresh and 250 authenticated reused results in
**16m14.099s**. `complete`, `qualified_release`, `clean_source` and `source_unchanged`
are true; every linked passing receipt was independently authenticated.
`target/m98/latency-final-qualification.json` records the verification.

The gate covers formatting, strict Clippy, native/headless and WASM parity, frontend,
archive/license checks, the unchanged 271-case golden, browser workflows and performance.
Folder coverage passes 101 Node tests and nine browser tests. Main browser coverage
reuses the authenticated 49 complete workflows and 17 initial sample/render checks from
`20260909T213304-b37d7894`, without skips or flaky results. The performance stage passes
in 122 seconds, including the 256-moving-body case in 114.81 seconds.

[The navigation repair report](M98_NAVIGATION_LATENCY.md) records exact queue/response
contracts, failed timeout attempts, focused diagnoses and browser synchronization changes.
The sample lifecycle now settles presentation after selection/isolation and allows eight
minutes for its complete two-edit lifecycle. The manifold browser test awaits its exact
parameter-edit response before retaining the original source and complete-export assertions.
These test synchronization changes preserve mathematical and accepted-state assertions.

Both previews now serve the qualified frozen/installed artifacts: static
`http://100.94.63.83:18106/` and the editable folder on Tailscale port 18108.
`target/m98/latency-static-preview-verification.json` passes exact static bytes and
actual-WASM manifold readiness. `target/m98/latency-preview-verification.json` verifies
13 folder HTTP routes against qualified bytes, a ready WebGL2 canvas with 364 finite
items, working SSE, and exact decoded identity/gzip equality: 628,107 bytes become
59,826 bytes. The new folder session URL is recorded there and in
`target/m98/tailnet-folder-location.json`.

Replacement preserves the original user folder at
`target/m98/installed-preview-20260909T144235-377abb33/manifold` and all seven authored
files. Current/accepted source identity remains
`01fe512c5221bccbc1a032eb32480101005c9ca3571f99e504a1619647b3d59a`.
Backend replacement creates a new token and authority epoch; read-only browser verification
leaves the new editing lease unclaimed and preserves authority/source/write state.
`target/m98/latency-preview-processes.json` records the preserved file hashes.

Private-copy probes use the exact installed product with `GEOSOLVE_DIST` unset. Under
100 ms simulated latency and 512 KiB/s throughput, 12 wheel inputs produce three requests
and three frames; the final response arrives 561.7 ms after input stops, compared with
over 14.6 seconds before repair. Final presentation follows at 703.4 ms. For 90 hover
inputs, the final response arrives after 302.6 ms unthrottled (77 requests), or 390.2 ms
with 40 ms latency and 1 MiB/s throughput
(34 requests), compared with 2.866 seconds before repair. Both probes report no browser
errors. Hover records no long tasks; wheel replay retains three main-thread long tasks
of 111–150 ms. These controlled measurements do not assert the user's actual network latency.
Evidence is `target/m98/latency-folder-wheel-browser-qualified-20260909T221531-9189674e.json`
and `target/m98/latency-folder-browser-qualified-20260909T221531-9189674e.json`.
Solving, Fit and semantic edit costs remain unchanged. M98 acceptance and closure remain open.

## Preceding qualified loading-feedback amendment

The preceding candidate is `a68fffa7ddd97d8b86f17eb98a7bf01c58db3fda`, qualified by
`20260909T173058-0f35c852`: **261/261 obligations**, 16 fresh and 245 authenticated reused
results, in 16m53.659s. [The amendment report](M98_LOADING_FEEDBACK.md) records implementation,
failed harness attempts, final commands, limitations and frozen-byte verification.
The canvas greys out after 500 ms and retains accepted geometry while standalone native
solving runs in a worker. Managed TypeScript compilation remains on the main thread.

That delivery refreshed both M98 previews and preserved the original user's writable
manifold folder and all seven existing files, leaving its editing lease unclaimed.
Its token and authority epoch are historical; current replacement verification is recorded
above. The following detailed base-milestone record is also historical. M98 acceptance
and closure remain open.

## Delivered authoring surface

- `packages/geosolve-sketch-code`: ordinary TypeScript generator recorder, optional
  `defineGenerator` inputs and existing equation-free SDK builders and custom patches.
- `crates/geosolve-sketch-engine`, dedicated WASM crate and `packages/geosolve-engine`:
  Node/browser evaluation, immutable accepted results, editable sessions, semantic design
  export and complete computed-profile export.
- `scripts/workspace-*`, `scripts/geosolve-cli.mjs` and `packages/geosolve-cli`:
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


First clean run `20260909T132628-47a2b33d` on `e0c2167bd56279262c4d29cfcf4fd9e0c509e124`
ended failed after 3,248.924 seconds: 246 stages passed, the offline package stage failed,
and 14 downstream stages did not run. Source stayed clean and unchanged. Its installed
CLI generator check hit the existing 90-second command timeout. This run is not release
qualification; successful independent stages remain available only through authenticated
runner reuse.

That run passed 97 folder Node tests, seven folder browser tests, seven engine Node tests,
two generator example Node tests and one complete generator browser workflow. Main browser
qualification passed 17 initial catalog/render witnesses plus 48 full workflows, without
skips, failures or flaky outcomes. Focused pre-gate findings remain in M98_HARDENING.md.

The second run `20260909T142852-ee5601c1` found the Split camera-baseline synchronization
race recorded in [hardening](M98_HARDENING.md); the exact geometry assertion remains after
waiting for the presented CSS extent. Third run `20260909T144055-07e2eff5` stopped in a
preflight process-cleanup self-test when `/proc/PID/stat` disappeared between its existence
check and read. The unchanged 13-test harness recheck passed, followed by final integrated
preflight. That pre-existing harness race remains unfixed; no assertion or process timeout
was relaxed. All failed attempts remain failed evidence, distinct from the final pass.

## Original frozen review candidate (superseded)

- Workbench: **http://100.94.63.83:18106/**.
- Editable manifold: installed CLI on **127.0.0.1:18108**. Open the exact `url` in
  `target/m98/folder-preview.log` or the manifold's `.geosolve/session.json`, including
  `?folder=1` and its session token. A remote browser can use an SSH loopback forward
  preserving port 18108 and the printed URL.
- Frozen production and archives:
  `target/m98/preview-20260909T144235-377abb33/`.
- Installed tools and writable manifold:
  `target/m98/installed-preview-20260909T144235-377abb33/`.

The freeze follows authenticated `prepare.browser` and `package.m98` receipts, checks their
complete output/evidence hashes, copies production and all three archives into a new directory
and removes write permission. No product rebuild followed qualification. The production
contains 12 files, 29,150,101 bytes; its ordered file hash is
`c7db42e3e4e0ec5e0e0f684d38815d3e9330809e51fa313bccfc598a89806180`.

| Archive | SHA-256 |
| --- | --- |
| `geosolve-sketch-code-0.2.0.tgz` | `dfb9bea7ee7bb8723653d8ac26b051c6bf9c79d8bacebe6649c63ee64cd9bc85` |
| `geosolve-engine-0.1.0.tgz` | `8761366ab9f0855bbd0423d6ab03f3f9c8d74e18ddd69bf933e578c2f6feece9` |
| `geosolve-cli-0.1.0.tgz` | `473d9199eedfdced4e90a0a3a5c799cc7a0bc1d5a5df0d0509030b087303fce0` |

The package manifest hash is
`d6edb22b9a28d84ece66403b3b295a28f41a682e01df6a1c7361629fa4bd4682`.
The preview installer uses `npm install --offline --ignore-scripts --no-audit --no-fund`
with those exact archives and a newly empty npm cache. It copies each manifold source
from the qualified Git commit and checks it against the signed source map.

Executed preview commands, from the M98 worktree:

```bash
python3 target/m98/freeze-preview.py 20260909T144235-377abb33
node target/m98/serve-preview.mjs
python3 target/m98/install-preview.py 20260909T144235-377abb33
GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome \
  node crates/geosolve-demo-web/frontend/scripts/verify-artifact.mjs \
  --manifest target/m98/preview-20260909T144235-377abb33/production.json \
  --directory target/m98/preview-20260909T144235-377abb33/geosolve-production \
  --url http://100.94.63.83:18106/ --receipt target/m98/static-preview-verification.json
env -u NODE_OPTIONS -u NODE_PATH -u GEOSOLVE_DIST \
  target/m98/installed-preview-20260909T144235-377abb33/node_modules/.bin/geosolve \
  serve target/m98/installed-preview-20260909T144235-377abb33/manifold --port 18108
GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome \
  node target/m98/verify-folder-preview.mjs
env -u NODE_OPTIONS -u NODE_PATH -u GEOSOLVE_DIST \
  target/m98/installed-preview-20260909T144235-377abb33/node_modules/.bin/geosolve \
  bake target/m98/installed-preview-20260909T144235-377abb33/manifold \
  --out target/m98/installed-manifold-profiles.json --chord-error-mm 0.02
python3 target/m98/verify-cli-quickstart.py
```

Static verification passes all 13 HTTP routes, MIME checks and actual-WASM manifold readiness
in Chromium 151, with 182 accepted geometry/computed items and no browser errors. Installed
folder verification separately passes all 13 byte-identical routes, a real accepted WebGL2
manifold, saved/accepted hash equality and visible 12 mm / 2.4 mm width values. Folder mode
runs native WASM in the CLI worker; it does not load a second WASM engine into the browser.
Its license assets use `text/plain`, unlike the canonical static verifier's MIME policy;
the folder receipt accurately records that separate policy rather than claiming canonical
static transport verification. The 18-region installed export independently has finite
vertices, positive outer/negative hole winding and exact net area 28,800 mm².

The [authoring quickstart](M98_AUTHORING_QUICKSTART.md) passes through the exact installed
CLI: fresh v2 init, inspect, serve, status, takeover, fresh status, 10→14 mm apply, identical
retry, acknowledged outcome and independent validation. Its temporary bridge was stopped;
the manifold preview remains running. After browser verification, the exact installed CLI was
restarted to release the automated browser's editing lease. A fresh authenticated status read
confirms the same accepted/current identity and an unclaimed editor; all 13 route hashes
were verified again. `target/m98/folder-preview-final-session.json` owns the current token URL.
Receipts are `target/m98/static-preview-verification.json`,
`folder-preview-20260909T144235-377abb33.json`, `installed-manifold-profile-check.json`
and `cli-quickstart-verification.json`. The generator website's complete Node/browser and
clean-installed browser evidence passes; its built website directory was not retained for
an additional frozen live endpoint. Follow its maintained README to run it locally.

Documentation-only handoff verification (`./scripts/release-gate.sh --docs-only --since 6509e9c`)
preserves this exact qualified product and archives; it does not nominate rebuilt package
README bytes. The final command and receipt are in `target/m98/docs-verification-final.log`.

## Review scope and limitations

Open the installed manifold folder's exact printed loopback URL. Inspect the shared 12 mm
channel width and 2.4 mm seal groove width, change a value or label, inspect plaintext,
Undo/Redo and observe an external patch save. Browser state and folder state have separate
owners. Explicit handoff is required when moving editing authority to another tab or agent.

The [generator website](../examples/generator-website/README.md) owns controls and rendering through only the installed
SDK and headless engine. Complete bounded 2D arrangement faces and holes do not infer
pocket depth, material removal or manufacturing semantics. Unsupported/open/uncertain
geometry fails closed.

Navigation avoids disk writes, recompilation and full history serialization. Development
measurements still show manifold Fit around 2.7 seconds; its label edit/Undo/Redo measured
17.9/7.3/7.4 seconds. These are recorded limitations, not optimized or isolated benchmark
claims. Existing integrated performance assertions remain required.

Locking/recovery targets Linux and cannot provide atomic CAS against independent writers
retaining old descriptors. Generator workers execute trusted code and are not security
sandboxes. An inline callback already running in a custom host cannot be preempted. New
init defaults to folder-v2; v1 lacks semantic dragging and dependency-history capabilities.
No public registry publication or external consumer repository change is included.

- [ ] Supervising user accepts the M98 candidate.
- [ ] Close M98 after that acceptance.
