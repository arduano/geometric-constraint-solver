<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 local folder prototype handoff

Status: **M98 PROTOTYPE_READY_FOR_UAT**; **not accepted or release-qualified**.

Branch `m98/file-workspace`, worktree
`/home/arduano/programming/geometric-constraint-solver-worktrees/m98-file-workspace`.
Pinned base: `d80bf22264f74b60870f2e99feb8cc6ccb9d0133`. This is the explicitly authorized
parallel M98 exception. The primary checkout's active M97 source-native metadata implementation
was neither read, changed nor integrated. No other agent/worktree was created or contacted.

## Delivered implementation and APIs

- `scripts/file-workspace.mjs`: `init`, `serve`/`open`, JSON `check` and `status`; one project,
  serialized workbench, content polling, SSE notifications, token/origin guarded local HTTP RPC,
  compare-and-swap writeback, retained plaintext recovery and fixed project paths.
- `frontend/src/lib/folder-adapter.ts`: opt-in transport implementation of existing
  `WorkbenchAdapter`; exact source/Inspector edit base, live updates, pending-intent recovery.
- `frontend/src/main.tsx` and `App.tsx`: folder bootstrap, browser-design persistence bypass,
  source-draft preservation and a small fixed-height status/refresh/download-intent strip.
- `frontend/scripts/workspace-runtime.ts` and `build-workspace.mjs`: bundle the existing
  `WasmWorkbenchAdapter` and `resolvePendingManagedMutationSnapshot` for the local Node process.
- `examples/file-workspace/`: minimal canonical managed source, two-field manifest and run guide.
- `scripts/file-workspace.test.mjs`: actual-files/actual-WASM/Chromium focused vertical slice.

No mathematical behavior, primitive, residual, Jacobian, rank, priority or branch policy changed.
The Node process loads this worktree's real Rust `WorkbenchHandle` from the WASM build. Both
external edits and GUI mutations use the existing compiler preparation/receipt resolution,
native materialization and independent validation. The existing React canvas and Inspector
consume its snapshots. No compiled JSON geometry file is the folder's authority.

## Commands and evidence

All commands run in this worktree; build/test outputs and npm/Deno caches are private.
See `examples/file-workspace/README.md` for reproducible setup and daily commands.

```bash
env CARGO_BUILD_JOBS=2 npm_config_cache=$PWD/target/m98/npm-cache DENO_DIR=$PWD/target/m98/deno nix-shell shell.nix --run 'npm ci --prefix packages/geosolve-intent && npm run build --prefix packages/geosolve-intent && npm ci --prefix packages/geosolve-sketch-code && npm ci --prefix crates/geosolve-demo-web/frontend && npm run wasm --prefix crates/geosolve-demo-web/frontend'
env CARGO_BUILD_JOBS=2 DENO_DIR=$PWD/target/m98/deno nix-shell shell.nix --run 'npm run wasm:release --prefix crates/geosolve-demo-web/frontend'
node crates/geosolve-demo-web/frontend/scripts/build-workspace.mjs
node scripts/file-workspace.mjs init target/m98/demo
node scripts/file-workspace.mjs check target/m98/demo
npm run check:types --prefix crates/geosolve-demo-web/frontend
npm run build:ui --prefix crates/geosolve-demo-web/frontend
npm run test --prefix crates/geosolve-demo-web/frontend -- src/App.test.tsx src/lib/wasm-adapter.test.ts src/lib/pending-managed-mutation.test.ts --maxWorkers=1
env CARGO_BUILD_JOBS=2 nix-shell shell.nix --run 'cargo fmt --all -- --check'
env CARGO_BUILD_JOBS=2 nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-demo-web --lib -- -D warnings'
node --check scripts/file-workspace.mjs
node --check scripts/file-workspace.test.mjs
node --test --test-reporter=tap --test-concurrency=1 scripts/file-workspace.test.mjs
git diff --check
```

Logs: `target/m98/logs/`. Actual-browser screenshots, exported project, pending intent,
latency and final JSON status: `target/m98/tests/`. Each browser run initializes a fresh
`target/m98/tests/project-*` folder, owns an ephemeral loopback server, restarts only that
server and closes only its own Chromium process. The ordinary-mode browser uses the same
locally built existing demo without `?folder=1`.

Final outcomes:

- Optimized WASM build passed (6m56s Cargo build plus wasm-bindgen/wasm-opt); existing default
  WASM dead-code warnings remain. UI production bundle/typecheck, JavaScript syntax checks,
  Rust formatting and focused warnings-denied native Clippy passed.
- Existing focused frontend suite: **67/67** across App, WASM adapter and managed transaction
  resolution. Final log: `target/m98/logs/frontend-tests.log`.
- Real Chromium workflow: **13/13 subtests** (Node reports 14/14 including the parent),
  **10.80 seconds**, in `target/m98/logs/e2e-final.log`. Actual two-way live synchronization
  was witnessed: radius 10→12 by external atomic rename, 12→14 by Inspector writeback, visible
  plaintext update and exactly one write/no watcher recompile. Independent painted-circle
  width ratio checks the changed radius at unchanged camera scale.
- Exact last-good painted geometry survives invalid source; located diagnostics and original
  disk text persist. Restoring the original valid source clears the error. Stale RPC, stale
  browser Code draft, and in-progress Inspector edit refuse overwrite and retain intent.
  Permission-denied writes return an error with compiled pending source. Token/origin/path
  rejection, restart/reopen, restart on invalid disk, and ordinary WASM demo startup/Start
  from code all pass.
