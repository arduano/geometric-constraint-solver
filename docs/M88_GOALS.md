<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 — Workflow-led authoring workbench redesign

Status: **complete and accepted on 2026-09-01**. The supervising user's milestone-level close
decision accepts M88-U1 through M88-U10 without claiming a separately logged row-by-row replay.
The accepted immutable F004 candidate is `/tmp/geosolve-m88-react-uat.KGhA7s`, served at
`http://100.94.63.83:18088/`; its eight-file manifest aggregate is
`700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071` and its optimized release-WASM
SHA-256 is `22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`.
The former port-`8080` Rust-DOM candidate remains historical rollback evidence only, and its
compatibility host is retired from source after acceptance. No Pages/public deployment or accepted
service retirement is inferred.
`docs/M88_AUDIT.md` remains the historical pre-redesign diagnosis and
`docs/M88_IMPLEMENTATION.md` owns current implementation and qualification evidence.

## Outcome

Make the workbench coherent for sketch-first, code-first and agent-assisted authoring. Frequent
actions receive primary space; advanced capabilities remain fully reachable through contextual
submenus, disclosures and secondary tabs. Code becomes a first-class central workspace rather
than a narrow auxiliary tab.

## Required behavior

1. Complete the Gridfinity stability prerequisites in the audited order: release build identity,
   an actual browser/WASM stack contract plus a separate one-MiB native proxy regression, shared-
   path stack reduction, duplicate checkpoint/no-op encoding removal, then an owner-qualified full-
   row-rank redundancy shortcut. Neither stack test substitutes for the other.
2. Replace the fixed four-column shell with Design, Split and Code modes. Split panes are
   resizable, keyboard-operable and resettable.
3. Keep presentation preferences separate from canonical sketch, code-project, reproduction,
   history and solver authority.
4. Introduce a compact app bar containing File/project, Undo/Redo, true project title/status,
   Design/Split/Code switch and Export. Put repro and trace beneath Diagnostics/overflow.
5. Replace the permanently expanded tool palette with a narrow Select/Sketch/Constraint/Dimension/
   Modify rail. Preserve every current tool and variant in click- and keyboard-accessible submenus;
   no command may require hover. Freeze an exact pre-redesign command/variant manifest and require
   one-for-one reachable parity.
6. Replace simultaneous Sketch tree and Design hierarchy with one collapsible Explorer containing
   Objects/Outline choices. Keep Intent IR and detailed History available under Advanced.
7. Make the central Code workspace fill available space, use at least 12 px source text and provide
   sticky Apply/Revert/status. Separate source tabs from Parameters, Problems, Generated and
   Artifacts surfaces. At `1024 x 720`, provide at least `720 x 500` CSS px of usable editor.
8. Preserve the selected file, source cursor, text selection, scroll position and dirty draft
   across canvas selection, durable projection changes, pane resize and layout switches.
9. Unify contextual work under right-side Inspector, Parameters and Problems tabs. Parameters and
   Problems each have one state model, rehosted into Code mode rather than duplicated. Keep advanced
   branch/generated detail behind disclosures without hiding current failure or dirty state.
10. Add exact Open-in-code routing from a managed Inspector parameter to its authenticated source
    owner while retaining stable canvas selection.
11. Add a searchable start/open surface covering New sketch, Start from code, all 25 native and 12
    code samples, project/reproduction import and recent workspaces.
12. Add canonical `project.json`/`sketch.ts` browser export and atomic import compatible with
    `geosolve-headless`, including stable source/control identity checks. Refuse canonical export
    with a typed dirty-draft result until Apply or Revert; allow a separate raw draft-source
    download that claims no accepted project authority.
13. Provide file download for large reproduction and trace products so clipboard size is not the
    only diagnostic transport.
14. Retire the old narrow Code-tab DOM/CSS only after replacement parity and UAT are proven.

## Measurable acceptance floor

- At 1440x900, Split mode exposes an editable source area at least 520 px wide and 500 px high.
- At 1024x720, Code mode exposes at least `720 x 500` px of usable editor and never silently
  disappears.
- Source text is at least 12 px, remains inside its parent and has no fixed 27 rem height cap.
- Pane resize and layout change perform zero solve, expansion, history publication or semantic
  workspace save.
- Every entry in the frozen command/variant manifest is reachable by pointer and keyboard without
  hover-only navigation; all 37 samples are keyboard-searchable.
- Invalid managed source keeps the prior accepted canvas and creates one persistent problem linked
  to its exact source position.
- A browser-exported canonical project is accepted by `geosolve-headless`; a headless-edited
  project reimports atomically with matching source/control identities.
- A dirty or invalid source draft produces a typed canonical-export refusal; downloading its raw
  source cannot be confused with exporting the last accepted project.
- Editor cursor, selection, scroll, dirty draft and selected file survive layout and canvas
  selection changes.
- On the recorded reference machine, five fresh optimized browser runs have Gridfinity cold-open
  median at most 2.0 s and maximum at most 2.5 s; the `baseBottomWidth: 35.6 -> 20` edit has median
  at most 1.25 s and maximum at most 1.75 s. The actual browser/WASM stack contract and separate
  one-MiB native proxy regression both pass without weakening validation.

## Authority boundaries

- Independent finite/domain/branch/hard-residual validation remains mandatory for every
  success-like solve or materialization result.
- UI presentation never becomes solver, accepted-scene, code-project or history authority.
- Managed source and instance overlay ownership remain distinct. Solved coordinates are never
  written back to source.
- The browser and Rust/WASM paths do not evaluate custom TypeScript. Pinned data-only artifacts
  remain the only custom-module runtime input.
- No LOD, adaptive-detail or reduced-paint mode is admitted. Every accepted presentation keeps the
  complete scene and hit authority.
- Headless and browser workflows share canonical projects and public rendering/domain APIs; neither
  duplicates equations.

## Non-goals

- A general-purpose IDE, language server, arbitrary TypeScript runtime or browser package manager.
- An AI chat panel, stateful agent service, prompt history or autonomous cloud execution.
- Mobile/tablet redesign; M88 targets usable desktop layouts down to 1024x720.
- New primitive, constraint, dimension, solver equation, branch heuristic or weighted priority
  semantic.
- Solid modeling, CAM, toolpaths, cutter compensation or production manufacturing validation.
- Reintroducing LOD or solving performance problems by hiding accepted geometry.
- Public deployment or release nomination before the implementation ledger and UAT are complete.
