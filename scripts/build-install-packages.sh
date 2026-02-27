#!/usr/bin/env bash
# 构建 Linux 安装包（deb/rpm/pkg.tar.zst），输出到项目根目录 Installation package/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT_DIR/Installation package"
VERSION_FILE="$ROOT_DIR/version"
BUNDLE_SEARCH_ROOT="$ROOT_DIR/src-tauri/target"
KEEP_TARGET_BUNDLES="${KEEP_TARGET_BUNDLES:-0}"

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 未找到版本文件: $VERSION_FILE" >&2
  exit 1
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "错误: 未找到 npm，请先安装 Node.js/npm" >&2
  exit 1
fi
if ! command -v makepkg >/dev/null 2>&1; then
  echo "错误: 未找到 makepkg（pacman 打包必需）" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
rm -f \
  "$OUT_DIR"/*.deb \
  "$OUT_DIR"/*.rpm \
  "$OUT_DIR"/*.pkg.tar.*

echo "==> 同步版本号"
bash "$ROOT_DIR/scripts/sync-version.sh"

echo "==> 构建 deb/rpm（Tauri）"
(
  cd "$ROOT_DIR"
  if command -v bun >/dev/null 2>&1; then
    npm run tauri build -- --bundles deb,rpm
  else
    npm run tauri build -- --bundles deb,rpm --config '{"build":{"beforeBuildCommand":"npx vue-tsc --noEmit && npx vite build"}}'
  fi
)

echo "==> 收集 deb/rpm 产物"
mapfile -t DEB_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.deb" 2>/dev/null | sort)
mapfile -t RPM_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.rpm" 2>/dev/null | sort)

if [[ ${#DEB_FILES[@]} -eq 0 ]]; then
  echo "错误: 未找到版本 $VERSION 对应的 .deb 产物（搜索目录: $BUNDLE_SEARCH_ROOT）" >&2
  exit 1
fi
if [[ ${#RPM_FILES[@]} -eq 0 ]]; then
  echo "错误: 未找到版本 $VERSION 对应的 .rpm 产物（搜索目录: $BUNDLE_SEARCH_ROOT）" >&2
  exit 1
fi

for f in "${DEB_FILES[@]}"; do
  cp -f "$f" "$OUT_DIR/"
done
for f in "${RPM_FILES[@]}"; do
  cp -f "$f" "$OUT_DIR/"
done

if [[ "$KEEP_TARGET_BUNDLES" != "1" ]]; then
  echo "==> 清理 target 中的 deb/rpm 产物"
  for f in "${DEB_FILES[@]}"; do
    rm -f "$f"
  done
  for f in "${RPM_FILES[@]}"; do
    rm -f "$f"
  done
fi

echo "==> 构建 pacman 包（makepkg）"
APP_BINARY="$ROOT_DIR/src-tauri/target/release/SSMT4-linux"
if [[ ! -x "$APP_BINARY" ]]; then
  echo "错误: 未找到编译后的二进制: $APP_BINARY" >&2
  exit 1
fi

TMP_PKG_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_PKG_DIR"
}
trap cleanup EXIT

ARCH_PKGVER="${VERSION//-/_}"

cat > "$TMP_PKG_DIR/PKGBUILD" <<EOF
pkgname=ssmt4-linux
pkgver=$ARCH_PKGVER
pkgrel=1
pkgdesc="SSMT4 - Super Simple Linux Game Tools 4th"
arch=('x86_64')
url='https://github.com/xiaobai01111/SSMT4-Linux'
license=('GPL-3.0-or-later')
options=('!debug')
depends=('gtk3' 'webkit2gtk-4.1' 'libsoup3' 'xdg-utils')
optdepends=(
  'xorg-xwayland: XWayland support'
  'wine: Windows game compatibility'
  'winetricks: Wine helper scripts'
  'umu-launcher: umu-run runtime launcher'
  'bubblewrap: sandbox mode (bwrap)'
  'vulkan-tools: Vulkan diagnostics (vulkaninfo)'
  'pciutils: GPU detection (lspci)'
  '7zip: split archive extraction'
  'unzip: Proton archive extraction'
  'git: Data-parameters repository sync'
  'polkit: privileged telemetry host edits (pkexec)'
  'procps-ng: process monitoring (ps/pgrep)'
  'libayatana-appindicator: tray icon support'
  'wayland: Wayland support'
)
_project_root='$ROOT_DIR'

package() {
  install -Dm755 "\$_project_root/src-tauri/target/release/SSMT4-linux" "\$pkgdir/usr/bin/SSMT4-linux"

  install -dm755 "\$pkgdir/usr/lib/ssmt4/resources"
  if [[ -d "\$_project_root/src-tauri/resources" ]]; then
    cp -r "\$_project_root/src-tauri/resources/"* "\$pkgdir/usr/lib/ssmt4/resources/" 2>/dev/null || true
  fi
  install -Dm644 "\$_project_root/version" "\$pkgdir/usr/lib/ssmt4/resources/version"
  install -Dm644 "\$_project_root/version-log" "\$pkgdir/usr/lib/ssmt4/resources/version-log"

  install -Dm644 /dev/stdin "\$pkgdir/usr/share/applications/ssmt4-linux.desktop" <<'DESKTOP'
[Desktop Entry]
Categories=Game;
Comment=SSMT4 Linux Launcher
Exec=SSMT4-linux
StartupWMClass=SSMT4-linux
Icon=SSMT4-linux
Name=SSMT4 Linux
Terminal=false
Type=Application
DESKTOP

  for size in 32x32 128x128; do
    install -Dm644 "\$_project_root/src-tauri/icons/\${size}.png" "\$pkgdir/usr/share/icons/hicolor/\${size}/apps/SSMT4-linux.png"
  done
  install -Dm644 "\$_project_root/src-tauri/icons/128x128@2x.png" "\$pkgdir/usr/share/icons/hicolor/256x256@2/apps/SSMT4-linux.png"
}
EOF

(
  cd "$TMP_PKG_DIR"
  makepkg -f --noconfirm --nodeps
)

find "$TMP_PKG_DIR" -maxdepth 1 -type f -name "ssmt4-linux-${ARCH_PKGVER}-*.pkg.tar.*" -exec cp -f {} "$OUT_DIR/" \;

echo "==> 打包完成，产物列表："
find "$OUT_DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' \) -print | sort
