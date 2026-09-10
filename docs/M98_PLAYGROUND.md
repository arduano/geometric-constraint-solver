<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 lightweight shared playground

On 2026-09-10 the user requested a simpler sketch with degrees of freedom for
multi-tab interaction. A separate shared document serves the unchanged qualified
M98 artifacts at `http://100.94.63.83:18112/`. The manifold remains on port 18111.
Private editor links and exact process/folder identity are recorded in
`target/m98/playground-preview-location.json`; use `tab-a` and `tab-b` to exercise
separate users and personal histories. Restart with the same source folder,
invitations and journal, omitting `--initialize true`.

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

The current server still takes **3.664 and 3.666 seconds** from pointer release to
peer accepted geometry on this tiny sample. Local navigation/drag prediction remains
local; peer geometry is published after server acceptance. Detached profiling of the
initial small arm/circle sketch measures a 6.87 ms native drag preview and 64.30 ms
commit preparation, but 2.82 seconds across historical/current reconstruction,
scene reconstruction and source reconciliation. Each uses fresh workers. The qualified
CLI has no cache configuration that removes this overhead. This delivery therefore
provides a lightweight sketch, not instant shared geometry publication. Retaining
properly authenticated worker/session/compiler/scene state is a separate product repair.
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
sessions remain the readiness check; this presentation symptom needs separate triage.

No solver/product implementation, qualified artifact or manifold source changed.
No full release rerun is required for a new authored document served by those same
bytes. Supervising-user acceptance and M98 closure remain open.
