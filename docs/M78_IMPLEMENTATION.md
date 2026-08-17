<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M78 implementation — CAD geometry tool families and authoring variants

Status: **active (opened 2026-08-17)**. Hardened product implementation is committed through
`4845df7`; focused post-hardening owner suites pass, while clean release nomination, frozen-
candidate review, human UAT, publication and closeout remain open.

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

## Implemented files, public API and compatibility

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
Non-exhaustive `GeometryDraftIssue` and `GeometryDraftStatus::issue` expose recoverable invalid
terminal geometry, incompatible snap intent, premature Finish and retained-plan rejection without
adding anything to the accepted document's Problems state.

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

The implementation is concentrated in:

- `crates/geosolve-constraint-editor/src/{geometry_tools,inference,commit_plan,coordinator,lib}.rs`
  for catalog, semantic input/status, exact recipes and atomic retained publication;
- `crates/geosolve-sketch/src/{document,semantic,m38}.rs` and
  `tests/m78_endpoint_contacts.rs` for whole-curve semantic endpoint resolution used by Tangent
  Arc, including first/last multi-span ownership and closed/periodic rejection;
- `crates/geosolve-sketch/src/{compiler,conics,model,residuals}.rs` and
  `tests/m78_extreme_finite.rs` for overflow-safe existing segment/conic validation and
  midpoint/symmetry residual evaluation without adding a residual equation;
- `crates/geosolve-demo-web/src/workbench/{geometry_palette,effect_adapter,icons,mod,scene}.rs`
  for thin family overlays, event translation, local issue copy and published preview rendering;
  and
- `crates/geosolve-constraint-editor/tests/{m78_geometry_variants,m78_extreme_finite}.rs` plus
  existing sketch, coordinator and web suites for exact owning-layer regression evidence.

## Mathematical and transactional contract

Tangent Arc derives its centre analytically from source endpoint `S`, target endpoint `E` and the
chosen source normal `n`:

```text
center = S + n * |E - S|² / (2 * dot(E - S, n))
```

The implementation evaluates the equivalent offset as
`0.5 * chord_length * (chord_length / normal_chord)` in a translated or absolute-normalized frame,
then validates both requested endpoints against the rounded centre/radius. It rejects a zero chord,
zero/invalid endpoint jet, zero denominator (the infinite-radius tangent line), non-finite radius,
failed endpoint incidence and vanishing sweep. The committed ordinary generic-tangency source keeps
contact, endpoint neighbourhood, orientation and sweep explicit. Three-point circle/arc recipes use
a normalized chord frame, scale-aware collinearity rejection and local point-to-centre incidence;
they do not reinterpret invalid input as convergence.

Intrinsic recipe sources are applied first, then Shift regularization, then compatible ambient
inference in stage order. A redundant/conflicting ambient source is shadowed according to its typed
provenance and subject, while compatible ambient orientation remains. Controlled execution charges
validation and proposal-specific lowering before candidate allocation. One trial session allocates,
solves and independently validates the complete plan before exact publication, and a positive
coordinator acknowledgement consumes the draft only after that exact publication is recorded.
Failure retains live document/history/allocator state and the terminal draft for correction.

After an intervening retained edit, semantic stages reauthenticate stored points, prospective
contacts, remembered point/midpoint/curve references and Tangent Arc endpoint jets from the next
authenticated scene. Scale-safe midpoint/reflection/circle projection and `hypot`-style segment/
conic norms avoid nonrepresentable intermediate sums or squares. Existing midpoint and symmetry
residual equations use an overflow-safe midpoint evaluation with unchanged analytical Jacobians.
Only finite representable live measurements are published.

## Implementation evidence

The nine-family/25-variant catalog, all semantic stage tables, independent Ctrl/Cmd and Shift
intent, Tab cycling, branch flip, step-back/Finish/Escape lifecycle, exact line/rectangle/circle/
arc/ellipse recipes and grouped advanced families are implemented. Every exact recipe emits one
`CommitConstructionPlan`; geometry-only plans no longer bypass that authority. Rectangle intrinsic
relations and Shift EqualLength are ordinary durable relations, and created-curve incidence is
authenticated against the actual circle/arc/ellipse support before allocation.

Tangent Arc uses semantic native open-curve endpoints, exact accepted endpoint jets and the
existing generic curve-tangency definition. Focused coverage freezes Start/End orientation,
multi-span nonlinear B-splines, trim visibility, shared-junction ambiguity, created contact order,
stale compare-and-swap, one-step history, Undo/Redo and checkpoint reload. The canonical 3-Point
Arc gesture is Start, End, then Through so endpoint identity exists before branch selection.

