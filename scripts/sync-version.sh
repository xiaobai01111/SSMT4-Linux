#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION_FILE="$ROOT_DIR/version"
PACKAGE_JSON="$ROOT_DIR/package.json"
TAURI_CONF="$ROOT_DIR/src-tauri/tauri.conf.json"
CARGO_TOML="$ROOT_DIR/src-tauri/Cargo.toml"

if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 未找到版本文件: $VERSION_FILE" >&2
  exit 1
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

replace_or_fail() {
  local file="$1"
  local pattern="$2"
  local replacement="$3"
  local label="$4"

  if ! grep -Eq "$pattern" "$file"; then
    echo "错误: $label 未匹配到版本字段: $file" >&2
    exit 1
  fi

  local tmp_file
  tmp_file="$(mktemp)"
  sed -E "s/${pattern}/${replacement}/" "$file" > "$tmp_file"
  mv "$tmp_file" "$file"
}

replace_or_fail \
  "$PACKAGE_JSON" \
  '"version"[[:space:]]*:[[:space:]]*"[^"]+"' \
  "\"version\": \"${VERSION}\"" \
  "package.json"

replace_or_fail \
  "$TAURI_CONF" \
  '"version"[[:space:]]*:[[:space:]]*"[^"]+"' \
  "\"version\": \"${VERSION}\"" \
  "tauri.conf.json"

replace_or_fail \
  "$CARGO_TOML" \
  '^version[[:space:]]*=[[:space:]]*"[^"]+"' \
  "version = \"${VERSION}\"" \
  "Cargo.toml"

echo "版本已同步: ${VERSION}"
