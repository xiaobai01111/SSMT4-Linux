#!/usr/bin/env bash
set -euo pipefail

# SSMT4 Linux dependency installer for Arch-based systems.
# Levels: core -> x -> xx -> xxl
# Example:
#   bash scripts/aur/install-arch-deps.sh core
#   bash scripts/aur/install-arch-deps.sh xx --print

usage() {
  cat <<'EOF'
Usage:
  install-arch-deps.sh list
  install-arch-deps.sh <level> [--print] [--noconfirm]

Levels:
  core  - GUI runtime required by SSMT4 itself
  x     - game runtime base (Wine/Winetricks/XWayland)
  xx    - advanced runtime tools (umu/bwrap/Vulkan/GPU detect/archive tools)
  xxl   - extended gaming stack (Steam/perf overlay)

Options:
  --print      Print package list only, do not install
  --noconfirm  Pass --noconfirm to pacman

Examples:
  bash scripts/aur/install-arch-deps.sh core
  bash scripts/aur/install-arch-deps.sh xx
  bash scripts/aur/install-arch-deps.sh xxl --noconfirm
EOF
}

CORE_PKGS=(
  gtk3
  webkit2gtk-4.1
  libsoup3
  xdg-utils
)

X_PKGS=(
  xorg-xwayland
  wine
  winetricks
  libayatana-appindicator
)

XX_PKGS=(
  umu-launcher
  bubblewrap
  vulkan-tools
  pciutils
  7zip
  unzip
  git
  polkit
  procps-ng
)

XXL_PKGS=(
  steam
  steam-devices
  mangohud
  lib32-mangohud
  gamescope
  gamemode
  lib32-vulkan-icd-loader
)

print_level() {
  local name="$1"
  shift
  printf '%s:\n' "$name"
  printf '  %s\n' "$@"
}

collect_pkgs() {
  local level="$1"
  local -n out_ref="$2"
  local merged=()

  case "$level" in
    core)
      merged=("${CORE_PKGS[@]}")
      ;;
    x)
      merged=("${CORE_PKGS[@]}" "${X_PKGS[@]}")
      ;;
    xx)
      merged=("${CORE_PKGS[@]}" "${X_PKGS[@]}" "${XX_PKGS[@]}")
      ;;
    xxl)
      merged=("${CORE_PKGS[@]}" "${X_PKGS[@]}" "${XX_PKGS[@]}" "${XXL_PKGS[@]}")
      ;;
    *)
      return 1
      ;;
  esac

  declare -A seen=()
  out_ref=()
  for pkg in "${merged[@]}"; do
    if [[ -z "${seen[$pkg]:-}" ]]; then
      seen["$pkg"]=1
      out_ref+=("$pkg")
    fi
  done
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

level="$1"
shift

if [[ "$level" == "list" ]]; then
  print_level "core" "${CORE_PKGS[@]}"
  echo
  print_level "x (core + x)" "${X_PKGS[@]}"
  echo
  print_level "xx (x + xx)" "${XX_PKGS[@]}"
  echo
  print_level "xxl (xx + xxl)" "${XXL_PKGS[@]}"
  exit 0
fi

print_only=0
pacman_flags=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --print)
      print_only=1
      ;;
    --noconfirm)
      pacman_flags+=("--noconfirm")
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

pkgs=()
if ! collect_pkgs "$level" pkgs; then
  echo "Unknown level: $level" >&2
  usage
  exit 1
fi

echo "Selected level: $level"
echo "Packages (${#pkgs[@]}): ${pkgs[*]}"

if [[ "$print_only" -eq 1 ]]; then
  exit 0
fi

if ! command -v pacman >/dev/null 2>&1; then
  echo "pacman not found. This script only supports Arch-based distributions." >&2
  exit 1
fi

if ! command -v sudo >/dev/null 2>&1; then
  echo "sudo not found. Please install sudo or run pacman manually as root." >&2
  exit 1
fi

sudo pacman -S --needed "${pacman_flags[@]}" "${pkgs[@]}"

echo "Dependency installation completed for level: $level"
