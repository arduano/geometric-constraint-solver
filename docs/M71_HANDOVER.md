<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 retained drafting relations: closure summary

M71 was accepted and closed on 2026-08-14, including M71-U1 through M71-U5.
The qualified product is `f8a45ae7b355ab9874bf268c9950e369814e8432`, tree
`f7bccc58f301a715bc91f40115ce6424ec5f391d`. The complete clean release gate passed,
and the seven-file release was independently verified against served bytes.

The implementation and regression chronology remains in
[M71 implementation](M71_IMPLEMENTATION.md); the review cases remain in
[M71 UAT](M71_UAT.md). This summary records the durable technical boundaries.

## Implemented scope

M71 promotes six definitions across five relation families into the ordinary retained
document/editor lifecycle:

- stored-point `HorizontalPoints` and `VerticalPoints`;
- stored-point-to-native-span-midpoint `HorizontalPointToMidpoint` and
  `VerticalPointToMidpoint`;
- semantic-center `Concentric`; and
- directed native-support `Collinear`.

The sketch domain owns validation, lowering, audit grouping, suppression, deletion, dependency
closure, retained solve/history and persistence. Canonical sketch v4 is isolated behind a private
frozen wire DTO and rejects M71 state with `UnsupportedM71State`; unsupported draft v5 carries the
new records in an omitted-when-empty side section and transactionally merges them into the complete
source order.

The headless editor owns variable-arity contextual authoring, semantic inference, candidate
ranking, bounds, prospective same-transaction operands, atomic commit plans and presentation
metadata. The browser adapter renders and dispatches those public DTOs and supplies one ordinary
editable **Retained drafting relations** sample. It owns no equations or applicability policy.

The original four definitions lower to existing `add_horizontal_points`, `add_vertical_points`,
center `add_coincident` and `add_collinear` operations. F003 adds one `AxisMidpointResidual`
family with analytic and finite-difference-checked Jacobian. Every path is followed by independent
finite hard-residual validation; no solver priority or implicit branch rule changes.

F004 composes one remembered point/native-midpoint axis with a complementary exact Cartesian new-
span direction. F005 composes complementary axes from two distinct remembered stored points while
retaining both positional references through line/polyline confirmation. F006 tightens only the
default capture envelope to 6/9 px for points/midpoints, 8/12 px for curves and 3/5 degrees for
directions. These inference corrections add no solver equation, branch or persistence format.

## Implicit-correctness law

The design principle is **implicit correctness**: prefer strong composable semantics over a
tool-specific edge-case table.

The implemented center rule is expressed through one operand capability:

- `CenteredPointOperand` means a stored construction point that will also be the semantic center
  of a prospective curve;
- for that operand only, an exact accepted semantic-center/Concentric candidate outranks incidental
  structural reuse of the stored center point;
- ordinary `PointOperand` retains M70 point-identity precedence;
- midpoint and PointOnCurve remain available;
- an explicit candidate preference remains authoritative; and
- disabling Concentric falls back to ordinary point identity.

This rule covers Circle, counter-clockwise Circular Arc, Ellipse, Elliptical Arc and Hyperbola
without ranking branches named after those tools. Distinct curves that share one stored center are
distinct retained operands and therefore produce an ambiguous choice; repeated scene occurrences
of one curve are deduplicated. Persistent IDs never silently break a semantic tie.

Scene collection is all-or-nothing and bounded before publication. Ordinary and semantic anchors
share one subject-relevant bound; suppression bypasses traversal; overflow publishes no prefix;
scope/visibility filtering, ambiguity and post-overflow reacquisition are directly tested.

### Exact human finding and intended behavior

After F004, one remembered point/native-midpoint axis could compose with a complementary exact
direction of a new line or polyline span. The remaining nitpick was to snap a constructed endpoint
to complementary point axes at once: one distinct remembered stored point supplies its Horizontal
Y coordinate while another supplies its Vertical X coordinate. Both must be visible in one exact
preview and retained atomically through either the line or polyline path.

The focused F005 reproducer remembers stored points at `[-4, 4]` and `[3, -4]`, then approaches
`[3.04, 4.05]`. Before correction, candidate identity and confirmed-reference handoff represented
only one point-tracking component, so no one semantic candidate could own both remembered axes and
polyline continuation could retain only one positional reference. Separately, the default capture
thresholds were still the broader historical M70 values—8/12 px for points/midpoints, 10/14 px for
curves and 4/6 degrees for directions—which made inference feel too eager. These are M71-F005 and
M71-F006, presentation-independent interaction defects owned by `geosolve-constraint-editor`.

### Root cause and corrected contract

