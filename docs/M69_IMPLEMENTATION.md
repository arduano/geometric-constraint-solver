<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M69 implementation — Profile and construction geometry semantics

Status: mechanically qualified implementation candidate, pending the nominated-source release-gate
record, release-asset publication and focused supervising-human UAT. ADR 0033 is the accepted
architecture. M69 must not be marked complete until `docs/M69_UAT.md` records explicit approval.

Candidate source: pending consolidation commit

Integrated release-gate result: **PENDING SUPERVISING RUN RESULT**

## 1. Files and APIs

M69 changes the existing ownership layers without adding a crate, residual, persistent point role
or persistence version.

- `geosolve-sketch` adds `GeometryRoleEdit`, atomic
  `SketchDocument::set_geometry_roles`, role-aware `add_curve_with_role` and
  `add_rectangle_with_role`, and the retained `DocumentEdit::SetGeometryRoles` /
  `DocumentCommandEffect::UpdatedGeometryRoles` path. Existing singular and role-unaware APIs
  remain Profile-default compatibility routes. Legacy Fillet output inherits Construction when
  either parent is Construction, while identity-retaining edits preserve the source curve role.
- `geosolve-sketch-ops` adds
  `SketchOperationSnapshot::prepare_with_geometry_role`, carries the source-free role through
  `PreparedSketchOperation` and `SketchOperationProposal`, and applies it to Rectangle, Regular
  Polygon and Slot macros. Mirror and Linear Pattern copies inherit their source role; Chamfer and
  Associative Fillet output become Construction when any native parent is Construction. Split,
  Break, Trim and Extend retain their existing curve identity and therefore its existing role.
- `geosolve-sketch-features` adds `ComputedEdge::role`,
  `ComputedConstructionFragment`, `ComputedConstructionFragmentId` and
  `ComputedConstructionFragmentProvenance`, plus separate snapshot collection and source/owner
  lookup APIs. `ComputedFeatureEvaluationPolicy::max_construction_fragments` independently bounds
  publication work. Effective source fragments inherit the native role and a generated Fillet arc
  is Construction when either parent is Construction.
- `geosolve-constraint-editor` adds `GeometryPickScope`, `GeometryVisibility`,
  `GeometryInteractionPolicy`, point-role incidence, native/computed role metadata and
  `SceneCurveOrigin::FilletDiscarded`. Policy-aware hit collection is shared by hover, selection,
  drag ownership, point snapping, ordinary constraint authoring and computed-Fillet authoring.
  Implicit-fragment hits retain their picked parameter and provenance while returning the existing
  complete `CurveSpan`. `EditorEffect::CommitConstruction { proposal, role }` is the authoritative
  role-aware drawing envelope; `ConstructionProposal::apply` remains Profile-default and
  `apply_with_role` is its atomic role-aware counterpart. The retained coordinator adds selected
  complete-curve role aggregation/toggling and replays the role together with construction intent.
- `geosolve-demo-web` adds a Construction palette action, All/Profile/Construction canvas scope,
  independent explicit-guide and Fillet-hidden visibility, role-aware Inspector content and
  Profile/Construction tree grouping. Native, retained and discarded occurrences render from
  headless scene metadata rather than CSS inference. Excluded computed arcs remain paintable when
  visible but expose no radius, branch or accessible action affordance. Workspace v4 continues to
  round-trip the existing persistent curve-role field without a schema migration.
- The ordinary **Construction and reference geometry** sample now includes a shared-corner
  Construction diagonal and exact Profile/Construction overlap. The ordinary **2D Fillet
  playground** remains the focused implicit-fragment specimen. Neither sample adds guide,
  protection or scenario-mode state.
- `docs/adr/0033-profile-and-construction-geometry-semantics.md`, `docs/M69_UAT.md`, this ledger,
  `PLAN.md`, `ACCEPTANCE.md`, `ARCHITECTURE.md` and `docs/SCENARIOS.md` record the architecture,
  qualification boundary and focused UAT.

## 2. Mathematical behavior

