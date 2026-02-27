#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_DIR="$ROOT_DIR/aur/ssmt4-linux"

if [[ $# -ne 1 ]]; then
  echo "用法: $0 <AUR仓库本地路径>" >&2
  echo "示例: $0 /home/xiaobai/dev/aur/ssmt4-linux" >&2
  exit 1
fi

AUR_REPO_DIR="$1"

if [[ ! -d "$AUR_REPO_DIR/.git" ]]; then
  echo "错误: 目标目录不是 Git 仓库: $AUR_REPO_DIR" >&2
  exit 1
fi

for f in PKGBUILD .SRCINFO; do
  if [[ ! -f "$SRC_DIR/$f" ]]; then
    echo "错误: 缺少源文件: $SRC_DIR/$f" >&2
    exit 1
  fi
done

cp -f "$SRC_DIR/PKGBUILD" "$AUR_REPO_DIR/PKGBUILD"
cp -f "$SRC_DIR/.SRCINFO" "$AUR_REPO_DIR/.SRCINFO"

echo "已同步 AUR 文件到: $AUR_REPO_DIR"
echo "下一步："
echo "  cd \"$AUR_REPO_DIR\""
echo "  git add PKGBUILD .SRCINFO"
echo "  git commit -m \"ssmt4-linux: update to $(sed -n 's/^pkgver=//p' "$SRC_DIR/PKGBUILD" | head -n1)-$(sed -n 's/^pkgrel=//p' "$SRC_DIR/PKGBUILD" | head -n1)\""
echo "  git push"
