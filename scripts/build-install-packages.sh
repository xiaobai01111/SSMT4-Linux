#!/usr/bin/env bash
# Build Linux packages (.deb/.rpm/.pkg.tar.zst) and portable artifacts
# (.AppImage when available, otherwise .AppDir.tar.gz fallback), then copy
# them to the project root "Installation package" directory.

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

resolve_appimagetool_bin() {
  local candidate

  if command -v appimagetool >/dev/null 2>&1; then
    command -v appimagetool
    return 0
  fi

  for candidate in \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage-squashfs/usr/bin/appimagetool" \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage/usr/bin/appimagetool"; do
    if [[ -x "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done

  return 1
}

ensure_appimage_runtime() {
  local runtime_dir runtime_file tmp_runtime candidate offset
  runtime_dir="$HOME/.cache/tauri"
  runtime_file="$runtime_dir/runtime-x86_64"

  if [[ -f "$runtime_file" ]] && [[ -r "$runtime_file" ]]; then
    echo "$runtime_file"
    return 0
  fi

  mkdir -p "$runtime_dir" 2>/dev/null || true
  for candidate in \
    "$HOME/.cache/tauri/linuxdeploy-x86_64.AppImage.bak" \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage.bak" \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"; do
    if [[ ! -x "$candidate" ]]; then
      continue
    fi

    offset="$("$candidate" --appimage-offset 2>/dev/null || true)"
    if [[ ! "$offset" =~ ^[0-9]+$ ]] || [[ "$offset" -le 0 ]]; then
      continue
    fi

    if dd if="$candidate" of="$runtime_file" bs=1 count="$offset" status=none 2>/dev/null; then
      chmod +x "$runtime_file" 2>/dev/null || true
      echo "$runtime_file"
      return 0
    fi

    tmp_runtime="$(mktemp /tmp/ssmt4-runtime-x86_64.XXXXXX)"
    if dd if="$candidate" of="$tmp_runtime" bs=1 count="$offset" status=none 2>/dev/null; then
      chmod +x "$tmp_runtime" 2>/dev/null || true
      echo "$tmp_runtime"
      return 0
    fi
    rm -f "$tmp_runtime"
  done

  return 1
}

build_appimage_from_appdir() {
  local appdir="$1"
  local output_file="$2"
  local appimagetool_bin runtime_file rc

  ensure_appdir_root_icon "$appdir"
  appimagetool_bin="$(resolve_appimagetool_bin)" || return 1
  runtime_file="$(ensure_appimage_runtime)" || return 1

  set +e
  ARCH=x86_64 "$appimagetool_bin" --no-appstream --runtime-file "$runtime_file" "$appdir" "$output_file"
  rc=$?
  set -e

  if [[ $rc -eq 0 ]]; then
    return 0
  fi

  if [[ -f "$output_file" ]] && [[ -s "$output_file" ]]; then
    echo "Warning: appimagetool returned non-zero but output exists: $output_file" >&2
    return 0
  fi

  return $rc
}

ensure_appdir_root_icon() {
  local appdir="$1"
  local desktop_file desktop_name icon_name icon_file candidate

  sanitize_appdir_symlinks "$appdir"

  desktop_file="$(find "$appdir/usr/share/applications" -maxdepth 1 -type f -name 'SSMT4-Linux.desktop' | head -n 1)"
  if [[ -z "$desktop_file" ]]; then
    desktop_file="$(find "$appdir/usr/share/applications" -maxdepth 1 -type f -name '*.desktop' | head -n 1)"
  fi
  if [[ -z "$desktop_file" ]]; then
    return 0
  fi

  desktop_name="$(basename "$desktop_file")"
  ln -sfn "usr/share/applications/$desktop_name" "$appdir/$desktop_name"

  icon_name="$(awk -F= '/^Icon=/{print $2; exit}' "$desktop_file" | tr -d '\r')"
  if [[ -z "$icon_name" ]]; then
    return 0
  fi

  for ext in png svg xpm; do
    if [[ -f "$appdir/$icon_name.$ext" ]]; then
      if [[ "$ext" == "png" ]]; then
        ln -sfn "$icon_name.png" "$appdir/.DirIcon"
      fi
      return 0
    fi
  done

  icon_file="$appdir/$icon_name.png"
  for candidate in \
    "$appdir/usr/share/icons/hicolor/256x256/apps/$icon_name.png" \
    "$appdir/usr/share/icons/hicolor/256x256@2/apps/$icon_name.png" \
    "$appdir/usr/share/icons/hicolor/128x128/apps/$icon_name.png" \
    "$appdir/usr/share/icons/hicolor/64x64/apps/$icon_name.png" \
    "$appdir/usr/share/icons/hicolor/48x48/apps/$icon_name.png" \
    "$appdir/usr/share/icons/hicolor/32x32/apps/$icon_name.png"; do
    if [[ -f "$candidate" ]]; then
      cp -f "$candidate" "$icon_file"
      ln -sfn "$icon_name.png" "$appdir/.DirIcon"
      return 0
    fi
  done

  return 0
}

sanitize_appdir_symlinks() {
  local appdir="$1"
  local link target rel

  while IFS= read -r -d '' link; do
    target="$(readlink "$link" || true)"
    if [[ -z "$target" ]]; then
      continue
    fi
    if [[ "$target" == "$appdir/"* ]]; then
      rel="$(realpath --relative-to="$(dirname "$link")" "$target" 2>/dev/null || true)"
      if [[ -z "$rel" ]]; then
        rel="${target#$appdir/}"
      fi
      if [[ -n "$rel" ]]; then
        ln -sfn "$rel" "$link"
      fi
    fi
  done < <(find "$appdir" -type l -print0 2>/dev/null)
}

ensure_linuxdeploy_strip_compat() {
  local cached_strip system_strip

  cached_strip="$HOME/.cache/tauri/linuxdeploy-squashfs/usr/bin/strip"
  if [[ ! -e "$cached_strip" ]]; then
    return 1
  fi

  system_strip="$(command -v strip || true)"
  if [[ -z "$system_strip" ]]; then
    return 1
  fi

  if [[ -L "$cached_strip" ]] && [[ "$(readlink -f "$cached_strip")" == "$system_strip" ]]; then
    return 0
  fi

  ln -sf "$system_strip" "$cached_strip"
}

run_tauri_appimage_build() {
  local tool_bin_dir rc
  tool_bin_dir="$(mktemp -d)"

  if [[ -x "$HOME/.cache/tauri/linuxdeploy-wrapper.sh" ]]; then
    ln -sf "$HOME/.cache/tauri/linuxdeploy-wrapper.sh" "$tool_bin_dir/linuxdeploy"
  fi
  if [[ -x "$ROOT_DIR/scripts/linuxdeploy-plugin-gtk.compat.sh" ]]; then
    ln -sf "$ROOT_DIR/scripts/linuxdeploy-plugin-gtk.compat.sh" "$tool_bin_dir/linuxdeploy-plugin-gtk"
  fi

  if command -v bun >/dev/null 2>&1; then
    PATH="$tool_bin_dir:$PATH" npm run tauri build -- --bundles appimage
  else
    PATH="$tool_bin_dir:$PATH" npm run tauri build -- --bundles appimage --config '{"build":{"beforeBuildCommand":"npx vue-tsc --noEmit && npx vite build"}}'
  fi
  rc=$?

  rm -rf "$tool_bin_dir"
  return $rc
}

if ! command -v npm >/dev/null 2>&1; then
  echo "Error: npm is required" >&2
  exit 1
fi
if ! command -v makepkg >/dev/null 2>&1; then
  echo "Error: makepkg is required for pacman package output" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
rm -f \
  "$OUT_DIR"/*.deb \
  "$OUT_DIR"/*.rpm \
  "$OUT_DIR"/*.AppImage \
  "$OUT_DIR"/*.AppDir*.tar.gz \
  "$OUT_DIR"/*.pkg.tar.*

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

APPIMAGE_FILES=()
TAURI_APPIMAGE_FILES=()
PORTABLE_FALLBACK_FILES=()
APPDIR_BUNDLE_ROOT="$BUNDLE_SEARCH_ROOT/release/bundle/appimage"

echo "==> Attempting portable build (.AppImage)"
# Avoid stale appimage bundle artifacts polluting desktop metadata.
rm -rf "$APPDIR_BUNDLE_ROOT"
set +e
(
  cd "$ROOT_DIR"
  ensure_linuxdeploy_strip_compat >/dev/null 2>&1 || true
  run_tauri_appimage_build
)
APPIMAGE_BUILD_RC=$?
set -e

if [[ $APPIMAGE_BUILD_RC -ne 0 ]]; then
  if ensure_linuxdeploy_strip_compat >/dev/null 2>&1; then
    echo "==> Retrying AppImage build with system strip compatibility"
    set +e
    (
      cd "$ROOT_DIR"
      run_tauri_appimage_build
    )
    APPIMAGE_BUILD_RC=$?
    set -e
  fi
fi

if [[ $APPIMAGE_BUILD_RC -eq 0 ]]; then
  mapfile -t TAURI_APPIMAGE_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.AppImage" 2>/dev/null | sort)
fi

if [[ ${#APPIMAGE_FILES[@]} -eq 0 ]]; then
  mapfile -t APPDIRS < <(find "$APPDIR_BUNDLE_ROOT" -maxdepth 1 -type d -name "*.AppDir" 2>/dev/null | sort)
  if [[ ${#APPDIRS[@]} -gt 0 ]]; then
    echo "==> Converting AppDir to AppImage (offline fallback)"
    for appdir in "${APPDIRS[@]}"; do
      base_name="$(basename "$appdir")"
      app_name="${base_name%.AppDir}"
      manual_appimage="$APPDIR_BUNDLE_ROOT/${app_name}_${VERSION}_amd64.AppImage"
      if build_appimage_from_appdir "$appdir" "$manual_appimage"; then
        APPIMAGE_FILES+=("$manual_appimage")
      else
        echo "Warning: AppDir conversion failed for $appdir" >&2
      fi
    done
  fi
fi

if [[ ${#APPIMAGE_FILES[@]} -eq 0 ]] && [[ ${#TAURI_APPIMAGE_FILES[@]} -gt 0 ]]; then
  echo "Warning: AppDir repack failed, falling back to Tauri-generated AppImage artifacts." >&2
  APPIMAGE_FILES=("${TAURI_APPIMAGE_FILES[@]}")
fi

if [[ ${#APPIMAGE_FILES[@]} -gt 0 ]]; then
  echo "==> Portable artifact ready: AppImage"
  for f in "${APPIMAGE_FILES[@]}"; do
    cp -f "$f" "$OUT_DIR/"
  done
  if [[ "$KEEP_TARGET_BUNDLES" != "1" ]]; then
    for f in "${APPIMAGE_FILES[@]}"; do
      rm -f "$f"
    done
  fi
else
  if [[ $APPIMAGE_BUILD_RC -ne 0 ]]; then
    echo "Warning: AppImage build failed and AppDir conversion was unavailable, falling back to AppDir tarball." >&2
  else
    echo "Warning: AppImage build produced no .AppImage and AppDir conversion failed, falling back to AppDir tarball." >&2
  fi

  mapfile -t APPDIRS < <(find "$APPDIR_BUNDLE_ROOT" -maxdepth 1 -type d -name "*.AppDir" 2>/dev/null | sort)
  if [[ ${#APPDIRS[@]} -gt 0 ]]; then
    for appdir in "${APPDIRS[@]}"; do
      base_name="$(basename "$appdir")"
      app_name="${base_name%.AppDir}"
      fallback_name="${app_name}_${VERSION}_portable.AppDir.tar.gz"
      tar -C "$(dirname "$appdir")" -czf "$OUT_DIR/$fallback_name" "$base_name"
      PORTABLE_FALLBACK_FILES+=("$OUT_DIR/$fallback_name")
    done
    echo "==> Portable artifact ready: AppDir tarball fallback"
  else
    echo "Warning: no AppDir found under $APPDIR_BUNDLE_ROOT, portable artifact skipped." >&2
  fi
fi

echo "==> Building pacman package via makepkg"
APP_BINARY="$ROOT_DIR/src-tauri/target/release/SSMT4-Linux"
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
pkgname=ssmt-linux-bin
pkgver=$ARCH_PKGVER
pkgrel=1
pkgdesc="SSMT4-Linux launcher"
arch=('x86_64')
url='https://github.com/xiaobai/ssmt4'
license=('MIT')
provides=('ssmt4-bin' 'ssmt-linux' 'ssmt4-linux')
conflicts=('ssmt4-bin' 'ssmt-linux')
replaces=('ssmt4-bin')
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
  install -Dm755 "\$_project_root/src-tauri/target/release/SSMT4-Linux" "\$pkgdir/usr/bin/SSMT4-Linux"
  ln -sf SSMT4-Linux "\$pkgdir/usr/bin/SSMT4-linux"
  ln -sf SSMT4-Linux "\$pkgdir/usr/bin/SSMT-Linux"
  ln -sf SSMT4-Linux "\$pkgdir/usr/bin/SSMT-linux"
  ln -sf SSMT4-Linux "\$pkgdir/usr/bin/ssmt-linux"
  ln -sf SSMT4-Linux "\$pkgdir/usr/bin/ssmt4-linux"

  install -dm755 "\$pkgdir/usr/lib/SSMT4-Linux"
  cp -r "\$_project_root/src-tauri/resources/"* "\$pkgdir/usr/lib/SSMT4-Linux/"
  install -Dm644 "\$_project_root/version" "\$pkgdir/usr/lib/SSMT4-Linux/version"
  install -Dm644 "\$_project_root/version-log" "\$pkgdir/usr/lib/SSMT4-Linux/version-log"
  ln -sfn . "\$pkgdir/usr/lib/SSMT4-Linux/resources"

  # Backward compatibility for older hardcoded lookup paths.
  install -dm755 "\$pkgdir/usr/lib/ssmt4"
  ln -sfn ../SSMT4-Linux "\$pkgdir/usr/lib/ssmt4/resources"

  install -Dm644 /dev/stdin "\$pkgdir/usr/share/applications/ssmt4-linux.desktop" <<'DESKTOP'
[Desktop Entry]
Categories=Game;
Comment=SSMT4-Linux Launcher
Exec=SSMT4-Linux
StartupWMClass=SSMT4-Linux
Icon=SSMT4-Linux
Name=SSMT4-Linux
Terminal=false
Type=Application
DESKTOP

  for size in 32x32 128x128; do
    install -Dm644 "\$_project_root/src-tauri/icons/\${size}.png" "\$pkgdir/usr/share/icons/hicolor/\${size}/apps/SSMT4-Linux.png"
  done
  install -Dm644 "\$_project_root/src-tauri/icons/128x128@2x.png" "\$pkgdir/usr/share/icons/hicolor/256x256@2/apps/SSMT4-Linux.png"
}
EOF

(
  cd "$TMP_PKG_DIR"
  makepkg -f --noconfirm --nodeps
)

find "$TMP_PKG_DIR" -maxdepth 1 -type f -name "ssmt-linux-bin-${ARCH_PKGVER}-*.pkg.tar.*" -exec cp -f {} "$OUT_DIR/" \;

echo "==> Packages ready:"
find "$OUT_DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' -o -name '*.AppDir*.tar.gz' -o -name '*.pkg.tar.*' \) -print | sort
