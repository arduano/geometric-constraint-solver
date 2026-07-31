# M65 implementation: continuation, bounded work and explicit assembly branches

Status: the `M65-F003` corrective implementation is present; final mechanical qualification and
focused human UAT remain pending. The preceding focused-UAT candidate is withdrawn.

## Files and API surface

`geosolve-constraint-editor` now owns:

- `ProjectedDragWorkEvidence` and `ProjectedDragRejectionStage`, with the complete
  `DocumentDragLocalityPlan` exposed as read-only work metadata;
- one accepted-preview continuation chain per pointer/point/design, including one locality plan
  frozen for the complete gesture;
- bounded alternate-branch search through `propose_alternate_branch`,
  `alternate_branch_proposal`, `cancel_alternate_branch` and
  `accept_alternate_branch`;
- `AlternateBranchSearchResult`, status/evidence/proposal DTOs and the public
  `ALTERNATE_BRANCH_MAX_SEEDS = 24`;
- `visible_preview_session`, which gives presentation adapters the independently accepted
  branch ghost before an ordinary drag preview;
- a complete finite ordinary-pointer vector: 16,384 items each for document validation,
  dependency/locality and lowering; 256 nonlinear iterations; 512 rejected trials; 1,024
  component linearizations; `256 x 256` dense dimensions plus 33,554,432 additive dense-kernel
  work units; 256 factorizations; 256 rank kernels; 512 diagnostic candidates; and 1,024
  diagnostic trials. Exhaustion is exposed as a controlled-operation rejection;
- semantic circle-to-center drag handles and pointer-offset-preserving gestures, so dragging a
  circumference moves its center without a cursor jump.

`geosolve-sketch` adds:

- public stable `SketchSolveWorkSummary`;
- crate-private `SketchDragLocalityAnchor` / `SketchDragLocalityPlan` runtime analysis artifacts;
  these are not public API;
- persistent read-only `DocumentDragLocalityAnchor` / `DocumentDragLocalityPlan` DTOs and
  `RetainedSketchDocumentSession::drag_locality_plan`;
- first-sample `reattempt_with_drag_locality_controlled` and continuation
  `reattempt_from_accepted_preview_with_drag_locality_controlled` seams;
- controlled reattempt seeded from an independently accepted preview while retaining the
  authoritative base design-publication and accepted-state provenance;
- exact preview publication for a point plus persistent line-branch edits, using no-motion
  certification that rebuilds accepted evidence without projection or optimization;
- persistent line branch getters/setters and atomic branch edit DTOs;
- candidate-shaped numerical seeding for history/restore, with a deterministic untouched-candidate
  fallback when compatible imported values violate candidate-only topology;
- reusable `BranchLockedElbow` and `BranchFourBar` alpha scenarios.

`geosolve-core` now owns a rank-aware least-squares seam for priority projection. Its `2 x N`
case builds a stable row-space basis and solves only the resulting fixed-size system; all shapes
must certify stationarity and minimum-norm/nullspace orthogonality before returning a solution.
The public operation-control surface adds `OperationWorkCounter::DenseKernelWorkUnits` and the
matching `OperationLimits::dense_kernel_work_units` / `OperationWork::dense_kernel_work_units`
fields. Every dense authorization charges the conservative additive
`max(rows, columns)^3` work before entering the kernel; row and column maxima remain separate.

The obsolete second `DocumentSolveRequest` stability target and its
`with_stability_target` builder are removed. The persistent-ID-ordered passive retry is also
deleted rather than retained as an alternate policy.

The sole workbench adds a selected-point **Assembly branch** inspector, Preview/Accept/Cancel
actions, bounded-search evidence and a gold dashed non-authoritative ghost. It consumes only the
public headless/session APIs. No equation, seed search or branch inference moved into WASM.

## Mathematical and lifecycle behavior

Each ordinary pointer sample executes one retained attempt. The cursor is the sole Temporary
target. Only the selected locality anchors become `PreviousState` Preferences; the drag path does
not ask every non-targeted point to remain fixed.

