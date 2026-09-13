<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M95 — connected code, Explorer and canvas selection

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

Status: **accepted and closed on 2026-09-07**. [M95_CLOSURE.md](M95_CLOSURE.md) records the
user's UAT acceptance. The separate frozen product is at the archived preview; [M95_QUALIFICATION.md](M95_QUALIFICATION.md) records
its exact product and evidence. M94 remains accepted at port 18096.

## Outcome

One native selection connects canvas objects, Explorer rows and authenticated source statements.
Canvas/Explorer selection highlights every relevant code section and scrolls the primary section
into view when source is visible, without taking focus or moving the code cursor. Ordinary
selection preserves the current workspace layout. Code-to-canvas navigation uses an explicit
**Show in canvas** action; ordinary cursor movement never selects geometry. **Show in code** and
**Show in canvas** open Split if their destination is hidden. Camera Fit remains separate.

## Ownership and implementation

- Add a public headless navigation-selection API using accepted owned outputs, separate from
  the existing singular declaration/mutation selection API. Preserve unique Inspector authority;
  multi-owner navigation cannot invent an arbitrary mutation target.
- Resolve declarations, invocation/group descendants and exact generated members through native
  ownership and authenticated source/expansion provenance. Select owned outputs, never borrowed
  input references. Hidden/suppressed/empty rows remain browsable without changing visibility.
- Cache the navigation index by accepted authority. The bridge publishes selected/partial row
  states, multiple source ranges and an opaque project/source/scene authority token. Explicit
  requests validate that token and exact UTF-8 boundaries; persisted formats are unchanged.
- Plain Explorer clicks replace; Shift/Ctrl/Meta toggle the selected target set consistently
  with native selection. Source requests resolve the statement under the cursor or all statements
  intersecting selected text. Unmatched/import/comment-only text retains selection with a notice.
- CodeMirror uses source decorations separate from text selection. Reveal runs only for changed
  selection. Dirty or retained-invalid displayed text clears decorations and disables source
  navigation until accepted bytes match; accepted canvas/Explorer browsing stays available.
- Reconcile or clear after Apply, Undo/Redo, deletion and replacement. Captured gestures block
  explicit navigation. UI-only navigation performs no compilation, solve, checkpoint encoding,
  durable history publication or automatic project save; camera/drag-preview hot paths stay narrow.

## Acceptance

- Exact canvas↔Explorer↔source selection for ordinary geometry, constraints, dimensions, computed
  Fillets, groups, generated members and multi-owner selections; ordinary projects keep truthful
  canvas/Explorer navigation without fabricated source links.
- Native and bridge regressions establish accepted ownership, finite geometry, exact source/history
  preservation, no producer/consumer confusion, stale-request rejection, dirty-source retention,
  Unicode boundaries, hidden/suppressed output behavior and lifecycle reconciliation.
- Frontend/browser checks establish stable focus/cursor/layout, one-time automatic reveal,
  explicit reverse navigation, actual selected canvas pixels, correct Explorer states and unchanged
  saved bytes. Repeat navigation in the dense robotic harness and compare hot-path costs to M94.
- Use the defect-hardening workflow for reproduced native gaps and focused owner tests during
  development. Nominate one clean candidate with integrated format/Clippy/native/WASM/golden/
  browser/build checks and authenticated unaffected reuse under RELEASE_QUALIFICATION.md.
- Preserve all 271 golden rows and existing browser obligations; freeze and byte-verify a separate
  preview candidate, retaining M94. maintainer acceptance closes M95 after delivery.

## Boundaries

No solver mathematics, source mutation semantics, language/primitive additions or desktop layout
redesign. Source navigation uses authenticated statement ownership, not arbitrary TypeScript
reference/dependency tracing or nearby-text guesses. Minor UI polish serves discoverable navigation.

Implementation and executed evidence are tracked in [M95_IMPLEMENTATION.md](M95_IMPLEMENTATION.md).
