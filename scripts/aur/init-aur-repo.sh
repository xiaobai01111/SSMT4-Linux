#!/usr/bin/env bash
# 初始化 AUR 仓库 ssmt4-linux
# 用法: bash scripts/aur/init-aur-repo.sh [本地目录]
# 示例: bash scripts/aur/init-aur-repo.sh ~/dev/aur/ssmt4-linux
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
SRC_DIR="$ROOT_DIR/aur/ssmt4-linux"
PKG_NAME="ssmt4-linux"
AUR_REMOTE="ssh://aur@aur.archlinux.org/${PKG_NAME}.git"

CLONE_DIR="${1:-$HOME/dev/aur/$PKG_NAME}"

if [[ -d "$CLONE_DIR/.git" ]]; then
  echo "AUR 仓库已存在: $CLONE_DIR"
  echo "如需重新初始化，请先删除该目录。"
  exit 1
fi

echo "==> 克隆 AUR 仓库（首次推送会自动创建）"
mkdir -p "$(dirname "$CLONE_DIR")"
GIT_SSH_COMMAND="ssh -i ~/.ssh/aur -o IdentitiesOnly=yes" \
  git clone "$AUR_REMOTE" "$CLONE_DIR" 2>/dev/null || {
    echo "==> 远程仓库为空或不存在，手动初始化本地仓库"
    mkdir -p "$CLONE_DIR"
    git -C "$CLONE_DIR" init
    git -C "$CLONE_DIR" remote add origin "$AUR_REMOTE"
  }

echo "==> 复制 PKGBUILD 和 .SRCINFO"
for f in PKGBUILD .SRCINFO; do
  if [[ ! -f "$SRC_DIR/$f" ]]; then
    echo "错误: 缺少源文件: $SRC_DIR/$f" >&2
    exit 1
  fi
  cp -f "$SRC_DIR/$f" "$CLONE_DIR/$f"
done

echo "==> 提交并推送到 AUR"
git -C "$CLONE_DIR" add PKGBUILD .SRCINFO
git -C "$CLONE_DIR" commit -m "Initial commit: ${PKG_NAME} $(sed -n 's/^pkgver=//p' "$SRC_DIR/PKGBUILD" | head -n1)-$(sed -n 's/^pkgrel=//p' "$SRC_DIR/PKGBUILD" | head -n1)"
GIT_SSH_COMMAND="ssh -i ~/.ssh/aur -o IdentitiesOnly=yes" \
  git -C "$CLONE_DIR" push -u origin master

echo ""
echo "AUR 仓库初始化完成！"
echo "  本地路径: $CLONE_DIR"
echo "  AUR 页面: https://aur.archlinux.org/packages/${PKG_NAME}"
