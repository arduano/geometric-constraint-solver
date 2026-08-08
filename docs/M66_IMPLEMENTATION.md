<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 implementation: computed 2D Fillet features

Status: complete. On 2026-08-08, the supervising human explicitly approved and closed M66 for its
mechanically qualified computed-Fillet scope, accepting `M66-KL001` as a deferred interaction
limitation. This does not claim a complete post-PF004 replay of every scripted UAT step.

The qualified but unapproved solver-owned ordinary-UI endpoint, commit `1034afc`, is preserved at
`origin/archive/m66-associative-fillet-2026-08-07`. The earlier three-tool experiment remains at
`origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`). Neither archive is active
qualification evidence.

## 1. Files and public APIs implemented

### Feature domain

- Added `crates/geosolve-sketch-features` as a pure safe-Rust crate depending among workspace crates
  only on `geosolve-sketch` and `geosolve-geometry`.
- Added a separately versioned `ComputedFeatureDocument` with stable document, feature and corner
  identities; allocation high-water; label/suppression state; and a closed
  `ComputedFeatureDefinition::FilletSet`.
- Feature persistence stores only intent. A FilletSet stores one shared positive radius and
  explicit source spans, picked parameters, neighborhoods/winding, normal sides, retained
  endpoints, output endpoint order and sweep. It never stores generated arcs, trimmed fragments
  or output IDs.
- Added exact-stamped `ComputedFeatureSnapshot` output with evaluation-local `ComputedEdgeId`s,
  stable feature/corner/source-interval provenance, typed issues and variable output cardinality.
- Version-one feature inputs are limited to native constrained sketch spans. Computed-on-computed
  references are not part of this cut.

### Editor and coordinator

- Replaced ordinary Fillet use of the superseded fixed two-pick operation collector with reusable
  `FeatureAuthoringState` grouped authoring. Interior polyline points remain corner targets;
  repeated corner or curve-pair picks accumulate a batch.
- Close-off removes the unreleased ADR 0030 editor `OperationAuthoring*` facade, coordinator
  operation-preview/replay DTOs/state and the editor's now-exclusive direct
  `geosolve-sketch-ops` dependency. Current feature authoring reads its sketch document through the
  computed-feature snapshot. M27/M28/M58 operation/domain APIs and ADR 0031 feature authoring are
  unchanged.
- Shared-radius preview starts from remembered state or `0.1 * model_scale`. Numeric editing and a
  preview arc/radius grip edit that same value. Apply/Enter persists one FilletSet without a final
  canvas radius-confirmation click.
- `RetainedEditorCoordinator` now owns the sketch session, feature document and current computed
  snapshot. Exact compare-and-swap includes complete sketch input/accepted identity, feature
  revision/digest and evaluator policy.
- Coordinator-owned pick and option transactions clone the headless authoring state and commit it
  only with a freshly `Current` whole-feature preview. Rejected local/global evaluation retains the
  previous state and exact preview. Refresh and final Apply defensively enforce the same current-
  evaluation invariant.
- `RetainedEditorCoordinator::transact_feature_authoring_pointer_down(...)` arbitrates a painted
  current-preview radius grip before ordinary native support collection. Painted identity is only
  an intent hint: admission requires the exact held candidate, current accepted/computed scene
  provenance and an independent headless hit on the named generated curve. Foreign/stale owners
  and a second radius press reject before mutation; the original gesture remains valid. The
  explicit grip path keeps its owner selected despite Shift/Control/Command without changing
  ordinary modifier-selection behavior.
- Native screen picks use a fixed 256-candidate limit before allocation/sorting and one bounded
  corner-incidence index. Incomplete endpoints and duplicate pending supports may fall through;
  true high-valence ambiguity never guesses an underlying curve.
- Native curves and computed source fragments receive eight seed subdivisions per non-linear span
  before the existing bounded chord-error refinement. Generated Fillet arcs receive at least eight
  bounded angular segments, advanced drafting previews use 64 subdivisions per semantic span, and
  the workbench applies one 0.25 px chord-error policy to native and computed scenes. Straight
  spans retain their two-point representation.
- Generated-arc selection resolves stable set/corner provenance. Arc/grip drag changes only feature
  radius; arc deletion removes its corner and final-corner deletion removes its set. Set suppression
  is separate from sketch source activation.
- `RetainedSketchDocumentSession::accepted_prepared_input()` exposes the exact accepted source
  stamp, and the feature domain resolves a complete corner batch atomically through
  `resolve_fillet_corners(...)`.
