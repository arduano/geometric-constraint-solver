<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 final signoff UAT

This runbook covers the complete M98 authoring/embedding milestone and its collaboration,
local-canvas and full-toolbar amendments. **F041–F043 repairs are qualified and delivered;
U02 remains Fail pending human recheck. Acceptance and closure remain open.** Product
`b49e339e4fbeeebf8b0d513de109875983f2cb58` passes 293/293 mechanical obligations in
`20260913T020232-eaefaaaf`. [Qualification](M98_QUALIFICATION.md#qualified-uat-repairs)
records authenticated tests and delivered artifacts. Those results do not supply human passes.

## Review setup

Prepare disposable projects using [Getting started](GETTING_STARTED.md) and
[the authoring quickstart](M98_AUTHORING_QUICKSTART.md). Start shared fixtures with
distinct editor A/B and viewer invitations; reusing one invitation does not create
independent personal histories. Use two side-by-side browser windows with WebGL2,
a mouse with middle-button pan, and at least 1024 × 720 for each workbench. Record
browser version, device and network context.

| Fixture | Starting content |
| --- | --- |
| Shared playground | Circles `bore`/`other`, segments `first`/`second`, keyed polylines `corner`/`offsetCorner` |
| Shared manifold | 240 × 120 mm plate, 60 mm reservoir, 12 mm channels, 2.4 mm groove |
| Single-editor folder | `Ring radius` parameter, circle, imported patch and helper |
| Generator website | Gridfinity-style footprint using the headless engine and authoring SDK |
| Generator workbench | The same source-defined inputs, read-only source and generated geometry |

Keep invitation tokens outside reports and source control. Use copied examples for
conflict/restart checks and preserve a pristine manifold for export. Refreshing a
page retains edits; it does not reset source. Restart shared projects with the same
journal and invitations, omitting `--initialize true`.

## Readiness evidence

### Repair readiness — 2026-09-13

Qualified product `b49e339` passes all 293 obligations, including 49 ordinary and
17 collaboration browser workflows. Four archives installed offline with all 284
files matching; actual WASM readiness and state retention across restart passed.
Installed cold-corner checks passed in client/server prediction with stationary
other points, peer visibility, personal Undo/Redo and reload.

Integrated manifold typing ACK/navigation p95 was 314.6/296.8 ms; warm drag-preview
p95 was about 83 ms with zero reversals. These are bounded automated results,
not human acceptance or a 60 Hz claim. The generator site passed a fresh 1/1 browser
workflow from 395 authenticated inputs. [Qualification](M98_QUALIFICATION.md)
records exact product and run identities.

### Initial readiness — 2026-09-12

The preceding `b005b9e` preparation authenticated its 293 obligations and 284 installed
files. Shared editor/viewer startup, all 45 toolbar entries, client/server prediction,
independent camera state and journal-preserving restart passed. The folder smoke
covered 13 CLI/lease/recovery/restart cases. Generator website and workbench readiness
passed; the website was freshly bundled from 395 authenticated inputs.

The pristine manifold check/bake produced 18 finite regions, 17 holes and 28,800 mm²
net area at 0.02 mm maximum chord error. This export remains identified with
`b005b9e`; later browser repairs do not relabel it as a new export. Chrome
152.0.7977.82 used software WebGL2 rendering. Initial helper mistakes were corrected
without changing product bytes or converting their failed attempts into passes.
Subsequent human U02 review opened M98-F041.

## Human result ledger

Record **Pass**, **Fail** or **Deferred** only after review, with the actor/date and actual
outcome. A deferred item needs an explicit signoff scope decision. Keep automated evidence
separate, including cases where a fast operation prevents observing a loading indicator.

| ID | Review | Human result |
| --- | --- | --- |
| U01 | Join, roles and independent context | Not run |
| U02 | Drag continuity and local navigation | Fail — maintainer, 2026-09-12; M98-F041 corner release snapped back; repair delivered, human recheck pending |
| U03 | Complete catalog and staged construction | Not run |
| U04 | Constraint/dimension preselection | Not run |
| U05 | Fillet, Offset and geometry roles | Not run |
| U06 | Concurrent contributions and personal history | Not run |
| U07 | Shared raw source and explicit Apply | Not run |
| U08 | Manifold intent, metadata and slow-work feedback | Not run |
| U09 | Reconnect, reload and durable restart | Not run |
| U10 | Editable folders, external source and CLI | Not run |
| U11 | Generator inputs and custom host | Not run |
| U12 | Complete project and profile export | Not run |

Start with U01–U05, then collaboration/source U06–U09. Finish with folder/embedding/export
U10–U12. U02 records the reported corner-drag failure. Other human outcomes, subjective latency
and timing-dependent observations remain unrecorded until performed.

## Scenarios

### U01 — Join, roles and independent context

Open shared playground A/B/V. Inspect **Shared document**, connected count and participant
tooltip. In A, pan, zoom, select `first` and hide/isolate it through Explorer. In B, keep
a different camera and select `other`. Browse in V and inspect its authoring controls.

**Expected:** model content is shared; camera, selection, Inspector, visibility and active
tools stay personal. Remote selections/cursors are distinguishable from local ones. V can
browse with mutation controls unavailable. A peer publication does not move your camera,
steal selection or change the code cursor. Restore A's visibility before continuing.

### U02 — Drag continuity and local navigation

**M98-F041 repair delivered; human recheck pending:** the reported interior-corner
snap-back is reproduced and repaired. Recheck an interior corner before moving its endpoints,
including rotation through the outgoing leg’s original direction hemisphere. The installed
client/server checks pass. Preserve current source/history and record the human outcome here.

In A, drag a free endpoint of `first` slowly, stop, reverse direction and release. Repeat
once warm. B observes the accepted update. Pan/zoom before and after release. Move a keyed
vertex on `offsetCorner`, then use A's toolbar **Undo** and **Redo**.

**Expected:** coherent provisional movement without repeated backward jumps or seconds-long
input freezes on this simple sketch. Both editors settle on the same accepted geometry.
History restores/reapplies the chosen point without losing geometry. Camera stays local.
Record first-use and warm responsiveness separately; automated software-rendered timings
are context, not a promise of 60 Hz on the reviewer's device.

### U03 — Complete catalog and staged construction

Open **Sketch**, **Constraint**, **Dimension**, **Modify**: inspect all 25/13/5/2 tools for
editor availability. Create **Sketch → Polyline** in empty space; use **Step Back** when
offered, then **Finish**. Start another and cancel with Escape. Create **Rational Quadratic**,
set **Middle weight** to `2`, and place its three defining points. Inspect an arc's branch
controls and a spline's staged guidance. B observes completed objects.

**Expected:** prompts/options and staged previews make sense; valid completion publishes
source and peer geometry, cancellation does not. Mid-tool zoom/pan preserves inputs. Escape
clears a collected draft first, then exits on another press; an empty tool may exit directly.
This is a complete menu-availability sweep plus selected manual constructions. All recipes
have automated native/WASM coverage; do not record all 45 as manually constructed.

### U04 — Constraint and dimension preselection

Select `first`, choose **Constraint → Parallel**, zoom, then pick `second`. Choose
**Dimension → Radius**, set the **Dimension** combobox to reference, return to **Select**,
preselect `bore`, and enter Radius again. Inspect the measurement and shared source.

**Expected:** preselection survives zoom; the native collector uses valid operands. Parallel
constrains the intended segments. Reference Radius reports the circle without unexpectedly
adding a driving constraint. The selected reference mode survives tool activation. Accepted
relations and dimensions appear in B. Undo this test's changes if needed for later free drags.

### U05 — Fillet, Offset and roles

Use `corner` for **Modify → Fillet**: **Radius** `2`, pick both spans, inspect **Branch target**
and **Alternate arc**, wait for the provisional rounded geometry, then **Finish**. Use
`offsetCorner` for **Modify → Offset**, **Offset distance** `2`. Pick once, Escape, verify
**Clear picks** clears while distance persists, then Escape again. Reopen Offset, choose
`offsetCorner` in Explorer, use **Flip offset** and Finish. Pan/zoom during the operation.

Select writable geometry and use the role button offering **Change to Construction**;
Undo/Redo the role change. Repeat a disposable Offset in a server-prediction
session, after confirming that the host enables it.

**Expected:** complete fillet/offset previews precede publication; ordered Explorer operands,
options and history remain coherent. Role styling reflects source. Both prediction locations
retain local navigation. A URL preference alone does not prove server prediction is supported.

### U06 — Concurrent contributions and personal history

A changes `bore` radius; B creates an unrelated segment. A Undo/Redo should preserve B's
segment. Next A writes a radius and B writes a newer value to the same property; A attempts
Undo. On disposable geometry, one editor removes a target another editor selected for a tool.

**Expected:** independent edits compose. Undo checks A's contribution rather than replacing
the whole document with an old snapshot. It cannot overwrite B's newer value or remove a
new dependency owned by B. Conflicts/rejections are explicit, leaving coherent accepted
geometry. Deleted or replaced selections cannot silently retarget another object. Record
actual notices and order of accepted edits instead of predicting a simultaneous race winner.

### U07 — Shared raw source and explicit Apply

Use **split** layout in A/B. A types an unfinished expression in the source editor. B confirms
the same raw draft, then makes a simple canvas edit. A clicks **Apply**, observes rejection,
repairs the syntax and Applies again. Add a harmless comment after submitting Apply. Try
**Undo my typing** separately from toolbar Undo.

**Expected:** raw text synchronizes before parsing. Invalid Apply preserves both draft and
accepted geometry; canvas edits remain possible. If incomplete source prevents safe writeback,
reconciliation is explicit and the draft remains intact. Apply captures a revision; later
typing stays unapplied. A fast Apply may finish before the later keystroke: that attempt does
not demonstrate the in-flight case. Typing Undo changes draft and requires Apply; toolbar Undo
owns personal model history. Finish with valid, deliberately applied source.

### U08 — Manifold intent and slow-work feedback

In the shared manifold's **Parameters** tab, change **Channel width** from `12` to `10` and
press Enter. While it solves, hover, pan and zoom; B types a harmless comment. Inspect all
four channels, the stair passage, rounded bends/ends and independent seal groove. Review
dimension display **Focused / All / Hidden** and contextual inspection. Change a source-owned
name through **Name and description**, or a **Show … in overview** option; inspect source writeback.

**Expected:** one width updates the four actual passages, with the separate 2.4 mm groove.
After about 0.5 s of slow work the canvas dims and **Solving…** explains that navigation
remains available. Accepted geometry stays visible; indication clears on completion/rejection.
Camera and peer typing remain useful throughout. Source owns labels/overview intent through
`isKeyConstraint` / `isKeyParameter`. Restore this test's edits. A solve finishing sooner
need not flash a loading indicator; mark that observation unperformed if necessary.

### U09 — Reconnect, reload and durable restart

On disposable playground only, create a recognizable object and leave a harmless unapplied
comment. Reload A/B. Ask the facilitator to restart this UAT server with the same journal
and invitations, omitting initialization. Observe connection state; use **Reconnect** if needed.
Optionally take A briefly offline, restore connectivity and inspect **Download pending work**
or **Recover saved work** if offered.

**Expected:** accepted geometry, raw draft, invitations and personal history survive.
Disconnected browsing remains local. Recovery does not duplicate accepted work or silently
replace current changes. Routine reconnect is not evidence of full offline semantic editing
or every lost-acknowledgment fault; those boundaries have separate automated qualification.

### U10 — Editable folder, external files and CLI

Open the single-editor fixture. Change **Ring radius**, inspect plaintext, Undo/Redo. Start
a field draft in A; have the facilitator save another value externally and attempt the stale
field action. Inspect pending intent, resolve the draft, and **Refresh from disk**. In B use
**Take over editing**. Edit the imported helper/patch, try invalid syntax, then repair it.
Use prepared installed-CLI status/takeover/fresh-status/apply/outcome commands and retry the
identical operation ID and payload.

**Expected:** imported dependencies participate in compilation. A stale field does not gain
a newer basis silently; pending intent remains inspectable. Only one tab owns this mode's
editing lease. Invalid external source stays on disk while prior accepted geometry remains.
CLI retry resolves one outcome. Source and semantic sidecar stay understandable. In shared
mode, external saves instead enter the raw draft and require Apply; these modes differ.

### U11 — Generator inputs and custom host

Open the generator website: default is 125.5 × 83.5 mm with 24 bores. Set **Columns** to `1`:
expect 41.5 mm width and eight bores. Try **Mounting** and **Rounded corners**. Rapidly change
Columns `3 → 2 → 1`, then set **Rows** to `0` and **Download profile**. Restore valid inputs,
try **Cancel update** when present, then **Reset**. Inspect source-declared input metadata.

**Expected:** loops/conditions change topology; only accepted results supply preview/download.
Invalid input or cancellation retains the previous accepted profile and accepted input values.
Invalid Rows `0` cannot become export provenance. Generator output has no reverse source-edit
or dragging authority. Cancellation may be too fast to observe: leave that subcheck unobserved
rather than assuming success. The website uses its own controls, independent of the workbench.

Open the same project in the **Generator workbench** as well. Use **Take over editing** if
the closed readiness browser still owns this single-editor lease. Inspect **Generator inputs**,
change **Columns** with **Apply Columns**, and compare accepted geometry. In Split, code is
read-only and geometry authoring is unavailable; source-defined generator inputs remain
editable. Restore Columns to `3`. This checks workbench capabilities separately from the
custom website's controls.

### U12 — Complete project and profile export

After handling the source draft, use **File → Export canonical project…**. Inspect/open it
in a separate standalone session. With the pristine manifold copy, run the prepared installed
CLI check/bake commands; write output outside watched folders. Review the resulting region,
hole and provenance report. Compare generator download metadata with its accepted inputs.

**Expected:** project handoff includes dependencies and semantic state needed to reconstruct
the design. Export includes channel walls, arcs, caps and seal geometry. The pristine manifold
has 18 bounded regions, 28,800 mm² complete net area and five radius-3 bores; edited fixtures
need their own expected values. Sampling is bounded in model space. Named outputs retain
holes; open, unsupported or uncertain topology fails explicitly instead of disappearing.

An optional SVG of the exported sampled rings can help inspect the profile.
Region colors do not identify pockets or machining depths.

## Facilitator commands and restart

The [CLI workflow](M98_AUTHORING_QUICKSTART.md#a-local-agent-edit) documents
status → takeover → fresh status → Apply → outcome. Browser smoke checks can leave
the folder lease with their closed test tab; use **Take over editing** when needed.
Ordinary folder restart rotates its session token; open its newly printed URL.

Use the current installed CLI and a fresh output path outside the source folder:

```bash
geosolve check ./manifold-uat
geosolve bake ./manifold-uat --out ./manifold-uat-profiles.json --chord-error-mm 0.02
```

For U09, stop editing and let pending work settle. Capture the disposable project's
source, working draft, accepted state and personal history, stop its server, and
restart the same project without initialization. Compare those states after reconnect.
Preserve unapplied draft text so the check exercises recovery beyond an empty session.
Do not use a historical milestone process helper as a general-purpose restart command.

## Failure reporting and signoff limits

For each failure capture case ID, fixture/user, browser/device, visible accepted revision,
steps and expected/actual behavior. Retain a screenshot or short recording and exact notice;
download pending work when offered. Keep invitation tokens out of reports. Preserve source,
draft and journals for diagnosis; avoid reset/reinitialization before capture.

Scope is the current React catalog; archived native-profile Fillet output remains separate.
Generated properties/roles need writable source paths. Trusted invitations, Linux folder
recovery and bounded 2D regions are supported; production identity providers, hostile-code
sandboxing, full offline semantic reconciliation, multi-server scale, 3D solids and manufacturing
depths are excluded. No arbitrary-size or 60 Hz claim is made. Automated four-editor and
eight-editor/24-viewer evidence remains distinct from these human sessions.

- [ ] Human results recorded and failures resolved or explicitly scoped.
- [ ] Maintainer accepts the complete amended M98 scope.
- [ ] Close M98 only after acceptance.
