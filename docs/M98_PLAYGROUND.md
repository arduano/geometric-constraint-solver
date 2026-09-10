<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 lightweight shared playground

On 2026-09-10 the user requested a simpler sketch with degrees of freedom for
multi-tab interaction. A separate shared document serves the qualified repaired
M98 artifacts at `http://100.94.63.83:18112/`. The manifold remains on port 18111.
Private editor links and exact process/folder identity are recorded in
`target/m98/playground-preview-location.json`; use `tab-a` and `tab-b` to exercise
separate users and personal histories. Restart with the same source folder,
invitations and journal, omitting `--initialize true`.

## Qualified drag repair — delivered on 2026-09-11

Both existing shared previews now serve `9b64c29`, qualified by all 288 obligations in
`20260910T233626-b8af5961`. [Qualification and delivery](M98_QUALIFICATION.md#qualified-shared-dragging-repair)
record zero backward jumps across three drags, warm preview p95 of 63–76 ms and
351–383 ms release-to-peer. First use remains about 427 ms locally. Retained
server workers avoid repeated reconstruction; provisional geometry survives accepted
pointer frames and pending commits. Navigation remains local in either prediction mode.
Native source bindings retain selected geometry and dimension pins after server updates.

The original playground folder, all 32 retained files, accepted revision 6, all three
users' text/semantic histories and invitation bytes remain exactly unchanged. Two editor
contexts load all three exact nominated WASM modules without page errors; all 24 HTTP/MIME
routes match the frozen artifact. Verification is read-only and a final state comparison
also passes after browser verification. Reload the existing tabs; their links are unchanged.
The manifold on port 18111 receives the same repair with its current revision 1 and histories
preserved. The original generated-polyline Undo failure below remains open.

## Initial playground and historical measurements

The source is in
`target/m98/installed-collaboration-preview-20260910T204328-4a05c31a/playground-simple/sketch.ts`.
Two circles have radii constrained to 8 and 12 mm and freely movable centres,
for four geometric degrees of freedom. This count follows the four free centre
coordinates; it is not a new exported rank/DOF diagnostic. Both are ordinary managed
SDK declarations, with source-owned overview dimensions. Drag the centre handles,
edit radii through the Inspector, or use the supported construction tools.

Installed-CLI `check` independently validates zero maximum normalized hard residual.
All 24 served HTTP/MIME routes match the existing qualified production artifact.
Two tabs with distinct client identities pass point dragging and peer geometry
updates. Checked Undo restores both original positions and accepted source exactly.
Verification uses the separate `tab-c` principal, leaving `tab-a` and `tab-b` without
verification contributions. Evidence:

- `target/m98/playground-simple-check.json`
- `target/m98/playground-simple-verification.json`
- `target/m98/playground-readiness-r2.json`

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
The operation journal retains the rejected result and unchanged accepted revision 2;
the two prior drags succeeded. The original folder and evidence remain at
`playground/`, `target/m98/playground-verification.json` and
`target/m98/playground-arm-preview-location.json`. Its test server was stopped before
launching the simpler document on the same port. The precise generated-owner inverse
repair remains unimplemented; no failed check is counted as a pass. A screenshot after
rapid Undo also captured a stale browsing cancellation notice, while fresh editor
sessions remain the readiness check. M98-F028 addresses that stale-cancellation
presentation symptom; the generated-owner Undo failure remains separate.

The initial playground changed only authored demo data. The later drag repair above
changes product code and has its own complete integrated qualification. Supervising-user
acceptance and M98 closure remain open.
