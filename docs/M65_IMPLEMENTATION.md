# M65 implementation: continuation, bounded work and explicit assembly branches

Status: implementation and mechanical qualification complete; focused human UAT pending.

## Files and public API

`geosolve-constraint-editor` now owns:

- `ProjectedDragWorkEvidence` and `ProjectedDragRejectionStage`;
- one accepted-preview continuation chain per pointer/point/design;
- bounded alternate-branch search through `propose_alternate_branch`,
  `alternate_branch_proposal`, `cancel_alternate_branch` and
  `accept_alternate_branch`;
- `AlternateBranchSearchResult`, status/evidence/proposal DTOs and the public
  `ALTERNATE_BRANCH_MAX_SEEDS = 24`;
- `visible_preview_session`, which gives presentation adapters the independently accepted
  branch ghost before an ordinary drag preview.

`geosolve-sketch` adds:

- stable `SketchSolveWorkSummary`;
- controlled reattempt seeded from an independently accepted preview while retaining the
  authoritative base accepted-state provenance;
- exact preview publication for a point plus persistent line-branch edits;
- persistent line branch getters/setters and atomic branch edit DTOs;
- reusable `BranchLockedElbow` and `BranchFourBar` alpha scenarios.

The sole workbench adds a selected-point **Assembly branch** inspector, Preview/Accept/Cancel
actions, bounded-search evidence and a gold dashed non-authoritative ghost. It consumes only the
public headless/session APIs. No equation, seed search or branch inference moved into WASM.

## Mathematical and lifecycle behavior

Each ordinary pointer sample executes one retained attempt. The cursor target is Temporary and
accepted previous coordinates are Preferences. A consecutive sample is seeded from the last
independently accepted preview, not the committed document. A failed sample retains that preview;
the next valid sample continues from it. Release publishes the exact preview seed through a
separate independently validated retained solve and creates one Undo checkpoint.

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

The previous pantograph explosion came from a positive-cost stationary Temporary pass followed by
recursive Preference trial reprojections. The exact-feasibility candidate reaches the attainable
cursor target directly. Zero-like Preference trials retract hard and Temporary rows together,
and changes below the established rank/roundoff resolution terminate truthfully as
`SecondaryStatus::Acceptable`.

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

Starting commit `927efb7` recorded pantograph drag work of:

| Corpus | Factorizations | Nonlinear iterations |
| --- | ---: | ---: |
| Pantograph at start | 244824 | 240953 |

The frozen final three-sample corpus records:

| Corpus | Factorizations | Nonlinear iterations |
| --- | ---: | ---: |
| Scotch yoke after guide deletion | 17 | 17 |
| Scissor jack | 18 | 18 |
| Five-stage scissor tower | 24 | 24 |
| Pantograph | 33 | 21 |

Every sample accepts with exactly one retained attempt. Pantograph is below twice the tower work
in both counters and improves by more than 99.9% from the starting commit. Wall-clock time is not
an acceptance metric.

## Direct acceptance coverage

- accepted preview continuation, base provenance and exact final commit;
- stale, foreign, provenance-mismatched, nonaccepted and point-mismatched preview rejection;
- atomic no-mutation failure behavior;
- twin-roller independence plus rejection/recovery continuation;
- Scotch-yoke two-DOF, scissor, tower and pantograph work corpus;
- zero-like Temporary row-space protection and bounded nonlinear rank-deficient work;
- tiny nonzero secondary objectives and sparse/dense backend parity;
- bounded branch ghost, exact stamps, persistent branch publication, stale rejection,
  Undo/Redo and replay;
- every sample builds an accepted coordinator and both branch samples produce proposals.

## Commands and outcomes

Focused and final commands:

```text
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m12'
nix-shell shell.nix --run 'cargo test -p geosolve-sketch --test m30'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m5_priority'
nix-shell shell.nix --run 'cargo test -p geosolve-core --test m16'
nix-shell shell.nix --run 'cargo test -p geosolve-constraint-editor deterministic_mechanism_drag_corpus_has_one_attempt_per_sample'
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features --quiet'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env NO_COLOR=true trunk build --release'
git diff --check
```

All commands pass on the final source state. The full M12 scale corpus validates the exact
Hard+Temporary machine-roundoff envelope at `1e-6`, `1` and `1e6`. The complete M30 construction
suite proves that Temporary-only offset, mirror and fillet drags retain their established
associated-motion behavior. The frozen mechanism corpus retains `(17,17)`, `(18,18)`, `(24,24)`
and `(33,21)` work evidence.

## Known limitations and next blocker

- Search is a bounded representable-alternative prototype, not global root enumeration.
- Only persistent line-branch directions can currently represent an accepted assembly switch.
- Ordinary drag deliberately retains its branch; changing branch requires the explicit action.
- M65 cannot close until the supervising human approves `docs/M65_UAT.md`.
