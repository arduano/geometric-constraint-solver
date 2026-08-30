<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 audit — Workflow-led authoring workbench redesign

Status: **audit/contract complete; M88 implementation and qualification are pending**.
M87 closed on clean-qualified source
`32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`. This audit turns the accumulated sketch, managed-code
and headless capabilities into a workflow-led information architecture before presentation code is
changed.

## Audit method and current evidence

The audit traced the current HTML/CSS shell, durable Rust/WASM render path, ordinary sample
catalog, bundled code-project catalog, managed-control ownership projection and native headless
loop. Browser screenshots were inspected at 1920x1080, 1440x900, 1280x800 and 900 px wide.

| Viewport | Current measured result |
|---|---|
| 1920x1080 | The Code tab is only about 200 px wide despite the available desktop width. |
| 1440x900 | The four permanent columns are approximately 168 px tools / 240 px documents / 744 px canvas / 288 px Inspector. Tree and Design stack inside the 240 px document column. |
| 1280x800 | The Code surface is about 167 px wide while its editor remains wider and is clipped. |
| below 928 px | The complete document surface is hidden, so Code silently disappears. |

The managed source editor uses `0.58rem`, approximately 9.3 px, and has a fixed vertical cap. Its
content can overflow the narrow parent. Code is the fourth Design tab after Outline, Intent IR and
History, then vertically concatenates source, artifact state, every managed parameter, generated
ownership and the latest action.

Every durable panel render replaces the Code markup with `set_inner_html`. That is a direct risk to
the source cursor, text selection and scroll position even when the source draft itself survives.
The workbench permanently presents about 33 tool buttons, giving advanced Conics, Splines and
Continuity the same hierarchy as Select, Segment and ordinary dimensions. Twenty-five native
samples and twelve code projects are reached through nested hover/focus flyouts. The
`open-managed-lens` route exists, but no current UI element emits it, so truthful
“Modifiable in sketch.ts” ownership is not navigable.

Other hierarchy findings:

- Copy repro, Copy trace and Load repro occupy primary app-bar space beside New and Undo/Redo.
- Sketch tree and Design Outline expose overlapping object hierarchy at the same time.
- The permanent Inspector consumes 288 px even when selection context is empty.
- Project identity is split among Samples, Code and the footer; the app bar can still say
  “Untitled sketch” for a named code project.
- Problems are distributed among lifecycle state, footer notice, canvas overlay, source
  diagnostics, markers and retained-failure Inspector state.
- A large reproduction is still primarily clipboard-shaped even though real projects can exceed
  a practical conversational copy/paste size.

## Prominent workflow traces

### Start, open and import

Fresh boot goes directly to an empty canvas. New sketch, Start from code, native samples, code
projects and reproduction import live in different surfaces. The 37 samples are not searchable,
and the nested flyouts depend on hover-like traversal. Local persistence and the currently opened
project identity are not explained as one coherent workspace.

### Sketch-first authoring

The user selects from the permanently expanded tool inventory, opens detached variant overlays,
authors geometry, then adds constraints or dimensions. Object selection is split across canvas,
Sketch tree and Design Outline, while properties live in the far-right Inspector. Finish and
Cancel can fall below the left-rail fold in a dense desktop view.

### Code-first authoring

Opening a code project switches to the narrow Code tab. Editing `sketch.ts`, Apply/Revert,
parameters, artifact state and generated ownership compete in one scrolling feed. Code is treated
as auxiliary metadata even though managed source now owns important design values and reusable
structure.

### Canvas-to-code ownership

The authority model is sound but fragmented:

- **Modifiable in sketch.ts** means an authenticated source control with disclosed consumers.
- **Modifiable instance** means a solver draft/overlay that does not rewrite source.
- **Encoded** means the value is structural or artifact-defined.
- **Blocked** means current draft or authority state prevents the edit.

Selection does not provide an exact “Open in code” path to the managed owner. A dirty draft may
block a canvas edit while Apply/Revert is hidden in another tab. Native item and declaration
selection are also exposed as distinct user-facing concepts when one contextual path would be
clearer.

### Problems, export and agent handoff

There is no single durable Problems home. Diagnostic transport dominates the app bar while large
repro/trace payloads have no file-first route. `geosolve-headless inspect/edit/render` already
provides browser-free canonical project, source, report, control, SVG and PNG products, but the
browser does not offer a clean canonical `project.json`/`sketch.ts` handoff to that loop.

## Gridfinity stability and performance diagnosis

These are diagnosis facts and ordered M88 prerequisites, not repaired behavior.

- The Gridfinity project materializes a valid 62x62 full-rank, zero-DoF system. It has no
  `FixedPoint` and one Y `FixedCoordinate`; the symptom is not caused by an excessive point lock.
- Changing `baseBottomWidth` from 35.6 to 20 succeeds in native and release WASM paths with
  independently validated maximum hard residual `4.85e-12`.
