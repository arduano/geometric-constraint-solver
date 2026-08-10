<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M70B focused UAT — Workspace reproduction handoff

Status: prepared, not started. Implementation qualification, candidate freeze/publication,
served-byte verification and every human result below are pending. This scorecard records no pass
or approval.

Candidate source: pending

Tailscale endpoint: pending

Release distribution manifest aggregate: pending

## Preconditions

- [ ] `docs/M70B_IMPLEMENTATION.md` records passing focused/direct qualification.
- [ ] The complete integrated release gate passes on one clean nominated source.
- [ ] A read-only release distribution is served through the usual Tailscale endpoint.
- [ ] Every served asset and `/` matches the frozen local bytes.
- [ ] The browser has hard-refreshed that exact candidate.

Use only the ordinary GeoSolve Sketch Workbench. The reproduction overlay is global workbench UI;
there is no scenario mode, protected fixture, alternate coordinator or restored legacy page. Direct
Native Rust tests remain authoritative for exact bytes, bounds, workspace/high-water fidelity and
atomicity; the same codec path must also compile for WASM. Human review assesses discoverability,
text handoff, visible restoration and failure recovery.

## M70B-U1 — Discover and copy a self-contained payload

1. Open **Samples → Curves & constructions → 2D Fillet playground** or another ordinary editable
   workspace.
2. Make recognizable accepted edits: create or adjust a computed Fillet, add one constraint or
   dimension and create or convert one Construction curve.
3. Activate **Copy repro**, which also attempts to place the payload on the clipboard.
4. Inspect the visible overlay and its payload/status, then paste into a plain text editor to verify
   the automatic copy.
5. If practical, deny clipboard permission and repeat; otherwise choose **Select text** and press
   Ctrl/Cmd+C to exercise the manual fallback.

Expected: the action is discoverable and produces one complete single-line value beginning
`GEOSOLVE_REPRO_V1:zlib-base64url:`. The overlay reports success or leaves the entire payload
available for manual copy; denial does not lose or truncate it. Opening/closing the overlay causes
no canvas resize, geometry move, solve or accepted-state change.

Result: **PENDING**

Notes:

## M70B-U2 — Restore the complete persisted workspace

1. Keep the copied text outside the workbench, such as in a plain text editor.
2. Record the visible geometry, Construction roles, constraints/dimensions and computed Fillet.
3. Replace the current scene with another sample or delete/move several recognizable objects.
4. Activate **Load repro**, paste the saved text and choose **Load payload**.
5. Inspect the tree, canvas, constraints/dimensions and computed feature, then make one ordinary edit.

Expected: one action restores the copied persisted design/accepted workspace coherently, including
the computed Fillet and Construction semantics. No intermediate or mixed scene is painted. The old
camera, selection, hover, active tool, sample label and pre-copy Undo stack are not restored; those
are intentional non-persisted state. The restored workspace remains normally editable.

Result: **PENDING**

Notes:

## M70B-U3 — Handoff is independent of browser storage

1. Copy a payload from the edited first tab/profile.
2. Open the nominated candidate in a separate fresh tab or browser profile whose ordinary
   workspace differs.
3. Activate **Load repro**, paste the payload and choose **Load payload** without copying any
   `localStorage` key or file.
4. Compare the visible sketch, roles, constraints/dimensions and computed feature with the source.

Expected: the text alone carries the complete persisted workspace needed for reproduction. The
destination's prior browser storage neither supplies missing scene data nor overrides the capsule.
Transient camera/tool/selection state may differ by design.

Result: **PENDING**

Notes:

## M70B-U4 — Corruption and invalid workspace are atomic

1. With a valid unrelated scene visible, change one character in the copied payload body and choose
   **Load payload**.
2. Load the original text successfully, return to the unrelated scene, then use **Load payload**
   with a truncated value and an unsupported version prefix.
3. After each rejection, close/reopen the overlay and manipulate the existing scene normally.

Expected: each bad input produces a specific visible error in the overlay. The current canvas,
tree, accepted geometry and persisted workspace remain unchanged; no partially loaded scene,
layout shift or frozen interaction remains. Returning to the untouched valid payload succeeds.

Result: **PENDING**

Notes:

## M70B-U5 — Ordinary workflow and text ergonomics remain coherent

1. Pan/zoom before opening the overlay and verify the canvas does not move under the pointer when
   it appears.
2. Scroll/select within a representative long payload and close it with the close button or Escape
   without choosing **Load payload**.
3. Reopen, load a valid payload and perform drawing, selection, drag, constraint authoring, camera
   and one new Undo/Redo operation.
4. Refresh after the accepted post-load edit.

Expected: the overlay contains its own overflow and never shifts the canvas. Closing without
loading is mutation-free. After load, ordinary authoring and camera controls work; new post-load history and
normal workspace persistence behave normally without claiming restoration of the old history
cursor.

Result: **PENDING**

Notes:

## Approval

M70B remains active until the supervising human records explicit approval here. A scoped approval
may accept M70B-U1 through M70B-U5 after objective findings receive owning-layer regressions and a
targeted recheck; it must not invent an unrecorded exhaustive replay. M71 remains deferred until
M70B is closed.
