#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUT_DIR="$ROOT_DIR/Installation package"

if ! command -v yay >/dev/null 2>&1; then
  echo "错误: 未安装 yay，请先安装 yay。" >&2
  exit 1
fi

shopt -s nullglob
pkgs=("$OUT_DIR"/ssmt4-bin-*.pkg.tar.*)
shopt -u nullglob

if [[ ${#pkgs[@]} -eq 0 ]]; then
  echo "未找到本地 pacman 包，开始构建..."
  bash "$ROOT_DIR/scripts/build-install-packages.sh"
  shopt -s nullglob
  pkgs=("$OUT_DIR"/ssmt4-bin-*.pkg.tar.*)
  shopt -u nullglob
fi

if [[ ${#pkgs[@]} -eq 0 ]]; then
  echo "错误: 构建后仍未找到 ssmt4-bin 包。" >&2
  exit 1
fi

latest_pkg="$(ls -1t "${pkgs[@]}" | head -n 1)"
echo "将安装: $latest_pkg"
yay -U "$latest_pkg"
