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
