<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 lightweight shared playground

The lightweight playground was introduced to exercise multi-tab editing with
free geometry and short solves. Its first version contained two circles with
radii constrained to 8 and 12 mm and freely movable centers, giving four geometric
degrees of freedom. Later UAT added segments and keyed polyline corners.
Use distinct editor identities to test separate personal histories.

This record preserves the progression of drag responsiveness. The final F041–F043
repair is [qualified](M98_QUALIFICATION.md#qualified-uat-repairs); human U02 remains
**Fail pending recheck**. It supersedes the historical nominations below.

## Qualified full toolbar amendment

Product `b005b9e` passed **293/293 obligations** in `20260911T164104-576e7789`.
[Qualification](M98_QUALIFICATION.md#qualified-shared-toolbar-parity) records the
complete catalog, options/preselection, local navigation and personal history.
Offline installation, exact HTTP/MIME and real-WASM readiness passed while
retaining source, working drafts and personal histories.

| Drag trial | Event-to-preview p95 | Release-to-peer | Backward jumps |
| --- | ---: | ---: | ---: |
| First use | 417.2 ms | 828.4 ms | 0 |
| Warm 1 | 142.5 ms | 476.3 ms | 0 |
| Warm 2 | 116.7 ms | 419.5 ms | 0 |

Held Offset client prediction: wheel/pan/resize 142.9 / 188.0 / 294.8 ms; zero navigation RPCs.

Held Offset server prediction: wheel/pan/resize 106.9 / 191.7 / 280.1 ms; zero navigation RPCs.

These are bounded integrated observations, not 60 Hz or arbitrary-document-size claims.
F031 repairs new generated-polyline Undo/Redo through the semantic source lens without
replacing peer changes. F039 validates GPU completion asynchronously while retaining
exact frame evidence. F040 prepares blur shaders during startup and restoration and removes
the extra RAF wait for coalesced input after asynchronous validation. Historical failures and earlier delivery remain separate; original rejected
operations are not rewritten.

## Qualified drag repair — delivered on 2026-09-11

This historical delivery is superseded by the current toolbar amendment above.

At that checkpoint, both existing shared previews served `9b64c29`, qualified by all 288 obligations in
`20260910T233626-b8af5961`. [Qualification and delivery](M98_QUALIFICATION.md#qualified-shared-dragging-repair)
record zero backward jumps across three drags, warm preview p95 of 63–76 ms and
351–383 ms release-to-peer. First use remains about 427 ms locally. Retained
server workers avoid repeated reconstruction; provisional geometry survives accepted
pointer frames and pending commits. Navigation remains local in either prediction mode.
Native source bindings retain selected geometry and dimension pins after server updates.

The installed repair passed exact HTTP/MIME and actual-WASM readiness while
preserving source and history. The original generated-polyline Undo failure was
still open at that checkpoint; M98-F031 subsequently repaired new history operations.

## Initial playground and historical measurements

The initial two-circle model used ordinary managed SDK declarations and
source-owned overview dimensions. Installed-CLI `check` reported zero maximum
normalized hard residual. Two distinct clients passed point dragging and peer
updates; Undo restored original positions and source exactly.

The preceding `513463f` server took **3.664 and 3.666 seconds** from pointer release to
peer accepted geometry on this tiny sample. Local navigation/drag prediction remains
local; peer geometry is published after server acceptance. Detached profiling of the
initial small arm/circle sketch measures a 6.87 ms native drag preview and 64.30 ms
commit preparation, but 2.82 seconds across historical/current reconstruction,
scene reconstruction and source reconciliation. Each used fresh workers. That earlier
CLI had no cache configuration removing this overhead. The initial delivery retained
slow shared publication; the repair above now retains authenticated worker/session/compiler
state.
Detailed diagnosis: `target/m98/coordination/simple-playground-latency.md` and its
`simple-playground-latency-timings.json` receipt.

An initial polyline arm plus circle also exposed a real rejected Undo:
`CollaborationDomainError: Cannot read properties of undefined (reading 'invocation')`.
The original operation remained rejected with accepted revision 2 unchanged;
the preceding two drags had succeeded. The precise generated-owner inverse repair
was not yet implemented, so that failure was not counted as a pass.
A screenshot after rapid Undo also captured a stale browsing cancellation notice;
M98-F028 subsequently addressed that presentation symptom. The rejected operation stays
rejected in its original journal. M98-F031 now regression-tests and repairs new generated-
owner Undo/Redo through the correct semantic lens, including peer changes; it does not
rewrite old failed outcomes.

The initial playground changed only authored demo data. The later drag repair above
changes product code and has its own complete integrated qualification. Maintainer
acceptance and M98 closure remain open.