- `RetainedEditorCoordinator::persistence_checkpoint()` captures current durable sketch/feature
  state plus live sketch, feature/corner and computed-evaluation allocator high-water. Historical
  `checkpoint()` remains the frozen Undo/Redo representation.

### Workbench and persistence

- Added a **Features** tree section, computed geometry rendering/hit metadata and feature/corner/
  source-attributed Problems/canvas markers.
- Native source points/spans remain selectable and draggable. Computed arcs are never offered as
  sketch constraint operands.
- Advanced the application workspace envelope from v3 to v4. It stores the separately versioned
  feature document next to the unchanged canonical-v4/draft-v5 sketch payload. Workspace v1-v3
  migration creates an empty feature document bound to the restored sketch.
- Restore/Undo/Redo preserve feature IDs, intent and allocator state, then regenerate fresh output
  IDs.
- `WorkspaceSnapshot::from_coordinator()` is the sole live save/sample capture path. When active
  computed output would make base-only profiles/fills misleading, the workbench withholds them
  with a typed “computed geometry not yet included” status.
- `M66-PF003` keeps the stable `fillet-workshop` key and presents an ordinary editable **2D Fillet
  playground** under **Samples → Curves & constructions**. Fixed line-line, line-circle,
  line-quadratic-Bezier and high-valence specimens sit beside unlocked batch/sequential and
  short-middle conflict polylines. Native screen/coordinator regressions exercise the fixture; it
  adds no guide, protected authoring state or alternate coordinator.
- The SVG and its descendants opt out of native text selection and element dragging; scoped
  `selectstart`/`dragstart` guards prevent those browser defaults. The sibling Fillet options and
  other HTML retain normal selection/input behavior. This is directly tested adapter policy, not
  browser E2E evidence.
- For active Fillet pointer-down, the web adapter resolves the closest stable
  `[data-editor-item]` owner from the painted SVG target and forwards it to the coordinator. It no
  longer attempts a native authoring pick before falling back to radius interaction, so a preview
  arc overlapping its parent cannot consume that parent and strand the grouped candidate.

The normal UI removes Driving/Reference radius choice and does not auto-create a radius scalar,
dimension, constraint, M28 association or `DocumentCurveTrimView`. M27/M28 public Fillet types and
`SketchOperationRequest::AssociativeFillet` remain available for advanced/backward-compatible
callers. Existing documents are not migrated.

## 2. Mathematical behavior implemented

All corners in one set evaluate from the same immutable independently accepted sketch snapshot.
Evaluation uses deterministic bounded construction and independently validates:

- finite source and output geometry;
- finite positive shared radius;
- contact domains, winding and explicit neighborhoods;
- normal sides and retained source endpoints;
- tangency and offset regularity/singularity;
- endpoint order and output sweep; and
- explicit local branch state.

M66 supports affine/affine and affine/non-affine corners. Two non-affine parents produce a typed
unsupported feature failure; this does not narrow M28's generic solver-owned API.

Evaluation composes endpoint claims without mutating sketch trim views. Opposite ends of a shared
span may belong to different sets. Duplicate claims, crossed claims or intervals that consume a
span fail every participating set. One invalid corner withholds its complete set; unrelated valid
sets remain publishable. A failed set exposes issues and no stale output.

A valid sketch edit remains acceptable even if it invalidates a feature. Source motion may recover
the same intent. Source deletion leaves a repairable missing-source failure, and Undo recovers the
same feature/corner identities. Changing only feature radius must leave canonical sketch identity,
accepted coordinates, residual evidence, numerical rank and reported DOF unchanged.

Generated IDs are meaningful only inside one exact computed snapshot. Stable provenance, not an
output ID, is the cross-revision identity seam. The output container accepts zero/one/many fragments
so a later Offset evaluator can represent self-intersection cuts and other topology changes without
redesigning persistence. M66 implements no Offset.

## 3. Exact commands and outcomes

Presentation-smoothed source `a34d137` passed the previously recorded full gate and editable-
playground source `02649cc` added the focused UAT fixture. Candidate source `ac31791` repeated the
following commands successfully on one implementation state:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

Outcomes:

- `geosolve-sketch-features`: 21 tests passed;
- `geosolve-constraint-editor`: 175 unit tests and 46 integration tests passed, including 29
  focused M66 interaction-engine tests (14 feature-authoring plus 15 matrix tests);
- `geosolve-demo-web`: 73 tests passed, including playground screen/coordinator transactions and
  the focused SVG browser-default guard presentation contract;
- the complete locked all-feature workspace test suite passed;
- warnings-denied workspace Clippy passed;
- the all-feature demo-web WASM check passed;
- formatting and `git diff --check` passed; and
- the release Trunk command exited zero with `INFO applying new distribution` and
  `INFO ✅ success`.

