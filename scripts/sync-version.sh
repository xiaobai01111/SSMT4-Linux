#!/usr/bin/env bash
# 从项目根目录的 version 文件同步版本号到所有配置文件。
# 用法: bash scripts/sync-version.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

VERSION_FILE="$ROOT_DIR/version"
if [[ ! -f "$VERSION_FILE" ]]; then
  echo "错误: 找不到 $VERSION_FILE" >&2
  exit 1
fi

VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
if [[ -z "$VERSION" ]]; then
  echo "错误: version 文件为空" >&2
  exit 1
fi

# ---------- package.json ----------
PKG="$ROOT_DIR/package.json"
if [[ -f "$PKG" ]]; then
  sed -i "s/\"version\": *\"[^\"]*\"/\"version\": \"$VERSION\"/" "$PKG"
fi

# ---------- src-tauri/Cargo.toml ----------
CARGO="$ROOT_DIR/src-tauri/Cargo.toml"
if [[ -f "$CARGO" ]]; then
  # 只替换 [package] 段下的 version，避免误改依赖版本
  sed -i "0,/^version = \"[^\"]*\"/s//version = \"$VERSION\"/" "$CARGO"
fi

# ---------- src-tauri/tauri.conf.json ----------
TAURI="$ROOT_DIR/src-tauri/tauri.conf.json"
if [[ -f "$TAURI" ]]; then
  sed -i "s/\"version\": *\"[^\"]*\"/\"version\": \"$VERSION\"/" "$TAURI"
fi

echo "版本已同步: $VERSION"
