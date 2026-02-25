#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUR_DIR="$ROOT_DIR/aur/ssmt4-bin"
VERSION_FILE="$ROOT_DIR/version"
PKGBUILD_FILE="$AUR_DIR/PKGBUILD"

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 未找到版本文件: $VERSION_FILE" >&2
  exit 1
fi

if [[ ! -f "$PKGBUILD_FILE" ]]; then
  echo "错误: 未找到 PKGBUILD: $PKGBUILD_FILE" >&2
  exit 1
fi

version_raw="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$version_raw" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

pkgver="${version_raw//-/_}"
sed -i "s/^pkgver=.*/pkgver=${pkgver}/" "$PKGBUILD_FILE"

(
  cd "$AUR_DIR"
  makepkg --printsrcinfo > .SRCINFO
)

echo "AUR 元数据已更新:"
echo "  pkgver=${pkgver}"
echo "  files: $PKGBUILD_FILE, $AUR_DIR/.SRCINFO"
