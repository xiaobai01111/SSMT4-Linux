#!/usr/bin/env bash
# Build Linux installation packages (.deb/.rpm/.pkg.tar.zst) and copy them
# to the project root "Installation package" directory.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT_DIR/Installation package"
VERSION_FILE="$ROOT_DIR/version"

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "Error: version file not found: $VERSION_FILE" >&2
  exit 1
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION" ]]; then
  echo "Error: version file is empty" >&2
  exit 1
fi
KEEP_TARGET_BUNDLES="${KEEP_TARGET_BUNDLES:-0}"
BUNDLE_SEARCH_ROOT="$ROOT_DIR/src-tauri/target"

if ! command -v npm >/dev/null 2>&1; then
  echo "Error: npm is required" >&2
  exit 1
fi
if ! command -v makepkg >/dev/null 2>&1; then
  echo "Error: makepkg is required for pacman package output" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.deb "$OUT_DIR"/*.rpm "$OUT_DIR"/*.pkg.tar.*

echo "==> Syncing version metadata"
bash "$ROOT_DIR/scripts/sync-version.sh"

echo "==> Building deb/rpm via Tauri"
(
  cd "$ROOT_DIR"
  if command -v bun >/dev/null 2>&1; then
    npm run tauri build -- --bundles deb,rpm
  else
    npm run tauri build -- --bundles deb,rpm --config '{"build":{"beforeBuildCommand":"npx vue-tsc --noEmit && npx vite build"}}'
  fi
)

echo "==> Collecting deb/rpm artifacts"
mapfile -t DEB_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.deb" 2>/dev/null | sort)
mapfile -t RPM_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.rpm" 2>/dev/null | sort)

if [[ ${#DEB_FILES[@]} -eq 0 ]]; then
  echo "Error: no .deb artifact found for version $VERSION under $BUNDLE_SEARCH_ROOT" >&2
  exit 1
fi
if [[ ${#RPM_FILES[@]} -eq 0 ]]; then
  echo "Error: no .rpm artifact found for version $VERSION under $BUNDLE_SEARCH_ROOT" >&2
  exit 1
fi

for f in "${DEB_FILES[@]}"; do
  cp -f "$f" "$OUT_DIR/"
done
for f in "${RPM_FILES[@]}"; do
  cp -f "$f" "$OUT_DIR/"
done

if [[ "$KEEP_TARGET_BUNDLES" != "1" ]]; then
  echo "==> Cleaning target bundle artifacts (.deb/.rpm)"
  for f in "${DEB_FILES[@]}"; do
    rm -f "$f"
  done
  for f in "${RPM_FILES[@]}"; do
    rm -f "$f"
  done
fi

echo "==> Building pacman package via makepkg"
APP_BINARY="$ROOT_DIR/src-tauri/target/release/SSMT4-linux"
if [[ ! -x "$APP_BINARY" ]]; then
  echo "Error: built binary not found: $APP_BINARY" >&2
  exit 1
fi

TMP_PKG_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_PKG_DIR"
}
trap cleanup EXIT

ARCH_PKGVER="${VERSION//-/_}"

cat > "$TMP_PKG_DIR/PKGBUILD" <<EOF
pkgname=ssmt4-bin
pkgver=$ARCH_PKGVER
pkgrel=1
pkgdesc="SSMT4 Linux launcher"
arch=('x86_64')
url='https://github.com/xiaobai/ssmt4'
license=('MIT')
options=('!debug')
depends=('gtk3' 'webkit2gtk-4.1' 'libsoup3' 'xdg-utils')
optdepends=(
  'xorg-xwayland: XWayland support'
  'wine: Windows game compatibility'
  'winetricks: Wine helper scripts'
  'libayatana-appindicator: tray icon support'
)
_project_root='$ROOT_DIR'

package() {
  install -Dm755 "\$_project_root/src-tauri/target/release/SSMT4-linux" "\$pkgdir/usr/bin/SSMT4-linux"

  install -dm755 "\$pkgdir/usr/lib/ssmt4/resources"
  cp -r "\$_project_root/src-tauri/resources/"* "\$pkgdir/usr/lib/ssmt4/resources/"
  install -Dm644 "\$_project_root/version" "\$pkgdir/usr/lib/ssmt4/resources/version"
  install -Dm644 "\$_project_root/version-log" "\$pkgdir/usr/lib/ssmt4/resources/version-log"

  install -Dm644 /dev/stdin "\$pkgdir/usr/share/applications/ssmt4.desktop" <<'DESKTOP'
[Desktop Entry]
Categories=Game;
Comment=SSMT4 Linux Launcher
Exec=SSMT4-linux
StartupWMClass=SSMT4-linux
Icon=SSMT4-linux
Name=SSMT4
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

find "$TMP_PKG_DIR" -maxdepth 1 -type f -name "ssmt4-bin-${ARCH_PKGVER}-*.pkg.tar.*" -exec cp -f {} "$OUT_DIR/" \;

echo "==> Packages ready:"
find "$OUT_DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' \) -print | sort
