# M64 editable sample library implementation

Status: implementation, mechanical qualification and human UAT complete as of 2026-07-30.

## 1. Files and APIs

- `geosolve-sketch` adds public `MotionFourBarCoupler`, `MotionPantograph` and
  `MotionDrawingArm` alpha fixtures with stable persistent role structs.
- `DocumentSolveRequest::with_previous_state_preferences` makes the interaction preference
  policy explicit at the persistent request boundary.
- `RetainedEditorCoordinator::resolve_projected_point_move` owns generic passive-freedom
  stabilization. The browser supplies only the dragged persistent point and target.
- `geosolve-demo-web::workbench::samples` owns 22 private selector definitions under exactly
  three purpose groups.
- The old `scenarios`, `scenario_fixtures` and `evidence` modules are deleted.

## 2. Mathematical behavior

- Four-bar coupler: two fixed grounds, three fixed bar lengths and a coupler midpoint produce one
  bidirectional degree of freedom.
- Pantograph: one fixed anchor, two driving arm lengths, two parallel translated-side relations
  and a diagonal midpoint produce two bidirectional degrees of freedom.
- Three-link drawing arm: one fixed anchor and three driving link lengths produce three
  bidirectional degrees of freedom.
- All three remain hard-valid with maximum normalized hard residual at most `1e-9` and stable IDs
  at model scales `1e-6`, `1` and `1e6`.
- Projected drag first asks the priority solver to retain non-targeted accepted points as
  preferences. If that pass cannot publish a valid preview, the coordinator tries finite
  non-fixed accepted points as one temporary passive anchor. Exact release publishes only the
  retained point edit; temporary drag/stability targets do not enter the publication request.

## 3. Qualification commands

The final gate passed on 2026-07-30 with these exact commands:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env NO_COLOR=true trunk build --release'
git diff --check
```

Direct regressions additionally cover the 22 accepted sample constructors, snapshot round-trip,
one-checkpoint history, fixed-constraint Delete/Undo, retired-harness absence and independent
twin-roller dragging in both directions.

All commands exited successfully. The `NO_COLOR=true` override adapts the host's
`NO_COLOR=1` value to the boolean syntax required by Trunk 0.21.14; it does not alter the
source or bundle.

## 4. Acceptance criteria

- Samples are ordinary editable workspaces, not a special runtime mode.
- Opening replaces the current workspace and resets history.
- Autosave, authoring, branch/dimension editing, Delete/Undo, zoom/pan and drag remain active.
- The catalog is grouped by purpose and contains no milestone folders or legacy key aliases.
- Guided actions, descriptions, verification points, transcript/evidence capture and reset/exit
  state are absent.
- Browser E2E infrastructure and `/#/dev/lab` remain retired.

## 5. Known limitations and next step

- The workbench is still a non-authoritative desktop UAT consumer.
- Samples intentionally contain no tutorial flow or protected starting state; reopening a sample
  is the way to obtain its pristine document.
- `docs/M64_UAT.md` records supervising-human approval. M65 awaits its detailed
  core-hardening/performance scope.
