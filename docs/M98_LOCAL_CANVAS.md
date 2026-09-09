<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 local canvas interaction amendment

Status: implementation and focused qualification complete; integrated release gate pending.
The qualified `b1243a6` previews remain available; this amendment has not been release-qualified
or delivered. M98 acceptance stays open.

The user reports folder interaction remains much slower than the accepted M97 demo even
after M98-F016 bounded stale queues and compressed replies. They explicitly require local
responsive navigation, selection and highlighting, with sketch edits allowed on the server.

## Reproduction and owner

Folder mode sends pointer, wheel and resize through serialized HTTP. The browser worker
path runs those operations locally. Folder transport also expands the native canvas-only
result into a full workbench snapshot and loses its private fast-path marker across Node
structured cloning/JSON. Every changed canvas frame consequently republishes source,
Explorer and Inspector objects through React. The preceding correction removed backlog;
it did not remove those architectural costs.

A private-copy comparison against qualified `b1243a6` production confirms these paths.
At simulated 100 ms latency / 512 KiB/s, twelve wheel samples produce twelve local frames
versus three folder frames. Folder responses take 236–243 ms and its final frame arrives
710 ms after input stops. The local worker incurs no HTTP input requests. Software-rendered
Chromium remains CPU-limited in both modes; these measurements are not a claim of 60 Hz
or the user's actual hardware/network. Evidence and executed probe are
`target/m98/navigation-path-comparison.json` and `probe-navigation-path-comparison.mjs`.
Earlier probe attempts used a fixed coordinate outside the narrower split-mode canvas;
they correctly rejected navigation and are retained as invalid comparison evidence.

## Boundary

The server exports a bounded detached scene from the independently accepted model.
The browser uses the same Rust camera, domain-curve reprojection, picking, ordered
selection and dimension-presentation rules. Import grants presentation capability only;
it cannot establish an accepted solver result, prepared-input authority or an editing
coordinator. No source materialization, solver or computed-feature evaluation runs on
ordinary client navigation.

The client owns camera and current selection. A delayed server response must not rewind
newer local interaction. Installing a new accepted scene retains the local view, reconciles
vanished semantic IDs, and preserves source/field draft authority. Read-only clients can
navigate their accepted scene while server mutations retain the editing lease requirement.

Server commands carry the exact installed scene identity and an absolute local interaction
state when needed. The server verifies epoch, lease, interaction/source basis and semantic
selection membership before applying presentation plus the command in one serialized
operation. The complete request binds retry receipts. Scene data and local predictions
never grant source or disk-publication authority.

Primary selection paints immediately. Inspector details may settle asynchronously from
the server. Semantic dragging keeps its original press/view and every subsequent sample;
it escalates to the server-owned edit path without replacing semantic movement. Loading
feedback continues after 500 ms of remote work while local browsing remains available.

## Required qualification

- Detached transport preserves finite geometry, computed channels, annotations and exact
  hover/click ownership, and cannot be upgraded to inferred-edit authority.
- Compare native and detached camera/selection/dimension traces against the same scene,
  including mixed wheel anchors, pan, resize, computed corners and construction visibility.
- Deliberately hold server responses while real browser navigation and selection continue;
  assert changed frames before response release and zero hover/wheel/resize RPCs.
- Verify stale replacement, selection sync, editing lease, pending drafts, operation retry,
  failed publication and server edit/Undo/Redo with latest local camera preserved.
- Preserve full canvas-only transport, responsive busy behavior and existing server editing
  semantics. Run focused owner checks, then the integrated authenticated gate and frozen
  preview verification before making a replacement delivery claim.


## Focused development evidence

All commands below use `nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'COMMAND'`.
The retained source pins the same Rust, Deno and wasm-bindgen versions as the qualified
baseline; the updated host default package set has different tool versions.

- `CARGO_PROFILE_TEST_OPT_LEVEL=1 cargo test --locked -p geosolve-demo-web --lib local_canvas_ -- --nocapture`:
  seven integration cases pass in `target/m98/local-canvas-native-r4.log`. They cover exact
  camera/pan/resize, local selection, shared hover, dimension preferences, stale/malformed
  rejection, explicit curve pick context, and compatible semantic preview retention.
- `cargo test --locked -p geosolve-constraint-editor --lib detached_scene_transport -- --nocapture`:
  three owner cases pass; shared dimension-hover transit adds one passing owner case.
- `cargo test --locked -p geosolve-constraint-editor --test m83_projectional_fillet detached_selection -- --nocapture`:
  two cases pass for exact native/discarded picks and atomic stale/forged/draft rejection.
- `cargo test --locked -p geosolve-sketch-render --lib detached_fillet_scene -- --nocapture`:
  one case passes, proving inactive detached Fillet actions do not acquire an unintended ghost,
  while an explicit native active action still previews. Native and detached items match.