Draft failures are now explicitly local. Invalid geometry, incompatible created-curve snapping,
premature variable-length Finish and atomic rejection publish typed correction guidance. Valid
preview/correction, step-back, applicable option or sweep change, Escape/tool switch, success and
retained-state invalidation clear it. The WASM adapter deliberately does not copy raw plan errors
into the persistent global notice.

### M78-F001 — representable Tangent Arc failed at extreme scale

Owner: `geosolve-constraint-editor` analytic Tangent Arc construction. Independent reproduction at
representable extreme scales showed that `chord_length² / (2 n·D)` could overflow or underflow even
when the resulting circle centre and radius were finite. The repair evaluates the equivalent
`0.5 * chord_length * (chord_length / normal_chord)` form. The exact internal extreme-scale test
and public `1e-6`, `1`, `1e6` metamorphic construction test now pass without changing a solver
residual or accepting invalid geometry.

### M78-F002 — recipe intent and ambient inference lacked durable precedence

Owner: `geosolve-constraint-editor` construction-plan lowering. Relations previously had no typed
source and therefore could depend on declaration order; an ambient horizontal/vertical or relative-
direction suggestion could duplicate or conflict with the selected rectangle/line recipe. Each
`ConstructionRelationDefinition` now carries `RecipeIntrinsic`, `RecipeRegularization` or
`AutoInference`, lowering is stable in that precedence order, and a recipe source shadows only the
ambient source that targets the same absolute span or unordered relative span pair. Exact per-created-
curve roles travel in the same plan. Direct tests prove conflicting ambient direction yields to the
recipe while a compatible oriented-rectangle baseline direction survives and publishes.

### M78-F003 — a positive host acknowledgement could consume an unpublished draft

Owner: retained editor/coordinator publication handshake. A token match and `accepted = true` were
not sufficient evidence that the exact expected construction plan had won retained publication.
The coordinator now marks the matching pending `(prepared input, plan)` only inside successful
staged publication; coordinator acknowledgement requires that mark. An unbacked positive
acknowledgement behaves as a local rejection and preserves correction state, while both ordinary and
controlled successful publication provide the mark and clear the terminal preview exactly once.

### M78-F004 — proposal lowering could exceed controlled work before mutation

Owner: `ConstructionCommitPlan::apply_in_controller` and the retained coordinator's controlled plan
path. The old path checkpointed document validation but did not charge proposal-specific allocation/
lowering work before cloning and applying a potentially large variable-topology proposal. Plans now
charge one validation item and a deterministic bounded `DocumentLoweringItems` amount derived from
the proposal family and operand/topology counts before candidate mutation. Zero validation or
lowering budget stops with the corresponding typed checkpoint and leaves document, accepted input,
history, transcript and both allocator high-water marks exact.

### M78-F005 — stale semantic stages reused cached coordinates and Tangent Arc jets

Owner: headless draft reauthentication. After an intervening retained edit rejected a pending plan,
the preserved correction-ready prefix could still contain old stored-point positions, contact
references or Tangent Arc source derivatives. Typed draft stages now enter `RefreshRequired` and
reauthenticate persistent points, prospective created-curve contacts, point/midpoint/curve references
and native endpoint contact/jet state from the next exact scene. A moved Tangent Arc source regenerates
centre, sweep, contact parameters/neighbourhoods and orientation; a deleted dependency produces no
preview/commit and remains a local issue recoverable through step-back or Escape.

### M78-F006 — representable midpoint, segment, conic and derived geometry overflowed intermediates

Owners: `geosolve-sketch` existing validation/residual evaluation and
`geosolve-constraint-editor` recipe derivation. Like-signed endpoint sums, squared Euclidean norms,
`2 * center` reflection and radius/sample-radius scaling could overflow even when the mathematical
result was finite. Midpoint/symmetry residuals now use a sign-aware midpoint form with the unchanged
analytic Jacobian; segment and conic axis validation use `hypot`; midpoint/reflection and circular
projection helpers avoid the nonrepresentable intermediate. Diameter-circle, Midpoint Line,
axis-endpoint ellipse/elliptical arc, centre rectangles and Center Arc extreme-finite regressions all
solve and independently validate. The midpoint residual also has direct finite-difference Jacobian
coverage at `1e-6`, `1` and `1e6` plus a public extreme-finite solve.

