<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 loading feedback amendment

The supervising user requested a canvas loading indicator for solves taking longer than
0.5 seconds, after observing manifold edits taking 5–10 seconds without visible feedback.
This extends M98; the previous qualified product is `6509e9c`, nominated by `ef79b4a`.
Supervising-user acceptance and milestone closure remain open.

The canvas keeps its accepted geometry and, after 500 ms of outstanding work, displays a
grey veil and a compact “Solving…” spinner. Fast operations do not flash. Multiple queued
requests and the prepared-edit compiler round trip retain one pending indication. Success,
rejection, disconnect notification and worker disposal retire their own activity, with no
late completion allowed to clear newer work. The status is announced politely; reduced
motion retains the text and stationary indicator. Canvas accessibility reports pending work
immediately, independently of the visual delay.

New canvas gestures and view controls pause while the veil is visible. Captured pointer
movement, release, cancellation and capture loss continue through the existing input queue.
The canvas, renderer and last accepted frame remain mounted; the indicator creates no
document, solver, source, history or editing authority.

## Owning implementation

- `WorkbenchActivity` counts outstanding work and owns the 500 ms delay. Folder requests
  include queue time, retries, generator inputs and explicit/background refreshes.
- A separate named SSE `activity` event reports server work, including external disk
  evaluation. Connections receive current activity; completion uses `finally`. Unchanged
  scans produce no event chatter. These hints never install a snapshot or trigger a refresh.
- `WorkerWorkbenchAdapter` moves the standalone preview's synchronous WASM calls into a
  module worker. The same existing adapter validates every result. Ordered transport retains
  canvas-only updates, decode sequence and immutable frames; canvas updates transfer only
  their frame and matching base identity. Worker loss rejects pending calls and retains the
  displayed accepted scene.
- `SolvingIndicator`, `useWorkbenchBusy`, the viewport and canvas controls own presentation.
  M98-F015 additionally preserves the measured camera when folder rejection restores an
  accepted checkpoint; its independently reproduced regression is in `M98_HARDENING.md`.

No equations, tolerances, branch choices or independently validated success semantics change.
This is feedback and browser responsiveness, not a solve-time optimization. Standalone
managed TypeScript compilation still runs on the main thread between worker calls, so
that phase can briefly delay animation. Folder compilation and solving already use workers.

## Verification ledger

Focused development checks are recorded here before integrated nomination:

- `nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npm test -- --no-cache --maxWorkers=2 src/lib/workbench-activity.test.ts src/lib/folder-adapter.test.ts src/components/canvas-viewport.test.tsx src/App.generator.test.tsx src/lib/folder-generator.test.ts src/lib/folder-lifecycle-review.test.ts'`
  passes 54 tests, including exact delay, fast completion, overlaps, rejection, reconnect,
  unsubscribe and captured-pointer terminal behavior. An earlier non-Nix run timed out in
  the unchanged generator UI test; its unchanged retry in the pinned environment passes.
- Initial real standalone-WASM witness: manifold opening showed the overlay at 554 ms;
  a live width edit retained geometry and a browser frame callback completed in 17.1 ms
  during solving. No page errors. This developmental artifact predates final qualification.
- The first slow external-generator browser witness caught a real rollback camera reset;
  M98-F015 owns the failed evidence and correction. Its geometry assertion is retained.
- `nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npm run check:types'`
  passes. `nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npm test -- --no-cache'`
  passes all 299 tests across 23 files. The worker tests include both accepted and rejected
  prepared-edit resolution without restarting the visible delay.
- `nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && GEOSOLVE_BROWSER_COMPILER_HARNESS=1 GEOSOLVE_DIST=../../../target/m98/geosolve-loading-check npm run build:ui'`
  passes against the rebuilt M98-F015 WASM.
- `nix-shell shell.nix --run 'GEOSOLVE_DIST=target/m98/geosolve-loading-check node --test --test-name-pattern="slow external generator evaluation" scripts/workspace-browser.test.mjs'`
  passes the unchanged slow-evaluation/accepted-geometry rejection witness after F015.
  `target/m98/loading-folder-browser.log` and `target/m98/browser/slow-folder-solving.json`
  retain the result. A real standalone-WASM workflow is also registered in the integrated
  browser inventory. Readiness helpers now wait for accepted project identity and pending
  activity, because rendering the previous canvas remains possible during worker execution.