- Manual existing **canonical project JSON export** was actually downloaded through File menu
  to `target/m98/tests/manual-project-export.json` (22 KB); raw source export remains in the
  same menu. No new export format was introduced.
- One rough small-sketch external-save-to-Inspector observation was **835 ms**, including
  polling, debounce, compile, native validation and browser update. This is no universal latency
  claim. `target/m98/tests/latency.json` records it.

Final implementation adds a project-source revision counter separate from the volatile native
workbench revision. Accepted/current hashes are durable content identities; source revision
counters are local to a bridge session and reset on restart.

The first development-WASM smoke probe hit `RuntimeError: memory access out of bounds`
at managed mutation resolution/free; the identical source succeeds with the standard release
WASM. Classified as an unconfirmed development-profile/runtime observation, not an established
solver defect; no Rust fix or new defect ID. Use `wasm:release` for this prototype.
An earlier Node compiler bundle lacked ESM equivalents for TypeScript's `__filename`/`__dirname`;
the bundle now supplies them. This was an M98 transport harness issue.
The first end-to-end run passed initialization and both sync directions, then its exact retained
geometry check caught viewport movement caused by a wrapping status strip. The fixed-height
strip corrects that presentation issue without relaxing the assertion. Initial failure evidence
is retained in `target/m98/logs/e2e-initial-failure.log`; subsequent dependent failures from the
unrestored invalid fixture were not treated as independent solver findings.

The second run caught exact-original-source restoration being sent to Apply, whose existing
contract rejects unchanged source. The folder adapter now routes this case to the existing
Revert operation; no Rust behavior changed. Its unchanged focused regression now passes.

## Retained review preview

Started with `node scripts/file-workspace.mjs serve target/m98/demo` from this worktree.
Owned process PID **1439145**, loopback port **37135**, log `target/m98/logs/preview.log`.
Editable source: `target/m98/demo/sketch.ts` (starter radius 10 mm).

Exact review URL:
`http://127.0.0.1:37135/?folder=1#token=5d381cd26130f402732b0ffa765ead80bc1db2b77dc174ec`

Ordinary mode: `http://127.0.0.1:37135/`. For remote review use
`ssh -L 37135:127.0.0.1:37135 user@host`, then open the exact folder URL above locally.
The token is scoped to this private running process; restart prints a new URL/token.
The process and worktree remain available. No existing preview port/service was touched.

## Integration seams and limitations

M97 is still active elsewhere. Expect conflicts in `App.tsx`, `main.tsx` and source presentation
when integrating. The adapter imports the existing compiler rather than copying its language,
so future document options/named parameters/key flags should flow through the rebuilt runtime.
Rebuild its bundle and recheck GUI field events after M97's Inspector/source-format changes.
Do not assume this pinned prototype validates those absent APIs or closes M97.

One browser, one `sketch.ts`, SDK imports only. Manifest changes require restart. Local source
dependencies/custom patches, multiple clients, semantic merge and migrations are deferred.
Supported source transactions preserve comments/labels according to the existing writer.
Native point/grip drag overlays are blocked with an explanation; source-backed dimensions and
ordinary authoring are the intended happy path. View state, selection, pins and history are
session-only. Reopening fits geometry reconstructed from disk. Browser localStorage never wins
over disk in folder mode; the ordinary mode keeps its existing browser persistence behavior.

`last-good.ts` is derived plaintext, read only to retain geometry when current disk source is
invalid. `.geosolve/session.json` contains this bridge's URL/token/PID. CAS publication retains
displaced source at `.geosolve/before-*.ts`, checks it, and exclusively links the staged file into
the entry path. A concurrent rename cannot be overwritten; recovery also retains writes through
old open descriptors. A brief missing-entry interval/crash can leave the prior source only in
recovery. Local Linux filesystems are the target; recovery files accumulate until manual review.
No filesystem transaction framework or power-loss guarantee is claimed.

The server is loopback-only. A server restart creates a new token: reopen its newly printed URL.
For remote UAT, forward the exact port with `ssh -L PORT:127.0.0.1:PORT user@host` and open the
printed `http://127.0.0.1:PORT/?folder=1#token=...` URL locally. No public reachability, service
installation, proxy/firewall change, deployment, merge or push was performed.

Full workspace tests, golden oracle, integrated release gate, complete browser catalog, production
qualification and human milestone acceptance were intentionally not run/claimed. This is a
working prototype for iteration. SVG/3D/Manifold integration and Pi case modelling are absent;
the demonstrated existing manual canonical sketch export remains available for later use.

## Next manual UAT

1. Open the retained demo URL. Switch between Design and Split; inspect the starter circle.
2. Change `value: mm(10)` in the demo's `sketch.ts` with a normal editor/agent save.
3. Change `ringRadius` in Inspector, press Enter, and inspect the actual file.
4. Introduce incomplete syntax; verify the retained canvas and visible diagnostic; repair it.
5. Keep an unapplied browser source draft while saving externally. Apply it, download the
   conflict intent, then Revert/Refresh and retry deliberately.
6. Use File → Export canonical project… or Download sketch.ts…. Restart only this bridge and
   open its newly printed URL to check disk reconstruction.