- `cargo clippy --locked -p geosolve-demo-web --lib --tests -- -D warnings`: passes after
  focused style repairs. No solver equation or branch implementation changed.
- `node crates/geosolve-demo-web/frontend/scripts/build-wasm.mjs --release`: passes;
  `build-workspace.mjs` and the paired browser artifact build also pass.
- Five focused backend local-interaction transaction tests pass against the rebuilt runtime;
  frontend type checks and 73 focused adapter/worker/canvas tests pass after browser followups;
  the later immediate-busy drag regression also passes with 67 relevant collateral tests.

The actual browser stalled-edit case passes against development artifact
`target/m98/local-canvas-browser-r1/geosolve-harness`: the server edit is held 1,655.6 ms,
local zoom becomes visible in 133.4 ms, and hover, pan and persistent selection continue.
All 20 mixed wheel samples reach local Rust exactly, with zero pointer/wheel/resize RPCs.
After release, the server publishes exactly one radius edit, accepts finite R12 geometry,
reconciles the latest selection and retains the exact local camera. Evidence is
`target/m98/local-canvas-browser-evidence-r1/local-folder-navigation.json`.
This measured bound includes test polling and software-rendered Chromium; it is not a
60 Hz claim or a user-hardware benchmark.

The separate slow external generator browser case exposed a stale background synchronization
error masking subsequent source diagnostics. That failed attempt remains recorded in
`target/m98/local-canvas-browser-r1.log`; its repaired r2 rerun passes. These development
checks do not establish full release qualification or replacement preview delivery.


Six actual-browser collateral cases pass in
`target/m98/local-canvas-browser-collateral-r1.log`: external rename and invalid-source
retention, stale draft and Revert, legacy cold reopen, source/Inspector handoff, generator
inputs, and complete manifold open/shared-channel edit/profile export. The manifold case
uses a private fixture; the user's installed folder remains untouched.

The real authoring followup also caught a tool activation handoff error: activating Polyline
already arms the native draft, so its first pointer move must continue that draft instead of
restoring selection again. The frontend now records the native active tool immediately.
Both this and the external-work synchronization error have focused failing-before and
passing-after regressions. Automatic synchronization waits for immediate pending activity
and installation of the resulting scene; a stale automatic synchronization response cannot
mask an authored diagnostic or relax the server's source/lease/revision guards. The rebuilt r2 browser confirms both repaired stalled-edit and slow-generator cases.


Final rebuilt development artifact `target/m98/local-canvas-browser-r2/geosolve-harness`
passes both stalled-edit and slow external generator cases (2/2, 27.645 s):
`GEOSOLVE_DIST=target/m98/local-canvas-browser-r2/geosolve-harness GEOSOLVE_BROWSER_EVIDENCE=target/m98/local-canvas-browser-evidence-r2 nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'node --test --test-name-pattern="M98-F016/F017|slow external generator" scripts/workspace-browser.test.mjs'`.
The server is held 1,861.0 ms; local zoom is visible in 276.6 ms under concurrent build/browser
host load, with all 20 wheel samples and zero navigation RPCs. Error diagnostics remain visible
after rejected generator execution. Log: `target/m98/local-canvas-browser-final-r2.log`.
A preceding invocation accidentally omitted `GEOSOLVE_DIST`, exercised old default frontend
bytes, and failed; `local-canvas-browser-r2.log` is not current-artifact evidence.

`cargo fmt --all -- --check` passes. The full drawing/dragging browser regression and integrated
release gate remain pending. No current development result establishes preview delivery.

The independent real-browser point-drag case passes in 7.75 s against r2. It checks exact
press/subthreshold/movement/release delivery, camera transfer once, finite accepted native
movement, one sidecar publication, unchanged TypeScript source, exact Undo/Redo and stable
camera. Evidence is `target/m98/local-canvas-drag-r2`; command uses the same pinned shell and
artifact with `node --test --test-name-pattern="M98-F017 local folder point drag" scripts/workspace-browser.test.mjs`.

The polyline workflow passes real draft previews, three clicks, Finish with one write,
local zoom/pan and Undo. Redo exposes a separate folder-history authentication rejection:
`Historical dependency bytes do not match the accepted Undo/Redo project` (HTTP 400).
The prior accepted source and model are retained transactionally. Its backend-owner repair
passes focused checks; the exact failing evidence remains in `target/m98/local-canvas-authoring-r2b`.


A final existing-WASM trace and actual r2 browser case also reproduce cancellation followed
by navigation in an armed geometry tool rejecting the next camera/selection handoff. The
editor projected `geometry_draft_status()` for an armed mode even when no shape stages
remained. The owner regression now distinguishes an armed mode from actual draft storage,
active pointer and pending construction commit. Exact final Rust/browser checks remain pending;
`target/m98/local-canvas-armed-r2.log` retains the browser failure.


