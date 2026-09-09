<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 navigation latency repair

Status: implementation and focused verification in progress; replacement qualification
and preview delivery remain pending. M98 acceptance and closure remain open.

The user reported 2–5 second latency for hover, zoom and clicking after the loading
amendment. M98-F016 in [the hardening ledger](M98_HARDENING.md) records the reproduction,
owner and exact queue contract. No mathematical or persisted format changes are needed.

## Reproduction

Private Chromium browser runs use current installed `a68fffa` and preceding `6509e9c`
with a copy of the user's manifold. They preserve source and accepted design; the actual
editable preview is not taken over. CDP simulates network latency/throughput. These are
controlled browser measurements, not a claim about the user's actual connection or
hardware latency. Local standalone current-build navigation was also profiled; its
individual hover/wheel/deferred dimension calls completed in tens of milliseconds.

| Input | Conditions | Before repair |
|---|---|---|
| 90 hover samples | 40 ms latency, 1 MiB/s | 79 requests; final response 2.866 s after input stopped |
| 12 wheel samples | 100 ms latency, 512 KiB/s | ~634 KB per frame; zoom still replaying after 14.6 s |

The preceding build reproduces comparable folder results. No watcher/SSE invalidation
loop, recompile or filesystem writes occurred during navigation. Existing manifold Fit
still costs about 2.7 seconds locally and is separate from this repair.

Durable task evidence: `target/m98/latency-backend-diagnosis.md`,
`latency-folder-comparison.json`, `latency-static-before.json`, and
`latency-folder-compression.json`. Reproduction helpers are
`probe-folder-navigation-latency.mjs` and `probe-folder-wheel-latency.mjs` in the same folder.

## Implementation and verification

The frontend scheduler retains the newest adjacent pending hover/pan position and
combines exact wheel samples across frames while awaiting an asynchronous result.
Down/up/cancel, semantic movement, modifier/pointer transitions and intervening
operations remain ordering barriers. It drops queued idle hover when the pointer leaves.
Folder JSON compression preserves exact decoded response fields and authority.

The focused held-request regression failed before the scheduler change. All 40 canvas
viewport tests and TypeScript checks pass afterward. The tests cover delayed hover plus
click, separate pan gestures/releases, pointer/modifier barriers, discarded hover,
521 wheel anchors split into 1/256/256/8 samples, mixed operation barriers, and every
semantic drag/authoring sample. The HTTP compression regression failed before the change and passes afterward, covering
raw identity/gzip byte equality, weighted/wildcard/refused negotiation, small responses,
errors and unchanged filesystem/accepted history.

A second browser characterization with 80 ms added request delay reduced 92 dispatched
requests to 31 for the same 90-sample motion, with aggregate request time reduced from
9.12 seconds to 3.08 seconds during a 2.93-second input period. This is queue behavior
evidence; the final delivery measurement will use the exact qualified artifact.

Controlled wheel replay after both fixes uses the same 100 ms latency / 512 KiB/s
conditions and exact 12 samples as the original reproduction. It makes three requests
instead of 12; the final response arrives **546 ms after input stops**, compared with
more than 14.6 seconds before repair. It preserves all wheel samples and reports no
browser errors. Evidence: `target/m98/latency-folder-wheel-browser-queue-gzip-fixed.json`.

Focused commands executed from this worktree (frontend commands from its directory):

```bash
npm test -- src/components/canvas-viewport.test.tsx
npm run check:types
npm run build:ui
node --test --test-name-pattern=M98-F016 scripts/workspace-http.test.mjs
GEOSOLVE_PROBE_LABEL=queue-gzip-fixed GEOSOLVE_DIST="$PWD/crates/geosolve-demo-web/dist" GEOSOLVE_PROBE_RUNTIME="$PWD/scripts/file-workspace.mjs" node target/m98/probe-folder-wheel-latency.mjs
git diff --check
```

The actual Chromium folder regression also fails against frozen pre-fix production and
passes against the corrected distribution (1/1, about 3.5 seconds). It holds a real
request, advances separate browser frames, checks first/latest hover before exact
empty-canvas down/up, and verifies wheel batches `[1,19]` retain every anchor, delta
and Ctrl flag. Chromium decodes seven real compressed canvas responses. Source,
accepted project/design, authority and write counters remain exact; inverse wheel
pairs restore the original camera scale. Evidence: `target/m98/latency-browser-regression-before.log`,
`latency-browser-regression-after.log` and `browser/queued-folder-navigation.json`.

```bash
GEOSOLVE_DIST="$PWD/crates/geosolve-demo-web/dist" node --test --test-name-pattern='M98-F016 delayed' scripts/workspace-browser.test.mjs
nix-shell shell.nix --run 'node --test scripts/workspace-http.test.mjs scripts/workspace-navigation.test.mjs'
```

## Limits

Folder mode still sends complete decoded snapshots and renders surrounding React UI
for changed frames. Slow solving, explicit Fit, startup and semantic editing costs are
unchanged. The scheduler does not replace semantic drag samples or approximate wheel
anchors. Existing external toolbar/keyboard dispatch is outside this queue; this repair
does not establish a new global ordering guarantee for those commands.

Focused HTTP/navigation collateral checks pass **15/15**, including all four sample
folders with zero writes, recompilation or history serialization. Raw compressed-response
regression also covers the 4 MiB synchronous compression cap, exact large-export fallback
and uncompressed SSE. Commands and logs are retained in
`target/m98/latency-gzip-implementation.md`.
