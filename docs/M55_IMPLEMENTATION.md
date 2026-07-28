# M55 implementation report

M55 is complete as of 2026-07-29. It restores action-surface parity for the preserved
M13-M14 alpha relation, dimension and explicit branch vocabulary through the reusable
headless editor and the sole workbench. It does not restore the deleted playground,
`/#/dev/lab`, browser E2E/CDP infrastructure, mobile behavior or browser-owned solver
semantics.

## 1. Files and APIs added

`geosolve-sketch` adds:

- `ContactBranchEdit`;
- `SketchDocument::curve_contact_domains`;
- `SketchDocument::add_curve_contact_with_domain`;
- `SketchDocument::set_contact_branches`; and
- `DocumentEdit::SetContactBranches`, including command characterization and accepted-history
  handling.

`geosolve-constraint-editor` expands `ConstraintKind` to fixed, coincident, horizontal,
vertical, point-on-curve, parallel, perpendicular, equal-length, equal-radius, midpoint,
symmetry, generic contact and generic tangency. It adds:

- `DimensionKind` for point distance, segment length, radius, diameter and oriented angle;
- `ContactActionChoice`, `ConstraintActionRequest`, `DimensionActionRequest` and `ActionChoice`;
- `BranchAction` and `ContactBranchAction`;
- typed dimension and branch identities in `CoordinatorActionKind`;
- `RetainedEditorCoordinator::{apply_constraint_action,apply_dimension_action,action_choices,
  branch_actions,set_contact_branches,set_selected_angle_orientation}`; and
- replay entries carrying exact selection, typed action requests and explicit branch state.

`geosolve-demo-web` adds `workbench/action_surface.rs`, the exact reusable WASM action
identity/label catalog, complete constraint/dimension selectors, explicit contact construction
controls, selection-scoped contact/angle branch editors, semantic glyph/dimension attributes and
rendered headless disabled reasons. The scenario selector root now contains the retained
**M53 Host semantics** subtree and a new **M55 Action parity** subtree with stable leaves
`alpha-parity-catalog` and `alpha-branch-recovery`.

## 2. Mathematical behavior implemented

M55 introduces no residual equation. Every relation and dimension lowers through existing public
`geosolve-sketch` definitions and retains independent solve/domain validation.

The headless applicability matrix covers all 13 preserved alpha relation identities and all five
dimension identities in both driving and reference mode. Dimension targets are evaluated from the
current public document geometry, while the workbench supplies only typed user choices.

Complete contact branch edits are atomic over persistent contact identity, owning-curve semantic
span, parameter domain and value, winding, neighborhood and optional tangent orientation. The
owned parameter scalar keeps its persistent identity while its unit/domain change consistently
with the contact domain. Cross-curve rebinding is rejected as topology, repeated/unrelated
contacts reject, and complete document validation runs before replacement.

Accepted-parent seeding copies independently accepted solver-owned contact scalar values directly
inside the seed document. It no longer misclassifies those internal seeds as forbidden ordinary
scalar edits, which restores correct undo/recovery after retained rejected branch candidates.

## 3. Exact commands run and outcomes

Focused qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all && cargo test --locked -p geosolve-constraint-editor --all-features --test m55'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --all-features --lib'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
```

The focused M55 suite passes 7/7, the editor unit suite passes 60/60, the demo-web suite
passes 34/34, the focused sketch library suite passes 18/18 and the WASM check passes.

Final milestone qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

The complete formatting, warnings-denied Clippy, locked all-feature workspace tests,
all-feature WASM check, Trunk 0.21.14 release build and diff check pass.

## 4. Acceptance criteria passed

- Every preserved alpha constraint action is present, selection-scoped, discoverable and
  executable through a typed coordinator request.
- Point distance, segment length, radius, diameter and oriented angle execute in driving and
  reference modes.
- Wrong arity and wrong operand kind are distinct headless disabled reasons.
- Contact creation exposes explicit semantic span, parameter domain/value, winding, neighborhood
  and tangent orientation; no coordinate heuristic selects a discrete branch.
- Selection-scoped branch editing preserves persistent contact/scalar identity across accepted
  span/domain changes and retained rejected orientation/winding candidates.
- Rejected contact/branch attempts retain the prior accepted finite scene and diagnostics; Undo
  clears retained rejected intent through fresh accepted recovery.
- The workbench renders typed labels, semantic attributes, controls and reasons without owning
  applicability, equations or audit interpretation.
- The two reusable M55 scenarios preserve ordinary workspace isolation and contain no legacy
  route, harness or browser-owned expected geometry.

## 5. Known limitations and next blocker

M55 is alpha action-surface parity, not advanced-curve authoring parity or a production UI.
Scenario mode remains an ephemeral review composition; ordinary authoring actions are exercised
in the normal workspace. Mobile/responsive behavior, browser E2E and human approval are not M55
claims.

The next blocker is M56: prepared jobs must capture exact immutable input revisions, return
non-mutating candidate patches and commit only through revision-checked compare-and-swap under
safe Rust native-worker and single-threaded WASM ownership contracts.
