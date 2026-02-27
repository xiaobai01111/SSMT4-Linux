#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUR_DIR="$ROOT_DIR/aur/ssmt4-linux"
VERSION_FILE="$ROOT_DIR/version"
PKGBUILD_FILE="$AUR_DIR/PKGBUILD"
OUT_DIR="$ROOT_DIR/Installation package"

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

# 读取 pkgrel，并优先使用本地已构建产物写入固定 SHA256（AUR 推荐）
pkgrel="$(sed -n 's/^pkgrel=//p' "$PKGBUILD_FILE" | head -n1)"
pkgrel="${pkgrel:-1}"

shopt -s nullglob
local_pkgs=("$OUT_DIR"/ssmt4-linux-"${pkgver}"-"${pkgrel}"-x86_64.pkg.tar.*)
shopt -u nullglob

if [[ ${#local_pkgs[@]} -gt 0 ]]; then
  pkg_file="$(ls -1t "${local_pkgs[@]}" | head -n1)"
  sha="$(sha256sum "$pkg_file" | awk '{print $1}')"
  sed -i "s/^sha256sums=.*/sha256sums=('${sha}')/" "$PKGBUILD_FILE"
  echo "检测到本地产物并写入 SHA256: $pkg_file"
else
  sed -i "s/^sha256sums=.*/sha256sums=('SKIP')/" "$PKGBUILD_FILE"
  echo "警告: 未找到本地产物，暂使用 SKIP。发布前建议先构建并重新执行本脚本以写入固定校验和。"
fi

(
  cd "$AUR_DIR"
  makepkg --printsrcinfo > .SRCINFO
)

echo "AUR 元数据已更新:"
echo "  pkgver=${pkgver}"
echo "  files: $PKGBUILD_FILE, $AUR_DIR/.SRCINFO"