F005 gives `CandidateKey` an independent `secondary_point_tracking` component. Candidate
generation pairs Horizontal and Vertical tracking work from distinct semantic anchors before
publishing singleton alternatives. Horizontal supplies Y, Vertical supplies X, and the canonical
H-then-V candidate owns `[vertical.x, horizontal.y]`, two terminating constraint-backed guides,
both remembered references and one atomic two-relation plan. Confirmed positional-reference
collection now retains each distinct reference represented by the relations, so both survive the
ordinary polyline stage handoff.

One semantic anchor cannot pair its own two axes because that would disguise point identity as two
redundant retained relations. Equal competing pairings remain `Ambiguous`; both tracking latches
retain only through their shared exit band; the first candidate-limit overflow returns raw
coordinates without a candidate/guide prefix; and F004 point-axis-plus-span-direction bundles
remain explicit alternatives where they encode different retained intent.

F006 changes only `DraftInferenceTolerances::default()`. Stored points, semantic centers and native
midpoints now use inclusive 6/9 px enter/leave thresholds; curves use 8/12 px; and world,
remembered and point-tracking directions use 3/5 degrees. Valid caller-supplied policy values,
validation, resource bounds, suppression and hysteresis transitions remain authoritative and
unchanged. Neither F005 nor F006 changes a residual, Jacobian, solver priority, branch rule,
persistence format or browser-owned policy.

### Regression and focused evidence

Committed implementation source is `4f5339fa0de6b12794647835ac9066af5520887e`. The public
regression `crates/geosolve-constraint-editor/tests/m71_f005_cross_axis.rs` proves line and polyline
preview/commit paths, exact H-then-V relation order, two guides, two retained constraints, one-step
line history, finite accepted coordinates, independently recomputed endpoint equations, hard
residual `<= 1e-9`, later reference edits and both positional references surviving polyline stage
handoff.

Inference-owner tests prove stable pair identity, exact competing-pair ambiguity, same-anchor
exclusion, one-pair and overflowing candidate bounds, both-axis exit hysteresis and coexistence
with F004 point-axis-plus-span-direction alternatives. The F006 regression
`m71_f006_tighter_default_capture_envelope_excludes_old_only_entry_samples` rejects a seven-pixel
point, nine-pixel curve and 3.5-degree direction in a fresh default engine; the boundary matrix
keeps comparisons inclusive at the new thresholds.

Development prequalification commands passed on committed implementation HEAD
`4f5339fa0de6b12794647835ac9066af5520887e` while documentation-only changes remained in the
worktree:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-constraint-editor --test m71_f005_cross_axis
cargo test --locked -p geosolve-demo-web --all-features
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

All passed: the editor has 319/319 unit tests plus every integration/doc test, the F005 public
regression passes 2/2, demo-web passes 104 library tests plus its decoder/doc tests, the unchanged
canonical golden remains 234/234 `PASS`, native and WASM M70/M71 transition parity pass 1/1 each,
demo-web WASM passes, and Trunk 0.21.14 emits seven files. This remains development evidence; the
unchanged-source clean qualification below supplies nomination authority.

## Deferred cleanup review

The completed audit found no solver or mathematical blocker. A later M73 activation audit corrected
and resolved its two cleanup questions:

1. M71 already made `construction_point_stage` a projection of `draft_inference_subject`; they do
   not contain separate exhaustive `EditorTool` matches. M73 therefore targets only the remaining
   `directional_span_stage`/`draft_span_slot`/line-polyline-handoff duplication while preserving
   centered, coordinate-only and prospective-slot behavior.
2. The older direct `ConstraintKind` plus `available_constraints`/`constraint_edit` surface and
   dependent `EditorError::IncompatibleConstraint` variant postdate published `0.2.0` and cover
   only part of contextual authoring. The public methods have no non-test caller; the coordinator's
   internal `ConstraintKind` use was the simple-lowering seam. M73 retired them rather than
   preserving a second applicability oracle; lower-level sketch builders and the contextual
   20-family route remain.

The parallel semantic-center vector/latch/candidate pipeline was reviewed as an implementation
shape, but its behavior is now governed by operand capability, retained curve identity, shared
bounds and subject-aware ranking. Do not refactor it merely for visual uniformity unless the
result makes those laws smaller and clearer.

## Qualification and historical candidates

The closing source passed `./scripts/release-gate.sh`, covering formatting,
warnings-denied workspace Clippy, locked all-feature tests, WASM, release assembly,
licenses, package checks and performance budgets. The reviewed golden remained
234/234 PASS. Focused evidence includes 319 editor unit tests, the two public F005
regressions, 104 demo library tests and native/WASM transition parity.

F003 and F004 candidates are superseded historical evidence. F004 source
`a2e51efba7d79f684d264094ffd7dd0e37a4d089` does not include the complete F005/F006
closing behavior. Source and artifact identities remain in the implementation record;
old process inventories and machine-specific launch instructions have been retired.

For current development and release procedures, use the [documentation index](README.md)
and [release guide](RELEASE_QUALIFICATION.md).