M69 changes no residual equation, Jacobian, solver priority, convergence status, independent
validation rule or branch cell. Persistent Construction remains ordinary lowerable and
constrainable curve geometry. Only existing visual/production-profile scope excludes it by
default. Atomic role conversion changes metadata in one validated transaction and preserves
accepted coordinates, residuals, rank, right-nullity/DOF and every explicit branch field.

For one successful open source composition with accepted base interval `[a,b]`, an optional start
claim at `s` and optional end claim at `e`, the effective source interval remains exactly

```text
[s if a start claim exists, otherwise a;
 e if an end claim exists, otherwise b].
```

The separate implicit-construction collection may contain `[a,s]` and `[e,b]`. Publication checks
that every interval and parameter is finite, contained in the exact base interval, correctly
attributed to its native span, owning Fillet corner and retained endpoint, and non-overlapping with
the effective interval. A complement no wider than the scale-aware parameter tolerance is omitted
without invalidating or changing the otherwise strictly interior Fillet claim or effective output.
This preserves pre-M69 Fillet validity while preventing zero/tolerance-empty construction ghosts.

Full-period parents do not participate in trimming and therefore remain whole. Failed,
suppressed, conflicting, stale, interrupted or invalid feature output publishes no discarded
fragment for that feature. Publication is evaluation-local, bounded by explicit policy and never
creates a persistent curve, effective edge, tree identity, constraint operand or workspace field.

Points remain role-neutral. Revision-local incidence records whether each persistent point is
referenced by Profile curves, Construction curves or both. A free point follows ordinary Profile
interaction; a construction-only point follows Construction visibility/scope; and a shared point
is available in both scopes without duplication.

In All scope, the headless hit policy first compares the nearest Profile and Construction role
classes. Profile wins a cross-role separation of at most one screen/CSS pixel; outside that band,
the nearer role wins. Within the admitted role, the established semantic point-before-curve,
distance, persistent-identity and curve-parameter ordering remains deterministic. Operand
compatibility is considered before ordinary kind priority during constraint and Fillet authoring,
so an incompatible point cannot mask a valid curve under the same click.

An implicit-fragment hit reports `SelectionItem::Curve(native_span)` plus the exact fragment
origin, source role and picked native parameter. Selection highlighting, role editing, Delete,
constraints and dimensions therefore use the complete persistent source; every retained and
discarded canvas occurrence shares the same selection context.

## 3. Commands and outcomes

Focused owner qualification recorded during implementation includes:

```text
cargo test --locked -p geosolve-sketch --test m69
cargo test --locked -p geosolve-sketch-ops --test m58
cargo test --locked -p geosolve-sketch-features --all-features
cargo test --locked -p geosolve-constraint-editor --test m69_geometry_semantics
cargo test --locked -p geosolve-demo-web --all-features
```

The focused sketch role suite passes 3/3 tests. The existing operation suite passes with added role
inheritance/source-free role assertions. The feature suite passes 41/41 tests, the M69 editor
integration suite passes 10/10 tests and the demo-web suite passes 71/71 tests.

The final tolerance-empty and complete-native command-routing repairs additionally passed:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch-features --all-features
cargo test --locked -p geosolve-constraint-editor --test m69_geometry_semantics
cargo clippy --locked -p geosolve-sketch-features -p geosolve-constraint-editor \
  --all-targets --all-features -- -D warnings
cargo test --locked -p geosolve-sketch-features \
  tolerance_empty_discarded_complements_preserve_effective_output_without_publication
cargo test --locked -p geosolve-constraint-editor --test m69_geometry_semantics \
  implicit_origin_selection_routes_dimension_and_delete_to_the_complete_native_curve -- --exact