- Debug browser cold open takes 23.3–37.7 seconds and a debug edit traps with WASM
  `memory access out of bounds`.
- Release cold open takes 2.67 seconds and the same release edit succeeds in 1.75 seconds.
- A native proxy reproducer overflows a 1 MiB thread stack while 2 MiB passes, supporting a shared
  stack-depth defect rather than invalid sketch mathematics. This proxy does not by itself prove an
  actual browser/WASM stack contract.
- The captured cold-open profile is
  `/tmp/geosolve-gridfinity-open.cpuprofile.json`, SHA-256
  `d7be3dcaad3fcf63342de0263b44c79bd7f95927b28ee5d59ab656244b0601f`.
- Cold project open restores the same checkpoint twice and immediately encodes/saves an unchanged
  checkpoint. Structural edit also checkpoint/restores and rehydrates.
- Redundancy analysis still performs decomposition work after numerical rank already equals every
  hard row. If independently reproduced at the solver owner, complete hard-row rank can safely
  imply empty redundancy evidence; it must not bypass independent residual validation.

M88 must address the prerequisites in this order:

1. establish release-mode UAT and exact build identity;
2. freeze the actual browser/WASM stack contract and a separate one-MiB native proxy regression,
   then reduce stack use on the shared owning path; neither result substitutes for the other;
3. remove duplicate checkpoint restore/validation and no-op post-open encoding;
4. only then add an owner-level full-row-rank redundancy shortcut with independent residual
   validation unchanged.

## Target information architecture

| Mode | Primary workspace | Supporting surfaces |
|---|---|---|
| Design | Complete canvas | Optional Explorer; right Inspector/Parameters/Problems host. |
| Split | Resizable complete canvas plus live source | Collapsible Explorer and contextual right host; either may collapse before the source drops below its minimum. |
| Code | Live source editor as the dominant center | Explorer collapsed by default at constrained widths; Parameters/Problems rehost into Code secondary tabs beside Generated/Artifacts, and the separate right host is not duplicated. |

- **Compact app bar:** File/project, Undo/Redo, true project title plus accepted/dirty/failed state,
  Design/Split/Code mode switch, Export, and an overflow Diagnostics area for repro/trace.
- **Narrow primary tool rail:** Select, Sketch, Constraint, Dimension and Modify, with the last-used
  tool visible. Variants and advanced families use click- and keyboard-accessible submenus.
- **One collapsible Explorer:** Objects and Outline share one contextual surface. Intent IR and
  detailed History remain available under Advanced rather than consuming permanent width.
- **Central workspace:** Design is canvas-first; Split is genuinely resizable; Code makes the
  source editor the primary central surface.
- **Right contextual tabs:** Inspector, Parameters and Problems in Design/Split. Parameters and
  Problems are single logical surfaces that rehost into Code mode, never independently stateful or
  simultaneously competing copies. Detailed branch, generated and artifact state is disclosed only
  when relevant.
- **First-class Code workspace:** sticky Apply/Revert/status, source tabs, source text at least
  12 px, at least `720 x 500` CSS px of usable editor at `1024 x 720`, and rehosted Parameters and
  Problems plus separate Generated and Artifacts surfaces.
- **Searchable start/open surface:** New sketch, Start from code, all 37 samples, project/repro
  import and recent workspaces.
- **Exact ownership navigation:** Open in code selects `sketch.ts`, focuses the authenticated owner
  span and preserves canvas selection.
- **Browser/headless handoff:** canonical `project.json` and `sketch.ts` export/import with stable
  source and control identities. Canonical export returns a typed refusal while an unapplied draft
  exists; draft-source download is separate and never masquerades as an accepted project.

## Authority boundaries

- Pane sizes, active layout and collapsed presentation surfaces are bounded presentation
  preferences, not canonical sketch, code-project, reproduction, solver or history authority.
- A pane resize or layout switch performs zero solve, code expansion, history publication or
  semantic workspace save.
- The last independently accepted scene remains visible under invalid code or presentation
  failure.
- Rust/WASM never executes TypeScript. Custom patch source stays user/AI-owned and read-only in the
  browser; pinned data-only artifacts remain runtime authority.
- Managed source edits retain exact-CAS control identity and one outer history transaction.
  Solver-instance point overlays never write solved coordinates into `sketch.ts`.
- Renderer presentation remains downstream of accepted scene authority. M88 does not reintroduce
  LOD, reduced-paint state or a second renderer/solver.
- `geosolve-sketch` and `geosolve-linkage` remain separate domain models over `geosolve-core`;
  the demo continues to consume public domain and audit APIs.

## Audit conclusion

M88 is a workflow and hierarchy redesign, not a cosmetic theme pass. The existing deep
customization remains available, but primary space and one-action commands must follow ordinary
start, sketch, inspect, parameter-edit and code-authoring flows. Advanced structure, diagnostics
and generated ownership remain reachable behind explicit, searchable and keyboard-accessible
surfaces.
