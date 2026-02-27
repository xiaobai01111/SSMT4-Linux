#!/usr/bin/env bash
# Publish AUR package: update PKGBUILD metadata, regenerate .SRCINFO,
# sync to local AUR repo clone, commit and push to AUR remote.
# Usage: bash scripts/aur/publish-aur.sh
# Env:   AUR_REPO_DIR  – override local AUR repo path
#        SKIP_PUSH=1   – commit but do not push

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
AUR_DIR="$ROOT_DIR/aur/ssmt4-linux"
VERSION_FILE="$ROOT_DIR/version"
OUT_DIR="$ROOT_DIR/Installation package"
PKGBUILD_FILE="$AUR_DIR/PKGBUILD"
PKG_NAME="ssmt4-linux"
AUR_REPO_DIR="${AUR_REPO_DIR:-$HOME/dev/aur/$PKG_NAME}"
SKIP_PUSH="${SKIP_PUSH:-0}"

# ── 前置检查 ──────────────────────────────────────────

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 未找到版本文件: $VERSION_FILE" >&2
  exit 1
fi

VERSION_RAW="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION_RAW" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

if [[ ! -f "$PKGBUILD_FILE" ]]; then
  echo "错误: 未找到 PKGBUILD: $PKGBUILD_FILE" >&2
  exit 1
fi

if ! command -v makepkg >/dev/null 2>&1; then
  echo "错误: makepkg 未安装" >&2
  exit 1
fi

if [[ ! -d "$AUR_REPO_DIR/.git" ]]; then
  echo "错误: AUR 本地仓库不存在: $AUR_REPO_DIR" >&2
  echo "请先运行: bash scripts/aur/init-aur-repo.sh" >&2
  exit 1
fi

PKGVER="${VERSION_RAW//-/_}"
PKGREL="$(sed -n 's/^pkgrel=//p' "$PKGBUILD_FILE" | head -n1)"
PKGREL="${PKGREL:-1}"

echo "==> 版本信息"
echo "    version : $VERSION_RAW"
echo "    pkgver  : $PKGVER"
echo "    pkgrel  : $PKGREL"

# ── 1. 更新 PKGBUILD pkgver ──────────────────────────

echo "==> 更新 PKGBUILD 版本号"
sed -i "s/^pkgver=.*/pkgver=${PKGVER}/" "$PKGBUILD_FILE"

# ── 2. 计算 SHA256（优先本地产物，否则 SKIP） ────────

echo "==> 计算 SHA256 校验和"
shopt -s nullglob
local_pkgs=("$OUT_DIR"/${PKG_NAME}-"${PKGVER}"-"${PKGREL}"-x86_64.pkg.tar.*)
shopt -u nullglob

if [[ ${#local_pkgs[@]} -gt 0 ]]; then
  pkg_file="$(ls -1t "${local_pkgs[@]}" | head -n1)"
  sha="$(sha256sum "$pkg_file" | awk '{print $1}')"
  sed -i "s/^sha256sums=.*/sha256sums=('${sha}')/" "$PKGBUILD_FILE"
  echo "    已写入 SHA256: $(basename "$pkg_file")"
  echo "    $sha"
else
  sed -i "s/^sha256sums=.*/sha256sums=('SKIP')/" "$PKGBUILD_FILE"
  echo "    警告: 未找到本地产物，暂使用 SKIP"
  echo "    产物路径: $OUT_DIR/${PKG_NAME}-${PKGVER}-${PKGREL}-x86_64.pkg.tar.*"
fi

# ── 3. 重新生成 .SRCINFO ─────────────────────────────

echo "==> 生成 .SRCINFO"
(
  cd "$AUR_DIR"
  makepkg --printsrcinfo > .SRCINFO
)
echo "    完成"

# ── 4. 同步到本地 AUR 仓库 ───────────────────────────

echo "==> 同步文件到 AUR 本地仓库: $AUR_REPO_DIR"
for f in PKGBUILD .SRCINFO; do
  if [[ ! -f "$AUR_DIR/$f" ]]; then
    echo "错误: 缺少源文件: $AUR_DIR/$f" >&2
    exit 1
  fi
  cp -f "$AUR_DIR/$f" "$AUR_REPO_DIR/$f"
done
echo "    已复制 PKGBUILD, .SRCINFO"

# ── 5. Diff 预览 ─────────────────────────────────────

echo "==> 变更预览"
(
  cd "$AUR_REPO_DIR"
  if git diff --quiet PKGBUILD .SRCINFO 2>/dev/null; then
    echo "    无变更，跳过提交。"
    exit 0
  fi
  git diff --stat PKGBUILD .SRCINFO 2>/dev/null || true
)

# ── 6. 提交 ──────────────────────────────────────────

COMMIT_MSG="${PKG_NAME}: update to ${PKGVER}-${PKGREL}"

echo "==> 提交: $COMMIT_MSG"
(
  cd "$AUR_REPO_DIR"
  git add PKGBUILD .SRCINFO
  git commit -m "$COMMIT_MSG"
)

# ── 7. 推送 ──────────────────────────────────────────

if [[ "$SKIP_PUSH" == "1" ]]; then
  echo "==> SKIP_PUSH=1，跳过推送"
  echo "    手动推送: cd \"$AUR_REPO_DIR\" && git push"
else
  echo "==> 推送到 AUR"
  (
    cd "$AUR_REPO_DIR"
    git push
  )
fi

echo ""
echo "==> AUR 发布完成！"
echo "    包名  : $PKG_NAME"
echo "    版本  : $PKGVER-$PKGREL"
echo "    仓库  : $AUR_REPO_DIR"
echo "    页面  : https://aur.archlinux.org/packages/${PKG_NAME}"
