#!/usr/bin/env bash
set -euo pipefail

version="${1:?version is required}"
target="${2:?Rust target is required}"
binary="${3:?binary path is required}"
output_dir="${4:?output directory is required}"

case "$target" in
  aarch64-apple-darwin) artifact_arch="aarch64" ;;
  x86_64-apple-darwin) artifact_arch="x86_64" ;;
  *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
esac

workspace_root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$output_dir"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

release_root="$work_dir/archive/portal-$version-$target"
mkdir -p "$release_root"
cp "$binary" "$release_root/portal"
chmod 0755 "$release_root/portal"
cp "$workspace_root/README.md" "$release_root/README.md"
cp "$workspace_root/LICENSE" "$release_root/LICENSE"
cp "$workspace_root/CHANGELOG.md" "$release_root/CHANGELOG.md"

if [[ -n "${MACOS_APPLICATION_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp \
    --sign "$MACOS_APPLICATION_IDENTITY" "$release_root/portal"
fi

tar -C "$work_dir/archive" -czf \
  "$output_dir/portal-$version-$target.tar.gz" \
  "portal-$version-$target"

package_root="$work_dir/package-root"
mkdir -p "$package_root/usr/local/bin"
cp "$release_root/portal" "$package_root/usr/local/bin/portal"

component_pkg="$work_dir/portal-component.pkg"
pkgbuild \
  --root "$package_root" \
  --identifier com.devinull.portal \
  --version "$version" \
  --install-location / \
  "$component_pkg"

package_name="Portal-$version-$artifact_arch.pkg"
if [[ -n "${MACOS_INSTALLER_IDENTITY:-}" ]]; then
  productsign \
    --sign "$MACOS_INSTALLER_IDENTITY" \
    --timestamp \
    "$component_pkg" "$output_dir/$package_name"
else
  cp "$component_pkg" "$output_dir/$package_name"
fi

echo "Created macOS release assets in $output_dir"
