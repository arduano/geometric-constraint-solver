<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 final signoff UAT

This runbook covers the complete M98 authoring/embedding milestone and its collaboration,
local-canvas and full-toolbar amendments. **F041–F043 repairs are qualified and delivered;
U02 remains Fail pending human recheck. Acceptance and closure remain open.** Product
`b49e339e4fbeeebf8b0d513de109875983f2cb58` passes 293/293 mechanical obligations in
`20260913T020232-eaefaaaf`. [Qualification](M98_QUALIFICATION.md#qualified-uat-repairs)
records authenticated tests and delivered artifacts. Those results do not supply human passes.

## Review setup

Open the [UAT launch page](http://100.94.63.83:18120/) for disposable invitation links and
fixture access. These are disposable UAT services; existing previews and user histories
remain preserved. Do not initialize or reset the existing ports 18111/18112 or the original
18108 manifold. The older static port 18110 is not this toolbar candidate.

| Fixture | Port | Starting content |
| --- | --- | --- |
| Shared playground | 18121 | Circles `bore`/`other`, segments `first`/`second`, keyed polylines `corner`/`offsetCorner` |
| Shared manifold | 18122 | Pristine qualified 240 × 120 mm plate, 60 mm reservoir, 12 mm channels, 2.4 mm groove |
| Single-editor folder | 18123 | `Ring radius` parameter, circle, imported patch and helper |
| Generator website | 18124 | Gridfinity-style footprint using the headless engine and authoring SDK |
| Generator workbench | 18125 | The same source-defined inputs, read-only source and generated geometry |

Fixtures and evidence live under `target/m98/uat-20260912`. The UAT manifold uses the
qualified source baseline; the user's separate 40 mm reservoir remains untouched.
Use different editor invitations A/B and viewer V. Reopening one user's invitation does
not establish different personal histories. Compare two side-by-side windows; use Chrome
or Chromium with WebGL2, a mouse with middle-button pan, and at least 1024 × 720 for the
workbench. Record browser version, device and network context.

The launch page grants access to these test documents to devices allowed by the existing
Tailscale network. It contains no invitations for the user's existing documents. Invitation
and process records stay in ignored mode-0600 files. Use these fixtures only for UAT.

The facilitator instructions below supply CLI/export commands and a safe restart.
Do not restart live user services. Refreshing a page retains the fixture's edits; it does
not reset its source. Use new disposable geometry for conflict tests and preserve a pristine
manifold copy for export checks.

## Readiness evidence

### Repair readiness — 2026-09-13

All 49 ordinary and 17 collaboration browser workflows pass the complete 293-obligation
qualification. Installed cold-corner checks pass in client/server prediction, with unchanged
other points, peer visibility, personal Undo/Redo and reload. Integrated manifold typing
ACK/navigation p95 is 314.6/296.8 ms; warm drag preview p95 is about 83 ms with zero reversals.
These are bounded automated results, not human acceptance or a 60 Hz claim.

| Replacement readiness | Outcome |
| --- | --- |
| Exact installation | Four authenticated archives; all 284 installed files match; 23 production files retained |
| Shared UAT playground/manifold | 24 exact HTTP/MIME routes each; two real editors and all three WASMs; revisions 6/0, 36/19 files and four invited histories preserved |
| Existing user playground/manifold | Same checks; revisions 12/1, 59/25 files and three invited histories preserved |
| Folder and generator workbench | Source/retained state preserved; 24 transport checks each, recorded HTML compatibility shim, accepted browser geometry |
| Generator website | Fresh bundle from 395 authenticated inputs; focused browser 1/1, ten served files, three denied routes and exact engine WASM pass |
| Launcher and facilitator | Current invitations/folder links, 18 links, five allowed routes and three denied routes verified; current installed CLI authenticated |

Reload the launcher and existing shared tabs. Shared invitations are unchanged. The two
single-editor folder tokens rotated; reopen their links from the launcher and use the ordinary
**Take over editing** handoff if needed. No fixture was reset. The pristine manifold export
remains historical `b005b9e` evidence; its geometry/provenance is not relabelled as a new export.

### Initial readiness — 2026-09-12

**Initial readiness as of 2026-09-12; subsequently U02 opened M98-F041.**
The following describes the prior preparation. Product source, solver mathematics, branches,
tolerances and the 271-case golden are unchanged. This preparation adds documentation and
ignored UAT fixtures/helpers, with no public API or implementation changes.

| Fresh readiness check | Outcome |
| --- | --- |
| Authenticated qualification and installed archives | 293/293 prior obligations authenticated; all 284 installed package files matched |
| Shared playground and manifold | 24 exact HTTP/MIME routes each; A/client, B/server and viewer opened with accepted WebGL2 geometry and no page errors |
| Toolbar and prediction | All 45 entries available in both editors on both fixtures; viewer authoring group empty; native Offset draft ready in client/server modes, successful server-preview requests observed |
| Navigation and fixture retention | Camera change in A leaves B unchanged; source, drafts, history and journals remain at revision 0, 11/19 durable files preserved |
| Guarded playground restart | Same document, invitations, histories and 11 durable files preserved; read-only invited clients reconnect |
| Single-editor folder | Actual accepted two-circle canvas and `Ring radius = 10`; 49 transport checks including the explicit HTML shim; 13 separate disposable CLI/lease/recovery/restart checks passed |
| Generator website | Existing focused browser workflow passed 1/1 in 7.663 s; 10 exact served files, three denied routes, actual engine WASM and visible accepted default passed |
| Generator workbench | 23 production routes checked; accepted geometry, defaults, read-only code, disabled Undo/Redo and empty authoring group verified; source and input defaults unchanged |
| Pristine complete manifold export | Check/bake passed: 18 finite regions, 17 holes, 28,800 mm² net area, maximum chord error 0.02 mm |
| Launch page | 18 expected links, five allowed routes, three denied routes and visible 18-region export preview passed |

Browser readiness used Chrome **152.0.7977.82** with software WebGL2 rendering on main-pc;
screenshots retain actual completed canvas output. Client prediction loaded the exact three
nominated WASM modules; server prediction loaded the demo/collaboration modules and used
server-native prediction. This readiness pass does not establish new latency measurements.
The qualified first-use/warm drag p95 values remain 417/143/117 ms with zero reversals;
four-editor/manifold/Gridfinity navigation p95 values remain 363/373/251 ms.

The website is a **new bundle from authenticated qualified inputs**, since the earlier
website distribution was not retained. Its 395 input files stayed unchanged and a second
build matched the frozen site. Its engine WASM is the exact qualified 14,303,000-byte module.
Single-editor CLI binds loopback; ports 18123/18125 use the earlier reviewed Tailscale proxy
pattern. Their JS/CSS/WASM assets are exact; HTML deliberately adds the recorded cryptographic
UUID compatibility shim needed over plain Tailscale HTTP. Shared ports need no transform.

The first five shared readiness attempts caught helper assumptions about asynchronous Escape,
mode-dependent WASM loading and empty viewer tool groups. The sixth passed. The generator-folder
helper similarly initially tried to inspect a menu in its empty authoring group; its second
attempt passed. The website helper's three early DOMRect/worker-response capture errors are
recorded separately. These were corrected in ignored readiness helpers without changing the
product, hiding earlier failures or rerunning the integrated gate. The launch-page check first
expected one excess link, then hit Node's HTTP parser assertion with unconsumed responses;
its third attempt passed after correcting the count and consuming response bodies.

Exact commands were run from the M98 worktree. Build/CLI smoke commands used the pinned
shell prefix `nix-shell shell.nix -I nixpkgs=/nix/store/6z7xnswwnq9dw8vvi7gb9cj3szdgasf6-source --run 'COMMAND'`:

```bash
python3 target/m98/verify-qualification.py 20260911T164104-576e7789
python3 target/m98/coordination/uat-readiness/prepare-shared.py
node target/m98/coordination/uat-readiness/verify-shared.mjs target/m98/uat-20260912/shared-readiness-6.json
node target/m98/coordination/uat-readiness/verify-folder-browser.mjs target/m98/uat-20260912/folder-browser-readiness-1.json
python3 target/m98/coordination/uat-readiness/folder-smoke.py
python3 target/m98/coordination/uat-readiness/prepare-generator.py
node target/m98/uat-20260912/generator-staging/examples/generator-website/scripts/build.mjs
node --test target/m98/uat-20260912/generator-staging/examples/generator-website/scripts/browser.test.mjs
node target/m98/coordination/uat-readiness/verify-generator.mjs
node target/m98/coordination/uat-readiness/verify-generator-folder.mjs target/m98/coordination/uat-readiness/generator-folder-browser-2.json
node target/m98/coordination/uat-readiness/verify-launcher.mjs target/m98/uat-20260912/launcher-readiness-3.json
```

All above completed successfully. Receipts and exact CLI argv/outcomes are indexed in
`target/m98/coordination/uat-readiness/folder-result.md`, `generator-result.md` and
`generator-folder-result.md`. The shared preservation receipts, screenshots and process
records live in `target/m98/uat-20260912/`. Source baseline maps are outside watched folders;
never restore them over a running journal. Documentation-only qualification preserves the
prior product rather than nominating a new build.

`target/m98/uat-20260912/readiness-summary.json` indexes thirteen passing evidence files
by hash and the six current service processes. `git diff --check` and
`./scripts/release-gate.sh --docs-only --since b005b9e1bdb1a120ec9363d4c8684d1f8f5c3d00`
passed for this prose-only handoff; format/Clippy/native/WASM release obligations retain
their authenticated product qualification rather than being rerun for documentation.

## Human result ledger

Record **Pass**, **Fail** or **Deferred** only after review, with the actor/date and actual
outcome. A deferred item needs an explicit signoff scope decision. Keep automated evidence
separate, including cases where a fast operation prevents observing a loading indicator.

| ID | Review | Human result |
| --- | --- | --- |
| U01 | Join, roles and independent context | Not run |
| U02 | Drag continuity and local navigation | Fail — supervising user, 2026-09-12; M98-F041 corner release snapped back; repair delivered, human recheck pending |
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
Undo/Redo the role change. Repeat a disposable Offset using the launch page's server-prediction
entry, after confirming that host supports it.

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

Open **Generator workbench** from the launch page as well. Use **Take over editing** if
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

The launch page also links the pristine profile JSON and an SVG drawn directly from its
sampled rings. Region colours aid inspection; they do not identify pockets or machining depths.

## Facilitator commands and restart

Run from the M98 worktree. [Folder/CLI instructions](http://100.94.63.83:18120/facilitator.html)
provide status → takeover → fresh status → Apply → outcome, source file locations and the
single-editor restart helper. Browser readiness can leave that mode's lease with its closed
test tab; **Take over editing** is the ordinary handoff. Single-editor restart rotates its
session token; reopen the updated launch link. Shared invitations remain stable on restart.

The following historical `b005b9e` CLI commands produced the retained pristine export.
The facilitator link supplies the current installed CLI. Choose a new output filename
for a later human run, keeping exports outside the watched source folder:

```bash
uat_cli="$PWD/target/m98/installed-drag-preview-20260911T164104-576e7789/node_modules/.bin/geosolve"
"$uat_cli" check target/m98/uat-20260912/manifold-export-source
"$uat_cli" bake target/m98/uat-20260912/manifold-export-source \
  --out target/m98/uat-20260912/exports/manifold-profiles.json --chord-error-mm 0.02
```

For U09, stop typing/dragging and let pending work settle. The scoped helper verifies process
identity, captures a private backup, restarts only port 18121 or 18122, omits initialization,
and refuses changed inputs. Use the current product below and a fresh capture filename
in both commands; the example destination must not already exist:

```bash
node target/m98/coordination/drag-repair/capture-preview.mjs \
  target/m98/uat-20260912/playground-location.json \
  target/m98/uat-20260912/playground-human-restart-before.json
python3 target/m98/coordination/uat-readiness/restart-uat-shared.py \
  target/m98/installed-drag-preview-20260913T020232-eaefaaaf/installation.json \
  target/m98/uat-20260912/playground-location.json \
  target/m98/uat-20260912/playground-location.json \
  target/m98/uat-20260912/playground-human-restart-before.json
```

The replacement restart preserved playground revision 6 and all four invited histories;
read-only capture/compare also passed after browser readiness. Both single-editor UAT
folders were upgraded with source/state preservation and new token links. Human U09
still needs edits and an unapplied draft before its separate restart observation.

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
- [ ] Supervising user accepts the complete amended M98 scope.
- [ ] Close M98 only after acceptance.
