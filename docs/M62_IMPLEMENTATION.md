<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M62 CAD-style constraint and dimension authoring

Status: mechanically complete as of 2026-07-29. Human approval belongs only in
`docs/M62_UAT.md`, so M62 remains open.

## Scope

M62 moves constraint and dimension creation from inspector forms into the workbench's left
authoring palette. A reusable selection-independent state machine owns operand collection,
contextual resolution, expected-next guidance and typed warnings. The host supplies immutable
selection snapshots or individual picks and remains the owner of application selection.

The retained coordinator accepts explicit operands and applies the same M55 contextual definitions
and five dimension definitions. No equation, residual, schema, scenario or persistence format is
added. The workbench remembers branch options only in memory and uses ordinary retained edits,
history and independent validation for every mutation.

## Interaction contract

- Compatible preselection applies once, preserves selection and remains in Select.
- Incompatible preselection warns without mutation.
- Empty preselection enters a persistent tool mode.
- Completed operand sets clear after accepted or retained-rejected mutation and the tool remains
  active; input errors retain operands.
- Escape clears pending operands before exiting the mode.
- Scenario mode remains read-only and ordinary persistence remains unchanged.

## 1. Files and public APIs

- `geosolve-constraint-editor::authoring` adds `AuthoringTool`, `AuthoringOperand`,
  `AuthoringOperandKind`, `AuthoringOptions`, `AuthoringState`, `AuthoringOutcome`,
  `AuthoringWarning` and `AuthoringApplication`.
- `RetainedEditorCoordinator` adds explicit-selection constraint/dimension application,
  `apply_authoring`, selected-dimension target metadata and retained target editing.
- The workbench index and styles replace the narrow geometry strip with a wider two-column palette.
  `workbench/mod.rs` routes palette, tree and canvas events; `scene.rs` and `panels.rs` render
  authoring-pending identities separately from selection.
- The right inspector retains post-creation branch editing and adds a selected-dimension target
  editor. Its constraint/dimension creation forms are deleted.

## 2. Mathematical behavior

No residual, equation, branch heuristic or solver behavior changed. The headless state resolves
the existing eleven M55 contextual intents and five dimension definitions against explicit
persistent operands. Curve picks retain their public semantic parameter. Tangency orientation,
curvature sign policy, continuity order/rates, dimension mode and angle direction remain explicit.
Every mutation still passes through retained sketch transactions and independent validation.

Dimensions are created at their current accepted measurement. Numeric editing locates the
dimension-owned scalar and emits `DocumentEdit::SetScalarValue`; Undo/Redo therefore uses ordinary
retained history.

## 3. Commands run

All commands completed successfully:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

Focused development gates also passed for the editor and demo-web crates, including the
all-feature WASM target. The workspace test gate retained only its pre-existing explicitly ignored
manual performance/cancellation measurements.

## 4. Acceptance passed

- Compatible preselection applies once without entering a mode; incompatible preselection returns
  a typed warning without mutation.
- Empty preselection enters repeated authoring. One-, two- and three-operand collection, role
  normalization, ordered operands, transaction completion, input-error retention, stale
  reconciliation and two-stage Escape have direct tests.
- Explicit coordinator operands do not read or clear application selection. Dimension target edits
  and Undo have direct retained-history coverage.
- The static workbench contract directly proves all sixteen palette identities exist and the old
  creation controls, action aliases and deleted `/#/dev/lab` route do not.
- Canvas/tree pending presentation, read-only scenario behavior, WASM compilation and the release
  bundle pass without adding a scenario or persistence field.

## 5. Known limitations and next gate

- Creation-time option flyouts intentionally expose only the approved M62 choices. Contact
  domain/neighborhood/winding details use deterministic headless defaults; their existing
  post-creation branch editor remains available.
- Authoring options are process-memory UI state and deliberately do not persist.
- Responsive/mobile behavior and browser E2E remain out of scope.
- The only remaining M62 blocker is explicit supervising-human approval of
  `docs/M62_UAT.md` in the ordinary workspace.