### M78-F007 — translated and diagonal circumcircles could miscompute or falsely validate incidence

Owner: `geosolve-constraint-editor` three-point circle/arc derivation. A translated chord frame loses
the construction when finite diagonal chord components have an overflowing `hypot`, while validating
normalized absolute world coordinates can let a large translation hide a rounded centre that misses
the samples. Circumcircles now prefer translated normalized chords and fall back to an absolute-
normalized frame when subtraction or chord length is nonrepresentable. The rounded result is accepted
only when local point-to-centre distances agree with the radius. Opposite-extrema, diagonal-extrema,
large-translation false-incidence and public circle/arc solve regressions pass; an unrepresentable
derived circle remains a local `InvalidTerminalGeometry` issue.

### M78-F008 — Tangent Arc validated construction algebra but not both rounded endpoints

Owner: `geosolve-constraint-editor` Tangent Arc derivation. A finite algebraic centre/sweep could be
rounded enough that the created circle no longer passed through the requested source or target. The
recipe now applies the same local point-to-centre radius validation to both endpoints before emitting
a plan. A moderately translated source retains the requested endpoint to the focused tolerance and
publishes; nonrepresentable endpoint incidence fails closed as a local draft issue. This does not
relax finite-jet, tangent-line-limit or sweep validation.

### M78-F009 — status could publish a non-finite derived measurement

Owner: headless `GeometryDraftStatus`. A representable finite circle radius near `f64::MAX` could
still emit `Diameter(2 * radius)` as infinity. Measurement assembly now publishes a derived diameter
only when it is representable and keeps all emitted length/radius/diameter/angle/ratio/width/height
values finite. The large-radius regression retains its truthful radius and omits only the impossible
diameter; the construction itself still solves and independently validates.

### M78-F010 — typed provenance risked changing legacy auto-contact labels

Owner: construction relation/contact lowering compatibility. Provenance-specific relation labels are
new audit information, but existing consumers and persisted/reproduction bytes already observe
ambient contact occurrence labels. Auto-inferred point-on-curve contacts therefore retain the exact
`auto point-on-curve contact N` spelling and numbering, while recipe-owned contact labels receive
their typed provenance prefix. The compatibility lowering therefore retains the legacy bytes.

## Qualification commands and outcomes

The following commands ran successfully after product commit `4845df7`:

```text
cargo test --locked -p geosolve-constraint-editor --lib
  # 362 passed
cargo test --locked -p geosolve-constraint-editor --test m78_geometry_variants
  # 32 passed
cargo test --locked -p geosolve-constraint-editor --test m78_extreme_finite
  # 7 passed
cargo test --locked -p geosolve-sketch --test m78_extreme_finite
  # 1 passed
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
  # passed with warnings denied
cargo test --locked -p geosolve-constraint-editor \
  --test golden_authoring_oracle golden_oracle_inventory_and_tsv_schema_are_exhaustive -- --exact
  # 1 passed
./scripts/golden-authoring-scene-oracle.sh --survey
  # 270 PASS; 0 DEFECT/PANIC/TIMEOUT/HARNESS_ERROR
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
  # both match the recorded checklist
```

The 271-line golden file (header plus 270 cases) remained unmodified at SHA-256
`7a4afd4fbd70d0ef6454e5f07f00fde7afb64eec59d329acfba7f761d986e343`; no systemic matrix
expansion was warranted.

Before the final numeric hardening in `4845df7`, complete editor and sketch all-feature suites and
the demo-web library suite (143/143) also passed. Those earlier broad runs are supporting evidence
only: they do not qualify the final implementation source and must be repeated by the clean
nomination/release workflow.

The final clean nomination still requires the complete workspace/release sequence, including:

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
./scripts/release-gate.sh
```

Only a clean nominated source may run `./scripts/release-gate.sh` as final qualification
authority. The frozen Trunk output must be served without rebuilding over Tailscale until human UAT
is accepted.

## Closeout evidence

Focused A1-A8 and M78-F001 through M78-F010 owner regressions pass with the exact post-hardening
counts above. Known scope limits remain the explicit deferrals in `docs/M78_GOALS.md`; there is no
interior/periodic Tangent Arc or multi-tangent circle workflow. The unchanged golden survey/check/
require-clean sequence passes; the complete clean workspace/release gate, exact no-rebuild frozen
artifact and Tailscale byte verification, explicit human UAT disposition, accepted-source
publication, hosted-byte verification and clean final worktree remain pending. M78 must not be
described as complete before those steps.