The plan is derived once, at gesture start, from the freshly accepted hard nullspace. The active
point response gives its controlled rank; the remaining component freedom is the passive rank to
cover. Candidate points are chosen by greatest uncovered-rank gain, then lower total mobility
rank, then compilation order. An incomplete or numerically ambiguous point cover fails closed.
Runtime identities and rank evidence are mapped to persistent IDs, but each anchor target is
copied from the accepted document geometry actually visible when the gesture begins.

`DocumentDragLocalityPlan` stamps the design identity, accepted-state identity, exact
process-local design-publication and accepted-state provenance, and persistent active point, and
carries hard-equality DOF, active rank, passive DOF and selected
`DocumentDragLocalityAnchor` values. Each provenance value is a private shared token: ordinary
lifecycle clones share it, every retained-design publication renews the design token and every
accepted publication renews the accepted token. Plan equality and stale validation compare both
tokens by identity, so divergent lifecycle clones at the same numeric revisions cannot exchange a
plan. Compile-local component ordering is deliberately absent. The coordinator freezes that
complete plan through continuation, rejection and recovery and publishes it in
`ProjectedDragWorkEvidence`. A consecutive sample may use the last independently accepted preview
as a numerical seed, but it reuses the gesture-start anchor targets. A failed sample retains the
complete last preview; the next valid sample continues from it. Release publishes the exact
preview seed through a separate independently validated retained solve and creates one Undo
checkpoint.

That release solve is a no-motion certification, not another projection. It rematerializes the
exact accepted preview, validates its domain/branch/reference state and independently rebuilds
residual, rank, bound, diagnostic and audit evidence. The same shared operation controller charges
solved-state materialization and changed-audit discovery as document-lowering work, and candidate
validation plus audit evaluation/refresh as document-validation work, with
`BeforeFinalValidation` and `AfterFinalValidation` checkpoints. Cancellation or work exhaustion
at any of those late stages publishes nothing: design, attempt, accepted identity, canonical
design/accepted bytes and revision high-water remain unchanged.

The same design token closes preview publication. A preview of the current design must share the
authoritative publication token. A next-revision branch preview must own a fresh child token and
name the authoritative token as its parent; both paths still prove the exact accepted parent.
Point and branch payload checks use exact floating bits. Regressions deliberately construct
ordinary-`PartialEq`-equal but signed-zero-distinct designs and branch directions, and prove stale
or mismatched previews reject atomically. Process-local tokens are not persisted:
interaction-free restore compares canonical bytes, retains the exact supplied design bytes and
independently reproduces the exact accepted bytes.

Undo/Redo/reload seed from the candidate graph, never the older accepted topology. Only compatible
point positions, curve/contact scalar coordinates and a topology-compatible rational-conic middle
coordinate may be imported as numerical state; dimension targets and every equation-bearing
constraint, dimension, contact, branch, activation and ownership field remain candidate owned. If
the imported values make candidate-only topology structurally invalid, the valid untouched
candidate becomes the deterministic seed.

Core retains the strict Hard → Temporary → Preference hierarchy. For a small dense component that
also owns movable Preference stabilizers, it may first try hard rows plus Temporary rows as an
exact feasibility candidate. That candidate is accepted as a zero-cost Temporary optimum only
after separate finite derivative evaluation, ordinary hard validation and Temporary residual
validation. Temporary-only construction edits retain the established general lexicographic path.
Preferences protect the complete zero-like Temporary row space and remain inside the reported
normalized numerical-resolution envelope. Tiny nonzero Temporary objectives still optimize,
infeasible Temporary levels use the general optimizer, and `SparsePreferred` retains its
established sparse hard-reprojection evidence. This is not weighted least squares and does not
relax hard success.

`M65-F001` correctly identified the previous pantograph explosion: a positive-cost stationary
Temporary pass recursively reran the Temporary optimizer for lower Preference line-search and
curvature trials. Its direct scalar-level retraction bounded that path, but `M65-F003` found unsafe
fallback and locality ownership. Failed scalar correction could return the raw post-Temporary
state as an `Acceptable` Preference outcome, and continuation had no frozen rank-derived anchor
contract to prevent already-solved passive drift from becoming later intent.