Close-off cleanup source `f133ad1` then removed the superseded ADR 0030 editor facade and repeated
the same complete gate successfully. The first close-off Clippy pass identified that the relocated
M58 rollback regression had made one test 104 lines long; extracting the control-stop invariant
into a focused helper resolved that warning before `f133ad1`. On the committed cleanup source,
`geosolve-constraint-editor` passes 138 unit tests and the same 46 integration tests, including all
29 focused M66 interaction tests; `geosolve-sketch-features` passes 21 tests;
`geosolve-sketch-ops --test m58` passes 20 tests; and `geosolve-demo-web` passes 73 tests. The
locked all-feature workspace suite, warnings-denied Clippy, all-feature demo-web WASM check,
formatting, release Trunk build and `git diff --check` all pass.

No browser E2E/CDP suite was run or restored.

The standard Cargo duplicate `license`/`license-file` warnings remain pre-existing and did not
fail Clippy. The old `1034afc` qualification belongs solely to the archived architecture.

## 4. Acceptance status

Mechanical acceptance passed. U1-U5 are accepted under the explicit 2026-08-08 supervising-human
scoped close decision; `M66-PF001` through `M66-PF004` are mechanically closed by the direct
regressions below rather than represented as individually repeated human tests. Direct
qualification covers:

- four-point/three-span two-corner batch output and middle-span two-end composition;
- blank/default radius, both line-pick orders, exact screen-hit progression, point-corner atomicity,
  overlap fallthrough, high-valence ambiguity, bounded crowding and state-neutral stale/rejected
  retries;
- coordinator-transactional pick/options/preview publication, rejected refresh/apply defenses and
  the complete first-set publication through adjacent second-set publication plus Undo/Redo;
- painted preview-arc ownership where the same screen position also hits a native parent,
  including exact preview/state preservation, complete radius move/release, state-neutral foreign
  and second-pointer rejection, survival of the original gesture and modifier-safe owner
  selection;
- inflected cubic pickability, minimal straight-line sampling, computed Fillet minimum chord density
  and the denser advanced-drafting preview;
- the ordinary editable playground's multi-corner batch, independent-line interior picks,
  high-valence ambiguity/explicit recovery, rejected short-middle second pick/retry and supported
  line-circle/line-quadratic-Bezier authoring;
- SVG-scoped suppression of native browser selection/drag defaults without suppressing the Fillet
  options or other HTML;
- reverse-selection canonicalization and sequential/batch visible parity;
- atomic claim conflict plus recovery;
- exact sketch-state/residual/rank/DOF invariance under shared-radius edits;
- every source-point drag, invalid-feature withholding and source deletion/Undo recovery;
- deleting/suppressing either adjacent set while retaining the other;
- Undo/Redo/reload, stale CAS, cancellation, exhaustion and allocator non-reuse, including a real
  encode/decode/fresh-process restore after Undo and a cancelled preview that preserves every live
  allocator high-water;
- evaluation-local output-ID invalidation and variable output count;
- ordinary-UI absence of M28 associations, trim views, constraints and radius dimensions; and
- M27/M28/M30/M58 backward compatibility.

The Tailscale UAT service was live-rebuilt from `ac31791`; its served HTML was verified to contain
the scoped canvas marker (`draggable="false"`) at the historical endpoint
`http://100.94.63.83:8080/`. The endpoint is not a continuing post-close requirement.

## 5. Known limitations or next blocker

- Production and visual-profile consumers do not include computed output in M66. Misleading
  base-only presentation must be withheld.
- Two non-affine-parent ordinary authoring remains typed unsupported; M28 advanced APIs remain.
- Version-one computed features reference native constrained spans only.
- Computed-on-computed chaining, Offset, Bake/Explode and cross-revision output topological naming
  are deferred.
- `M66-KL001` retains radius-drag and branch-choice interaction as an accepted limitation. Radius
  drag measures pointer distance from the held/old arc center while evaluation moves the center
  and contacts, so tracking may drift or feel inverted. Post-placement contact/root,
  retained-parent direction and alternate-arc choices lack intuitive controls, especially for
  line-circle Fillets. Numeric radius editing, explicit persisted branch state, independent
  validation, rollback and sketch-state invariance remain correct. The playground line-circle
  specimen starts at radius `0.5`, near a branch fold.
- Future, unassigned work may add a headless one-dimensional grip derivative/rail, frozen absolute
  branch intent, contact/retention handles, continuation arrows, alternate-arc previews and a
  friendlier sample while retaining the fold as a regression fixture. It is not assigned to M67.
