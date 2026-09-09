<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 implementation and review candidate

M98 is implemented and mechanically qualified on 2026-09-09. Supervising-user acceptance
and milestone closure remain open. Work is isolated on `m98/file-workspace`; accepted M97
and its preview at http://100.94.63.83:18105/ remain unchanged.

## Qualified navigation latency repair

Current candidate `b1243a6deec4eddad7dd0a0f941b29ea47e538a4`, tree
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
