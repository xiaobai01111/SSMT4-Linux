#!/usr/bin/env bash
# 构建 Linux 安装包（deb/rpm/pkg.tar.zst）和便携包（AppImage），输出到 Installation package/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$ROOT_DIR/Installation package"
VERSION_FILE="$ROOT_DIR/version"
BUNDLE_SEARCH_ROOT="$ROOT_DIR/src-tauri/target"
KEEP_TARGET_BUNDLES="${KEEP_TARGET_BUNDLES:-0}"
TAURI_CACHE_HOME="${TAURI_CACHE_HOME:-$ROOT_DIR/.cache/tauri-build}"
APPIMAGE_ONLY=0
APPIMAGE_RETRY_PURGE_CACHE="${APPIMAGE_RETRY_PURGE_CACHE:-0}"
APPIMAGE_MAX_RETRY="${APPIMAGE_MAX_RETRY:-1}"

for arg in "$@"; do
  case "$arg" in
    --appimage-only)
      APPIMAGE_ONLY=1
      ;;
    *)
      echo "错误: 未知参数: $arg" >&2
      echo "用法: $0 [--appimage-only]" >&2
      exit 1
      ;;
  esac
done

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 未找到版本文件: $VERSION_FILE" >&2
  exit 1
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

run_tauri_bundle_build() {
  local bundles="$1"
  mkdir -p "$TAURI_CACHE_HOME"
  if command -v bun >/dev/null 2>&1; then
    XDG_CACHE_HOME="$TAURI_CACHE_HOME" npm run tauri build -- --bundles "$bundles"
  else
    XDG_CACHE_HOME="$TAURI_CACHE_HOME" npm run tauri build -- --bundles "$bundles" --config '{"build":{"beforeBuildCommand":"npx vue-tsc --noEmit && npx vite build"}}'
  fi
}

