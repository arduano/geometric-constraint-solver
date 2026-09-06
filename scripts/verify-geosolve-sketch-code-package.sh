#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

package_name="geosolve-sketch-code"
package_id="$(cargo pkgid -p "$package_name")"
package_version="${package_id##*#}"
package_version="${package_version##*@}"
cargo_target_dir="$(cargo metadata --locked --offline --format-version 1 --no-deps | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')"
archive="$cargo_target_dir/package/$package_name-$package_version.crate"
staging_parent="${TMPDIR:-/tmp}"
case "$staging_parent" in
  /*) ;;
  *)
    echo "TMPDIR must be an absolute path: $staging_parent" >&2
    exit 1
    ;;
esac
staging_parent="$(cd "$staging_parent" && pwd -P)"
staging_dir="$(mktemp -d "$staging_parent/geosolve-sketch-code-package.XXXXXX")"

cleanup() {
  case "$staging_dir" in
    "$staging_parent"/geosolve-sketch-code-package.*) rm -rf -- "$staging_dir" ;;
    *) return 1 ;;
  esac
}
trap cleanup EXIT

contents="$(cargo package --locked --allow-dirty --list -p "$package_name")"
for required in \
  assets/bootstrap/authored-empty.compiled.json \
  assets/bootstrap/authored-empty.sketch.ts \
  assets/bundled-samples/README.md \
  assets/bundled-samples/theo-jansen-leg/manifest.json \
  assets/bundled-samples/theo-jansen-leg/sketch.compiled.json \
  assets/bundled-samples/theo-jansen-leg/sketch.ts \
  assets/bundled-samples/theo-jansen-leg/witnesses.json \
  assets/bundled-samples/pc-water-manifold/patches/water-channel.artifact.json \
  assets/bundled-samples/pc-water-manifold/patches/water-channel.patch.ts \
  assets/bundled-samples/gridfinity-bin-section/NOTICE.md \
  assets/bundled-samples/robotic-harness-backplane/sketch.ts
do
  grep -Fqx "$required" <<<"$contents"
done

if grep -Eq '^assets/(demos|samples|artifacts|patches)/' <<<"$contents"; then
  echo "retired flat sample assets remain in the package" >&2
  exit 1
fi

local_dependencies=(
  geosolve-constraint-editor
  geosolve-sketch
  geosolve-sketch-features
  geosolve-sketch-intent
)

manifest_local_dependencies="$({
  sed -nE \
    's/^([a-zA-Z0-9_-]+)[[:space:]]*=.*path[[:space:]]*=[[:space:]]*"\.\.\/[^\"]+".*$/\1/p' \
    "$repo_root/crates/$package_name/Cargo.toml"
} | LC_ALL=C sort)"
patched_local_dependencies="$(printf '%s\n' "${local_dependencies[@]}" | LC_ALL=C sort)"
if [[ "$manifest_local_dependencies" != "$patched_local_dependencies" ]]; then
  echo "package verifier patches do not match direct local dependencies" >&2
  diff -u \
    <(printf '%s\n' "$manifest_local_dependencies") \
    <(printf '%s\n' "$patched_local_dependencies") >&2 || true
  exit 1
fi

patches=()
for dependency in "${local_dependencies[@]}"; do
  patches+=(
    --config "patch.crates-io.$dependency.path=\"$repo_root/crates/$dependency\""
  )
done

# Direct GeoSolve dependencies are not published yet. The patches let Cargo
# construct the real normalized archive while retaining local dependency
# authority; the extracted code crate itself has no workspace-relative asset.
cargo package --locked --allow-dirty --no-verify -p "$package_name" "${patches[@]}"
tar -xzf "$archive" -C "$staging_dir"
# Reuse dependency compilation across independently extracted archives. Cargo still
# checks this newly extracted package and its exact normalized manifest every time.
cargo check \
  --target-dir "$cargo_target_dir/package-verification" \
  --locked \
  --offline \
  --manifest-path "$staging_dir/$package_name-$package_version/Cargo.toml" \
  "${patches[@]}"