Positive-cost Temporary handling is now baseline-first. Core optimizes Preferences while
protecting Hard rows and every normalized row of the complete attained Temporary residual vector.
The result becomes the certified locality baseline only after finite evaluation, endpoint
restoration, ordinary hard/priority validation, exact-row preservation and Preference-cost
validation. This baseline can remove avoidable passive/null-direction drift without altering the
cursor residual vector.

Only after that baseline certifies may bounded scalar-level refinement seek additional legitimate
Preference motion along the larger nonlinear constant-cost manifold. A failed, stalled, worse or
uncertified refinement cannot replace the exact-row baseline. If the baseline itself cannot be
certified, the solve returns a non-successful Preference result; the editor rejects that one
pointer sample and retains the complete last accepted preview. Raw post-Temporary geometry is
never a success-like fallback. Neither stage recursively invokes the Temporary optimizer, and
neither weakens the Hard → Temporary → Preference hierarchy.

The rank-deficient least-squares solve uses the authoritative unsquared singular-value cutoff
throughout. For a wide `2 x N` pointer system it chooses the dominant row, reorthogonalizes the
other row into a stable row-space basis, compresses to at most `2 x 2`, and uses the fixed-size
SVD with that same cutoff. Dynamic shapes reuse one SVD for bounded refinement. The common success
certificate independently checks normal-equation stationarity and verifies that the returned
solution equals its retained-row-space projection within roundoff, excluding arbitrary nullspace
motion. The restored exact A5 round-trip path covers `1e-6`, `1` and `1e6`.

Alternate branch search is explicit and bounded. It checks eight canonical directions at radii
`0.5`, `1` and `2`, for at most 24 seeds. A proposal requires:

- exact design and accepted-state stamps;
- finite independently accepted geometry;
- normalized hard residual `<= 1e-9`;
- unchanged degrees of freedom;
- a distinct point position and persistent line-branch directions.

Zero alternatives, multiple distinct alternatives, unrepresentable geometry and exhausted work
are separate outcomes. A proposal is a ghost only. Accept atomically publishes the point and
branch directions; Cancel and stale acceptance mutate nothing. Undo/Redo and replay include the
accepted branch transaction.

## Deterministic evidence

This is the current corrective-source characterization, not the still-pending workspace-wide
mechanical gate. Each ordinary sample performs one retained attempt. Counts are deterministic;
wall-clock time is not an acceptance metric.

| Representative path | Per-sample peak `(F, I)` | Aggregate `(F, I)` |
| --- | ---: | ---: |
| Scotch yoke after guide deletion | `(39, 31)` | `(148, 116)` |
| Scissor jack | `(43, 35)` | `(156, 124)` |
| Five-stage scissor tower | `(56, 46)` | `(148, 122)` |
| Pantograph input arm | `(116, 93)` | `(328, 265)` |
| Pantograph guide arm | `(103, 84)` | `(291, 242)` |
| Pantograph wide output path | `(67, 57)` | `(598, 502)` |
| Pantograph wide center path | `(67, 57)` | `(598, 502)` |
| Natural twin rollers, both directions | global `(155, 147)` | left `(878, 818)`; right `(920, 860)` |
| Difficult twin-roller reject/recover | global `(120, 89)` | left `(130, 100)`; right `(127, 103)` |
| MotionCam lifecycle move | `(112, 106)` | `(220, 204)` |

Here `F` is factorizations and `I` is nonlinear iterations. Across these representative samples,
the other observed per-sample maxima are 222 validation items, 363
document-dependency/locality items, 218 lowering items, 76 rejected trials, 12 component
linearizations, a `43 x 22` dense kernel and 15 rank items.

The production pointer vector is:

| Controlled work | Per-sample limit |
| --- | ---: |
| Document validation items | 16,384 |
| Document dependency/locality items | 16,384 |
| Document lowering items | 16,384 |
| Nonlinear iterations | 256 |
| Rejected trials | 512 |
| Component linearizations | 1,024 |
| Dense-kernel row/column dimensions | `256 x 256` |
| Additive dense-kernel work units | 33,554,432 |
| Factorizations | 256 |
| Rank kernels | 256 |
| Diagnostic candidates | 512 |
| Diagnostic trials | 1,024 |

A 128/128 factorization/iteration ceiling fails the representative matrix; 256/256 passes it.
This leaves measured headroom above the global `155/147` peak without returning to the withdrawn
much higher provisional limit.

The pantograph measurements exercise the original public M64 fixture: two ordinary `Parallel`
relations close the translated sides, two ordinary driving arm-length dimensions retain their
radii and one ordinary `Midpoint` locates the diagonal center. The corrective work does not
replace that construction with affine coordinate equations and does not require a draft-v5
exception.

## Direct acceptance coverage

The current focused suite retains:

- accepted preview continuation, exact design/accepted parent provenance and exact final commit;
- stale, foreign, provenance-mismatched, nonaccepted and point-mismatched preview rejection;
- signed-zero-distinct same-revision preview and branch rejection with atomic no-mutation behavior;
- exact-byte design/accepted restore plus candidate-shaped Undo/Redo/reload seeding;
- the M61 pathological contact candidate still directly rejects its bounded-contact ambiguity,
  while exact checkpoint restore preserves its retained design and prior accepted graph
  byte-for-byte before a small accepted-state-seeded retry;
- stable rank-aware `2 x N` stationarity/minimum-norm certification and exact all-scale A5
  round-trip;
- atomic no-mutation failure behavior;
- exact/on-manifold twin-roller motion plus rejection/recovery continuation;
- both circle circumferences resolving to their own semantic center gesture without pointer jump;
- difficult roller projection bounded before valid continuation recovery;
- Scotch-yoke two-DOF, scissor, tower and pantograph work corpus;
- natural off-manifold pantograph input motion with accepted bounded work;
- zero-like Temporary row-space protection and bounded nonlinear rank-deficient work;
- positive Temporary scalar-level preservation without recursive reoptimization;
- tiny nonzero secondary objectives and sparse/dense backend parity;
- bounded branch ghost, exact stamps, persistent branch publication, stale rejection,
  Undo/Redo and replay;
- every sample builds an accepted coordinator and both branch samples produce proposals.

`M65-F003` adds or strengthens these direct headless owners:

- `drag_locality_plan_uses_accepted_visible_targets_and_freezes_them_through_continuation`
  covers persistent stamps, accepted-visible target capture, anchor-only Preferences and
  continuation reuse;
- `drag_locality_plan_is_stale_after_release_and_fixed_points_use_an_empty_plan` covers lifecycle
  staleness and the zero-DOF empty-plan case;
- the divergent-same-revision locality regression proves that exact process-local design and
  accepted provenance, not numeric revision equality, owns a plan;
- the signed-zero preview/branch/restore regressions prove the exact process-local versus
  cross-process identity boundary;
- `m65_history` proves structurally distinct rejected designs retain their topology and an invalid
  topology-dependent numerical merge falls back identically for Undo, Redo and reload;
- `m65_interaction_lifecycle` proves queued projected results cannot outlive release/cancel,
  alternating pantograph gestures recapture locality through history, both roller
  circumferences publish byte-exact stable releases with symmetric rejection/recovery, and wide
  pantograph output/center gestures reverse near their boundary;
- `exact_state_certification_accounts_for_every_post_core_stage` and the late release-exhaustion
  lifecycle assertions prove no-motion materialization, validation and audit work is typed,
  bounded and atomic;
- the rank-one core regressions prove the stable fixed-SVD path is stationary, bounded and
  minimum-norm, while M14 restores exact A5 coverage at every supported model scale;
- `cam_motion_projects_one_roller_while_locality_keeps_the_other_stationary` covers the sketch
  domain path without a scenario policy;