git diff --check
```

All commands pass. The two package test totals remain 41/41 and 10/10 respectively. Focused strict
Clippy reports only Cargo's pre-existing `license` plus `license-file` manifest advisory; it emits
no Rust warning and the explicit warnings-denied check passes.

An earlier integrated WASM owner check passed:

```text
cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
```

The nominated consolidation source must record the complete clean qualification sequence below.
Its outcome is deliberately not inferred from focused package runs:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
RUSTFLAGS="-D warnings" cargo check --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown
cargo clippy --locked -p geosolve-demo-web --all-features \
  --target wasm32-unknown-unknown -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo bench --locked --workspace --all-features --no-run
cargo run --locked --release -p geosolve-sketch --example m14_performance
cargo run --locked --release -p geosolve-sketch --example m32_performance
cargo test --locked --release -p geosolve-linkage --test m23_performance \
  exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain \
  -- --exact --ignored --nocapture
cargo deny check licenses
for package in geosolve-geometry geosolve-core geosolve-sketch geosolve-linkage \
  geosolve-sketch-features geosolve-sketch-ops geosolve-sketch-topology \
  geosolve-constraint-editor; do
  cargo package --locked --allow-dirty --list -p "$package"
done
nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Integrated release-gate outcome: **PENDING SUPERVISING RUN RESULT**. Replace this marker with the
exact command results, nominated commit and any execution-environment note before freezing the
release distribution. Do not describe M69 as clean-gate-qualified while the marker remains.

Release distribution SHA-256 manifest: **PENDING RELEASE BUILD**

Tailscale byte verification: **PENDING RELEASE CANDIDATE**

## 4. Acceptance criteria

Direct evidence covers the implementation criteria as follows:

- persistent Construction remains solver-active, constrainable and excluded only from the
  existing default Profile/topology scope;
- batch conversion and role-aware authoring are atomic, Profile-default-compatible and preserve
  accepted geometry, residual/rank/DOF and branch state through Undo/Redo;
- Mirror, Pattern, Chamfer, legacy Fillet and source-free macro role propagation follow ADR 0033;
- effective computed edges carry role metadata and mixed-parent Fillet arcs cannot be promoted to
  Profile;
- successful open-parent Fillets publish exact, finite and bounded discarded complements in a
  separate collection, while tolerance-empty complements preserve effective output but publish no
  fragment;
- full-period, failed, suppressed, conflicting and interrupted cases publish no inappropriate
  construction ghost;
- scene DTOs carry persistent role, point incidence and exact implicit origin/provenance;
- an implicit hit retains its native parameter and routes selection, a compatible dimension and
  Delete to the complete native source;
- one shared headless policy controls hover, selection, drag ownership, snapping, ordinary
  constraint authoring, computed-Fillet authoring and computed branch/radius affordances;
- one-pixel Profile overlap priority, same-role semantic ordering and compatibility fallback are
  deterministic;
- the sole workbench exposes authoring/conversion, grouped tree presentation, independent
  explicit/implicit visibility and compact pick scope without a second renderer-side policy;
- workspace v4 round-trips existing persistent roles and M69 adds no sketch, feature or workspace
  persistence version; and
- the focused ordinary samples and `docs/M69_UAT.md` cover role authoring/conversion, overlap,
  shared points, implicit Fillet portions, failure withholding and closed-loop preservation.

Implementation and focused owner acceptance evidence pass. The integrated release-gate result must
replace the explicit placeholder in section 3. Release publication, byte verification and explicit
supervising-human UAT remain open and are intentionally not claimed here.

## 5. Known limitations or next blocker

The next blocker is procedural: record the nominated-source integrated release-gate result, build
and hash the release distribution, serve and byte-verify that exact distribution over Tailscale,
then obtain explicit supervising-human approval of `docs/M69_UAT.md`. M69 remains open until those
steps pass.

M69 intentionally has no persistent point role. Pick scope and visibility remain independent
session state: changing scope does not hide painted geometry or alter history, while hiding
explicit/implicit Construction removes the corresponding interaction targets. Implicit fragments
are revision-local presentation/provenance and cannot be independently deleted, constrained,
dimensioned, persisted or listed as fake tree rows; those actions resolve to the complete native
source by design.

Persistent point roles, canonical sketch v5, workspace migration, marquee/cycling/search,
Offset/Mirror UI, computed-on-computed chaining, Bake/Explode, computed-feature production-topology
consumption, new residuals, browser E2E, mobile behavior and legacy UI remain outside M69.
