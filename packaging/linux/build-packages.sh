#!/usr/bin/env bash
set -euo pipefail

version="${1:?version is required}"
target="${2:?Rust target is required}"
binary="${3:?binary path is required}"
output_dir="${4:?output directory is required}"

case "$target" in
  x86_64-unknown-linux-gnu)
    deb_arch="amd64"
    rpm_arch="x86_64"
    ;;
  aarch64-unknown-linux-gnu)
    deb_arch="arm64"
    rpm_arch="aarch64"
    ;;
  *) echo "Unsupported Linux target: $target" >&2; exit 1 ;;
esac

workspace_root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$output_dir"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

archive_root="$work_dir/archive/portal-$version-$target"
mkdir -p "$archive_root"
cp "$binary" "$archive_root/portal"
chmod 0755 "$archive_root/portal"
cp "$workspace_root/README.md" "$archive_root/README.md"
cp "$workspace_root/LICENSE" "$archive_root/LICENSE"
cp "$workspace_root/CHANGELOG.md" "$archive_root/CHANGELOG.md"
tar -C "$work_dir/archive" -czf \
  "$output_dir/portal-$version-$target.tar.gz" \
  "portal-$version-$target"

deb_root="$work_dir/deb"
mkdir -p "$deb_root/DEBIAN" "$deb_root/usr/bin" "$deb_root/usr/share/doc/portal"
install -m 0755 "$binary" "$deb_root/usr/bin/portal"
install -m 0644 "$workspace_root/README.md" "$deb_root/usr/share/doc/portal/README.md"
install -m 0644 "$workspace_root/LICENSE" "$deb_root/usr/share/doc/portal/copyright"
cat > "$deb_root/DEBIAN/control" <<EOF
Package: portal
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: Portal contributors <devinull@users.noreply.github.com>
Description: A portal to any machine, from any terminal
 Portal provides an interactive terminal interface for desktop peers on a
 Tailscale network and opens RustDesk desktop or file-transfer sessions.
EOF
dpkg-deb --root-owner-group --build "$deb_root" \
  "$output_dir/portal_${version}_${deb_arch}.deb"

rpm_root="$work_dir/rpmbuild"
mkdir -p "$rpm_root/BUILD" "$rpm_root/BUILDROOT" "$rpm_root/RPMS" \
  "$rpm_root/SOURCES" "$rpm_root/SPECS" "$rpm_root/SRPMS"
cp "$binary" "$rpm_root/SOURCES/portal"
sed "s/@VERSION@/$version/g" \
  "$workspace_root/packaging/linux/portal.spec.in" \
  > "$rpm_root/SPECS/portal.spec"
rpmbuild -bb \
  --define "_topdir $rpm_root" \
  --target "$rpm_arch" \
  "$rpm_root/SPECS/portal.spec"
find "$rpm_root/RPMS" -type f -name '*.rpm' -exec cp {} "$output_dir/" \;

echo "Created Linux release assets in $output_dir"
