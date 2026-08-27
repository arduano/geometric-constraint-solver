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
staging_dir="$(mktemp -d /tmp/geosolve-sketch-code-package.XXXXXX)"

cleanup() {
  case "$staging_dir" in
    /tmp/geosolve-sketch-code-package.*) rm -rf -- "$staging_dir" ;;
    *) return 1 ;;
  esac
}
trap cleanup EXIT

contents="$(cargo package --locked --allow-dirty --list -p "$package_name")"
for required in \
  assets/artifacts/adaptive-lanterns.artifact.json \
  assets/artifacts/bridge-cables.artifact.json \
  assets/artifacts/compass-core.artifact.json \
  assets/artifacts/cross-brace.artifact.json \
  assets/artifacts/fillet-record.artifact.json \
  assets/artifacts/mounting-plate.artifact.json \
  assets/artifacts/round-every-corner.artifact.json \
  assets/artifacts/water-channel.artifact.json \
  assets/demos/pc-water-manifold.sketch.ts \
  assets/patches/adaptive-lanterns.patch.ts \
  assets/patches/braced-frame.patch.ts \
  assets/patches/bridge-cables.patch.ts \
  assets/patches/compass-core.patch.ts \
  assets/patches/mounting-plate.patch.ts \
  assets/patches/rounded-polyline.patch.ts \
  assets/patches/typed-panel.patch.ts \
  assets/patches/water-channel.patch.ts
do
  grep -Fqx "$required" <<<"$contents"
done

patches=(
  --config "patch.crates-io.geosolve-constraint-editor.path=\"$repo_root/crates/geosolve-constraint-editor\""
  --config "patch.crates-io.geosolve-sketch.path=\"$repo_root/crates/geosolve-sketch\""
  --config "patch.crates-io.geosolve-sketch-intent.path=\"$repo_root/crates/geosolve-sketch-intent\""
)

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
