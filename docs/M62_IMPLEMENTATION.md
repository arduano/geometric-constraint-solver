<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M62 CAD-style constraint and dimension authoring

Status: active. This report records objective implementation evidence; human approval belongs only
in `docs/M62_UAT.md`.

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

## Qualification record

Pending implementation.
