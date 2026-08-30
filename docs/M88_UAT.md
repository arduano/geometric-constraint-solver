<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 UAT — Workflow-led authoring workbench redesign

Status: **planned; all rows are pending and no M88 UI acceptance is claimed**.

## Test conditions

Run against an exact optimized/release browser build whose source, tree and served bytes are
recorded. Use fresh and restored local workspaces. Exercise pointer and keyboard at 1440x900 and
1024x720; repeat code-state and resize rows at 1920x1080 when useful. Do not use a debug build for
performance acceptance. Use five fresh runs for each Gridfinity timing, from sample-open dispatch
to accepted-frame presentation and from Apply dispatch to replacement accepted-frame presentation.
Run the actual browser/WASM stack check and separate one-MiB native proxy regression; neither
substitutes for the other.

| Row | Exercise | Pass condition | Status |
|---|---|---|---|
| M88-U1 | Open the application with no prior workspace. Use the start/open surface to create a native sketch, Start from code, find one native and one code sample by typing, import a project/repro and inspect recents. | Every entry is reachable within two deliberate actions. All 25 native and 12 code samples are keyboard-searchable without hover. The opened project title and accepted/dirty/failed state agree in one app-bar location. | pending |
| M88-U2 | At 1440x900 switch Design -> Split -> Code, resize both Split panes by pointer and keyboard, reset them, reload, then return to Design. | Split provides at least 520x500 px of editable source. The canvas and editor remain usable, sizes reset predictably and bounded presentation preferences reload without entering project/repro authority. | pending |
| M88-U3 | At 1024x720 open Typed Panel directly in Code mode, switch files and secondary Code surfaces, then return through Split and Design. | Code never disappears and provides at least `720 x 500` CSS px of usable editor. Text is at least 12 px, stays inside its parent and has no fixed 27 rem height cap. Parameters and Problems retain one logical state while rehosting; no duplicate copy diverges. | pending |
| M88-U4 | Put the caret in `sketch.ts`, select text, scroll deeply and create an unapplied dirty draft. Select several canvas items, resize panes and switch Design/Split/Code repeatedly. | Selected file, cursor, text selection, scroll and dirty draft remain exact. Sticky Apply/Revert/status remains visible whenever relevant; no durable render replaces the active editor state. | pending |
| M88-U5 | Observe the presentation work ledger while resizing/collapsing panes, switching layout and changing right/Explorer tabs. Repeat while an accepted dense scene is visible. | Every presentation-only action performs zero solve, code expansion, history publication and semantic workspace save. Accepted complete paint remains visible and future camera/selection input works; no LOD or reduced-paint state exists. | pending |
| M88-U6 | Generate the frozen pre-redesign command/variant manifest, traverse every entry by pointer and keyboard, then semantically exercise Select, Segment, Rectangle, Coincident, Distance, Fillet, Conics, Splines, Continuity and branch/display options. | The post-redesign inventory matches the manifest one for one. Frequent families are one action away; every advanced entry is within two actions, retains its current semantics and never requires hover. Finish/Cancel and current tool state remain visible when applicable. | pending |
| M88-U7 | In Typed Panel select either generated Fillet, invoke Open in code, edit `radius: mm(4)` to `mm(2)` and Apply. Then inspect a solver-instance point, encoded field and blocked dirty-draft field. | Open in code focuses the exact authenticated source owner and preserves canvas selection. Both Fillets update in one outer history row. Source/instance/encoded/blocked language and permitted actions remain consistent across Inspector, Parameters and Code. | pending |
| M88-U8 | Introduce invalid managed source at a known line/column, navigate among canvas, Inspector and Code, then correct and Apply it. | The prior accepted canvas remains complete. Exactly one persistent Problems entry identifies and focuses the source position without losing the draft; correction clears/resolves it through one accepted outer transaction. | pending |
| M88-U9 | With an unapplied valid draft and then an invalid draft, attempt canonical export and separately download raw draft source. Apply/Revert to clean state, export `project.json` and `sketch.ts`, inspect with `geosolve-headless`, perform one exact-CAS edit, render, and atomically import the emitted project. Download a deliberately large repro/trace rather than copying it. | Dirty and invalid drafts each produce a typed canonical-export refusal; raw draft download claims no accepted-project authority. After acceptance, browser/headless source, project and control identities agree. The edited project imports only after complete validation and matches the headless scene. Large diagnostics have a file route under Diagnostics. | pending |
| M88-U10 | With exact build identity recorded, perform five fresh release cold-opens of Gridfinity and five `baseBottomWidth: 35.6 -> 20` edits. Run the actual browser/WASM stack contract/no-trap check and separate one-MiB native proxy regression. Maximize, restore and resize while inspecting the complete profile. | Cold-open median/max are at most 2.0/2.5 s and edit median/max at most 1.25/1.75 s on the recorded machine. Both distinct stack checks pass. Geometry is finite, zero-DoF/full-rank authority is truthful, independently validated hard residual is at most `1e-9`, and full accepted paint remains interactive. | pending |

## Required mechanical evidence before disposition

- Exact owner regressions for stack reduction, checkpoint/open deduplication and any full-row-rank
  redundancy shortcut, with separate native-proxy and actual browser/WASM stack evidence.
- Independent finite and hard-residual validation for Gridfinity after open and edit.
- Locked native/WASM tests covering canonical browser/headless project interchange.
- Presentation work-ledger tests proving pane/layout actions have no semantic work.
- Editor-state tests covering durable canvas selection and layout transitions.
- Frozen command/variant manifest parity plus keyboard/focus checks for every entry, pane
  separators, tabs, start/search and Open in code.
- Formatting, warnings-denied workspace Clippy/tests, relevant TypeScript checks, Rustdoc, locked
  WASM and release Trunk assembly before nomination.

Human review may accept visual hierarchy and interaction feel. It cannot replace the owning-layer
stack, solver, accepted-scene, transaction or headless parity evidence.
