<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# M98 optional server authoring preview

Local native authoring prediction remains the default. A trusted host may enable
`createCollaborationPreviewService` for clients which prefer server computation.
The service runs retained point drags and construction gestures through the same
public Rust/WASM engine APIs used by local authoring. Preview operations are
separate from the durable mutation queue and cannot publish an accepted result.

The host supplies two synchronous callbacks: `authenticate(connection)` validates
the current server-issued editor connection, and `captureBasis(connection,basis)`
returns the immutable accepted native project and design for the requested
`{documentEpoch,revision,sourceDesignDigest}`. A network request cannot supply
project, design, source files, accepted scene or success claims. The worker reopens
the captured model and independently verifies exact source/design export parity
and digest before starting its gesture.

`createCollaborationPreviewRoute(service)` is a thin callback for an authenticated
HTTP POST route. The host must retain its normal token, role, origin, rate and
request-body checks, forward request cancellation, call `dropConnection` when a
session leaves or expires, and await `close` before releasing the document host.
The service reauthenticates before operations and before returning worker results.
Preview tickets are random, process-local and scoped to the exact connection.
They do not survive reconnect or server restart.

## Messages

| Action | Input | Result |
| --- | --- | --- |
| `begin` | Accepted basis, gesture ID, point target or construction tool/role, fixed authoring viewport | Opaque ticket, basis, detached native presentation |
| `advance` | Ticket and 1–256 ordered native samples | Basis, detached native presentation, native point/construction frame |
| `finish` | Ticket | Semantic point terminal or construction command; ticket consumed |
| `cancel` | Ticket | Cancellation; retained worker terminated |

Native sample order and the 4096-sample gesture bound remain unchanged. Exactly
one operation may run per ticket; another operation receives backpressure instead
of entering an unbounded queue. A malformed native trace terminates its preview.
Cancellation can interrupt a running native operation. Failed or lost preview
responses should lead to cancellation/restart; the preview route is not a durable
retry protocol.

The returned `presentation` is the native `presentationJSON()` string. The browser
renders it with the existing `renderAuthoringPreview` and its current local view.
Camera changes, selection, highlighting and picking stay client-owned; navigation
does not make preview requests. Clients cache the latest presentation and discard
obsolete model/generation responses. A completed preview command still enters the
ordinary authenticated mutation route, where the server independently replays,
validates residuals and explicit branches, resolves conflicts and persists it.

An in-progress preview retains its authenticated original accepted basis. A new
accepted model does not silently rewrite the trace. The browser cancels/replaces
its preview on model replacement; a finished original command may use the
separate trusted latest-model replay protocol when submitted for publication.

## Bounds and configuration

The service is disabled unless explicitly enabled. Defaults are four retained
workers per document, one per connection, 64 MiB per captured basis, 128 MiB total
captured basis, 1 MiB per request, 64 MiB per response, 60 seconds per native
operation and 30 seconds idle. Configuration is validated against fixed ceilings.
Each worker also has a 512 MiB V8 heap limit and a 64 KiB diagnostic output limit;
this is not an operating-system address-space limit on WASM memory. Native engine
resource bounds remain active. Workers terminate before count and captured-memory
reservations are released. Shutdown drains all termination promises.

Owner tests in `scripts/collaboration-preview.test.mjs` use actual engine WASM and
compare server preview frames and terminals with direct native local gestures.
They independently check accepted source/design retention, bar length and hard
validation; refuse forged native generations, stale basis, arbitrary models,
viewer/cross-session access and unordered samples; and exercise cancellation,
reauthentication, count/byte/time/idle bounds and shutdown cleanup. HTTP/client
composition and release qualification are separate integration obligations.

The reference folder host now accepts
`authoringPreview: { enabled: true, preferred: "client" | "server", limits? }`.
`geosolve serve ... --collaboration true --authoring-preview client` enables the
fallback while retaining local prediction as the default; `server` requests remote
authoring prediction, and `disabled` is the default. Both modes keep navigation,
picking, camera and rendering local. The HTTP path is
`POST /api/collaboration/authoring-preview`; document state advertises availability
and preference. Preview messages are ephemeral and never enter the durable edit
queue. The final semantic command enters that queue through the normal route.

Focused HTTP/runtime integration passes in `preview-http-runtime-r1.log`:
`node --test --test-name-pattern="preview HTTP" scripts/collaboration-http.test.mjs`
and `node --test --test-name-pattern="configured HTTP authoring preview" scripts/collaboration-runtime.test.mjs`.
This covers viewer/unauthenticated refusal, text/read independence, connection
retirement and a real remote native point gesture committed after another editor's
intervening edit.

The browser chooses the advertised preference after joining. An explicit
`?authoringPreview=client|server` overrides it; unavailable server mode gives a
clear error. Server mode creates a presentation-only worker and does not open a
client engine session. The existing bounded authoring mailbox batches ordered
samples, caches server presentation and renders camera changes locally. Model
replacement aborts outstanding HTTP work and retires old preview tickets; late
replies cannot paint or clear a newer gesture. Preview RPCs never enter the durable
outbox or replay after reconnect.

`remote-preview-frontend-r1.log` records six client transport tests and eighteen
frontend tests, with strict TypeScript checking. They cover ephemeral cancellation,
typing during a held preview, ordered samples, zero navigation RPCs, stale response
retirement and model replacement. The combined release WASM rebuild passes and the
development-r4 artifact validates 23 files, including the three native modules.
The real-browser remote-mode case passes in `collaboration-browser-followup-r4.log`.
It holds an actual server response while local navigation continues without preview
or model requests, then checks the exact native terminal, one durable publication,
changed design overrides, unchanged raw source, reload and cold server restart.
Integrated qualification remains pending.
