<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M67 focused UAT — cleaned workbench

Status: mechanically qualified candidate serving; supervising-human review pending.

Candidate source: `3d52b29fc11f5cef572fe86f58a95897ec8c8214` on `main`.

Temporary Tailscale endpoint: `http://100.94.63.83:8080/`

Release distribution manifest: `e9d410c71290e7200595aaf9be6327523a812a1fa7d23abfa9d12c8279c176ac`

The clean release gate built the seven-file distribution. A non-watching static server exposes
only those completed files on the Tailscale address, and all seven HTTP responses matched their
local SHA-256 values before handoff. This is one focused human checkpoint; direct Rust/WASM tests
remain the correctness authority.

## M67-U1 — one surviving application

1. Open `/`, then `/#/dev/lab`, then an arbitrary fragment.
2. Confirm each shows the same ordinary GeoSolve workbench and no alternate lab/runtime.
3. Confirm the header brand is not a misleading hash-navigation link.

Expected: there is one application only. No playground root, legacy controls, guided harness,
scenario transcript/evidence UI or browser-only qualification surface appears.

Result: Pending.

## M67-U2 — cleaned inspector and trustworthy errors

1. Inspect the right sidebar with nothing selected and with geometry, a constraint, a dimension
   and a computed Fillet selected.
2. Trigger or open a sample with an attributable problem, then inspect its canvas marker and
   Problems panel.
3. Trigger a failure that cannot be attributed to one geometry item if a suitable current sample
   is available.

Expected: Production topology, Host-state evidence and Accepted redundancy cards are absent.
Selection, advanced-curve options, branch/dimension/feature editing and Problems remain coherent.
Attributed errors still highlight their owners and an unattributable failure remains global.

Result: Pending.

## M67-U3 — ordinary CAD workflow

1. Create a new workspace, draw a polyline and circle, and apply representative Coincident,
   Horizontal or Perpendicular and angle/radius dimension actions.
2. Select/delete an item, then Undo and Redo.
3. Pan, wheel-zoom and Fit the canvas.
4. Refresh and confirm the workspace restores.

Expected: authoring applicability, contextual glyphs, dimension editing, history, camera and
workspace persistence behave as before the cleanup. The canvas does not select surrounding HTML
text during interaction.

Result: Pending.

## M67-U4 — editable Samples and computed Fillets

1. Open one movable mechanism and drag a representative free control.
2. Open **Curves & constructions → 2D Fillet playground**.
3. Author a Fillet, edit its numeric radius, select its generated arc, delete/Undo it and move a
   native source point.
4. Switch to another sample and confirm normal authoring remains available.

Expected: Samples remain ordinary editable save-like workspaces. Computed output, feature history,
source editability and recoverable failures remain usable; no read-only or guided mode returns.
`M66-KL001` remains an accepted known limitation rather than an M67 regression.

Result: Pending.

## Approval

M67 closes only after all four areas are reviewed and the supervising human explicitly approves
this cleanup scope. Record findings as `M67-Fnnn` with exact actions, observed state and expected
state; objective failures require an owning-layer regression before retest.
