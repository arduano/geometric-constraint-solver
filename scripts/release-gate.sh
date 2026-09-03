#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [[ "${GEOSOLVE_ALLOW_DIRTY:-0}" != "1" ]] && [[ -n "$(git status --porcelain)" ]]; then
  printf '%s\n' "release gate requires a clean tree; set GEOSOLVE_ALLOW_DIRTY=1 for development verification" >&2
  exit 1
fi

cargo metadata --locked --offline --format-version 1 >/dev/null
cargo fmt --all -- --check
git diff --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
RUST_MIN_STACK=16777216 cargo test --locked --workspace --all-features
./scripts/golden-authoring-scene-oracle.sh --require-clean
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m74_reference_geometry \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m75_hover_pointer_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m77_curve_control_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m79_inference_lifecycle \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-demo-web --lib \
  actual_wasm_ \
  --target wasm32-unknown-unknown
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
(
  cd packages/geosolve-intent
  npm ci --ignore-scripts
  npm test
)
(
  cd packages/geosolve-sketch-code
  npm ci --ignore-scripts
  npm test
)
cargo test --locked -p geosolve-headless --test m87_headless \
  inspect_edit_solve_and_static_render_share_one_exact_control_authority \
  -- --exact --ignored
cargo test --locked -p geosolve-headless --test m87_headless \
  cli_inspect_render_and_edit_are_browser_free_and_never_overwrite_outputs \
  -- --exact --ignored
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo bench --locked --workspace --all-features --no-run

cargo run --locked --release -p geosolve-sketch --example m14_performance
cargo run --locked --release -p geosolve-sketch --example m32_performance
cargo run --locked --release -p geosolve-constraint-editor --example m83_performance
cargo test --locked --release -p geosolve-constraint-editor \
  --test m83_interaction_performance -- --ignored --nocapture --test-threads=1
cargo test --locked --release -p geosolve-linkage --test m23_performance \
  exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain \
  -- --exact --ignored --nocapture

if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check licenses
else
  nix-shell -p cargo-deny --run 'cargo deny check licenses'
fi

for package in \
  geosolve-geometry \
  geosolve-core \
  geosolve-sketch \
  geosolve-linkage \
  geosolve-sketch-features \
  geosolve-sketch-intent \
  geosolve-sketch-ops \
  geosolve-sketch-topology \
  geosolve-constraint-editor \
  geosolve-sketch-code \
  geosolve-sketch-render \
  geosolve-headless
do
  contents="$(cargo package --locked --allow-dirty --list -p "$package")"
  grep -qx 'LICENSE' <<<"$contents"
  grep -qx 'README.md' <<<"$contents"
done

./scripts/verify-geosolve-sketch-code-package.sh

(
  cd crates/geosolve-demo-web/frontend
  npm ci --ignore-scripts
  npm run check
  npm run validate:dist -- ../dist ./
)
