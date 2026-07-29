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

## Contextual-authoring follow-up completed during M61 remediation

### 1. Files and APIs added

`geosolve-constraint-editor` adds the compact public `ConstraintIntent` vocabulary,
selection-resolved `ResolvedConstraintKind`, explicit `ConstraintRelationChoice`, relation choices
on `ActionChoice`, and `RetainedEditorCoordinator::resolved_constraint`. `SceneCurve`, `Hit` and
`ConstraintEditor` retain curve-pick parameter metadata.

The workbench action surface now contains eleven stable authoring identities: Lock, Coincident,
Horizontal, Vertical, Parallel, Perpendicular, Equal, Midpoint, Symmetric, Tangent and Continuity.
The old Point-on-curve, Equal-length, Equal-radius, Generic-contact and Generic-tangency workbench
identities are removed rather than retained as aliases.

### 2. Mathematical behavior implemented

No residual equation was added or changed. Typed selected operands resolve Coincident to point
coincidence, point-on-curve or curve contact; Equal to equal length, radius or curvature; Parallel
to a line-pair relation; Perpendicular / Normal to either a line-pair relation or circular-centre
incidence on a selected line; Tangent to generic curve-pair tangency; and Continuity to endpoint
G0/G1/G2 or rate-explicit parametric C2.

Contact span/domain/parameter/winding/neighborhood/orientation, tangent orientation, curvature
sign/magnitude branch, continuity order and positive C2 rates remain explicit choices. Curve hit
testing retains the selected parameter for contact seeding, and endpoint Start/End selection maps
to exact bounded endpoint parameters. A circular normal has no arbitrary side choice: the existing
point-on-curve definition constrains the circle or arc centre onto the selected line.

### 3. Exact commands run and outcomes

Focused qualification:

```bash
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features --lib'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features --test m55'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings'
```

The editor unit suite passes 63/63, focused M55 integration passes 10/10, demo-web passes
46/46, the demo-web WASM check passes and focused warnings-denied Clippy passes.

Final qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

Formatting, warnings-denied workspace Clippy, the complete locked all-feature workspace suite,
all-feature demo-web WASM check and Trunk 0.21.14 release build pass. The final documentation commit
also passes formatting and diff checks.

`M61-F005` requalification after replacing direction-only circle authoring:

```bash
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features --test m55'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell ../../shell.nix --run 'env -u NO_COLOR trunk build --release'
git diff --check
```

The focused M55 integration suite passes 13/13, demo-web passes 49/49, focused and workspace
warnings-denied Clippy pass, the complete locked all-feature workspace suite passes, the
all-feature WASM check passes, Trunk 0.21.14 produces the release distribution, and the diff check
is clean.

### 4. Acceptance criteria passed

- One compact intent vocabulary preserves the complete relevant M55 mathematical surface.
- The headless boundary owns contextual resolution, underlying-definition metadata, branch-choice
  progression and typed disabled reasons.
- The browser owns labels and controls only; former equation-shaped action identities are absent.
- Curve contact, true curve tangency, circular radial normal incidence, equal curvature and
  endpoint continuity execute through public retained sketch edits with accepted/rejected
  lifecycle coverage.
- Reusable UAT scenarios demonstrate the new relations without restoring `/#/dev/lab`, browser
  E2E or a legacy harness.

### 5. Known limitations and next blocker

The public domain-level `CurveDirection` definition remains available for explicit-contact
consumers, but compact Parallel/Perpendicular authoring intentionally does not expose
direction-only line/curve relations. Concentric, Collinear, point-pair Horizontal/Vertical,
Point/Entity Symmetry, EqualDistance, EqualAngle and BlockEntity remain in the separate M36/M37
semantic catalog until M62 freezes their retained lifecycle and schema.

The active product gate remains the remediated M61 human advanced geometry/topology UAT.