- `positive_temporary_exact_row_baseline_is_optimized_before_scalar_fallback` covers the
  certified complete-row baseline;
- `coupled_failed_scalar_refinement_retains_the_certified_complete_row_baseline` covers optional
  refinement failure;
- `natural_twin_roller_paths_keep_the_passive_roller_at_the_gesture_baseline` covers symmetric
  horizontal/vertical/diagonal/reversing gestures with an unchanged plan and passive-center
  movement no greater than `1e-8`;
- the bounded rejection/recovery regression covers complete-preview retention and later valid
  recovery on the same continuation chain; and
- the wide pantograph regression asserts the representative document still owns exactly its two
  ordinary `Parallel` relations and exercises input, guide, output and center paths under the
  256/256 ceiling.

The focused provenance, locality, priority, history and work-characterization assertions pass on
the current corrective source. They are not a substitute for the full final gate, which remains
pending together with the release build and human UAT.

## Commands and outcomes

Focused commands underlying the current provenance and work characterization:

```text
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m34_lifecycle drag_locality_plan_rejects_a_divergent_same_revision_accepted_parent'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m34_lifecycle drag_locality_plan_uses_accepted_visible_targets_and_freezes_them_through_continuation'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m34_lifecycle drag_locality_plan_is_stale_after_release_and_fixed_points_use_an_empty_plan'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m14 cam_motion_projects_one_roller_while_locality_keeps_the_other_stationary'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m5_priority positive_temporary_exact_row_baseline_is_optimized_before_scalar_fallback'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m16 coupled_failed_scalar_refinement_retains_the_certified_complete_row_baseline'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor natural_twin_roller_paths_keep_the_passive_roller_at_the_gesture_baseline'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor difficult_twin_roller_projection_is_bounded_and_recovery_retains_continuation'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m12'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m30'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m5_priority'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m16'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor off_manifold_pantograph_cursor_path_is_accepted_with_bounded_work'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor off_manifold_pantograph_guide_path_keeps_the_input_arm_fixed'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor pantograph_output_and_center_follow_wide_reversible_positive_assembly_paths'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor difficult_twin_roller_projection_is_bounded_and_recovery_retains_continuation'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor motion_cam_circumference_gesture_publishes_the_exact_visible_preview'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor circle_circumferences_drag_their_semantic_centers_without_pointer_jump'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor deterministic_mechanism_drag_corpus_has_one_attempt_per_sample'
```

The latest focused implementation snapshot (2026-07-31) additionally ran these locked suites:

```text
nix-shell shell.nix --run 'cargo test --locked -p geosolve-core --lib'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-core --test m5_priority'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-core --test m15'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-core --test m16'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m34_lifecycle'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --test m65_history'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --test m65_interaction_lifecycle'
```

| Focused suite | Outcome |
| --- | ---: |
| `geosolve-core --lib` | 46 passed, 1 ignored |
| `m5_priority` | 28 passed |
| `m15` | 13 passed |
| `m16` | 49 passed |
| `m34_lifecycle` | 32 passed |
| `m65_history` | 2 passed |
| `m65_interaction_lifecycle` | 6 passed |

Those focused suites, regressions and the 256/256 characterization pass. A 128/128
characterization fails and is not the production policy. The final mechanical gate remains
pending:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features --quiet'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env NO_COLOR=true trunk build --release'
git diff --check
```

No format, warnings-denied workspace Clippy, locked all-feature workspace, WASM or release Trunk
outcome is claimed here for the current corrective source state.

## Known limitations and next blocker

- Search is a bounded representable-alternative prototype, not global root enumeration.
- Only persistent line-branch directions can currently represent an accepted assembly switch.
- Ordinary drag deliberately retains its branch; changing branch requires the explicit action.
- The prior endpoint must not be treated as the current candidate until `M65-F003` passes the
  complete mechanical gate and is rebuilt from that exact source state.
- M65 cannot close until the supervising human approves the refreshed `docs/M65_UAT.md`.
