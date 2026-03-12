#!/usr/bin/env bash

resolve_aur_source_dir() {
  local script_dir="$1"
  local root_dir="$2"
  local primary="$script_dir/ssmt4-linux"
  local legacy="$root_dir/aur/ssmt4-linux"

  if [[ -d "$primary" ]]; then
    printf '%s\n' "$primary"
    return 0
  fi

  if [[ -d "$legacy" ]]; then
    printf '%s\n' "$legacy"
    return 0
  fi

  printf '%s\n' "$primary"
}

ensure_aur_srcinfo() {
  local aur_dir="$1"

  if ! command -v makepkg >/dev/null 2>&1; then
    echo "错误: makepkg 未安装，无法生成 .SRCINFO" >&2
    return 1
  fi

  (
    cd "$aur_dir"
    makepkg --printsrcinfo > .SRCINFO
  )
}