resolve_appimagetool_bin() {
  local candidate plugin_appimage extract_dir

  if command -v appimagetool >/dev/null 2>&1; then
    command -v appimagetool
    return 0
  fi

  for candidate in \
    "$TAURI_CACHE_HOME/tauri/linuxdeploy-plugin-appimage-squashfs/usr/bin/appimagetool" \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage-squashfs/usr/bin/appimagetool"; do
    if [[ -x "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done

  extract_dir="$TAURI_CACHE_HOME/tauri/linuxdeploy-plugin-appimage-squashfs"
  for plugin_appimage in \
    "$TAURI_CACHE_HOME/tauri/linuxdeploy-plugin-appimage.AppImage" \
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"; do
    if [[ ! -f "$plugin_appimage" ]]; then
      continue
    fi
    chmod +x "$plugin_appimage" 2>/dev/null || true
    mkdir -p "$TAURI_CACHE_HOME/tauri"
    rm -rf "$extract_dir"
    (
      cd "$TAURI_CACHE_HOME/tauri"
      "$plugin_appimage" --appimage-extract >/dev/null
      mv squashfs-root "$extract_dir"
    ) || continue
    candidate="$extract_dir/usr/bin/appimagetool"
    if [[ -x "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done

  return 1
}

resolve_appimage_runtime() {
  local candidate runtime_file appimage offset

  for candidate in \
    "$TAURI_CACHE_HOME/tauri/runtime-x86_64" \
    "$HOME/.cache/tauri/runtime-x86_64"; do
    if [[ -f "$candidate" ]] && [[ -r "$candidate" ]]; then
      echo "$candidate"
      return 0
    fi
  done

  mkdir -p "$TAURI_CACHE_HOME/tauri"
  runtime_file="$TAURI_CACHE_HOME/tauri/runtime-x86_64"
  for appimage in \
    "$TAURI_CACHE_HOME/tauri/linuxdeploy-x86_64.AppImage" \
    "$HOME/.cache/tauri/linuxdeploy-x86_64.AppImage"; do
    if [[ ! -x "$appimage" ]]; then
      continue
    fi
    offset="$("$appimage" --appimage-offset 2>/dev/null || true)"
    if [[ "$offset" =~ ^[0-9]+$ ]] && [[ "$offset" -gt 0 ]]; then
      if dd if="$appimage" of="$runtime_file" bs=1 count="$offset" status=none 2>/dev/null; then
        chmod +x "$runtime_file" || true
        echo "$runtime_file"
        return 0
      fi
    fi
  done

  return 1
}

ensure_appdir_root_icon_alias() {
  local appdir="$1"
  local desktop_file icon_name ext target candidate base lower_base lower_expect

  desktop_file="$(find "$appdir" -maxdepth 1 \( -type f -o -type l \) -name '*.desktop' | head -n 1)"
  if [[ -z "$desktop_file" ]]; then
    desktop_file="$(find "$appdir/usr/share/applications" -maxdepth 1 \( -type f -o -type l \) -name '*.desktop' 2>/dev/null | head -n 1)"
  fi
  if [[ -z "$desktop_file" ]]; then
    return 0
  fi

  icon_name="$(awk -F= '/^Icon=/{print $2; exit}' "$desktop_file" | tr -d '\r')"
  if [[ -z "$icon_name" ]]; then
    return 0
  fi

  for ext in png svg xpm; do
    target="$appdir/$icon_name.$ext"
    if [[ -f "$target" ]]; then
      return 0
    fi
  done

  for candidate in "$appdir"/*.png "$appdir"/*.svg "$appdir"/*.xpm; do
    [[ -f "$candidate" ]] || continue
    base="$(basename "$candidate")"
    ext="${base##*.}"
    lower_base="$(printf '%s' "${base%.*}" | tr '[:upper:]' '[:lower:]')"
    lower_expect="$(printf '%s' "$icon_name" | tr '[:upper:]' '[:lower:]')"
    if [[ "$lower_base" == "$lower_expect" ]]; then
      ln -sf "$base" "$appdir/$icon_name.$ext"
      [[ -e "$appdir/.DirIcon" ]] || ln -sf "$appdir/$icon_name.$ext" "$appdir/.DirIcon"
      return 0
    fi
  done

  return 0
}

manual_build_appimage_from_appdir() {
  local appimage_root appimagetool runtime appdir app_name output_file built=0

  appimage_root="$BUNDLE_SEARCH_ROOT/release/bundle/appimage"
  if [[ ! -d "$appimage_root" ]]; then
    return 1
  fi

  appimagetool="$(resolve_appimagetool_bin)" || return 1
  runtime="$(resolve_appimage_runtime)" || return 1

  shopt -s nullglob
  for appdir in "$appimage_root"/*.AppDir; do
    [[ -d "$appdir" ]] || continue
    ensure_appdir_root_icon_alias "$appdir"
    app_name="$(basename "$appdir" .AppDir)"
    output_file="$appimage_root/${app_name}_${VERSION}_amd64.AppImage"
    echo "==> 使用 appimagetool 手动封装: $(basename "$output_file")"
    ARCH=x86_64 "$appimagetool" --no-appstream --runtime-file "$runtime" "$appdir" "$output_file"
    if [[ -f "$output_file" ]]; then
      built=1
    fi
  done
  shopt -u nullglob

  [[ $built -eq 1 ]]
}

prepare_appimage_tool_shims() {
  local shim_dir
  shim_dir="$(mktemp -d)"

cat > "$shim_dir/linuxdeploy" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
TAURI_DIR="$CACHE_HOME/tauri"
APPIMAGE="$TAURI_DIR/linuxdeploy-x86_64.AppImage"
EXTRACT_DIR="$TAURI_DIR/linuxdeploy-squashfs"

if [[ ! -x "$APPIMAGE" ]]; then
  echo "linuxdeploy wrapper: missing $APPIMAGE" >&2
  exit 127
fi

echo "[wrapper] linuxdeploy $*" >> "$TAURI_DIR/linuxdeploy-wrapper.log"

if [[ ! -x "$EXTRACT_DIR/usr/bin/linuxdeploy" ]]; then
  rm -rf "$EXTRACT_DIR"
  (
    cd "$TAURI_DIR"
    "$APPIMAGE" --appimage-extract >/dev/null
    rm -rf "$EXTRACT_DIR"
    mv squashfs-root "$EXTRACT_DIR"
  )
fi

args=()
for arg in "$@"; do
  if [[ "$arg" != "--appimage-extract-and-run" ]]; then
    args+=("$arg")
  fi
done

export LD_LIBRARY_PATH="$EXTRACT_DIR/usr/lib:${LD_LIBRARY_PATH:-}"
exec "$EXTRACT_DIR/usr/bin/linuxdeploy" "${args[@]}"
EOF
  chmod +x "$shim_dir/linuxdeploy"

  cat > "$shim_dir/linuxdeploy-plugin-gtk" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

CACHE_HOME="${XDG_CACHE_HOME:-$HOME/.cache}"
TAURI_DIR="$CACHE_HOME/tauri"
ORIG="$TAURI_DIR/linuxdeploy-plugin-gtk.sh"
PATCHED="$TAURI_DIR/linuxdeploy-plugin-gtk.compat.sh"

if [[ ! -f "$ORIG" ]]; then
  echo "linuxdeploy-plugin-gtk wrapper: missing $ORIG" >&2
  exit 127
fi

if [[ ! -f "$PATCHED" || "$ORIG" -nt "$PATCHED" ]]; then
  sed 's/for elem in \"${src\\[@\\]}\"; do/for elem in \"${src[@]}\"; do\\n        if [ ! -e \"$elem\" ]; then\\n            echo \"WARNING: skip missing path: $elem\"\\n            continue\\n        fi/' "$ORIG" > "$PATCHED"
  chmod +x "$PATCHED"
fi

exec "$PATCHED" "$@"
EOF
  chmod +x "$shim_dir/linuxdeploy-plugin-gtk"
  ln -sf "$shim_dir/linuxdeploy-plugin-gtk" "$shim_dir/linuxdeploy-plugin-gtk.sh"

  echo "$shim_dir"
}

run_appimage_build_with_retry() {
  local max_attempts="$APPIMAGE_MAX_RETRY"
  local attempt=1
  local rc=1
  local shim_dir

  if ! [[ "$max_attempts" =~ ^[0-9]+$ ]] || [[ "$max_attempts" -lt 1 ]]; then
    max_attempts=1
  fi

  shim_dir="$(prepare_appimage_tool_shims)"
  trap 'rm -rf "$shim_dir"' RETURN

  while [[ $attempt -le $max_attempts ]]; do
    if [[ $attempt -gt 1 ]]; then
      echo "==> 第 $attempt/$max_attempts 次尝试构建 AppImage"
    fi

    if (
      cd "$ROOT_DIR"
      PATH="$shim_dir:$PATH" LINUXDEPLOY_PLUGIN_DIR="$shim_dir" run_tauri_bundle_build "appimage"
    ); then
      return 0
    else
      rc=$?
    fi

    if [[ $attempt -lt $max_attempts ]]; then
      if [[ "$APPIMAGE_RETRY_PURGE_CACHE" == "1" ]]; then
        echo "警告: AppImage 构建失败，清理 Tauri 下载缓存后重试..." >&2
        rm -rf "$TAURI_CACHE_HOME/tauri"
      else
        echo "警告: AppImage 构建失败，保留 Tauri 下载缓存后重试（APPIMAGE_RETRY_PURGE_CACHE=1 可改为清理缓存）..." >&2
      fi
      sleep 1
    fi

    attempt=$((attempt + 1))
  done

  return $rc
}

if ! command -v npm >/dev/null 2>&1; then
  echo "错误: 未找到 npm，请先安装 Node.js/npm" >&2
  exit 1
fi
if [[ "$APPIMAGE_ONLY" != "1" ]] && ! command -v makepkg >/dev/null 2>&1; then
  echo "错误: 未找到 makepkg（pacman 打包必需）" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
rm -f \
  "$OUT_DIR"/*.deb \
  "$OUT_DIR"/*.rpm \
  "$OUT_DIR"/*.pkg.tar.* \
  "$OUT_DIR"/*.AppImage

echo "==> 同步版本号"
bash "$ROOT_DIR/scripts/sync-version.sh"
echo "==> 使用 Tauri 构建缓存目录: $TAURI_CACHE_HOME"

if [[ "$APPIMAGE_ONLY" != "1" ]]; then
  echo "==> 构建 deb/rpm（Tauri）"
  (
    cd "$ROOT_DIR"
    run_tauri_bundle_build "deb,rpm"
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
fi

echo "==> 构建便携包（AppImage）"
APPIMAGE_FILES=()

if run_appimage_build_with_retry; then
  APPIMAGE_BUILD_RC=0
else
  APPIMAGE_BUILD_RC=$?
fi

if [[ $APPIMAGE_BUILD_RC -ne 0 ]]; then
  echo "警告: AppImage 构建失败，尝试从现有 AppDir 手动封装 AppImage..." >&2
  if ! manual_build_appimage_from_appdir; then
    echo "错误: AppImage 构建失败（已自动重试且尝试 AppDir 手动封装，且不回退 AppDir.tar.gz）。" >&2
    exit 1
  fi
fi

mapfile -t APPIMAGE_FILES < <(find "$BUNDLE_SEARCH_ROOT" -type f -name "*${VERSION}*.AppImage" 2>/dev/null | sort)
if [[ ${#APPIMAGE_FILES[@]} -eq 0 ]]; then
  echo "错误: 未找到版本 $VERSION 对应的 .AppImage 产物（搜索目录: $BUNDLE_SEARCH_ROOT）" >&2
  exit 1
fi

echo "==> 收集 AppImage 产物"
for f in "${APPIMAGE_FILES[@]}"; do
  cp -f "$f" "$OUT_DIR/"
done
if [[ "$KEEP_TARGET_BUNDLES" != "1" ]]; then
  for f in "${APPIMAGE_FILES[@]}"; do
    rm -f "$f"
  done
fi

if [[ "$APPIMAGE_ONLY" != "1" ]]; then
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
fi

echo "==> 打包完成，产物列表："
find "$OUT_DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' -o -name '*.AppImage' \) -print | sort
