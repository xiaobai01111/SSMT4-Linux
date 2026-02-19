#!/usr/bin/env bash

set -euo pipefail

if [[ "${1:-}" == "--plugin-api-version" ]]; then
  echo "0"
  exit 0
fi

APPDIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --appdir)
      APPDIR="${2:-}"
      shift 2
      ;;
    --help)
      echo "Usage: $0 --appdir <AppDir>"
      exit 0
      ;;
    *)
      shift
      ;;
  esac
done

if [[ -z "$APPDIR" ]]; then
  echo "ERROR: --appdir is required" >&2
  exit 1
fi

APPIMAGETOOL_BIN="$HOME/.cache/tauri/linuxdeploy-plugin-appimage-squashfs/usr/bin/appimagetool"
if [[ ! -x "$APPIMAGETOOL_BIN" ]]; then
  echo "ERROR: appimagetool not found: $APPIMAGETOOL_BIN" >&2
  exit 1
fi

RUNTIME_DIR="$HOME/.cache/tauri"
RUNTIME_FILE="$RUNTIME_DIR/runtime-x86_64"

if [[ ! -f "$RUNTIME_FILE" ]]; then
  CANDIDATE_APPIMAGES=(
    "$HOME/.cache/tauri/linuxdeploy-x86_64.AppImage.bak"
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage.bak"
    "$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage"
  )

  SOURCE_APPIMAGE=""
  for candidate in "${CANDIDATE_APPIMAGES[@]}"; do
    if [[ -x "$candidate" ]]; then
      SOURCE_APPIMAGE="$candidate"
      break
    fi
  done

  if [[ -z "$SOURCE_APPIMAGE" ]]; then
    echo "ERROR: no local AppImage found to extract runtime" >&2
    exit 1
  fi

  OFFSET="$("$SOURCE_APPIMAGE" --appimage-offset)"
  if [[ -z "$OFFSET" || "$OFFSET" -le 0 ]]; then
    echo "ERROR: failed to read runtime offset from $SOURCE_APPIMAGE" >&2
    exit 1
  fi

  mkdir -p "$RUNTIME_DIR"
  dd if="$SOURCE_APPIMAGE" of="$RUNTIME_FILE" bs=1 count="$OFFSET" status=none
  chmod +x "$RUNTIME_FILE"
fi

exec "$APPIMAGETOOL_BIN" --runtime-file "$RUNTIME_FILE" "$APPDIR"
