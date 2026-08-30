<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 implementation ledger — Workflow-led authoring workbench redesign

Status: **audit/contract complete; every implementation item is pending**. No M88 production code,
test, qualification artifact, immutable candidate or publication is claimed. Incoming M87 authority is
clean-qualified source `32c72892772ee09f8b904153484b02fd9923dc25`, tree
`38f7175f93c87d11422f5de00e78208f8cf315bb`.

## Phase 0 — Stability and measurement prerequisite

Complete these items in order before treating browser layout UAT as credible:

- [ ] Record the exact release-mode browser build, toolchain, served-byte identity, reference
  machine and five fresh Gridfinity cold-open/edit timings. Require cold-open median/max at most
  2.0/2.5 s and edit median/max at most 1.25/1.75 s on that machine. Freeze timing boundaries from
  sample-open dispatch to accepted-frame presentation and from Apply dispatch to replacement
  accepted-frame presentation.
- [ ] Reproduce the Gridfinity edit through the smallest public owning boundary. Freeze the actual
  browser/WASM stack contract and browser no-trap check separately from a one-MiB native thread-
  stack proxy regression. Reduce stack use on the shared owning path until both pass without
  weakening geometry validation; do not treat either test as proof of the other.
- [ ] Remove duplicate cold-open checkpoint restore/validation and the immediate unchanged
  post-open encode/save. Prove accepted project, history, persistence, generated identity and
  failure retention remain byte/semantically equivalent as applicable.
- [ ] Re-audit structural edit rehydrate/checkpoint work and remove only independently proven
  duplication.
- [ ] At `geosolve-core`, reproduce full-hard-row rank behavior and add the smallest regression
  proving full row rank produces complete empty redundancy evidence. Implement a shortcut only
  behind that regression; retain independent residual validation unchanged.
- [ ] Re-run release/native/WASM Gridfinity open and edit measurements and record the resulting
  profile evidence.

Incoming diagnosis, not completion evidence: the valid Gridfinity system is 62x62, full rank and
zero DoF, with no `FixedPoint` and one Y `FixedCoordinate`. Changing `baseBottomWidth` from 35.6 to
20 succeeds natively and in release WASM at maximum hard residual `4.85e-12`. Debug cold open is
23.3–37.7 seconds and debug edit traps memory OOB; release open/edit are 2.67/1.75 seconds. A 1 MiB
native proxy stack overflows and 2 MiB passes; this is evidence about shared stack depth, not the
browser/WASM stack setting itself. Profile:
`/tmp/geosolve-gridfinity-open.cpuprofile.json`,
SHA-256 `d7be3dcaad3fcf63342de0263b44c79bd7f95927b28ee5d59ab656244b0601f`.

## Phase 1 — Workflow and information-architecture freeze

- [ ] Freeze a machine-readable manifest of every current command and variant, then classify every
  entry as primary, contextual, advanced or diagnostic using the audited start/sketch/code/
  ownership/problem flows.
- [ ] Freeze the Design, Split and Code layout contract at 1920x1080, 1440x900 and 1024x720.
- [ ] Freeze focus order, keyboard navigation, pane minimums, reset behavior and presentation-only
  persistence schema.
- [ ] Freeze one project identity/status model and one durable Problems model before markup changes.

## Phase 2 — Shell and layout foundation

- [ ] Implement the compact app bar and true project title/accepted-dirty-failed state.
- [ ] Implement Design, Split and Code modes with pointer and keyboard-resizable panes.
- [ ] Persist only bounded presentation preferences; never put them in canonical project/repro
  authority.
- [ ] Prove resize/layout callbacks perform no solve, code expansion, history publication or
  semantic workspace save and do not strand accepted paint.
- [ ] Implement one collapsible Explorer and right Inspector/Parameters/Problems tabs.

## Phase 3 — Primary navigation

- [ ] Replace the 33-button permanent palette with the narrow primary tool rail.
- [ ] Put every manifest entry in explicit click/keyboard navigation with last-used affordances and
  prove one-for-one parity; no registered command or variant may disappear behind the redesign.
- [ ] Implement the searchable start/open surface for New, Start from code, all 37 samples,
  project/repro import and recents.
- [ ] Move reproduction/trace commands to Diagnostics and add file-first large-payload download.

## Phase 4 — First-class Code workspace

- [ ] Move Code from the narrow Design tab into the central Design/Split/Code workspace.
- [ ] Add sticky Apply/Revert/status and source tabs with at least 12 px source text and at least
  `720 x 500` CSS px of usable editor in Code mode at `1024 x 720`.
- [ ] Give Parameters and Problems one state model each, rehosted between the Design/Split right
  tabs and Code secondary tabs rather than duplicated. Keep Generated and Artifacts as separate
  secondary surfaces; collapse and search large generated inventories by default.
- [ ] Reconcile durable panel updates without replacing the active editor DOM.
- [ ] Preserve selected file, cursor, text selection, scroll and dirty draft through canvas
  selection, resize and layout switching.

## Phase 5 — Ownership and Problems cohesion

- [ ] Expose one exact Open-in-code action for every modifiable managed owner and focus its source
  span without losing canvas selection.
- [ ] Use consistent Modifiable in source / Modifiable instance / Encoded / Blocked language across
  Inspector, Parameters, Code and Problems.
- [ ] Give retained source failures one durable Problems entry with exact line/column focus while
  the prior accepted canvas remains visible.
- [ ] Keep generated, branch and artifact detail available through disclosures without making it
  primary chrome.

## Phase 6 — Browser/headless handoff

- [ ] Export canonical `project.json` and `sketch.ts` from the browser without copying solver state
  into a second format. Return a typed refusal whenever a dirty or invalid unapplied draft exists;
  provide raw draft-source download as a separate non-canonical action.
- [ ] Atomically import a canonical headless-emitted project after complete decode, expansion,
  materialization and independent validation.
- [ ] Prove browser and headless project/source/control digests agree before and after one exact-CAS
  edit.
- [ ] Keep custom patch source byte-identical and read-only; do not add TypeScript execution, AI
  chat or a stateful service.

## Phase 7 — Cleanup and qualification

- [ ] Delete superseded narrow Code-tab markup/CSS/routes only after replacement coverage passes.
- [ ] Run formatting, warnings-denied Clippy, workspace tests, relevant native owner tests, locked
  WASM build/tests, TypeScript package checks and release Trunk assembly.
- [ ] Run every pending row in `docs/M88_UAT.md` at both required desktop sizes and record exact
  browser/build identity.
- [ ] Record known limitations and obtain explicit supervising-user disposition before nomination
  or publication.

## Work and performance invariants

- Presentation-only actions must record zero parse, expand, materialize, solve, history and
  semantic-save work.
- Invalid or rejected source retains complete prior accepted scene, project, revision, generated
  identity and history authority.
- Any solver optimization receives an owning-layer regression and independent residual oracle; UI
  timing is not mathematical correctness evidence.
- Browser snapshots remain presentation/UAT evidence, never a solver oracle.
- No phase may restore LOD or hide accepted geometry as a performance substitute.

## Qualification record

No M88 implementation command has run. The clean M87 gate identifies the incoming baseline only;
it does not qualify this prospective milestone. Exact commands and outcomes must be appended here
when each phase produces evidence.
