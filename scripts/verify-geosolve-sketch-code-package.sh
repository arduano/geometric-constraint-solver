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
archive="$repo_root/target/package/$package_name-$package_version.crate"
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
  assets/artifacts/adaptive-lanterns.artifact.json \
  assets/artifacts/bridge-cables.artifact.json \
  assets/artifacts/compass-core.artifact.json \
  assets/artifacts/corner-reliefs.artifact.json \
  assets/artifacts/cross-brace.artifact.json \
  assets/artifacts/fillet-record.artifact.json \
  assets/artifacts/harness-route.artifact.json \
  assets/artifacts/mounting-plate.artifact.json \
  assets/artifacts/round-every-corner.artifact.json \
  assets/artifacts/water-channel.artifact.json \
  assets/demos/cnc-joinery-fit-coupon.sketch.ts \
  assets/demos/gridfinity-1x1x3-section.NOTICE.md \
  assets/demos/gridfinity-1x1x3-section.sketch.ts \
  assets/demos/pc-water-manifold.sketch.ts \
  assets/demos/robotic-routing-board.sketch.ts \
  assets/patches/adaptive-lanterns.patch.ts \
  assets/patches/braced-frame.patch.ts \
  assets/patches/bridge-cables.patch.ts \
  assets/patches/compass-core.patch.ts \
  assets/patches/corner-reliefs.patch.ts \
  assets/patches/harness-route.patch.ts \
  assets/patches/mounting-plate.patch.ts \
  assets/patches/rounded-polyline.patch.ts \
  assets/patches/typed-panel.patch.ts \
  assets/patches/water-channel.patch.ts
do
  grep -Fqx "$required" <<<"$contents"
done

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
cargo check \
  --locked \
  --offline \
  --manifest-path "$staging_dir/$package_name-$package_version/Cargo.toml" \
  "${patches[@]}"
