<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 navigation latency repair

Status: M98-F016 is implemented, mechanically qualified and delivered from clean
candidate `b1243a6` in `20260909T221531-9189674e`. M98 acceptance and closure remain open.

The user reported 2–5 second latency for hover, zoom and clicking after the loading
amendment. M98-F016 in [the hardening ledger](M98_HARDENING.md) records the reproduction,
owner and exact queue contract. No mathematical or persisted format changes are needed.

## Reproduction

Isolated Chromium reproductions use previously installed `a68fffa` and preceding `6509e9c`
with a copied manifold, preserving source and accepted design. CDP simulates network latency/throughput. These are
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
Folder JSON compression preserves exact decoded response fields and authority. Successful
authenticated RPC responses negotiate gzip level 1 for payloads from 16 KiB through
4 MiB; small/oversized responses, errors and SSE remain uncompressed.

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
evidence; the final installed-product measurements below use the qualified artifact.

The initial development-build wheel replay after both fixes uses the same 100 ms latency / 512 KiB/s
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

## Qualification synchronization finding

Run `20260909T191739-6f32214c` failed exact compiler parity after the host update
changed Deno 2.9.4 to 2.9.6. Subsequent qualification explicitly uses the retained
Nixpkgs source matching the earlier qualified tools. Run `20260909T192403-5a85f572`
was interrupted; neither attempt establishes a qualified release.

The first pinned complete attempt, `20260909T195555-1f5f64d4`, retained 254 passing
stages but failed two browser timeouts. The folder manifold edit exceeded its 30-second
source assertion; an isolated exact-artifact replay passed that original assertion in
28.85 seconds, with all 18 exported regions and no errors. The unchanged retry
`20260909T204021-0d315da4` passed all nine folder browser cases and 259 stages, but
again timed out on the dense fixture's five-second Restore-enabled assertion.

An instrumented replay against the exact prepared harness records a correct isolation
snapshot after 675 ms, followed by a 6.5-second persistence request and delayed browser
assertion completion. The original five-second assertion still fails; after the existing
bounded presentation wait, isolation and restoration pass. The isolated geometry differs
as intended, restoration has zero coordinate difference, and source and design-history
availability remain unchanged. A separate earlier replay also caught point selection
still pending at its five-second Inspector assertion. Evidence is in
`target/m98/fixture-isolation-diagnosis/diagnosis.json` and
`target/m98/fixture-selection-timeout-diagnosis/diagnosis.json`.

The sample browser workflow now uses its existing `settlePresentation` helper after
selection and isolation, as it already does after Fit and reload. All original enabled,
ownership, geometry, source, history and scrolling assertions remain. This is browser
synchronization, not a solver or product change, and does not claim lower persistence
cost. The full focused fixture workflow then passed both edits and Undo/Redo but
reached its six-minute whole-test deadline during the final reload. The complete
two-edit sample lifecycle now has an eight-minute overall ceiling; individual
operation deadlines, numerical assertions and the separate performance gate remain
unchanged. The original timeout is retained in `target/m98/fixture-isolation-focused.log`.
The focused complete fixture workflow passes (1/1, 7.1 minutes), including its final
reload. Evidence: `target/m98/fixture-isolation-focused-completion.log`; the exact
executed command is retained in `target/m98/fixture-isolation-focused-process.json`.
The final integrated run below qualifies this browser synchronization change.

Run `20260909T213304-b37d7894` passes all 49 full browser workflows and 17 initial
sample/render checks, without skips or flaky results, on the synchronization repair.
Its folder browser stage again exceeds the manifold's 30-second source poll. That
test now waits up to 60 seconds for the exact `parameter.edit` HTTP response, checks
HTTP success and absence of an RPC error, then retains the original disk-source and
18-region export assertions. Its 180-second whole-test deadline is unchanged. This
distinguishes operation completion from post-response publication correctness.
The focused manifold case passes (1/1, 69.3 seconds) against the exact prepared
harness; `target/m98/manifold-response-focused.log` and its process record retain
the result and executed command.

## Final qualification and delivery

Clean candidate `b1243a6deec4eddad7dd0a0f941b29ea47e538a4` passes **261/261 obligations**
in `20260909T221531-9189674e`: 11 fresh and 250 authenticated reused results,
974.099 seconds (16m14.099s). Product repair is `72ab926`; descendants `b6e0a27`
and `b1243a6` add the browser synchronization changes described above. Every linked
passing receipt and the clean/unchanged/complete qualification manifest were independently
authenticated by `verify-qualification.py`. Earlier failed or interrupted runs remain
failed or interrupted; only their authenticated unchanged-input passing stages can be reused.

Coverage includes 307 frontend tests, 101 folder Node tests, nine folder browser cases,
seven engine Node tests, two generator Node tests, one generator browser case and two
offline package tests. The main browser evidence includes 49 complete workflows and
17 initial sample/render checks without failures, skips or flaky results. Required
format, Clippy, native/WASM, package and licence checks pass, and the unchanged
271-case golden remains clean. The performance stage passes in 122.0 seconds with
the 256-moving-body crossover at 114.81 seconds. Qualification evidence is
`target/m98/latency-final-qualification.json`; the log is
`target/m98/latency-release-gate-response.log`.

The final command was:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260909T213304-b37d7894'
```

Exact-byte/MIME and actual WASM/WebGL2 readiness passed. A representative folder
response shrank from **628,107 bytes to 59,826 bytes**, with exact decoded equality.
The historical HTTP preview's explicit UUID compatibility insertion was accounted
for separately; it did not alter the qualified JS/CSS/WASM. Source and accepted
state were retained across restart. Isolated probes used the exact installed CLI
and distribution, with these controlled network observations:

| Input | Conditions | Qualified installed product |
|---|---|---|
| 90 hover samples | Unthrottled | 77 requests; final response 302.6 ms after input stopped |
| 90 hover samples | 40 ms latency, 1 MiB/s | 34 requests; final response 390.2 ms after input stopped |
| 12 wheel samples | 100 ms latency, 512 KiB/s | Three requests; final response 561.7 ms after input stopped |

Both probes report no browser errors. Hover produces no changed presentation frames
in this empty-canvas motion probe. Wheel preserves all 12 ordered samples in two
batches, followed by dimension-navigation completion; it presents three frames with
the final presentation 703.4 ms after input stopped. Its longest request is 243.3 ms;
three main-thread tasks take 111–150 ms, so this is not a claim of 60 Hz rendering.
Evidence: `target/m98/latency-folder-browser-qualified-20260909T221531-9189674e.json`
and `target/m98/latency-folder-wheel-browser-qualified-20260909T221531-9189674e.json`.
These measurements establish improvement under controlled network conditions, not
the user's actual connection latency or lower solving cost.

Documentation handoff uses `./scripts/release-gate.sh --docs-only --since b1243a6`;
the six-file check passes and preserves the qualified product above without rebuilding.
Its log is `target/m98/latency-docs-only.log`.

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