Integrated candidate qualification and refreshed preview evidence are pending. The previous
261-obligation qualification does not qualify these changed bytes.

### First integrated amendment attempt and harness corrections

Clean product `5e3c8ad` ran the integrated command
`nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260909T144235-377abb33'`.
Run `20260909T160542-79b4d8a4` remains failed (37m53.6s), with unchanged source.
Native/headless, strict Clippy, WASM/browser preparations and all 17 sample opening
prefixes passed. Of 49 freshly executed full browser workflows, 40 passed and nine
failed without skips/retries; the separate catalog workflow passed in the prefix stage.
No failed result supplies qualification evidence.

The failures exposed browser harness assumptions previously masked by synchronous WASM:

- Artifact readiness and four reload workflows used default five-second assertions
  while the worker was still opening/restoring the accepted project. The verifier retains
  its overall 60-second deadline, hashes/MIME checks and actual accepted-WASM assertions;
  reload checks await idle activity and paint before examining the scene.
- M95 attempted to type while the previous bootstrap editor was still read-only. New-code
  setup now waits for the accepted project header and editable editor.
- Segment, Cubic and Circle source assertions expired after 30 seconds while real native
  evaluation still displayed “Solving…”. The subsequent captured contexts show accepted
  publication. Source checks now await bounded worker readiness first, retaining their
  exact declaration, history and geometry assertions.
- The Fillet Undo baseline read transient CodeMirror measurement DOM, capturing one invalid
  line although accepted source and surrounding DOM snapshots were correct. The history
  comparison now uses complete persisted accepted source; visible-source/row/path checks remain.

Independent trace review found no new solver/history defect in these failures. Focused
repairs consume the same frozen artifact. `npm run check:types` and
`node --test scripts/test-release-artifact.mjs` pass (18 artifact tests). The corrected
verifier passes all 13 HTTP routes and actual manifold readiness at temporary loopback
port 18119 (`target/m98/loading-transport-r3.json`). The first local verifier invocation
omitted the pinned Chromium path and failed browser launch because the downloaded binary
lacked `libglib`; retry with `GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome`
passes without changing product bytes.

The focused rerun passes all nine affected workflows in 9.1 minutes against the
unchanged prepared harness, including manifold Segment/Cubic/Circle publication,
Fillet history, pins, reloads and both large fixture workflows. Exact invocation is
retained in `target/m98/loading-harness-commands.txt`; output and traces are
`target/m98/loading-browser-r2.log` and `target/m98/loading-browser-r2-results/`.

### Second integrated attempt: full browser pass and folder paint synchronization

Harness candidate `f4a2012` ran
`nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260909T160542-79b4d8a4'`.
Run `20260909T165704-00d40529` remains failed (31m55.5s), with unchanged clean source.
All 17 opening prefixes and all 49 full browser workflows pass without skips/retries.
Artifact transport/readiness, 299 frontend tests, 100 folder Node tests, engine tests,
native/headless reused evidence, documentation and completed WASM/package stages pass.

The folder browser stage passes seven of eight cases, including slow external loading,
rejected-geometry retention and complete manifold export. Its first external-rename case
reads the old canvas immediately after the parameter field shows the newly accepted value.
The field is published before the renderer's next animation frame. The focused replay
confirms a 1.2 width ratio at unchanged camera scale; the repaired test polls for the changed
presented geometry before retaining the original exact ratio, history, source and invalid-text
retention assertions. No product files change. Failed evidence is retained in that run's
`stages/folder.browser/output.log`.

Focused command against the same frozen harness:

```bash
nix-shell shell.nix --run 'GEOSOLVE_DIST=target/release-gate/prepared/021e46ed7e514f33084e7d2b6e8937bf1e10ea3c1f4c42bff25829d07ae9922d/browser/geosolve-harness GEOSOLVE_BROWSER_EVIDENCE=target/m98/folder-paint-fixed node --test --test-name-pattern="external rename updates" scripts/workspace-browser.test.mjs'
```

The focused external-rename workflow passes in 7.5 seconds, including rejected text,
GUI source writeback, restart and independent camera-scale checks
(`target/m98/folder-paint-fixed.log`).
