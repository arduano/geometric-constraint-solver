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
cargo test --locked --workspace --all-features
./scripts/golden-authoring-scene-oracle.sh --require-clean
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity \
  --target wasm32-unknown-unknown
env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity \
  --target wasm32-unknown-unknown
cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo bench --locked --workspace --all-features --no-run

cargo run --locked --release -p geosolve-sketch --example m14_performance
cargo run --locked --release -p geosolve-sketch --example m32_performance
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
  geosolve-sketch-ops \
  geosolve-sketch-topology \
  geosolve-constraint-editor
do
  contents="$(cargo package --locked --allow-dirty --list -p "$package")"
  grep -qx 'LICENSE' <<<"$contents"
  grep -qx 'README.md' <<<"$contents"
done

nix-shell "$root/shell.nix" --run \
  "cd '$root/crates/geosolve-demo-web' && env -u NO_COLOR trunk build --release"