F018 backend checks pass: three focused cases in
`scripts/workspace-transaction-review.test.mjs`, followed by
`node --test scripts/workspace-project.test.mjs`
(five existing project cases, 26.6 s). Both use the pinned shell. Exact original shorthand
source restores on Undo, authored project/design restores on Redo, history survives reopen,
and subsequent canvas authoring retains the monotonic `geometry2` name. Forged actual source
bytes with a matching forged hash still fail independent historical compilation before
publication; accepted source/project/design remain unchanged and writes stay zero.
`target/m98/coordination/backend-redo-status.md` records the original failing regression and
field-level mismatch. No solver equations changed.


The armed-tool correction passes both owning tests: `selection_handoff_allows_armed_mode_but_rejects_live_draft_and_pending_commit`
and `local_canvas_navigation_handoff_preserves_armed_tool_and_uses_new_camera`.
Commands use `CARGO_PROFILE_TEST_OPT_LEVEL=1 cargo test --locked -p geosolve-constraint-editor --lib selection_handoff_allows_armed_mode -- --nocapture`
and the corresponding `-p geosolve-demo-web --lib local_canvas_navigation_handoff` filter.
The first fails against the previous guard and passes after repair; actual committed segment
coordinates match the client's inverse camera after cancellation and zoom. Staged drafts,
active pointers and pending construction commits remain protected. Evidence is
`target/m98/armed-selection-before-r2.log` and `armed-selection-after.log`.


## Final focused browser result

Final rebuilt r3 passes **4/4** browser cases in **37.335 s**: stalled server edit with local
navigation/selection; real Polyline drafts, local zoom/cancellation, three-point Finish, exact
original-source Undo/Redo and point dragging; independent exact-sample point drag/history;
and slow external generator acceptance/rejection with navigable loading feedback.

Executed command:

```bash
GEOSOLVE_DIST=target/m98/local-canvas-browser-r3/geosolve-harness \
GEOSOLVE_BROWSER_EVIDENCE=target/m98/local-canvas-browser-evidence-r3 \
nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source \
  --run 'node --test --test-name-pattern="M98-F016/F017|M98-F017 local folder|slow external generator" scripts/workspace-browser.test.mjs'
```

`target/m98/local-canvas-browser-final-r3.log` and the evidence directory retain exact input
samples, camera, native geometry and publication counts. Original shorthand source is restored
byte-for-byte; the earlier explicit-binding fixture workaround has been removed. Final demo
WASM release, workspace runtime, paired harness/production builds and scoped strict Clippy pass.
These focused checks nominate the implementation for the integrated release gate; previews
remain on the previous qualified product until that gate and served-artifact verification pass.

## Integrated qualification followup

Candidate `2d0259a` completed run `20260910T005858-8cb8b936` with 251 passing stages,
one failed browser stage and nine downstream stages not run (4,575.809 s). All 17 browser
opening prefixes and 48 of 49 full workflows passed. The failed standalone invalid-source
workflow expected its terminal header within five seconds without awaiting asynchronous
completion. This failed attempt remains failed; the followup gate must authenticate reuse
of unaffected successes and execute the failed and outstanding obligations.

An initial test correction used the existing presentation-completion helper, but that helper
excluded the intentionally hidden canvas in Code layout. Its 60-second timeout reported
`element(s) not found`, not a busy canvas. An actual worker probe against the exact immutable
gate artifact confirmed rejection after 2.511 s, one positioned Problem, hidden-canvas
`aria-busy=false`, no pending worker requests, successful Revert and no browser errors.
Evidence: `target/m98/invalid-source-worker-probe.json`. No production lifecycle defect was
established and no production code changed for this qualification followup.

The helper now includes the retained hidden application when awaiting presentation completion;
the workflow waits after both Apply and Revert. All existing status, exact accepted-frame,
single positioned Problem and export-rejection assertions remain unchanged. Focused replay
passes (1/1, 21.5 s) on the original immutable gate harness:

```bash
GEOSOLVE_E2E_ARTIFACT_MANIFEST="$PWD/target/release-gate/prepared/decdf35058108f3c1a0d24e446208d1a797d2d6f070f86b3482b46934e8be135/browser/harness.json" \
GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome \
nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source \
  --run 'cd crates/geosolve-demo-web/frontend && npx playwright test tests/e2e/workbench.spec.ts --grep "invalid source retains" --workers 1 --output ../../../target/m98/local-canvas-invalid-source-r2'
```

Log: `target/m98/local-canvas-invalid-source-r2.log`. Integrated replacement qualification
and preview delivery remain pending.
