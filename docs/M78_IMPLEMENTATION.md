<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 implementation — CAD geometry tool families and authoring variants

Status: **active (2026-08-17)**. This record is intentionally incomplete until implementation,
qualification, frozen-candidate review and closeout are finished.

Architecture owner: ADR 0036.

## Approved implementation boundary

- `geosolve-constraint-editor` owns the exact family/variant catalog, semantic stages, typed draft
  operands, modifiers, branch actions, recipe generation, relation provenance, candidate priority,
  correction-ready rejection and atomic retained publication.
- `geosolve-sketch` continues to own ordinary geometry, relation mathematics, explicit contact and
  branch state, solve/validation, persistent identities and document history. M78 uses existing
  constraints and generic tangency; it adds no residual equation.
- `geosolve-demo-web` owns family-menu layout, accessible labels and platform event translation. It
  renders headless stage/preview/status DTOs and never reconstructs a recipe, circumcircle, tangent
  arc, ellipse projection, inference priority or document edit.

## Planned public API and compatibility

The exact headless catalog uses non-exhaustive `GeometryToolFamily` and `GeometryToolVariant` types
with stable `key()`, `family()`, `variants()` and `default_variant()` metadata.
`ConstraintEditor::activate_geometry_tool` activates an exact recipe and
`ConstraintEditor::geometry_tool_variant` reports it. Existing `EditorTool` remains the coarse
legacy projection and existing activation remains source-compatible.

Semantic pointer input extends inference input without conflating modifiers:

```rust
pub struct DraftAuthoringInput {
    pub inference: DraftInferenceInput,
    pub regularized: bool,
}
```

Compatibility pointer wrappers set `regularized = false`. Headless draft status exposes the exact
variant, semantic stage, progress, finishability, explicit branch and typed live measurements.

Private draft state replaces parallel point/position assumptions with typed operands for stored
points, coordinate-only samples, prospective created-curve contacts and accepted curve contacts.
All recipes lower through authenticated `CommitConstructionPlan`, including geometry-only plans.
`ConstructionRelationDefinition` records `RecipeIntrinsic`, `RecipeRegularization` or
`AutoInference` provenance and supports only the additional ordinary construction forms required
by M78: EqualLength, created-curve incidence and generic tangency.

Geometry proposals add only exact recipe seams needed by the milestone: explicit four-edge
rectangle loops, midpoint-line and open/closed polyline construction, sweep-explicit circular arcs
and per-created-curve role assignment. Compatibility rectangle and counterclockwise-arc proposals
remain available but do not own new M78 interaction semantics.

## Mathematical and transactional contract

Tangent Arc derives its centre analytically from source endpoint `S`, target endpoint `E` and the
chosen source normal `n`:

```text
center = S + n * |E - S|² / (2 * dot(E - S, n))
```

It rejects a zero chord, zero/invalid endpoint jet, zero denominator (the infinite-radius tangent
line), non-finite radius and vanishing sweep. The committed ordinary generic-tangency source keeps
contact, endpoint neighbourhood, orientation and sweep explicit. Three-point circle/arc recipes use
scale-aware collinearity rejection and do not reinterpret invalid input as convergence.

Intrinsic recipe sources are applied first, then Shift regularization, then compatible ambient
inference in stage order. A newly fully or partially redundant ambient source is rejected or
dropped according to its provenance without discarding a valid intrinsic recipe. One trial session
allocates, solves and independently validates the complete plan before exact publication. Failure
retains live document/history/allocator state and the terminal draft for correction.

## Implementation evidence

Pending. Record exact files and public APIs, recipe behavior, focused owning-layer tests, reviewed
golden rows if any, native/WASM parity and thin workbench evidence as implementation lands.

## Qualification commands

The final record must distinguish commands actually run from planned targets. Milestone
qualification includes at least:

```text
cargo test --locked -p geosolve-constraint-editor \
  --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
```

Only a clean nominated source may run `./scripts/release-gate.sh` as final qualification
authority. The frozen Trunk output must be served without rebuilding over Tailscale until human UAT
is accepted.

## Closeout evidence

Pending explicit human UAT disposition, accepted-source GitHub Pages publication, hosted-byte
verification and a clean final worktree. M78 must not be described as complete before those steps.
