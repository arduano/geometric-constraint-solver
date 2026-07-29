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
- Completed operand sets clear after every terminal application attempt and the tool remains
  active; pre-application warnings retain the valid pending prefix.
- Escape clears pending operands before exiting the mode.
- Scenario mode remains read-only and ordinary persistence remains unchanged.

## 1. Files and public APIs

- `geosolve-constraint-editor::authoring` adds `AuthoringTool`, `AuthoringOperand`,
  `AuthoringOperandKind`, `AuthoringOptions`, `AuthoringState`, `AuthoringOutcome`,
  `AuthoringWarning` and `AuthoringApplication`.
- `RetainedEditorCoordinator` adds explicit-selection constraint/dimension application,
  `apply_authoring`, selected-dimension target metadata and retained target editing. Follow-up
  `M62-F001` adds `DimensionTargetDisplayUnit`, `DisplayDimensionTarget`,
  `display_dimension_target` and branch-preserving `set_dimension_display_target`.
- The workbench index and styles replace the narrow geometry strip with a wider two-column palette.
  `workbench/mod.rs` routes palette, tree and canvas events; `scene.rs` and `panels.rs` render
  authoring-pending identities separately from selection. Follow-up `M62-F002` assigns canvas
  authoring exclusively to the parameter-bearing pointer-down path, leaves tree authoring on its
  single click path and removes completed operands after every terminal coordinator attempt.
- The right inspector retains post-creation branch editing and adds a selected-dimension target
  editor. Its constraint/dimension creation forms are deleted.

## 2. Mathematical behavior

No residual, equation or solver behavior changed. The headless state resolves
the existing eleven M55 contextual intents and five dimension definitions against explicit
persistent operands. Curve picks retain their public semantic parameter. Tangency orientation,
curvature sign policy, continuity order/rates, dimension mode and angle direction remain explicit.
Every mutation still passes through retained sketch transactions and independent validation.
Contact branch choices are generated only for definitions that own contact state. Simple
Horizontal, Vertical, Parallel, Perpendicular, Equal Length and Equal Radius authoring therefore
emits only its exact persistent definition rather than incompatible latent contact metadata.

Dimensions are created at their current independently accepted measurement rather than retained
design coordinates that may differ from the visible accepted canvas. Numeric editing locates the
dimension-owned scalar and emits `DocumentEdit::SetScalarValue`; Undo/Redo therefore uses ordinary
retained history.

`M62-F001` keeps oriented-angle storage and residual evaluation in explicit directed radians.
Headless metadata projects that value to the acute angle between the supporting lines in degrees,
which is independent of hidden endpoint direction and selects the acute intersection rays when
the visual origin is otherwise ambiguous. An edit maps acute degrees back through the existing
directed quadrant and complete-turn branch. Creation therefore preserves accepted geometry,
including when retained design seeds diverge, while editing remains branch-explicit.

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
- `M62-F001` directly covers accepted/design coordinate divergence, no-move angle creation,
  reversed line endpoints, all four directed quadrants, a branch-preserving 45-to-60 degree edit,
  invalid values above 90 degrees and acute-degree canvas annotation.
- `M62-F002` directly feeds the real canvas pointer-down plus bubbled-click sequence to Horizontal
  and Normal/Perpendicular collection. Each physical click contributes exactly one operand,
  single-item repetition re-arms, pair collection applies after two distinct lines, and tree
  clicks remain independently owned.
- `M62-F003` applies authored Horizontal and line-line Perpendicular to skew free lines through the
  complete state-machine/coordinator path, requires accepted publication and inspects the exact
  persistent definitions. The exhaustive resolved-kind dispatch keeps every other simple
  line/radius relation contact-free.
- The static workbench contract directly proves all sixteen palette identities exist and the old
  creation controls, action aliases and deleted `/#/dev/lab` route do not.
- Canvas/tree pending presentation, read-only scenario behavior, WASM compilation and the release
  bundle pass without adding a scenario or persistence field.

## 5. Known limitations and next gate

- Creation-time option flyouts intentionally expose only the approved M62 choices. Contact
  domain/neighborhood/winding details use deterministic headless defaults; their existing
  post-creation branch editor remains available.
- Authoring options are process-memory UI state and deliberately do not persist.
- Acute-angle presentation is currently defined for the existing line-line oriented-angle
  dimension. The persisted dimension remains directed and branch-explicit; no schema migration is
  required.
- Responsive/mobile behavior and browser E2E remain out of scope.
- The only remaining M62 blocker is explicit supervising-human approval of
  `docs/M62_UAT.md` in the ordinary workspace.
