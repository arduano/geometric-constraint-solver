<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 implementation — production-quality constraint annotations

Status: active. Evidence is added only after the corresponding implementation and command passes.

## Implementation ledger

- [x] Editor-owned annotation layout state and semantic placement forms.
- [x] Exact shared paint/hit primitives and deterministic automatic layout.
- [x] Seven dimension-family renderers and compact/reference formatting.
- [x] Twenty contextual constraint categories plus Display “show all”.
- [x] Select-mode movement, cancellation and selected/global reset.
- [x] Workspace v6 optional cache with v1-v5 migration and fail-soft recovery.
- [x] Native/WASM/demo regressions.
- [ ] Complete clean release qualification and immutable Tailscale nomination.
- [ ] Human UAT approval and GitHub Pages publication.

## Implemented behavior

- `SceneAnnotationGeometry` owns exact linear/radial/angular paths, witnesses, leaders,
  arrowheads, label bounds and compact-glyph bounds consumed by both painting and picking.
- Automatic layout deterministically reserves points, curve corridors, viewport margins, fixed
  right-angle marks, prior annotations and manual placements. Genuine finite perpendicular corners
  retain fixed squares; other marks receive geometry-derived local rotation and movable leaders.
- All seven dimension families use compact four-significant-digit CAD notation and all twenty
  constraint categories have an ordinary editable sampler. The canvas title/ARIA text and the
  property inspector retain full semantic family, mode, value and unit descriptions.
- Typed layout moves preview after 3 px, commit only on pointer-up, cancel on every interaction
  invalidation path, and remain solve-, revision-, branch- and Sketch Undo/Redo-neutral.
- Workspace v6 stores an optional self-versioned cache. v1-v5 migrate empty; wrong outer types,
  versions, rows, identities, sources, marker indices, kinds, placement forms and non-finite values
  are discarded without rejecting otherwise-valid sketch data.

## Focused evidence

- `cargo test --locked -p geosolve-constraint-editor --lib` — 351 passed.
- `cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity` — 4 passed.
- `nix-shell shell.nix --run 'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity --target wasm32-unknown-unknown'` — 4 passed.
- `cargo test --locked -p geosolve-demo-web --lib` — 122 passed.
- `cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings` — passed.
- `cargo fmt --all -- --check`, `git diff --check` and
  `./scripts/golden-authoring-scene-oracle.sh --check` — passed; the reviewed 270-row oracle is
  unchanged.

Clean release qualification, immutable artifact identity, Tailscale serving evidence, human UAT
and public publication are intentionally not claimed yet.
