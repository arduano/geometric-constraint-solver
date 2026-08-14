<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M72 implementation — Public workbench bulk fixes

Status: M72-F001 through M72-F003 are implemented and directly qualified. The pinned Pages
workflow and repository-prefixed artifact build for M72-F004 are also implemented and locally
validated. Clean release qualification, repository publication, live-site verification and
supervising-human UAT approval remain before milestone closure.

Implementation commits:

- `9e38032` — clear stale retained Problems;
- `bcbaf6f` and `392637d` — keep interactive rectangles free-size and prove construction/resize
  history;
- `a0758db` — unify canvas tool options and exact-set Problems dismissal;
- `a1663d0` — qualify and deploy the repository-prefixed Pages artifact.

## 1. Files and APIs

- `crates/geosolve-constraint-editor/src/coordinator.rs` clears prior computed setup errors at the
  start of each durable refresh. Same-sketch Undo/Redo and reload shortcuts now require accepted
  authority for the current input; a rejected live attempt is rebuilt from the selected checkpoint.
  Focused tests cover rejected-dimension recovery, suppression repair, current parameter/external
  failures, feature-only Undo and same-sketch reload after failed no-history attempts.
- `crates/geosolve-constraint-editor/src/lib.rs` changes only the interactive
  `ConstructionProposal::Rectangle` lowering. It removes the macro-generated fixed-point source
  and width/height dimensions through their ordinary owned-state removal path. The canonical
  `SketchDocument::add_rectangle` macro is unchanged.
- `crates/geosolve-constraint-editor/tests/m72_free_rectangle.rs` owns the exact rectangle
  topology, finite accepted geometry, independently validated residual, rank/DOF, projected resize
  and construction/resize Undo/Redo contract.
- `crates/geosolve-demo-web/index.html`, `styles.css` and `src/workbench/mod.rs` replace sidebar
  flyouts with one nonmodal bottom-left canvas surface. An internal `OptionOverlayKind` owns Equal,
  Tangent, Continuity, all five dimensions, Fillet, five conic-family tools, NURBS and Construction
  display. It provides mutual exclusion, family-local controls and parsing, bounded scrolling,
  deterministic focus entry/return, Escape, close and light-dismiss behavior.
- The Problems card uses a presentation-only exact problem-set identity. Dismissal hides only the
  currently rendered set; a changed set opens automatically, and recovery removes the card without
  changing headless diagnostics, tree state or canvas markers.
- `.github/workflows/pages.yml` runs the complete release gate, performs a separate Trunk build
  using the Pages base path, validates exactly seven regular non-symlink files and deploys that
  artifact. Every third-party action and the Nixpkgs source is commit-pinned.
- The workbench footer exposes Source and License links. `README.md` links the approved public URL.

No new public Rust API, solver equation, residual, Jacobian, branch rule, priority, persistence
schema or golden case was added.

## 2. Mathematical and lifecycle behavior

F001 is an accepted-authority correction. Computed refresh state from a previous failed attempt is
discarded before evaluating the durable current input. A genuine failure produced again by the
current parameters or external snapshots remains visible. Undo, Redo, repair and reload retain the
ordinary transactional rule: rejected design intent may remain in history, but only geometry
accepted for the restored current input is authoritative.

F002 preserves four shared rectangle corners, four explicit line branches and four hard H/V rows.
It publishes no fixed-point source, dimension or private scalar. The focused accepted state has
finite coordinates, normalized hard residual at most `1e-9`, numerical rank four and right nullity
four. Projected corner movement changes width and height, and construction plus resize each owns
one normal history step. The separately public constrained rectangle macro retains its historical
anchor and dimensions.

F003 and the Problems disclosure are browser presentation changes over existing public coordinator
and audit data. Only the active option family is read or validated, so malformed hidden C2, conic
or NURBS fields cannot block an unrelated tool. No browser-owned equation or accepted-state path
was introduced.

## 3. Commands run and outcomes

The following development qualification commands have run successfully on the committed M72
implementation:

```text
cargo fmt --all -- --check
git diff --check
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo clippy --locked -p geosolve-demo-web --lib --all-features -- -D warnings
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release --locked'
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
/tmp/tmp.ywhCssvJw5/actionlint .github/workflows/pages.yml
```

Outcomes:

- constraint-editor: **321/321 unit tests** and every integration/doc test pass; the focused
  `m72_interactive_rectangle_has_free_size_and_retained_history` regression passes **1/1**;
- demo-web: **104/104 library tests**, the decoder test and doc tests pass;
- focused warnings-denied web Clippy and the WASM target check pass;
- the unchanged canonical authoring/scene oracle remains **234/234 `PASS`** and passes survey,
  check and require-clean modes;
- formatting and diff hygiene pass;
- Trunk 0.21.14 produces seven files;
- a separate build with `--public-url /geometric-constraint-solver/` produced exactly seven regular
  non-symlink files with prefixed JavaScript, WASM and stylesheet references;
- `actionlint` passes for the Pages workflow.

Local Chromium checks using the ordinary release build passed at `1440x900` and `1024x720`. They
opened every option family, verified one active panel, subtype-specific controls, containment,
focus entry/return, Escape, explicit close, outside dismissal without swallowed zoom, invalid
inactive-field isolation, Problems close/reopen, Source/License links and browser-local scene
persistence across reload. Screenshots are retained at `/tmp/m72-overlay-1440x900.png` and
`/tmp/m72-overlay-1024x720.png`.

## 4. Acceptance passed

- F001 stale native/computed Problems recovery and genuine-current-failure retention pass at the
  retained coordinator owner.
- F002 free-size interactive rectangle topology, residual, rank/DOF, resize and history pass; the
  canonical macro is unchanged.
- F003's unified option surface, exact Problems dismissal and compact/ordinary desktop containment
  pass native/web tests and local Chromium review.
- The reviewed 234-row golden and all mathematical/persistence meanings remain unchanged.
- The Pages workflow and repository-prefixed seven-file artifact pass local static validation.

## 5. Remaining gate

- Commit this implementation/UAT handoff and run the complete release gate from that clean source.
- Re-scan the complete Git history for secrets, make the repository public, enable workflow Pages,
  push `main` and set the repository homepage.
- Verify the deployed HTML/assets, WASM media type and hashes, then repeat Chromium load/reload
  persistence against the public URL.
- Receive explicit supervising-human approval of the focused checks in `docs/M72_UAT.md`.

